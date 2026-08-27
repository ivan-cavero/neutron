//! Ticket-wavefront decoration-order simulator (vanilla 26.2).
//!
//! Deterministic single-threaded port of
//! `tools/worldgen-probe/src/ProbeDecorateOrderSchedule.java` extended with
//! the neighbor-promotion cascade, reproducing the canonical 424242 ref world
//` byte-for-byte (disk footprint = forced ∪ Chebyshev-2 halo, see below).
//!
//! Canonical procedure (tools/nbt-ref/new-mc-version.sh + its server log):
//! headless server 26.2, view-distance=10, `forceload add -128 -128 127 127`
//` (chunks [-8..7]²) at t=0, `forceload add -192 -192 -161 191` (west strip
//` cols [-12..-11] × rows [-12..11]) ~31 s later; save-all + stop ~3 min
//` after that. Modeled as discrete command ticks (full drain between).
//!
//! Constants mirrored from the 26.2 decompile
//! (`tools/mc-decompiler/output/26.2/src`):
//! - `ChunkMap.java:128` `FORCED_TICKET_LEVEL = ChunkLevel.byStatus(
//!   FullChunkStatus.ENTITY_TICKING) = 31` (ChunkLevel.java:48-55).
//! - `ChunkLevel.java:10-15` FULL=33; the FULL step of
//!   `ChunkPyramid.GENERATION_PYRAMID` accumulates 12 rings [SPAWN@0,
//!   INITIALIZE_LIGHT@1, CARVERS@2, BIOMES@3, STRUCTURE_STARTS@4..11] ⇒
//!   `RADIUS_AROUND_FULL_CHUNK=11`, `MAX_LEVEL=44` (machine-checked mirror of
//!   ChunkStep.Builder.buildAccumulatedDependencies). Ring→status map in
//!   [`generation_status`]; per-status layer radii
//!   (`ChunkStep.getAccumulatedRadiusOf`, ChunkDependencies.radiusByDependency
//!   last-covering-radius) in [`LAYER_RADII`]: EMPTY/STRUCTURE_STARTS 11,
//!   STRUCTURE_REFERENCES/BIOMES 3, NOISE/SURFACE/CARVERS 2, FEATURES/
//!   INITIALIZE_LIGHT 1, LIGHT/SPAWN/FULL 0.
//! - `LoadingChunkTracker.java:6,11` tracker MAX_LEVEL = 45, levelCount 46.
//! - `ChunkTaskPriorityQueue.java:11` `PRIORITY_LEVEL_COUNT = MAX_LEVEL + 2`;
//!   `pop()` = lowest non-empty ticket-level bucket, first-inserted key first
//!   (Long2ObjectLinkedOpenHashMap insertion age); `resortChunkTasks` removes
//!   + re-appends (insertion age resets).
//! - `ChunkTaskDispatcher` ops through the 4-lane PriorityConsecutiveExecutor
//!   (resort 0 < submit 2 < poll 3); a submit that catches the dispatcher
//!   asleep schedules one poll; each poll runs the popped chunk's whole
//!   segment list then `.thenAccept(this::pollTask)`.
//! - `ChunkGenerationTask` idealized to ONE status LAYER per dispatcher poll
//!   (probe MODEL rule): layer radius = FULL_STEP.getAccumulatedRadiusOf,
//!   sweep x-outer/z-inner ascending (ChunkGenerationTask.java:120-134),
//!   first failed apply parks the task (ChunkMap.applyStep parent-missing +
//!   `ChunkLevel.generationStatus` reachability, ChunkMap.java:630-655).
//!
//! Beyond the probe (validated against the ref disk): vanilla 26.2
//! auto-promotes every chunk whose ticket level reaches FULL-accessibility —
//! `ChunkHolder.updateFutures` (ChunkHolder.java:273-305) calls
//! `prepareAccessibleChunk` → `getChunkRangeFuture(range=1, FULL)`
//! (ChunkMap.java:306-330) which schedules FULL-target generation tasks for
//! the cell and its 3×3 ring. Hence unforced cells within Chebyshev distance
//! 2 of any forced source (level 31+d ≤ 33) run their OWN task ladders and
//! reach FULL — exactly the extra 224 chunks on the ref disk — while
//! distance-3 cells (level 34, FEATURES-capable) decorate via their
//! decorated neighbours' ±1 FEATURES sweeps but are dropped unsaved, and
//! distance-4 cells (level 35 = CARVERS cap) can never decorate.
//!
//! Deterministic replacements for thread/wall-clock effects (same seams the
//! probe documents): ticket-level propagation evaluated at its fixed point
//! per command tick (point sources ⇒ level = 31 + chebyshev distance),
//! `changedHolders`/promotion waves replayed in ascending packed-key order,
//! worker pool serialized FIFO.

