//! `JigsawPlacement.addPieces` port (vanilla 26.2) for `ancient_city`.
//!
//! Vanilla flow (`JigsawStructure.findGenerationPoint` → `JigsawPlacement.
//! addPieces` → `Placer.tryPlacingChildren`):
//! 1. startHeight absolute −27 (no RNG draw — constant height provider)
//! 2. center rotation: `random.nextInt(4)` (`Rotation.getRandom`)
//! 3. center template: pool flat list, `random.nextInt(len)`
//! 4. anchor via the `minecraft:city_anchor`-named jigsaw in the center piece
//! 5. BFS over pieces (FIFO — all placement priorities are 0 here): for each
//!    source jigsaw (shuffled + selection-priority sort, all 0 → no-op):
//!    target pool flat list shuffled (`Util.shuffledCopy`), then fallback
//!    templates appended, then rotations shuffled (`Rotation.getShuffled`);
//!    first `canAttach` match that does not overlap the free-shape set wins.
//!
//! Overlap check: vanilla uses VoxelShapes over piece AABBs with a 0.25
//! deflate on the candidate; modeled as AABB overlap here (piece shapes are
//! plain boxes, so a voxel union of boxes ≡ box list; ONLY_SECOND-overlap ⇔
//! candidate box intersects any placed box).

use super::pools::{Pool, PoolElem};
use super::templates::{tpl_by_name, Tpl};

pub(crate) const START_Y: i32 = -27;
pub(crate) const MAX_DEPTH: i32 = 7;
pub(crate) const MAX_DISTANCE_H: i32 = 116;
/// `max_distance_from_center` vertical defaults to full Y range; the shape
/// clamp then never binds below WORLD_TOP. Use a large vertical.
pub(crate) const MAX_DISTANCE_V: i32 = 512;
pub(crate) const ANCHOR_NAME: &str = "minecraft:city_anchor";

/// `net.minecraft.core.Direction` (horizontal subset + vertical flags).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Dir {
    North,
    South,
    West,
    East,
    Up,
    Down,
}

impl Dir {
    fn opposite(self) -> Self {
        match self {
            Dir::North => Dir::South,
            Dir::South => Dir::North,
            Dir::West => Dir::East,
            Dir::East => Dir::West,
            Dir::Up => Dir::Down,
            Dir::Down => Dir::Up,
        }
    }

    fn step_y(self) -> i32 {
        match self {
            Dir::Up => 1,
            Dir::Down => -1,
            _ => 0,
        }
    }

    fn offset(self, x: i32, y: i32, z: i32) -> (i32, i32, i32) {
        match self {
            Dir::North => (x, y, z - 1),
            Dir::South => (x, y, z + 1),
            Dir::West => (x - 1, y, z),
            Dir::East => (x + 1, y, z),
            Dir::Up => (x, y + 1, z),
            Dir::Down => (x, y - 1, z),
        }
    }
}

/// `net.minecraft.world.level.block.Rotation`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Rot {
    None,
    Cw90,
    Cw180,
    Ccw90,
}

const ALL_ROTS: [Rot; 4] = [Rot::None, Rot::Cw90, Rot::Cw180, Rot::Ccw90];

impl Rot {
    /// `Rotation.rotate(Direction)` — Y axes pass through.
    fn rotate_dir(self, d: Dir) -> Dir {
        use Dir::*;
        match self {
            Rot::None => d,
            Rot::Cw90 => match d {
                North => East,
                East => South,
                South => West,
                West => North,
                y => y,
            },
            Rot::Cw180 => match d {
                North => South,
                South => North,
                East => West,
                West => East,
                y => y,
            },
            Rot::Ccw90 => match d {
                North => West,
                West => South,
                South => East,
                East => North,
                y => y,
            },
        }
    }

    /// `StructureTemplate.transform(pos, mirror=NONE, rotation, pivot=ZERO)`.
    pub(crate) fn transform(self, x: i32, z: i32) -> (i32, i32) {
        match self {
            Rot::None => (x, z),
            Rot::Cw90 => (-z, x),
            Rot::Cw180 => (-x, -z),
            Rot::Ccw90 => (z, -x),
        }
    }
}

