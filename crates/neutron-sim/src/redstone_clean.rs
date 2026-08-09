//! Redstone simulation for Minecraft 26.2.
//! Implements wire, torches, levers, doors, comparators, repeaters,
//! observers, hoppers, TNT, pistons, and quasi-connectivity.

use std::collections::{HashMap, VecDeque};

pub const MAX_POWER: u8 = 15;
const TORCH_BURNOUT_THRESHOLD: u8 = 9;
const TORCH_BURNOUT_WINDOW: u64 = 60;
const TORCH_RELIGHT_DELAY: u64 = 160;
const HOPPER_COOLDOWN: u64 = 8;
const TNT_FUSE: u64 = 40;
const TNT_BLAST_RADIUS: f64 = 4.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction { W, E, N, S, D, U }

impl Direction {
    pub fn offset(self) -> (i32, i32, i32) {
        match self {
            Direction::W => (-1, 0, 0), Direction::E => (1, 0, 0),
            Direction::N => (0, 0, -1), Direction::S => (0, 0, 1),
            Direction::D => (0, -1, 0), Direction::U => (0, 1, 0),
        }
    }
    pub fn opposite(self) -> Direction {
        match self {
            Direction::W => Direction::E, Direction::E => Direction::W,
            Direction::N => Direction::S, Direction::S => Direction::N,
            Direction::D => Direction::U, Direction::U => Direction::D,
        }
    }
}