use std::collections::{BTreeSet, HashMap, HashSet, VecDeque};
use std::sync::OnceLock;

// ---- constants mirrored from the 26.2 decompile ---------------------------

/// FORCED_TICKET_LEVEL (ChunkMap.java:128).
pub const FORCED_TICKET_LEVEL: i32 = 31;
/// ChunkLevel.MAX_LEVEL = FULL_CHUNK_LEVEL + RADIUS_AROUND_FULL_CHUNK(11).
pub const MAX_LEVEL: i32 = 44;
/// LoadingChunkTracker.MAX_LEVEL = ChunkLevel.MAX_LEVEL + 1 (:6).
pub const TRACKER_MAX_LEVEL: i32 = MAX_LEVEL + 1;
/// Tracker levelCount = MAX_LEVEL + 1 (LoadingChunkTracker :11).
pub const LEVEL_COUNT: i32 = TRACKER_MAX_LEVEL + 1;
/// ChunkTaskPriorityQueue.PRIORITY_LEVEL_COUNT = ChunkLevel.MAX_LEVEL + 2.
pub const PRIORITY_LEVEL_COUNT: usize = (MAX_LEVEL + 2) as usize;
/// FULL_CHUNK_LEVEL (ChunkLevel.java:10).
pub const FULL_CHUNK_LEVEL: i32 = 33;
/// RADIUS_AROUND_FULL_CHUNK = size(FULL-step acc dependencies) - 1.
pub const RADIUS_AROUND_FULL_CHUNK: i32 = 11;

// ChunkStatus indices (ChunkStatus.java registration order).
const S_EMPTY: usize = 0;
const S_STRUCT_STARTS: usize = 1;
const S_STRUCT_REFS: usize = 2;
const S_CARVERS: usize = 6;
const S_FEATURES: usize = 7;
const S_INIT_LIGHT: usize = 8;
const S_LIGHT: usize = 9;
const S_SPAWN: usize = 10;
const S_FULL: usize = 11;

/// Per-status layer radius inside the FULL step of GENERATION_PYRAMID
/// (`getRadiusForLayer`, ChunkGenerationTask.java:131-134). FULL-step acc
/// rings [SPAWN@0, IL@1, CARVERS@2, BIOMES@3, SS@4..11] collapsed through
/// ChunkDependencies.radiusByDependency (last covering radius wins).
const LAYER_RADII: [i32; 12] = [11, 11, 3, 3, 2, 2, 2, 1, 1, 0, 0, 0];

const _: () = {
    assert!(LAYER_RADII[S_FEATURES] == 1);
    assert!(LAYER_RADII[S_INIT_LIGHT] == 1);
    assert!(LAYER_RADII[S_LIGHT] == 0);
    assert!(LAYER_RADII[S_SPAWN] == 0);
    assert!(LAYER_RADII[S_FULL] == 0);
    assert!(RADIUS_AROUND_FULL_CHUNK == 11);
    assert!(MAX_LEVEL == 44);
};

/// `ChunkLevel.generationStatus(level)`: highest status reachable at a given
/// ticket level (ring→status table above; ≤33 = FULL).
fn generation_status(level: i32) -> Option<usize> {
    let d = level - FULL_CHUNK_LEVEL;
    if d <= 0 {
        return Some(S_FULL);
    }
    if d > RADIUS_AROUND_FULL_CHUNK {
        return None;
    }
    Some(match d {
        1 => S_INIT_LIGHT,
        2 => S_CARVERS,
        3 => 3, // BIOMES
        _ => S_STRUCT_STARTS,
    })
}