/// Parsed jigsaw-block `orientation` (`front_top`, e.g. `west_up`).
fn orientation(state: &str) -> (Dir, Dir) {
    // state = "minecraft:jigsaw|orientation=west_up" (only orientation modeled)
    let o = state
        .split("orientation=")
        .nth(1)
        .unwrap_or("north_up")
        .split(';')
        .next()
        .unwrap_or("north_up");
    let (f, t) = o.split_once('_').unwrap_or(("north", "up"));
    (dir_of(f), dir_of(t))
}

fn dir_of(s: &str) -> Dir {
    match s {
        "north" => Dir::North,
        "south" => Dir::South,
        "west" => Dir::West,
        "east" => Dir::East,
        "up" => Dir::Up,
        "down" => Dir::Down,
        _ => Dir::North,
    }
}

/// World-space AABB (`net.minecraft.world.level.levelgen.structure.BoundingBox`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Bb {
    pub(crate) min_x: i32,
    pub(crate) min_y: i32,
    pub(crate) min_z: i32,
    pub(crate) max_x: i32,
    pub(crate) max_y: i32,
    pub(crate) max_z: i32,
}

impl Bb {
    fn span(&self) -> (i32, i32, i32) {
        (
            self.max_x - self.min_x + 1,
            self.max_y - self.min_y + 1,
            self.max_z - self.min_z + 1,
        )
    }

    fn moved(&self, dx: i32, dy: i32, dz: i32) -> Self {
        Bb {
            min_x: self.min_x + dx,
            min_y: self.min_y + dy,
            min_z: self.min_z + dz,
            max_x: self.max_x + dx,
            max_y: self.max_y + dy,
            max_z: self.max_z + dz,
        }
    }

    fn intersects(&self, o: &Bb) -> bool {
        self.min_x <= o.max_x
            && self.max_x >= o.min_x
            && self.min_y <= o.max_y
            && self.max_y >= o.min_y
            && self.min_z <= o.max_z
            && self.max_z >= o.min_z
    }

    /// `AABB.deflate(0.25)` overlap semantics: candidate shrunk by 0.25 on
    /// each axis intersects `o` iff (shrunk box vs o box) overlap. With the
    /// 0.25 shrink, integer boxes only overlap when their overlap span is
    /// ≥ 1 on every axis AFTER shrinking the candidate by 0.5 total — i.e.
    /// strict adjacency (span-1 touch) does not count.
    /// Candidate shrunk by 0.25 intersects `o` (integer boxes: overlap must
    /// exceed a 0-wide touch on every axis).
    fn intersects_shrunk(&self, o: &Bb) -> bool {
        const D: f64 = 0.25;
        let (ax0, ax1) = (self.min_x as f64 + D, self.max_x as f64 + 1.0 - D);
        let (ay0, ay1) = (self.min_y as f64 + D, self.max_y as f64 + 1.0 - D);
        let (az0, az1) = (self.min_z as f64 + D, self.max_z as f64 + 1.0 - D);
        let (bx0, bx1) = (o.min_x as f64, o.max_x as f64 + 1.0);
        let (by0, by1) = (o.min_y as f64, o.max_y as f64 + 1.0);
        let (bz0, bz1) = (o.min_z as f64, o.max_z as f64 + 1.0);
        ax0 < bx1 && ax1 > bx0 && ay0 < by1 && ay1 > by0 && az0 < bz1 && az1 > bz0
    }