pub const WIRE_PP_ORDER: [Direction; 6] = [Direction::W,Direction::E,Direction::N,Direction::S,Direction::D,Direction::U];
pub const WIRE_NC_ORDER: [Direction; 6] = [Direction::N,Direction::S,Direction::W,Direction::E,Direction::D,Direction::U];
pub const ALL_DIRECTIONS: [Direction; 6] = [Direction::W,Direction::E,Direction::N,Direction::S,Direction::D,Direction::U];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockType {
    Air, RedstoneWire, RedstoneTorch, Lever, Door, Solid,
    Comparator, Repeater, Observer, Hopper, Tnt, Piston, StickyPiston,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HopperItem { pub item_id: u16, pub count: u8 }
impl HopperItem { fn new(id: u16, c: u8) -> Self { Self { item_id: id, count: c } } }

#[derive(Debug, Clone)]
pub enum BlockMeta {
    Wire { power: u8 }, Torch { lit: bool }, Lever { on: bool }, Door { open: bool }, Solid, Air,
    Comparator { powered: bool, facing: Direction, locked: bool, container_signal: u8 },
    Repeater { lit: bool, delay: u8, locked: bool, input_power: u8, facing: Direction, delay_progress: u64 },
    Observer { facing: Direction, pulse_active: bool },
    Hopper { cooldown: u64, items: Vec<HopperItem> },
    Tnt { primed: bool, fuse: u64 },
    Piston { extending: bool, facing: Direction, extending_progress: u64, powered: bool },
    StickyPiston { extending: bool, facing: Direction, extending_progress: u64, powered: bool, pulled_block: Option<(i32, i32, i32)> },
}

struct TorchStateChange { tick: u64 }

pub struct RedstoneState {
    blocks: HashMap<(i32, i32, i32), BlockType>,
    meta: HashMap<(i32, i32, i32), BlockMeta>,
    power_levels: HashMap<(i32, i32, i32), u8>,
    update_queue: VecDeque<(i32, i32, i32)>,
    current_tick: u64,
    torch_changes: HashMap<(i32, i32, i32), Vec<TorchStateChange>>,
    burnout_ticks: HashMap<(i32, i32, i32), u64>,
    update_order_log: Vec<(i32, i32, i32)>,
    last_update_order: Vec<Direction>,
}

impl RedstoneState {
    pub fn new() -> Self {
        Self {
            blocks: HashMap::new(), meta: HashMap::new(), power_levels: HashMap::new(),
            update_queue: VecDeque::new(), current_tick: 0,
            torch_changes: HashMap::new(), burnout_ticks: HashMap::new(),
            update_order_log: Vec::new(), last_update_order: Vec::new(),
        }
    }

    // ---- Placement / mutation ----
    pub fn place_wire(&mut self, x: i32, y: i32, z: i32) {
        let pos = (x, y, z); self.blocks.insert(pos, BlockType::RedstoneWire);
        self.meta.insert(pos, BlockMeta::Wire { power: 0 });
    }
    pub fn place_torch(&mut self, x: i32, y: i32, z: i32) {
        let pos = (x, y, z); self.blocks.insert(pos, BlockType::RedstoneTorch);
        self.meta.insert(pos, BlockMeta::Torch { lit: true });
        self.power_levels.insert(pos, MAX_POWER);
        self.neigh(x, y, z, &ALL_DIRECTIONS);
    }
    pub fn toggle_lever(&mut self, x: i32, y: i32, z: i32) {
        let pos = (x, y, z);
        if !matches!(self.blocks.get(&pos), Some(BlockType::Lever)) {
            self.blocks.insert(pos, BlockType::Lever);
        }
        let cur = match self.meta.get(&pos) { Some(BlockMeta::Lever { on }) => *on, _ => false };
        let nw = !cur;
        self.meta.insert(pos, BlockMeta::Lever { on: nw });
        self.power_levels.insert(pos, if nw { MAX_POWER } else { 0 });
        self.neigh(x, y, z, &ALL_DIRECTIONS);
    }
    pub fn place_door(&mut self, x: i32, y: i32, z: i32) {
        let pos = (x, y, z); self.blocks.insert(pos, BlockType::Door);
        self.meta.insert(pos, BlockMeta::Door { open: false });
    }
    pub fn place_comparator(&mut self, x: i32, y: i32, z: i32, fac: Direction) {
        let pos = (x, y, z); self.blocks.insert(pos, BlockType::Comparator);
        self.meta.insert(pos, BlockMeta::Comparator { powered: false, facing: fac, locked: true, container_signal: 0 });
        self.neigh(x, y, z, &ALL_DIRECTIONS);
    }
    pub fn set_comparator_container_signal(&mut self, x: i32, y: i32, z: i32, sig: u8) {
        if let Some(BlockMeta::Comparator { container_signal, .. }) = self.meta.get_mut(&(x, y, z)) {
            *container_signal = sig.min(15);
        }
        self.rpow((x, y, z));
    }
    pub fn set_comparator_subtraction_mode(&mut self, x: i32, y: i32, z: i32) {
        if let Some(BlockMeta::Comparator { locked, .. }) = self.meta.get_mut(&(x, y, z)) {
            *locked = false;
        }
    }
    pub fn place_repeater(&mut self, x: i32, y: i32, z: i32, fac: Direction, delay: u8) {
        let pos = (x, y, z); let dl = delay.clamp(1, 4);
        self.blocks.insert(pos, BlockType::Repeater);
        self.meta.insert(pos, BlockMeta::Repeater { lit: false, delay: dl, locked: false, input_power: 0, facing: fac, delay_progress: 0 });
        self.neigh(x, y, z, &ALL_DIRECTIONS);
    }
    pub fn place_observer(&mut self, x: i32, y: i32, z: i32, fac: Direction) {
        let pos = (x, y, z); self.blocks.insert(pos, BlockType::Observer);
        self.meta.insert(pos, BlockMeta::Observer { facing: fac, pulse_active: false });
        self.neigh(x, y, z, &ALL_DIRECTIONS);
    }
    pub fn place_hopper(&mut self, x: i32, y: i32, z: i32) {
        let pos = (x, y, z); self.blocks.insert(pos, BlockType::Hopper);
        self.meta.insert(pos, BlockMeta::Hopper { cooldown: 0, items: Vec::new() });
    }
    pub fn hopper_add_item(&mut self, x: i32, y: i32, z: i32, id: u16, cnt: u8) {
        if let Some(BlockMeta::Hopper { items, .. }) = self.meta.get_mut(&(x, y, z)) {
            items.push(HopperItem::new(id, cnt));
        }
    }
    pub fn hopper_transfer_out(&mut self, x: i32, y: i32, z: i32) -> Option<HopperItem> {
        match self.meta.get_mut(&(x, y, z)) {
            Some(BlockMeta::Hopper { cooldown, items }) => {
                if *cooldown > 0 || items.is_empty() { return None; }
                let it = items.remove(0); *cooldown = HOPPER_COOLDOWN; Some(it)
            }
            _ => None,
        }
    }
    pub fn place_tnt(&mut self, x: i32, y: i32, z: i32) {
        let pos = (x, y, z); self.blocks.insert(pos, BlockType::Tnt);
        self.meta.insert(pos, BlockMeta::Tnt { primed: false, fuse: 0 });
    }
    pub fn prime_tnt(&mut self, x: i32, y: i32, z: i32) {
        if let Some(BlockMeta::Tnt { primed, fuse }) = self.meta.get_mut(&(x, y, z)) {
            *primed = true; *fuse = TNT_FUSE;
        }
    }
    pub fn place_piston(&mut self, x: i32, y: i32, z: i32, fac: Direction) {
        let pos = (x, y, z); self.blocks.insert(pos, BlockType::Piston);
        self.meta.insert(pos, BlockMeta::Piston { extending: false, facing: fac, extending_progress: 0, powered: false });
        self.neigh(x, y, z, &ALL_DIRECTIONS);
    }
    pub fn place_sticky_piston(&mut self, x: i32, y: i32, z: i32, fac: Direction) {
        let pos = (x, y, z); self.blocks.insert(pos, BlockType::StickyPiston);
        self.meta.insert(pos, BlockMeta::StickyPiston { extending: false, facing: fac, extending_progress: 0, powered: false, pulled_block: None });
        self.neigh(x, y, z, &ALL_DIRECTIONS);
    }
    pub fn place_solid(&mut self, x: i32, y: i32, z: i32) {
        let pos = (x, y, z); self.blocks.insert(pos, BlockType::Solid);
        self.meta.insert(pos, BlockMeta::Solid);
    }

    // ---- Power queries ----
    pub fn get_power(&self, x: i32, y: i32, z: i32) -> u8 { *self.power_levels.get(&(x, y, z)).unwrap_or(&0) }
    pub fn get_block(&self, x: i32, y: i32, z: i32) -> Option<BlockType> { self.blocks.get(&(x, y, z)).copied() }
    pub fn get_redstone_power(&self, x: i32, y: i32, z: i32) -> u8 {
        let mut m = 0u8;
        for d in ALL_DIRECTIONS { let (dx, dy, dz) = d.offset(); m = m.max(self.get_power(x + dx, y + dy, z + dz)); }
        m
    }
    pub fn is_door_open(&self, x: i32, y: i32, z: i32) -> bool {
        match self.meta.get(&(x, y, z)) { Some(BlockMeta::Door { open }) => *open, _ => false }
    }
    pub fn is_torch_lit(&self, x: i32, y: i32, z: i32) -> bool {
        match self.meta.get(&(x, y, z)) { Some(BlockMeta::Torch { lit }) => *lit, _ => false }
    }
    pub fn is_lever_on(&self, x: i32, y: i32, z: i32) -> bool {
        match self.meta.get(&(x, y, z)) { Some(BlockMeta::Lever { on }) => *on, _ => false }
    }

    pub fn notify_neighbors(&mut self, x: i32, y: i32, z: i32) { self.neigh(x, y, z, &ALL_DIRECTIONS); }
    pub fn schedule_update(&mut self, x: i32, y: i32, z: i32) { self.update_queue.push_back((x, y, z)); }
    /// Remove a block at position and trigger neighbor updates.
    pub fn remove_block(&mut self, x: i32, y: i32, z: i32) {
        let pos = (x, y, z);
        self.blocks.remove(&pos);
        self.power_levels.remove(&pos);
        self.neigh(x, y, z, &ALL_DIRECTIONS);
    }
    pub fn current_tick(&self) -> u64 { self.current_tick }
    pub fn update_order_log(&self) -> &[(i32, i32, i32)] { &self.update_order_log }
    pub fn last_update_order(&self) -> &[Direction] { &self.last_update_order }
    pub fn clear_update_log(&mut self) { self.update_order_log.clear(); }
    pub fn pending_updates(&self) -> usize { self.update_queue.len() }
    #[cfg(test)] fn torch_change_count(&self, pos: (i32, i32, i32)) -> usize { self.torch_changes.get(&pos).map_or(0, |v| v.len()) }

    fn neigh(&mut self, x: i32, y: i32, z: i32, order: &[Direction]) {
        self.last_update_order.clear();
        for &d in order { let (dx, dy, dz) = d.offset();
            self.update_queue.push_back((x + dx, y + dy, z + dz));
            self.update_order_log.push((x + dx, y + dy, z + dz));
            self.last_update_order.push(d);
        }
    }

    fn rpow(&mut self, pos: (i32, i32, i32)) {
        let bt = match self.blocks.get(&pos) { Some(b) => *b, None => return };
        match bt {
            BlockType::RedstoneWire => self.rp_wire(pos),
            BlockType::RedstoneTorch => self.rp_torch(pos),
            BlockType::Lever => {},
            BlockType::Door => self.rp_door(pos),
            BlockType::Solid => self.rp_solid(pos),
            BlockType::Air => {},
            BlockType::Comparator => self.rp_comp(pos),
            BlockType::Repeater => self.rp_rep(pos),
            BlockType::Observer => self.rp_obs(pos),
            BlockType::Hopper | BlockType::Tnt => {},
            BlockType::Piston => self.rp_pis(pos, false),
            BlockType::StickyPiston => self.rp_pis(pos, true),
        }
    }

    fn rp_wire(&mut self, pos: (i32, i32, i32)) {
        let mn = self.get_redstone_power(pos.0, pos.1, pos.2);
        let np = if mn > 0 { mn.saturating_sub(1) } else { 0 };
        let op = self.get_power(pos.0, pos.1, pos.2);
        self.power_levels.insert(pos, np);
        if op != np { self.neigh(pos.0, pos.1, pos.2, &WIRE_NC_ORDER); }
    }
    fn rp_torch(&mut self, pos: (i32, i32, i32)) {
        if self.burnout_ticks.contains_key(&pos) {
            let l = match self.meta.get(&pos) { Some(BlockMeta::Torch { lit }) => *lit, _ => true };
            if !l { return; }
        }
        let bp = self.get_power(pos.0, pos.1 - 1, pos.2);
        let l = match self.meta.get(&pos) { Some(BlockMeta::Torch { lit }) => *lit, _ => true };
        if l != (bp == 0) { self.set_tl(pos, bp == 0); }
    }
    fn rp_door(&mut self, pos: (i32, i32, i32)) {
        let pw = self.get_redstone_power(pos.0, pos.1, pos.2);
        let want = pw > 0;
        let cur = match self.meta.get(&pos) { Some(BlockMeta::Door { open }) => *open, _ => false };
        if cur != want { self.sdo(pos, want); }
    }
    fn rp_solid(&mut self, pos: (i32, i32, i32)) {
        let np = self.get_redstone_power(pos.0, pos.1, pos.2);
        let op = self.get_power(pos.0, pos.1, pos.2);
        self.power_levels.insert(pos, np);
        if op != np { self.neigh(pos.0, pos.1, pos.2, &ALL_DIRECTIONS); }
    }
    fn rp_comp(&mut self, pos: (i32, i32, i32)) {
        let (fac, csig, locked) = match self.meta.get(&pos) {
            Some(BlockMeta::Comparator { facing, container_signal, locked, .. }) => (*facing, *container_signal, *locked),
            _ => return,
        };
        let (bdx, bdy, bdz) = fac.offset();
        // Input from behind the comparator (opposite of facing) for subtraction
        let inp = self.get_power(pos.0 - bdx, pos.1 - bdy, pos.2 - bdz);
        let out = if locked { csig } else { csig.saturating_sub(inp) };
        let cp = self.get_power(pos.0, pos.1, pos.2);
        if cp != out {
            self.power_levels.insert(pos, out);
            // Output goes from the BACK of the comparator (opposite of facing)
            let nb = (pos.0 - bdx, pos.1 - bdy, pos.2 - bdz);
            self.power_levels.insert(nb, out);
            self.neigh(nb.0, nb.1, nb.2, &ALL_DIRECTIONS);
        } else {
            self.power_levels.insert(pos, out);
        }
    }
    fn rp_rep(&mut self, pos: (i32, i32, i32)) {
        let (fac, delay, lock, lit) = match self.meta.get(&pos) {
            Some(BlockMeta::Repeater { facing, delay, locked, lit, .. }) => (*facing, *delay, *locked, *lit),
            _ => return,
        };
        let (bdx, bdy, bdz) = fac.offset();
        let inp = self.get_power(pos.0 - bdx, pos.1 - bdy, pos.2 - bdz);
        let want = inp > 0 || (lock && lit);
        if lit != want {
            if let Some(BlockMeta::Repeater { lit: l, locked: lk, .. }) = self.meta.get_mut(&pos) {
                *l = want; *lk = want;
            }
            self.neigh(pos.0, pos.1, pos.2, &ALL_DIRECTIONS);
        }
    }
    fn rp_obs(&mut self, pos: (i32, i32, i32)) {
        let fac = match self.meta.get(&pos) {
            Some(BlockMeta::Observer { facing, pulse_active }) => {
                if *pulse_active { return; };
                *facing
            }
            _ => return,
        };
        let (bdx, bdy, bdz) = fac.offset();
        // Observer fires when block in FACING direction changes (pos + offset)
        if self.update_queue.contains(&(pos.0 + bdx, pos.1 + bdy, pos.2 + bdz))
        || self.update_queue.contains(&pos) {
            if let Some(BlockMeta::Observer { pulse_active: pa, .. }) = self.meta.get_mut(&pos) {
                *pa = true;
            }
            self.power_levels.insert(pos, MAX_POWER);
            // Output goes from BACK of observer (pos - offset, opposite of facing)
            let nb = (pos.0 - bdx, pos.1 - bdy, pos.2 - bdz);
            self.power_levels.insert(nb, MAX_POWER);
            self.neigh(nb.0, nb.1, nb.2, &ALL_DIRECTIONS);
        }
    }

    // ---- Piston / QC helpers ----
    fn ps_st(&self, pos: (i32, i32, i32)) -> (Direction, bool) {
        let (f, e) = match self.meta.get(&pos) {
            Some(BlockMeta::Piston { facing, extending, .. }) => (*facing, *extending),
            Some(BlockMeta::StickyPiston { facing, extending, .. }) => (*facing, *extending),
            _ => return (Direction::U, false),
        };
        (f, e)
    }
    fn set_pe(&mut self, pos: (i32, i32, i32), ext: bool) {
        if let Some(BlockMeta::Piston { extending, powered, .. }) = self.meta.get_mut(&pos) {
            *extending = ext; *powered = ext;
        }
        if let Some(BlockMeta::StickyPiston { extending, powered, .. }) = self.meta.get_mut(&pos) {
            *extending = ext; *powered = ext;
        }
    }
    fn rp_pis(&mut self, pos: (i32, i32, i32), _sticky: bool) {
        let (fac, ext) = self.ps_st(pos);
        let hp = self.get_redstone_power(pos.0, pos.1, pos.2) > 0;
        let qc = self.check_qc(pos, fac);
        if (hp || qc) && !ext { self.set_pe(pos, true); }
        else if !hp && !qc && ext { self.set_pe(pos, false); }
    }

    // ---- QC ----
    fn check_qc(&self, pos: (i32, i32, i32), _facing: Direction) -> bool {
        // Check 4 side-adjacent blocks (not facing, not opposite)
        for &d in &[Direction::W, Direction::E, Direction::N, Direction::S] {
            if d.opposite() == _facing { continue; }
            let (dx, dy, dz) = d.offset();
            let sb = (pos.0 + dx, pos.1 + dy, pos.2 + dz);
            if self.qc_pw(sb) { return true; }
        }
        false
    }
    fn qc_pw(&self, pos: (i32, i32, i32)) -> bool {
        let bt = self.blocks.get(&pos).copied().unwrap_or(BlockType::Air);
        match bt {
            BlockType::RedstoneWire | BlockType::RedstoneTorch | BlockType::Lever => true,
            BlockType::Solid | BlockType::Comparator | BlockType::Repeater => {
                self.get_power(pos.0, pos.1, pos.2) > 0
            }
            _ => false,
        }
    }
    fn is_pushable(&self, bt: BlockType) -> bool {
        matches!(bt, BlockType::Air | BlockType::RedstoneWire | BlockType::RedstoneTorch
            | BlockType::Lever | BlockType::Door | BlockType::Tnt)
    }

    // ---- Push limit: max 12 blocks ----
    fn push_blocked(&self, start: (i32, i32, i32), facing: Direction) -> bool {
        let (dx, dy, dz) = facing.offset();
        for i in 1..=12 {
            let bp = (start.0 + dx * i, start.1 + dy * i, start.2 + dz * i);
            if let Some(bt) = self.blocks.get(&bp).copied() {
                if !self.is_pushable(bt) { return true; }
            }
        }
        false
    }

    // ---- Extend / retract ----
    fn extend_pis(&mut self, pos: (i32, i32, i32), facing: Direction, _sticky: bool) {
        let (dx, dy, dz) = facing.offset();
        let tp: Vec<(i32, i32, i32)> = (1..=12)
            .map(|i| (pos.0 + dx * i, pos.1 + dy * i, pos.2 + dz * i))
            .collect();
        if self.push_blocked(pos, facing) { return; }
        // Push blocks in reverse order (farthest first)
        for bp in tp.iter().rev() {
            if let Some(bt) = self.blocks.get(bp).copied() {
                if self.is_pushable(bt) { continue; }
                let nxt = (bp.0 + dx, bp.1 + dy, bp.2 + dz);
                if let Some(nb) = self.blocks.get(&nxt).copied() {
                    if !self.is_pushable(nb) { return; }
                }
                let mt = self.meta.get(bp).cloned().unwrap_or(BlockMeta::Air);
                self.blocks.insert(nxt, bt); self.meta.insert(nxt, mt);
                self.power_levels.insert(nxt, self.get_power(bp.0, bp.1, bp.2));
                self.blocks.insert(*bp, BlockType::Air); self.meta.insert(*bp, BlockMeta::Air);
                self.power_levels.remove(bp);
            }
        }
    }

    fn retract_pis(&mut self, pos: (i32, i32, i32), fac: Direction, sticky: bool) {
        if !sticky { return; }
        // Clone values first to avoid borrow checker issues
        let pulled_opt = self.meta.get(&pos)
            .and_then(|m| if let BlockMeta::StickyPiston { pulled_block, .. } = m { pulled_block.clone() } else { None });
        if let Some(bp) = pulled_opt {
            let (dx, dy, dz) = fac.offset();
            let dest = (pos.0 + dx, pos.1 + dy, pos.2 + dz);
            let bt_opt = self.blocks.get(&bp).copied();
            let mt_opt = self.meta.get(&bp).cloned();
            if let Some(b) = bt_opt { self.blocks.insert(dest, b); }
            self.meta.insert(dest, mt_opt.unwrap_or(BlockMeta::Air));
            self.power_levels.insert(dest, self.get_power(bp.0, bp.1, bp.2));
            self.blocks.insert(bp, BlockType::Air);
            self.meta.insert(bp, BlockMeta::Air);
            self.power_levels.remove(&bp);
        }
    }

    fn sdo(&mut self, pos: (i32, i32, i32), open: bool) {
        if let Some(BlockMeta::Door { open: o, .. }) = self.meta.get_mut(&pos) { *o = open; }
        self.neigh(pos.0, pos.1, pos.2, &ALL_DIRECTIONS);
    }

    fn set_tl(&mut self, pos: (i32, i32, i32), lit: bool) {
        let old = match self.meta.get(&pos) { Some(BlockMeta::Torch { lit: l }) => *l, _ => true };
        if old == lit { return; }
        self.meta.insert(pos, BlockMeta::Torch { lit });
        self.power_levels.insert(pos, if lit { MAX_POWER } else { 0 });
        self.torch_changes.entry(pos).or_default().push(TorchStateChange { tick: self.current_tick });
        self.neigh(pos.0, pos.1, pos.2, &ALL_DIRECTIONS);
    }

    fn chk_burn(&mut self, pos: (i32, i32, i32)) {
        let lit = match self.meta.get(&pos) { Some(BlockMeta::Torch { lit }) => *lit, _ => true };
        if !lit { return; }
        if let Some(changes) = self.torch_changes.get(&pos) {
            let ws = self.current_tick.saturating_sub(TORCH_BURNOUT_WINDOW);
            if changes.iter().filter(|c| c.tick >= ws).count() >= TORCH_BURNOUT_THRESHOLD as usize {
                self.meta.insert(pos, BlockMeta::Torch { lit: false });
                self.power_levels.insert(pos, 0);
                self.burnout_ticks.insert(pos, self.current_tick);
                self.neigh(pos.0, pos.1, pos.2, &ALL_DIRECTIONS);
            }
        }
    }
    fn chk_rel(&mut self, pos: (i32, i32, i32)) {
        let lit = match self.meta.get(&pos) { Some(BlockMeta::Torch { lit }) => *lit, _ => true };
        if lit { return; }
        if let Some(bt) = self.burnout_ticks.get(&pos) {
            if self.current_tick < *bt + TORCH_RELIGHT_DELAY { return; }
        }
        let bp = self.get_power(pos.0, pos.1 - 1, pos.2);
        if bp == 0 { self.set_tl(pos, true); }
    }

    // ---- Component timing (per-tick) ----
    fn tn_tnt(&mut self, pos: (i32, i32, i32)) {
        let (primed, mut fuse) = match self.meta.get(&pos) { Some(BlockMeta::Tnt { primed, fuse }) => (*primed, *fuse), _ => return };
        if !primed { return; }
        fuse = fuse.saturating_sub(1);
        if fuse == 0 {
            self.blocks.insert(pos, BlockType::Air);
            self.meta.insert(pos, BlockMeta::Air);
            self.power_levels.remove(&pos);
            let r = TNT_BLAST_RADIUS as i32;
            for dx in -r..=r { for dy in -r..=r { for dz in -r..=r {
                if dx * dx + dy * dy + dz * dz <= r * r {
                    let bp = (pos.0 + dx, pos.1 + dy, pos.2 + dz);
                    if self.blocks.contains_key(&bp) {
                        self.blocks.insert(bp, BlockType::Air);
                        self.meta.insert(bp, BlockMeta::Air);
                        self.power_levels.remove(&bp);
                    }
                }
            }} }
        } else {
            if let Some(BlockMeta::Tnt { fuse: f, .. }) = self.meta.get_mut(&pos) { *f = fuse; }
        }
    }
    fn tn_rep(&mut self, pos: (i32, i32, i32)) {
        let (lit, delay, mut dp, fac) = match self.meta.get(&pos) {
            Some(BlockMeta::Repeater { lit, delay, delay_progress, facing, .. }) => (*lit, *delay, *delay_progress, *facing),
            _ => return,
        };
        if !lit { return; }
        dp += 1;
        if dp >= delay as u64 {
            let nb = (pos.0 + fac.offset().0, pos.1 + fac.offset().1, pos.2 + fac.offset().2);
            self.power_levels.insert(nb, MAX_POWER);
            self.neigh(nb.0, nb.1, nb.2, &ALL_DIRECTIONS);
        } else {
            if let Some(BlockMeta::Repeater { delay_progress: d, .. }) = self.meta.get_mut(&pos) { *d = dp; }
        }
    }
    fn tn_hop(&mut self, pos: (i32, i32, i32)) {
        if let Some(BlockMeta::Hopper { cooldown, .. }) = self.meta.get_mut(&pos) {
            if *cooldown > 0 { *cooldown = (*cooldown).saturating_sub(1); }
        }
    }
    fn tn_obs(&mut self, pos: (i32, i32, i32)) {
        let was = match self.meta.get(&pos) { Some(BlockMeta::Observer { facing, pulse_active, .. }) => *pulse_active, _ => return };
        if was {
            if let Some(BlockMeta::Observer { pulse_active: pa, .. }) = self.meta.get_mut(&pos) { *pa = false; }
            self.power_levels.remove(&pos);
            self.neigh(pos.0, pos.1, pos.2, &ALL_DIRECTIONS);
        }
    }
    fn tn_pis(&mut self, pos: (i32, i32, i32), sticky: bool) {
        let (fac, ext, mut ep) = if sticky {
            match self.meta.get(&pos) {
                Some(BlockMeta::StickyPiston { facing, extending, extending_progress, .. }) => (*facing, *extending, *extending_progress),
                _ => return,
            }
        } else {
            match self.meta.get(&pos) {
                Some(BlockMeta::Piston { facing, extending, extending_progress, .. }) => (*facing, *extending, *extending_progress),
                _ => return,
            }
        };
        if !ext {
            if ep > 0 { self.retract_pis(pos, fac, sticky); }
            return;
        }
        ep += 1;
        if ep >= 2 {
            self.extend_pis(pos, fac, sticky);
            if sticky {
                if let Some(BlockMeta::StickyPiston { extending_progress: e, .. }) = self.meta.get_mut(&pos) { *e = 2; }
            } else {
                if let Some(BlockMeta::Piston { extending_progress: e, .. }) = self.meta.get_mut(&pos) { *e = 2; }
            }
        } else {
            if sticky {
                if let Some(BlockMeta::StickyPiston { extending_progress: e, .. }) = self.meta.get_mut(&pos) { *e = ep; }
            } else {
                if let Some(BlockMeta::Piston { extending_progress: e, .. }) = self.meta.get_mut(&pos) { *e = ep; }
            }
        }
    }

    pub fn tick(&mut self) {
        self.current_tick += 1;
        // Process all queued updates (cascading propagation within one tick).
        // Use a processed set to avoid infinite loops from edge cases.
        let mut processed = std::collections::HashSet::new();
        const MAX_ITER: usize = 500;
        let mut iter = 0;
        while !self.update_queue.is_empty() && iter < MAX_ITER {
            iter += 1;
            let q: Vec<(i32, i32, i32)> = self.update_queue.drain(..).collect();
            for p in q {
                if processed.contains(&p) { continue; }
                processed.insert(p);
                self.rpow(p);
            }
        }
        let t: Vec<(i32, i32, i32)> = self.blocks.iter()
            .filter(|(_, bt)| **bt == BlockType::RedstoneTorch)
            .map(|(p, _)| *p).collect();
        for p in &t { self.chk_burn(*p); self.chk_rel(*p); }
        let c: Vec<(i32, i32, i32)> = self.blocks.keys().copied().collect();
        for p in &c {
            match self.blocks.get(p) {
                Some(BlockType::Tnt) => self.tn_tnt(*p),
                Some(BlockType::Repeater) => self.tn_rep(*p),
                Some(BlockType::Hopper) => self.tn_hop(*p),
                Some(BlockType::Observer) => self.tn_obs(*p),
                Some(BlockType::Piston) => self.tn_pis(*p, false),
                Some(BlockType::StickyPiston) => self.tn_pis(*p, true),
                _ => {},
            }
        }
    }
}