#[inline]
fn layer_radius(status: usize) -> i32 {
    LAYER_RADII[status]
}

#[inline]
fn parent_index(status: usize) -> i32 {
    if status == S_EMPTY { 0 } else { (status - 1) as i32 }
}

#[inline]
pub fn pack(x: i32, z: i32) -> i64 {
    (((x as i64) & 0xFFFF_FFFF) << 32) | ((z as i64) & 0xFFFF_FFFF)
}
#[inline]
pub fn unpack_x(key: i64) -> i32 {
    (key >> 32) as i32
}
#[inline]
pub fn unpack_z(key: i64) -> i32 {
    key as i32
}

// ---- canonical command batches ---------------------------------------------

/// One forceload command: inclusive chunk-rect plus ingestion order
/// (ForceLoadCommand.changeForceLoad iterates x outer asc, z inner asc —
/// ForceLoadCommand.java:151-181).
#[derive(Clone, Copy, Debug)]
pub struct Batch {
    pub x1: i32,
    pub z1: i32,
    pub x2: i32,
    pub z2: i32,
}

impl Batch {
    pub fn rect(x1: i32, z1: i32, x2: i32, z2: i32) -> Self {
        Self { x1, z1, x2, z2 }
    }
    fn coords(&self) -> Vec<(i32, i32)> {
        let mut v = Vec::with_capacity(((self.x2 - self.x1 + 1) * (self.z2 - self.z1 + 1)) as usize);
        for x in self.x1..=self.x2 {
            for z in self.z1..=self.z2 {
                v.push((x, z));
            }
        }
        v
    }
}

/// The canonical ref procedure measured from the 424242 server log
/// (tools/nbt-ref/vanilla-fresh-424242/logs/2026-08-26-2.log.gz):
/// `Marked 256 chunks [-8,-8]..[7,7]`, +31 s `Marked 48 chunks
/// [-12,-12]..[-11,11]`, save-all/stop. Each batch is a phase (full drain).
pub fn canonical_batches() -> Vec<Batch> {
    vec![Batch::rect(-8, -8, 7, 7), Batch::rect(-12, -12, -11, 11)]
}

// ---- scheduler replicas -----------------------------------------------------

/// ChunkTaskPriorityQueue bucket: insertion-ordered keys with pending
/// runnable segments.
#[derive(Default)]
struct Bucket {
    order: VecDeque<i64>,
    segs: HashMap<i64, VecDeque<usize>>,
}

impl Bucket {
    fn submit(&mut self, key: i64, task: usize) {
        if !self.segs.contains_key(&key) {
            self.order.push_back(key);
        }
        self.segs.entry(key).or_default().push_back(task);
    }
    fn take(&mut self, key: i64) -> Option<VecDeque<usize>> {
        let segs = self.segs.remove(&key)?;
        if let Some(i) = self.order.iter().position(|&k| k == key) {
            self.order.remove(i);
        }
        Some(segs)
    }
    fn pop_first(&mut self) -> (i64, VecDeque<usize>) {
        let key = self.order.pop_front().expect("pop on non-empty bucket");
        let segs = self.segs.remove(&key).unwrap_or_default();
        (key, segs)
    }
    fn append(&mut self, key: i64, segs: VecDeque<usize>) {
        if !self.segs.contains_key(&key) {
            self.order.push_back(key);
        }
        self.segs.entry(key).or_default().extend(segs);
    }
    fn is_empty(&self) -> bool {
        self.segs.is_empty()
    }
}

/// ChunkTaskPriorityQueue replica.
struct TaskQueues {
    buckets: Vec<Bucket>,
    top: usize,
}

impl Default for TaskQueues {
    fn default() -> Self {
        let mut buckets = Vec::with_capacity(PRIORITY_LEVEL_COUNT);
        buckets.resize_with(PRIORITY_LEVEL_COUNT, Bucket::default);
        Self { buckets, top: PRIORITY_LEVEL_COUNT }
    }
}