    /// `self AND NOT cut` — box subtraction, up to 6 remainder boxes.
    fn subtract(&self, cut: &Bb) -> Vec<Bb> {
        let ix0 = self.min_x.max(cut.min_x);
        let iy0 = self.min_y.max(cut.min_y);
        let iz0 = self.min_z.max(cut.min_z);
        let ix1 = self.max_x.min(cut.max_x);
        let iy1 = self.max_y.min(cut.max_y);
        let iz1 = self.max_z.min(cut.max_z);
        if ix0 > ix1 || iy0 > iy1 || iz0 > iz1 {
            return vec![*self]; // no intersection
        }
        let mut out = Vec::new();
        if self.min_z < iz0 {
            out.push(Bb { min_x: self.min_x, max_x: self.max_x, min_y: self.min_y, max_y: self.max_y, min_z: self.min_z, max_z: iz0 - 1 });
        }
        if cut.max_z < self.max_z {
            out.push(Bb { min_x: self.min_x, max_x: self.max_x, min_y: self.min_y, max_y: self.max_y, min_z: iz1 + 1, max_z: self.max_z });
        }
        if self.min_x < ix0 {
            out.push(Bb { min_x: self.min_x, max_x: ix0 - 1, min_y: self.min_y, max_y: self.max_y, min_z: iz0, max_z: iz1 });
        }
        if cut.max_x < self.max_x {
            out.push(Bb { min_x: ix1 + 1, max_x: self.max_x, min_y: self.min_y, max_y: self.max_y, min_z: iz0, max_z: iz1 });
        }
        if self.min_y < iy0 {
            out.push(Bb { min_x: ix0, max_x: ix1, min_y: self.min_y, max_y: iy0 - 1, min_z: iz0, max_z: iz1 });
        }
        if cut.max_y < self.max_y {
            out.push(Bb { min_x: ix0, max_x: ix1, min_y: iy1 + 1, max_y: self.max_y, min_z: iz0, max_z: iz1 });
        }
        out
    }

    fn overlaps_shrunk(&self, o: &Bb) -> bool {
        const D: f64 = 0.25;
        let (ax0, ax1) = (self.min_x as f64 + D, self.max_x as f64 + 1.0 - D);
        let (ay0, ay1) = (self.min_y as f64 + D, self.max_y as f64 + 1.0 - D);
        let (az0, az1) = (self.min_z as f64 + D, self.max_z as f64 + 1.0 - D);
        let (bx0, bx1) = (o.min_x as f64, o.max_x as f64 + 1.0);
        let (by0, by1) = (o.min_y as f64, o.max_y as f64 + 1.0);
        let (bz0, bz1) = (o.min_z as f64, o.max_z as f64 + 1.0);
        ax0 < bx1 && ax1 > bx0 && ay0 < by1 && ay1 > by0 && az0 < bz1 && az1 > bz0
    }
}

/// Template jigsaw block in world space, post-rotation.
#[derive(Clone, Debug)]
pub(crate) struct Jigsaw {
    pub(crate) pos: (i32, i32, i32),
    /// orientation front AFTER rotation.
    pub(crate) front: Dir,
    /// orientation top AFTER rotation.
    pub(crate) top: Dir,
    pub(crate) name: &'static str,
    pub(crate) target: &'static str,
    pub(crate) pool: &'static str,
    pub(crate) joint_rollable: bool,
}

/// Resolve a template + rotation into world jigsaws at `position` offset
/// (`StructureTemplate.getJigsaws`: transform each jigsaw pos with pivot
/// ZERO, rotate the orientation, add `position`).
pub(crate) fn world_jigsaws(
    tpl: &'static Tpl,
    position: (i32, i32, i32),
    rot: Rot,
) -> Vec<Jigsaw> {
    let mut out = Vec::new();
    for (_, x, y, z, pal, id, _fs, name, pool, _prio, joint, target) in tpl.marked {
        if *id != "minecraft:jigsaw" {
            continue;
        }
        let (tx, tz) = rot.transform(*x, *z);
        let state = *super::templates::PALETTE.get(*pal as usize).unwrap_or(&"");
        let (front0, top0) = orientation(state);
        out.push(Jigsaw {
            pos: (position.0 + tx, position.1 + *y, position.2 + tz),
            front: rot.rotate_dir(front0),
            top: rot.rotate_dir(top0),
            name,
            target,
            pool,
            joint_rollable: *joint == "rollable",
        });
    }
    out
}

/// Template world BB (`getBoundingBox`, pivot ZERO).
fn world_bb(tpl: &'static Tpl, position: (i32, i32, i32), rot: Rot) -> Bb {
    let [sx, sy, sz] = tpl.size;
    let delta = (sx - 1, sy - 1, sz - 1);
    let c1 = rot.transform(0, 0);
    let c2 = rot.transform(delta.0, delta.2);
    let (min_x, max_x) = (c1.0.min(c2.0), c1.0.max(c2.0));
    let (min_z, max_z) = (c1.1.min(c2.1), c1.1.max(c2.1));
    let (min_y, max_y) = (0.min(delta.1), 0.max(delta.1));
    Bb {
        min_x: position.0 + min_x,
        max_x: position.0 + max_x,
        min_y: position.1 + min_y,
        max_y: position.1 + max_y,
        min_z: position.2 + min_z,
        max_z: position.2 + max_z,
    }
}