impl TaskQueues {
    fn has_work(&self) -> bool {
        self.top < PRIORITY_LEVEL_COUNT
    }
    /// submit (ChunkTaskPriorityQueue.java:39-42).
    fn submit(&mut self, key: i64, task: usize, level: i32) {
        debug_assert!((0..PRIORITY_LEVEL_COUNT as i32).contains(&level));
        let l = level.clamp(0, PRIORITY_LEVEL_COUNT as i32 - 1) as usize;
        self.buckets[l].submit(key, task);
        self.top = self.top.min(l);
    }
    /// resortChunkTasks (:22-37).
    fn resort(&mut self, old: i32, pos: i64, new: i32) {
        if old < 0 || old >= PRIORITY_LEVEL_COUNT as i32 {
            return;
        }
        let removed = self.buckets[old as usize].take(pos);
        if old as usize == self.top {
            while self.has_work() && self.buckets[self.top].is_empty() {
                self.top += 1;
            }
        }
        if let Some(segs) = removed {
            if !segs.is_empty() {
                let n = new.clamp(0, PRIORITY_LEVEL_COUNT as i32 - 1) as usize;
                self.buckets[n].append(pos, segs);
                self.top = self.top.min(n);
            }
        }
    }
    /// pop (:63-78).
    fn pop(&mut self) -> Option<(i64, VecDeque<usize>)> {
        if !self.has_work() {
            return None;
        }
        let (key, segs) = self.buckets[self.top].pop_first();
        while self.has_work() && self.buckets[self.top].is_empty() {
            self.top += 1;
        }
        Some((key, segs))
    }
}

/// ChunkTaskDispatcher / PriorityConsecutiveExecutor ops.
enum DispOp {
    Resort { key: i64, new_level: i32 },
    Submit { task: usize },
    Poll,
}

impl DispOp {
    fn lane(&self) -> usize {
        match self {
            DispOp::Resort { .. } => 0,
            DispOp::Submit { .. } => 2,
            DispOp::Poll => 3,
        }
    }
}

/// FixedPriorityQueue(4) + AbstractConsecutiveExecutor run-cycle dedup.
#[derive(Default)]
struct Dispatcher {
    lanes: [VecDeque<DispOp>; 4],
    running: bool,
}

impl Dispatcher {
    fn schedule(&mut self, op: DispOp, exec: &mut VecDeque<Item>) {
        self.lanes[op.lane()].push_back(op);
        if !self.running && self.lanes.iter().any(|l| !l.is_empty()) {
            self.running = true;
            exec.push_back(Item::DispRun);
        }
    }
    fn pop_op(&mut self) -> Option<DispOp> {
        self.lanes.iter_mut().find_map(|l| l.pop_front())
    }
}

enum Item {
    /// AbstractConsecutiveExecutor.run(): pops exactly ONE dispatcher op.
    DispRun,
    /// One worker-runnable segment: one ChunkGenerationTask layer attempt.
    Work { task: usize },
}

/// ChunkHolder analogue + StaticCache2D working-set cell state.
#[derive(Clone, Copy)]
struct Holder {
    ticket_level: i32,
    queue_level: i32,
    /// Highest finished ChunkStatus index (-1 none).
    completed: i32,
}

/// ChunkGenerationTask replica: one status LAYER per worker segment.
#[derive(Clone, Copy)]
struct GenTask {
    cx: i32,
    cz: i32,
    scheduled: Option<usize>,
}

/// A parked task: real vanilla parks on a dependency future and resumes via
/// `future.thenRun` when the dependency completes. `need = -1` marks a
/// ticket-level block (resolves on ingestion); otherwise the task waits for
/// `cell.completed >= need` ("Parent chunk missing" gate).
#[derive(Clone, Copy)]
struct Park {
    task: usize,
    cell: i64,
    need: i32,
}

/// Where a parked segment stopped ("Parent chunk missing" cell + needed
/// parent status, or a ticket-level cap with need = -1).
struct Blocker {
    cell: i64,
    need: i32,
}

struct Sim {
    holders: HashMap<i64, Holder>,
    tickets: HashSet<i64>,
    /// Scheduled generation tasks by center key (one per cell).
    tasks: HashMap<i64, usize>,
    pending_tasks: VecDeque<usize>,
    parked: Vec<Park>,
    task_list: Vec<GenTask>,
    queues: TaskQueues,
    disp: Dispatcher,
    sleeping: bool,
    exec: VecDeque<Item>,
    active_group: usize,
    level_field: HashMap<i64, i32>,
    field_dirty: bool,
    decorated: HashSet<i64>,
    events: Vec<(i32, i32)>,
}

impl Sim {
    fn new() -> Self {
        Self {
            holders: HashMap::new(),
            tickets: HashSet::new(),
            tasks: HashMap::new(),
            pending_tasks: VecDeque::new(),
            parked: Vec::new(),
            task_list: Vec::new(),
            queues: TaskQueues::default(),
            disp: Dispatcher::default(),
            sleeping: true,
            exec: VecDeque::new(),
            active_group: 0,
            level_field: HashMap::new(),
            field_dirty: true,
            decorated: HashSet::new(),
            events: Vec::new(),
        }
    }

    #[inline]
    fn steady_level(&self, key: i64) -> i32 {
        self.level_field.get(&key).copied().unwrap_or(TRACKER_MAX_LEVEL)
    }

    fn spawn_task(&mut self, cx: i32, cz: i32) -> usize {
        self.task_list.push(GenTask { cx, cz, scheduled: None });
        let idx = self.task_list.len() - 1;
        self.tasks.insert(pack(cx, cz), idx);
        idx
    }

    fn ingest_commands(&mut self, batch: &Batch) {
        for (cx, cz) in batch.coords() {
            let key = pack(cx, cz);
            if !self.tickets.insert(key) {
                continue;
            }
            self.field_dirty = true;
        }
    }

    /// Converged point-source field min(31 + chebyshev) over FORCED tickets —
    /// the exact fixed point of LoadingChunkTracker's DynamicGraphMinFixedPoint
    /// propagation for this source set (probe steadyStateLoadLevel).
    fn apply_distance_updates(&mut self, changed: &mut BTreeSet<i64>) {
        if self.field_dirty {
            self.rebuild_level_field();
            self.field_dirty = false;
        }
        // LoadingChunkTracker.setLevel materializes ChunkHolders along the
        // decreasing wave: every ticket-reachable cell gets a holder at its
        // converged level (default TRACKER_MAX_LEVEL elsewhere).
        for (&key, &lvl) in self.level_field.iter() {
            self.holders.entry(key).or_insert(Holder {
                ticket_level: lvl,
                queue_level: lvl,
                completed: -1,
            });
        }
        for key in self.holders.keys().copied().collect::<Vec<i64>>() {
            let tl = self.steady_level(key);
            let h = self.holders.get_mut(&key).unwrap();
            if tl != h.ticket_level {
                h.ticket_level = tl;
                changed.insert(key);
            }
        }
    }

    fn rebuild_level_field(&mut self) {
        self.level_field.clear();
        let mut dq: VecDeque<i64> = VecDeque::new();
        for &k in &self.tickets {
            dq.push_back(k);
            self.level_field.insert(k, FORCED_TICKET_LEVEL);
        }
        while let Some(k) = dq.pop_front() {
            let l = self.level_field[&k];
            let (x, z) = (unpack_x(k), unpack_z(k));
            for dx in -1i32..=1 {
                for dz in -1i32..=1 {
                    if dx == 0 && dz == 0 {
                        continue;
                    }
                    let nb = pack(x + dx, z + dz);
                    let nl = l + 1;
                    if nl > LEVEL_COUNT - 1 {
                        continue;
                    }
                    match self.level_field.get(&nb) {
                        Some(&cur) if cur <= nl => {}
                        _ => {
                            self.level_field.insert(nb, nl);
                            dq.push_back(nb);
                        }
                    }
                }
            }
        }
    }

    fn dispatch_resort(&mut self, key: i64, new_level: i32) {
        self.disp_schedule(DispOp::Resort { key, new_level });
    }

    fn disp_schedule(&mut self, op: DispOp) {
        self.disp.schedule(op, &mut self.exec);
    }