/// Weight-expanded flat template list (`StructureTemplatePool.templates`).
fn flat_templates(pool: &'static Pool) -> Vec<&'static PoolElem> {
    let mut v = Vec::new();
    for e in pool.elements {
        for _ in 0..e.weight {
            v.push(e);
        }
    }
    v
}

fn pool_by_key(key: &str) -> Option<&'static Pool> {
    use super::pools::*;
    Some(match key {
        "city_center" => &POOL_CITY_CENTER,
        "city_center_walls" => &POOL_CITY_CENTER_WALLS,
        "city_entrance" => &POOL_CITY_ENTRANCE,
        "structures" => &POOL_STRUCTURES,
        "walls" => &POOL_WALLS,
        "walls_no_corners" => &POOL_WALLS_NO_CORNERS,
        "sculk" => &POOL_SCULK,
        _ => return None,
    })
}

/// A placed piece (assembly output).
#[derive(Clone, Debug)]
pub(crate) struct Piece {
    /// templates.rs const name.
    pub(crate) tpl_name: &'static str,
    /// Piece origin (template-space (0,0,0) corner in world space).
    pub(crate) position: (i32, i32, i32),
    pub(crate) rot: Rot,
    pub(crate) bb: Bb,
    /// feature_pool_element pieces place a sculk_patch at their jigsaw.
    pub(crate) is_feature: bool,
    /// Jigsaw depth at placement (center = 0).
    pub(crate) depth: i32,
    /// Free-space snapshot AT ACCEPT TIME (vanilla carries the
    /// MutableObject<VoxelShape> contents in the PieceState).
    pub(crate) free_at_accept: Vec<Bb>,
    /// `PoolElementStructurePiece.getGroundLevelDelta()` — for rigid pieces:
    /// source delta − deltaY.
    pub(crate) ground_level_delta: i32,
}

/// RNG consumed exactly like vanilla's `WorldgenRandom` here:
/// `nextInt(bound)` via the shared `LegacyRandom`.
pub(crate) struct Assembler<'r> {
    pub(crate) rng: &'r mut crate::legacy_rng::LegacyRandom,
    pieces: Vec<Piece>,
    /// Global AABB clamp (min_x, max_x, min_z, max_z).
    global: (i32, i32, i32, i32),
}

/// All city pool elements are `single`/`list`/`feature` pool elements with
/// RIGID projection — `getGroundLevelDelta()` = source delta − deltaY.
fn target_rigid_probe(_tpl: &str) -> bool {
    true
}

/// Non-rigid elements would use `targetElement.getGroundLevelDelta()`
/// (StructurePoolElement default = 1). Unused for the city.
fn target_elem_ground_level_delta() -> i32 {
    1
}

fn attach_inside_source_hit(source: &Piece, pos: (i32, i32, i32)) -> bool {
    pos.0 >= source.bb.min_x
        && pos.0 <= source.bb.max_x
        && pos.1 >= source.bb.min_y
        && pos.1 <= source.bb.max_y
        && pos.2 >= source.bb.min_z
        && pos.2 <= source.bb.max_z
}

/// `Util.shuffle` — Fisher–Yates from the tail: for i in (1..len).rev(),
/// swap i-1 with nextInt(i). Vanilla: `for (i = size; i > 1; i--) swap(i-1,
/// nextInt(i))`.
fn shuffle<T>(v: &mut [T], rng: &mut crate::legacy_rng::LegacyRandom) {
    let size = v.len();
    for i in (2..=size).rev() {
        let swap_to = rng.next_int(i as i32) as usize;
        v.swap(i - 1, swap_to);
    }
}

fn shuffled_rots(rng: &mut crate::legacy_rng::LegacyRandom) -> Vec<Rot> {
    let mut v = ALL_ROTS.to_vec();
    shuffle(&mut v, rng);
    v
}