    fn execute_disp_op(&mut self, op: DispOp) {
        match op {
            DispOp::Resort { key, new_level } => {
                let old = self.holders.get(&key).map(|h| h.queue_level).unwrap_or(TRACKER_MAX_LEVEL);
                self.queues.resort(old, key, new_level);
                if let Some(h) = self.holders.get_mut(&key) {
                    h.queue_level = new_level;
                }
            }
            DispOp::Submit { task } => {
                let t = self.task_list[task];
                let key = pack(t.cx, t.cz);
                let level = self.holders.get(&key).map(|h| h.queue_level).unwrap_or(TRACKER_MAX_LEVEL);
                self.queues.submit(key, task, level);
                if self.sleeping {
                    self.sleeping = false;
                    self.disp_schedule(DispOp::Poll);
                }
            }
            DispOp::Poll => match self.queues.pop() {
                Some((_, segs)) => {
                    self.sleeping = false;
                    self.active_group = segs.len();
                    for t in segs {
                        self.exec.push_back(Item::Work { task: t });
                    }
                }
                None => self.sleeping = true,
            },
        }
    }

    fn run_segment(&mut self, task: usize) {
        match self.try_advance_one_layer(task) {
            Ok(advanced) => {
                if self.reached_target(task) {
                    return;
                }
                if advanced {
                    self.disp_schedule(DispOp::Submit { task });
                } else {
                    unreachable!("NoAdvance must carry a blocker");
                }
            }
            Err(blocker) => {
                if self.reached_target(task) {
                    return;
                }
                self.parked.push(Park { task, cell: blocker.cell, need: blocker.need });
            }
        }
    }

    fn reached_target(&self, task: usize) -> bool {
        self.task_list[task].scheduled == Some(S_FULL)
    }

    /// One LAYER per dispatcher poll (probe MODEL rule).
    fn try_advance_one_layer(&mut self, task: usize) -> Result<bool, Blocker> {
        if self.reached_target(task) {
            return Ok(true);
        }
        let t = self.task_list[task];
        let next = t.scheduled.map_or(S_EMPTY, |s| s + 1);
        let radius = layer_radius(next);
        for dx in -(radius)..=(radius) {
            for dz in -(radius)..=(radius) {
                match self.apply(next, t.cx + dx, t.cz + dz) {
                    Ok(true) => {}
                    Ok(false) => {
                        return Err(Blocker { cell: pack(t.cx + dx, t.cz + dz), need: -1 });
                    }
                    Err(need) => {
                        return Err(Blocker { cell: pack(t.cx + dx, t.cz + dz), need });
                    }
                }
            }
        }
        self.task_list[task].scheduled = Some(next);
        Ok(true)
    }

    /// One apply == inline ChunkMap.applyStep (ChunkMap.java:630-655).
    /// Ok(true): applied. Ok(false): ticket-level reachability block
    /// (resolves on ingestion). Err(parent): parent-completion block —
    /// the cell's `completed` must reach `parent` ("Parent chunk missing").
    fn apply(&mut self, status: usize, ax: i32, az: i32) -> Result<bool, i32> {
        let key = pack(ax, az);
        let steady = self.steady_level(key);
        let h = self.holders.entry(key).or_insert(Holder {
            ticket_level: steady,
            queue_level: steady,
            completed: -1,
        });
        let Some(reachable) = generation_status(h.ticket_level) else {
            return Ok(false);
        };
        if reachable < status {
            return Ok(false);
        }
        let parent = parent_index(status);
        if status != S_EMPTY && h.completed < parent {
            return Err(parent);
        }
        let before;
        let raised_to;
        {
            let h = self.holders.get_mut(&key).unwrap();
            before = h.completed;
            h.completed = h.completed.max(status as i32);
            raised_to = h.completed;
        }
        if status == S_FEATURES {
            self.record_decorate(ax, az, key);
        }
        // Dependency completion: resume parked tasks waiting on this cell
        // (real vanilla: future.thenRun → runGenerationTask → dispatcher).
        if raised_to > before {
            let mut woke = Vec::new();
            self.parked.retain(|p| {
                if p.cell == key && p.need >= 0 && p.need <= raised_to {
                    woke.push(p.task);
                    false
                } else {
                    true
                }
            });
            for task in woke {
                self.disp_schedule(DispOp::Submit { task });
            }
        }
        Ok(true)
    }