/// `JigsawBlock.canAttach`.
fn can_attach(src: &Jigsaw, tgt: &Jigsaw) -> bool {
    src.front == tgt.front.opposite()
        && (src.joint_rollable || src.top == tgt.top)
        && src.target == tgt.name
}

impl<'r> Assembler<'r> {
    /// Assemble the city for a start chunk. `start_x/start_z` = chunk min
    /// block coords. Returns `None` when vanilla would bail (no anchor).
    pub(crate) fn assemble(
        rng: &'r mut crate::legacy_rng::LegacyRandom,
        start_x: i32,
        start_z: i32,
    ) -> Option<(Vec<Piece>, (i32, i32, i32))> {
        // JigsawStructure.findGenerationPoint: constant start height → no draw.
        let start_pos = (start_x, START_Y, start_z);
        // Rotation.getRandom: nextInt(4)
        let center_rot = ALL_ROTS[rng.next_int(4) as usize];
        // center template: nextInt(flat size)
        let center_pool = flat_templates(pool_by_key("city_center").unwrap());
        let center_elem = center_pool[rng.next_int(center_pool.len() as i32) as usize];
        let center_tpl_name = center_elem.single?;
        let center_tpl = tpl_by_name(center_tpl_name)?;

        // The center template's world BB at start_pos (before anchor adjust).
        let center_bb0 = world_bb(center_tpl, start_pos, center_rot);
        // getRandomNamedJigsaw: shuffled jigsaws, find name == city_anchor.
        let mut jigsaws = world_jigsaws(center_tpl, start_pos, center_rot);
        shuffle(&mut jigsaws, rng);
        let anchor = jigsaws.iter().find(|j| j.name == ANCHOR_NAME)?;
        // anchoredPosition = anchor pos; localAnchor = anchor - startPos;
        // adjustedPosition = startPos - localAnchor (piece moves so the anchor
        // lands on startPos).
        let local_anchor = (
            anchor.pos.0 - start_pos.0,
            anchor.pos.1 - start_pos.1,
            anchor.pos.2 - start_pos.2,
        );
        let adjusted_pos = (
            start_pos.0 - local_anchor.0,
            start_pos.1 - local_anchor.1,
            start_pos.2 - local_anchor.2,
        );
        let center_bb = world_bb(center_tpl, adjusted_pos, center_rot);
        let _ = center_bb0;
        // projectStartToHeightmap is EMPTY → bottomY = adjustedPosition.getY()
        // (the ANCHOR-ADJUSTED position, not startPos). With a city_anchor at
        // template y=24, adjustedPos.y = -27 - 24 = -51 = bb.minY → the move
        // is a no-op (dy = bottomY - (bb.minY + 0) = 0).
        let bottom_y = adjusted_pos.1;
        // StructurePoolElement.getGroundLevelDelta() = 1 (all city elements
        // are rigid single/list/feature elements and none override it).
        let old_ground = center_bb.min_y + 1;
        let dy = bottom_y - old_ground;
        let center_bb = center_bb.moved(0, dy, 0);
        let center_pos = (adjusted_pos.0, adjusted_pos.1 + dy, adjusted_pos.2);
        let global_center = (
            (center_bb.min_x + center_bb.max_x) / 2,
            (center_bb.min_y + center_bb.max_y) / 2,
            (center_bb.min_z + center_bb.max_z) / 2,
        );
        let global = (
            global_center.0 - MAX_DISTANCE_H,
            global_center.0 + MAX_DISTANCE_H + 1,
            global_center.2 - MAX_DISTANCE_H,
            global_center.2 + MAX_DISTANCE_H + 1,
        );

        let mut asm = Assembler { rng, pieces: Vec::new(), global };
        let center_free: Vec<Bb> = {
            // global AABB minus center box (the branch shape the center was
            // "accepted" into)
            let gy0 = (global_center.1 - MAX_DISTANCE_V).max(crate::generator::WORLD_BOTTOM);
            let gy1 = (global_center.1 + MAX_DISTANCE_V + 1).min(crate::generator::WORLD_TOP);
            vec![Bb {
                min_x: global.0,
                max_x: global.1,
                min_y: gy0,
                max_y: gy1,
                min_z: global.2,
                max_z: global.3,
            }]
        };
        asm.pieces.push(Piece {
            depth: 0,
            tpl_name: center_tpl_name,
            position: center_pos,
            rot: center_rot,
            bb: center_bb,
            is_feature: false,
            free_at_accept: center_free,
            ground_level_delta: 1,
        });
        if MAX_DEPTH > 0 {
            // Branch shape = Shapes.join(create(globalAABB), create(centerBox),
            // ONLY_FIRST) = globalAABB minus center box.
            let gy0 = (global_center.1 - MAX_DISTANCE_V).max(crate::generator::WORLD_BOTTOM);
            let gy1 = (global_center.1 + MAX_DISTANCE_V + 1).min(crate::generator::WORLD_TOP);
            let mut branch: Vec<Bb> = vec![Bb {
                min_x: global.0,
                max_x: global.1,
                min_y: gy0,
                max_y: gy1,
                min_z: global.2,
                max_z: global.3,
            }]
            .into_iter()
            .flat_map(|b| b.subtract(&asm.pieces[0].bb))
            .collect();
            let a = asm.pieces[0].clone();
            asm.try_placing_children(&a, 0, &mut branch);
            // FIFO drain — SequencedPriorityIterator with equal priorities.
            // Vanilla shares ONE MutableObject<VoxelShape> down the branch
            // (the child's `childrenFree` IS the parent's shape object, once
            // carved) — so a single shared set is carried through the drain.
            let mut idx = 1;
            while idx < asm.pieces.len() {
                let p = asm.pieces[idx].clone();
                // vanilla pushes a PieceState only when depth + 1 <= maxDepth;
                // deeper pieces are never expanded. The PieceState's `free` is
                // the SAME MutableObject the piece was accepted against — one
                // shared, progressively-carved shape per branch (siblings keep
                // carving it after this piece was accepted).
                if p.depth <= MAX_DEPTH {
                    asm.try_placing_children(&p, p.depth, &mut branch);
                }
                idx += 1;
            }
        }
        Some((std::mem::take(&mut asm.pieces), global_center))
    }

    fn try_placing_children(
        &mut self,
        source: &Piece,
        depth: i32,
        branch_free: &mut Vec<Bb>,
    ) {
        let source_tpl = match tpl_by_name(source.tpl_name) {
            Some(t) => t,
            None => return,
        };
        let source_box_y = source.bb.min_y;
        // vanilla: `if (sourceFree.get() == null) sourceFree.setValue(
        // Shapes.create(AABB.of(sourceBB)))` — the inside-source branch shape
        // starts as the FULL source box (not carved).
        let mut source_free: Vec<Bb> = vec![source.bb];
        let mut source_jigsaws = world_jigsaws(source_tpl, source.position, source.rot);
        shuffle(&mut source_jigsaws, self.rng);

        'sources: for source_jigsaw in &source_jigsaws {
            let (sjx, sjy, sjz) = source_jigsaw.pos;
            let target_jigsaw_pos = source_jigsaw.front.offset(sjx, sjy, sjz);
            let source_jigsaw_local_y = sjy - source_box_y;
            let mut source_jigsaw_base_height = i32::MIN;
            let pool_key = source_jigsaw
                .pool
                .strip_prefix("minecraft:ancient_city/")
                .unwrap_or("")
                .replace('/', "_");
            let Some(target_pool) = pool_by_key(&pool_key) else {
                continue;
            };
            let fallback_pool = if target_pool.fallback.is_empty() {
                None
            } else {
                pool_by_key(target_pool.fallback)
            };

            // targetPieces = (depth != maxDepth ? shuffled pool : []) + fallback flat
            let mut target_pieces: Vec<&'static PoolElem> = Vec::new();
            if depth != MAX_DEPTH {
                let mut flat = flat_templates(target_pool);
                shuffle(&mut flat, self.rng);
                target_pieces.extend(flat);
            }
            if let Some(fb) = fallback_pool {
                target_pieces.extend(flat_templates(fb));
            }

            let placement_priority: i32 = 0; // all city jigsaws are 0
            'elements: for target_elem in target_pieces {
                if target_elem.single == Some("") && !target_elem.feature {
                    break; // EmptyPoolElement.INSTANCE → break
                }
                for (rot_i, target_rot) in shuffled_rots(self.rng).into_iter().enumerate() {
                    let _ = rot_i;
                    // list_pool_element: children templates tried IN ORDER at
                    // the same position (vanilla ListPoolElement: each child
                    // is placed stacked at same pos — getShuffledJigsawBlocks
                    // of a list = concat of children's jigsaws). Model: use
                    // the first child for geometry; children share one BB.
                    let child_names: Vec<&'static str> = match target_elem.single {
                        Some(n) => vec![n],
                        None => target_elem.list.to_vec(),
                    };
                    // ListPoolElement: BB = union of all non-empty children,
                    // jigsaws = FIRST child only (vanilla :57), placement
                    // stacks all children at the same position.
                    let mut union_bb: Option<Bb> = None;
                    for cn in &child_names {
                        let Some(t) = tpl_by_name(cn) else { continue };
                        let bb = world_bb(t, (0, 0, 0), target_rot);
                        union_bb = Some(match union_bb {
                            None => bb,
                            Some(u) => Bb {
                                min_x: u.min_x.min(bb.min_x),
                                min_y: u.min_y.min(bb.min_y),
                                min_z: u.min_z.min(bb.min_z),
                                max_x: u.max_x.max(bb.max_x),
                                max_y: u.max_y.max(bb.max_y),
                                max_z: u.max_z.max(bb.max_z),
                            },
                        });
                    }
                    let mut all_jigsaws: Vec<Jigsaw> = Vec::new();
                    if target_elem.feature {
                        // FeaturePoolElement: NO RNG — one jigsaw at the
                        // position (orientation DOWN/SOUTH, name/pool/target
                        // all "minecraft:empty"), BB = Vec3i.ZERO → 1×1×1.
                        all_jigsaws.push(Jigsaw {
                            pos: (0, 0, 0),
                            // FrontAndTop.fromFrontAndTop(DOWN, SOUTH)
                            front: Dir::Down,
                            top: Dir::South,
                            // DEFAULT_JIGSAW_NAME = "minecraft:bottom"
                            name: "minecraft:bottom",
                            target: "minecraft:empty",
                            pool: "minecraft:empty",
                            joint_rollable: true,
                        });
                        // degenerate single-block BB at origin: minY = maxY = 0
                        union_bb = Some(Bb {
                            min_x: 0,
                            max_x: 0,
                            min_y: 0,
                            max_y: 0,
                            min_z: 0,
                            max_z: 0,
                        });
                    } else if let Some(first) = child_names.first() {
                        if let Some(t) = tpl_by_name(first) {
                            let mut tj = world_jigsaws(t, (0, 0, 0), target_rot);
                            // getShuffledJigsawBlocks shuffles per (element,
                            // rotation) — consumes RNG before canAttach.
                            shuffle(&mut tj, self.rng);
                            all_jigsaws.extend(tj);
                        }
                    }
                    let Some(raw_bb) = union_bb else { continue; };
                    let hack_box_y_span = raw_bb.max_y - raw_bb.min_y + 1;
                    let _ = hack_box_y_span; // doExpansionHack=false for city

                    for target_jigsaw in &all_jigsaws {
                        if !can_attach(source_jigsaw, target_jigsaw) {
                            continue;
                        }
                        let (tjx, tjy, tjz) = target_jigsaw.pos;
                        let raw_box_pos = (
                            target_jigsaw_pos.0 - tjx,
                            target_jigsaw_pos.1 - tjy,
                            target_jigsaw_pos.2 - tjz,
                        );
                        // Vanilla: rawTargetBB = element.getBoundingBox AT
                        // rawTargetBoxPos (so its minY = rawTargetBoxPos.y +
                        // template minY = raw_box_pos.y), then moved by
                        // y_offset = target_box_y - rawTargetY. Net: the
                        // template lands with its minY at target_box_y.
                        // both rigid (all city elements are rigid)
                        let delta_y = source_jigsaw_local_y - tjy + source_jigsaw.front.step_y();
                        let target_box_y = source_box_y + delta_y;
                        let y_offset = target_box_y - (raw_box_pos.1 + raw_bb.min_y);
                        let target_bb = raw_bb.moved(
                            raw_box_pos.0,
                            raw_box_pos.1 + y_offset,
                            raw_box_pos.2,
                        );
                        // Overlap: childrenFree (branch) ∩ shrunk(target) or,
                        // if the target jigsaw sits inside the source box,
                        // sourceFree. attachInsideSource:
                        // Vanilla free-shape semantics (BooleanOp decoded —
                        // vanilla SKIPS the candidate when joinIsNotEmpty(
                        // free, shrunk(cand), ONLY_SECOND) is TRUE, i.e. when
                        // (cand ∧ ¬free) ≠ ∅): a candidate is accepted only
                        // when it is FULLY inside the free space (disjoint
                        // from every placed piece and inside the global AABB).
                        // On accept: free := free ∧ ¬cand (ONLY_FIRST carve).
                        let inside = attach_inside_source_hit(source, target_jigsaw_pos);
                        let free: &mut Vec<Bb> = if inside {
                            &mut source_free
                        } else {
                            branch_free
                        };
                        // cand ∧ ¬free ≠ ∅ ⟺ NOT(cand ⊆ free): subtract every
                        // free box from the candidate; a surviving remainder
                        // means part of the candidate lies OUTSIDE free.
                        let pokes_out = {
                            let mut rest: Vec<Bb> = vec![target_bb];
                            for r in free.iter() {
                                if rest.is_empty() {
                                    break;
                                }
                                let mut next_rest: Vec<Bb> = Vec::new();
                                for c in rest.iter() {
                                    next_rest.extend(c.subtract(r));
                                }
                                rest = next_rest;
                            }
                            !rest.is_empty()
                        };
                        if pokes_out {
                            continue; // overlaps placed pieces / global bounds
                        }
                        let mut next: Vec<Bb> = Vec::new();
                        for b in free.iter() {
                            next.extend(b.subtract(&target_bb));
                        }
                        *free = next;
                        if std::env::var_os("NEUTRON_CITY_TRACE").is_some() && self.pieces.len() < 4 {
                            eprintln!(
                                "ATTACH src={} srcbb=({},{},{}) src_jig=({},{},{}) front={:?} local_y={} \
tgt={} tgt_jig=({},{},{}) tgt_front={:?} delta_y={} raw_pos=({},{},{}) y_offset={} \
bb=({},{},{})",
                                source.tpl_name, source.bb.min_x, source.bb.min_y, source.bb.min_z,
                                sjx, sjy, sjz, source_jigsaw.front, source_jigsaw_local_y,
                                child_names[0], tjx, tjy, tjz, target_jigsaw.front,
                                delta_y, raw_box_pos.0, raw_box_pos.1, raw_box_pos.2, y_offset,
                                target_bb.min_x, target_bb.min_y, target_bb.min_z
                            );
                        }
                        if self.pieces.len() > 400 {
                            panic!(
                                "piece cap hit at src={} depth-next pos={:?}",
                                source.tpl_name,
                                (target_bb.min_x, target_bb.min_y, target_bb.min_z)
                            );
                        }
                        let source_ground_level_delta = source.ground_level_delta;
                        let target_ground_level_delta = if target_rigid_probe(child_names[0]) {
                            source_ground_level_delta - delta_y
                        } else {
                            target_elem_ground_level_delta()
                        };
                        self.pieces.push(Piece {
                            depth: depth + 1,
                            tpl_name: child_names[0],
                            position: raw_box_pos,
                            rot: target_rot,
                            bb: target_bb,
                            is_feature: target_elem.feature,
                            ground_level_delta: target_ground_level_delta,
                            free_at_accept: {
                                let mut snap = free.clone();
                                snap.push(target_bb);
                                snap
                            },
                        });
                        if depth + 1 <= MAX_DEPTH {
                            // FIFO: children processed in insertion order.
                            // Handled by the drain loop in `assemble`.
                        }
                        continue 'sources; // vanilla label129: next SOURCE jigsaw
                    }
                }
            }
            let _ = placement_priority;
        }
    }
}