    fn record_decorate(&mut self, cx: i32, cz: i32, key: i64) {
        if self.decorated.insert(key) {
            self.events.push((cx, cz));
        }
    }

    /// One simulated phase: ingest batch → distance updates → resorts →
    /// promotion cascade → FIFO runGenerationTasks → parked revival → drain.
    fn run_phase(&mut self, batch: &Batch) {
        self.ingest_commands(batch);

        let mut changed = BTreeSet::new();
        self.apply_distance_updates(&mut changed);

        // Promotion cascade (ChunkHolder.updateFutures → prepareAccessibleChunk):
        // every cell whose ticket level reached FULL accessibility gets a
        // FULL-target generation task (getChunkRangeFuture range=1). Cells
        // whose queue level changed get resorted; distance-3 cells
        // (level 34, FEATURES-capable) get no own task — they decorate via
        // their neighbours' ±1 FEATURES sweeps and are dropped unsaved.
        for &key in &changed {
            let new_level = self.holders.get(&key).map(|h| h.ticket_level).unwrap_or(TRACKER_MAX_LEVEL);
            self.dispatch_resort(key, new_level);
        }
        let mut keys: Vec<i64> = self
            .holders
            .iter()
            .filter(|(k, h)| h.ticket_level <= FULL_CHUNK_LEVEL && !self.tasks.contains_key(*k))
            .map(|(k, _)| *k)
            .collect();
        keys.sort_unstable();
        for key in keys {
            let (cx, cz) = (unpack_x(key), unpack_z(key));
            let task = self.spawn_task(cx, cz);
            self.pending_tasks.push_back(task);
        }

        while let Some(t) = self.pending_tasks.pop_front() {
            self.disp_schedule(DispOp::Submit { task: t });
        }
        // Ticket levels changed: level-blocked parked tasks get another chance
        // (probe's parked-revival seam).
        if !self.parked.is_empty() {
            let mut revived = Vec::new();
            self.parked.retain(|p| {
                if p.need < 0 {
                    revived.push(p.task);
                    false
                } else {
                    true
                }
            });
            for task in revived {
                self.pending_tasks.push_back(task);
            }
        }

        self.drain();
    }

    fn drain(&mut self) {
        let mut guard = 0u64;
        while let Some(item) = self.exec.pop_front() {
            guard += 1;
            if guard > 200_000_000 {
                panic!("deco_schedule: executor livelock");
            }
            match item {
                Item::DispRun => {
                    let op = self.disp.pop_op();
                    if let Some(op) = op {
                        self.execute_disp_op(op);
                    }
                    self.disp.running = false;
                    if self.disp.lanes.iter().any(|l| !l.is_empty()) {
                        self.disp.running = true;
                        self.exec.push_back(Item::DispRun);
                    }
                }
                Item::Work { task } => {
                    self.run_segment(task);
                    self.active_group -= 1;
                    if self.active_group == 0 {
                        self.disp_schedule(DispOp::Poll);
                    }
                }
            }
        }
    }
}

/// Run an explicit batch procedure; returns the global chronological decorate
/// order (world chunk coords). Batches are phases: full drain between them.
pub fn simulate_batches(batches: &[Batch]) -> Vec<(i32, i32)> {
    let mut sim = Sim::new();
    for b in batches {
        sim.run_phase(b);
    }
    sim.events
}

/// Simulate the canonical ref procedure.
pub fn simulate_canonical_pregen() -> Vec<(i32, i32)> {
    simulate_batches(&canonical_batches())
}

/// Global chronological decorate sequence of the canonical ref procedure
/// (computed once per process).
pub fn decorate_sequence() -> &'static Vec<(i32, i32)> {
    static SEQ: OnceLock<Vec<(i32, i32)>> = OnceLock::new();
    SEQ.get_or_init(simulate_canonical_pregen)
}

/// Buffer-window origin order for the `ticket_sim` arm: the window origins in
/// their simulated global decorate sequence (window-relative `(x,z)` mapped
/// to world chunks via `origin_x`/`origin_z`, stable sort ascending by seq).
pub fn window_order(chunks: i32, origin_x: i32, origin_z: i32) -> Vec<(i32, i32)> {
    let seq = decorate_sequence();
    let mut rank: HashMap<(i32, i32), usize> = HashMap::with_capacity(seq.len());
    for (i, &p) in seq.iter().enumerate() {
        rank.entry(p).or_insert(i);
    }
    let bx = origin_x >> 4;
    let bz = origin_z >> 4;
    let mut keyed: Vec<(u8, u64, i32, i32)> = Vec::with_capacity((chunks * chunks) as usize);
    for czl in 0..chunks {
        for cxl in 0..chunks {
            let stage_rank = match rank.get(&(bx + cxl, bz + czl)) {
                Some(&r) => (0u8, r as u64),
                // Windows outside the pregenned area trail deterministically.
                None => (1u8, u64::MAX),
            };
            keyed.push((stage_rank.0, stage_rank.1, czl, cxl));
        }
    }
    keyed.sort();
    keyed.into_iter().map(|(_, _, czl, cxl)| (cxl, czl)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_step_tables_match_derivation() {
        assert_eq!(generation_status(31), Some(S_FULL));
        assert_eq!(generation_status(33), Some(S_FULL));
        assert_eq!(generation_status(34), Some(S_INIT_LIGHT));
        assert_eq!(generation_status(35), Some(S_CARVERS));
        assert_eq!(generation_status(36), Some(3));
        assert_eq!(generation_status(37), Some(S_STRUCT_STARTS));
        assert_eq!(generation_status(38), Some(S_STRUCT_STARTS));
        assert_eq!(generation_status(44), Some(S_STRUCT_STARTS));
        assert_eq!(generation_status(45), None);
        assert_eq!(layer_radius(S_STRUCT_REFS), 3);
        assert_eq!(layer_radius(S_CARVERS), 2);
        assert_eq!(layer_radius(S_FEATURES), 1);
        assert_eq!(layer_radius(S_EMPTY), 11);
    }

    #[test]
    fn ticket_sim_matches_ref_footprint_and_facts() {
        let seq = simulate_canonical_pregen();
        assert!(!seq.is_empty(), "no decoration events");
        let decorated: HashSet<(i32, i32)> = seq.iter().copied().collect();
        // Ref disk footprint: forced square+west strip plus their
        // Chebyshev-2 halo (validated byte-for-byte against the .mca slot
        // map); all of it must decorate, nothing beyond distance 3.
        let forced = canonical_batches();
        let mut core = HashSet::new();
        for b in &forced {
            for x in b.x1..=b.x2 {
                for z in b.z1..=b.z2 {
                    core.insert((x, z));
                }
            }
        }
        let mut halo = HashSet::new();
        for &(x, z) in &core {
            for dx in -2i32..=2 {
                for dz in -2i32..=2 {
                    let p = (x + dx, z + dz);
                    if (-14..=9).contains(&p.0) && (-14..=13).contains(&p.1) {
                        halo.insert(p);
                    }
                }
            }
        }
        let expect: HashSet<(i32, i32)> = core.union(&halo).copied().collect();
        assert!(expect.is_subset(&decorated), "ref disk cells missing from decorate set");
        // Extras are exactly the distance-3 band (level 34 = FEATURES-capable
        // via neighbour sweeps, dropped unsaved in vanilla).
        for &(x, z) in decorated.difference(&expect) {
            let d = core.iter().map(|(cx, cz)| (x - cx).abs().max((z - cz).abs())).min().unwrap();
            assert_eq!(d, 3, "decorated extra ({x},{z}) at distance {d}");
        }
        eprintln!("first 8 decorated: {:?}", &seq[..8.min(seq.len())]);
        let rank = |p: (i32, i32)| seq.iter().position(|&x| x == p).unwrap();
        // Vanilla (0,0)-diag case: (-1,+1) decorates BEFORE the center.
        assert!(rank((-1, 1)) < rank((0, 0)), "(-1,1) must decorate before (0,0)");
    }
}

