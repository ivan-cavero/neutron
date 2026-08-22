//! Mineshaft piece types and generation (`MineshaftPieces`).
//!
//! RNG order is parity-critical: every `next_int` here mirrors the Java
//! constructor/addChildren sequence exactly.

use super::{
    MAGIC_START_Y, MAX_DEPTH, MAX_DIST, SEA_LEVEL, WORLD_MIN_Y,
};
use crate::legacy_rng::LegacyRandom;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Dir {
    North,
    South,
    West,
    East,
}

impl Dir {
    pub(super) fn axis_z(self) -> bool {
        matches!(self, Dir::North | Dir::South)
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) struct Bb {
    pub(super) min_x: i32,
    pub(super) min_y: i32,
    pub(super) min_z: i32,
    pub(super) max_x: i32,
    pub(super) max_y: i32,
    pub(super) max_z: i32,
}

impl Bb {
    pub(super) fn new(a: i32, b: i32, c: i32, d: i32, e: i32, f: i32) -> Self {
        Self {
            min_x: a.min(d),
            min_y: b.min(e),
            min_z: c.min(f),
            max_x: a.max(d),
            max_y: b.max(e),
            max_z: c.max(f),
        }
    }

    pub(super) fn x_span(self) -> i32 {
        self.max_x - self.min_x + 1
    }
    pub(super) fn y_span(self) -> i32 {
        self.max_y - self.min_y + 1
    }
    pub(super) fn z_span(self) -> i32 {
        self.max_z - self.min_z + 1
    }

    fn move_by(&mut self, dx: i32, dy: i32, dz: i32) {
        self.min_x += dx;
        self.max_x += dx;
        self.min_y += dy;
        self.max_y += dy;
        self.min_z += dz;
        self.max_z += dz;
    }

    fn intersects(self, o: Bb) -> bool {
        self.max_x >= o.min_x
            && self.min_x <= o.max_x
            && self.max_y >= o.min_y
            && self.min_y <= o.max_y
            && self.max_z >= o.min_z
            && self.min_z <= o.max_z
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) enum Kind {
    Room,
    Corridor,
    Crossing,
    Stairs,
}

#[derive(Clone, Debug)]
pub(super) struct Piece {
    pub(super) kind: Kind,
    pub(super) bb: Bb,
    pub(super) depth: i32,
    pub(super) dir: Dir,
    /// `StructurePiece.orientation` — None for Room/Crossing (NBT O=-1).
    pub(super) orient: Option<Dir>,
    pub(super) entrances: Vec<Bb>,
}

/// `findGenerationPoint` + `moveBelowSeaLevel`: the room seed piece plus all
/// children generated with `setLargeFeatureSeed`.
pub(super) fn generate_start(level_seed: i64, cx: i32, cz: i32) -> Vec<Piece> {
    let mut rng = LegacyRandom::new(0);
    rng.set_large_feature_seed(level_seed, cx, cz);
    // findGenerationPoint: nextDouble() then pop
    let _ = rng.next_f64();

    let rx = (cx << 4) + 2; // ChunkPos.getBlockX(2)
    let rz = (cz << 4) + 2;
    let room_bb = Bb::new(
        rx,
        MAGIC_START_Y,
        rz,
        rx + 7 + rng.next_int(6),
        54 + rng.next_int(6),
        rz + 7 + rng.next_int(6),
    );
    let room = Piece {
        kind: Kind::Room,
        bb: room_bb,
        depth: 0,
        dir: Dir::North,
        orient: None,
        entrances: Vec::new(),
    };
    let mut pieces = vec![room];
    add_room_children(0, &mut pieces, &mut rng);

    // moveBelowSeaLevel(seaLevel, minY, random, 10)
    let y_cap = SEA_LEVEL - 10;
    let union = union_bb(&pieces);
    let mut j = union.y_span() + WORLD_MIN_Y + 1;
    if j < y_cap {
        j += rng.next_int(y_cap - j);
    }
    let dy = j - union.max_y;
    for p in &mut pieces {
        p.bb.move_by(0, dy, 0);
        for e in &mut p.entrances {
            e.move_by(0, dy, 0);
        }
    }
    pieces
}

fn union_bb(pieces: &[Piece]) -> Bb {
    let mut u = pieces[0].bb;
    for p in pieces.iter().skip(1) {
        u.min_x = u.min_x.min(p.bb.min_x);
        u.min_y = u.min_y.min(p.bb.min_y);
        u.min_z = u.min_z.min(p.bb.min_z);
        u.max_x = u.max_x.max(p.bb.max_x);
        u.max_y = u.max_y.max(p.bb.max_y);
        u.max_z = u.max_z.max(p.bb.max_z);
    }
    u
}

fn find_collision(pieces: &[Piece], bb: Bb) -> bool {
    pieces.iter().any(|p| p.bb.intersects(bb))
}

fn generate_and_add(
    start_idx: usize,
    pieces: &mut Vec<Piece>,
    rng: &mut LegacyRandom,
    x: i32,
    y: i32,
    z: i32,
    dir: Dir,
    gen_depth: i32,
) -> Option<usize> {
    if gen_depth > MAX_DEPTH {
        return None;
    }
    let start_bb = pieces[start_idx].bb;
    if (x - start_bb.min_x).abs() > MAX_DIST || (z - start_bb.min_z).abs() > MAX_DIST {
        return None;
    }
    let child_depth = gen_depth + 1;
    let piece = create_random_shaft(pieces, rng, x, y, z, dir, child_depth)?;
    pieces.push(piece);
    let idx = pieces.len() - 1;
    add_children(idx, start_idx, pieces, rng);
    Some(idx)
}

fn create_random_shaft(
    pieces: &[Piece],
    rng: &mut LegacyRandom,
    x: i32,
    y: i32,
    z: i32,
    dir: Dir,
    depth: i32,
) -> Option<Piece> {
    let roll = rng.next_int(100);
    if roll >= 80 {
        if let Some(bb) = find_crossing(pieces, rng, x, y, z, dir) {
            return Some(Piece {
                kind: Kind::Crossing,
                bb,
                depth,
                dir,
                orient: None,
                entrances: Vec::new(),
            });
        }
    } else if roll >= 70 {
        if let Some(bb) = find_stairs(pieces, x, y, z, dir) {
            return Some(Piece {
                kind: Kind::Stairs,
                bb,
                depth,
                dir,
                orient: Some(dir),
                entrances: Vec::new(),
            });
        }
    } else if let Some(bb) = find_corridor(pieces, rng, x, y, z, dir) {
        // Corridor ctor: hasRails = nextInt(3)==0; spider = !rails && nextInt(23)==0
        let _has_rails = rng.next_int(3) == 0;
        if !_has_rails {
            let _spider = rng.next_int(23) == 0;
        }
        return Some(Piece {
            kind: Kind::Corridor,
            bb,
            depth,
            dir,
            orient: Some(dir),
            entrances: Vec::new(),
        });
    }
    None
}

fn find_corridor(
    pieces: &[Piece],
    rng: &mut LegacyRandom,
    x: i32,
    y: i32,
    z: i32,
    dir: Dir,
) -> Option<Bb> {
    let mut n = rng.next_int(3) + 2;
    while n > 0 {
        let len = n * 5;
        let mut bb = match dir {
            Dir::North => Bb::new(0, 0, -(len - 1), 2, 2, 0),
            Dir::South => Bb::new(0, 0, 0, 2, 2, len - 1),
            Dir::West => Bb::new(-(len - 1), 0, 0, 0, 2, 2),
            Dir::East => Bb::new(0, 0, 0, len - 1, 2, 2),
        };
        bb.move_by(x, y, z);
        if !find_collision(pieces, bb) {
            return Some(bb);
        }
        n -= 1;
    }
    None
}

fn find_crossing(
    pieces: &[Piece],
    rng: &mut LegacyRandom,
    x: i32,
    y: i32,
    z: i32,
    dir: Dir,
) -> Option<Bb> {
    let h = if rng.next_int(4) == 0 { 6 } else { 2 };
    let mut bb = match dir {
        Dir::North => Bb::new(-1, 0, -4, 3, h, 0),
        Dir::South => Bb::new(-1, 0, 0, 3, h, 4),
        Dir::West => Bb::new(-4, 0, -1, 0, h, 3),
        Dir::East => Bb::new(0, 0, -1, 4, h, 3),
    };
    bb.move_by(x, y, z);
    if find_collision(pieces, bb) {
        None
    } else {
        Some(bb)
    }
}

fn find_stairs(pieces: &[Piece], x: i32, y: i32, z: i32, dir: Dir) -> Option<Bb> {
    let mut bb = match dir {
        Dir::North => Bb::new(0, -5, -8, 2, 2, 0),
        Dir::South => Bb::new(0, -5, 0, 2, 2, 8),
        Dir::West => Bb::new(-8, -5, 0, 0, 2, 2),
        Dir::East => Bb::new(0, -5, 0, 8, 2, 2),
    };
    bb.move_by(x, y, z);
    if find_collision(pieces, bb) {
        None
    } else {
        Some(bb)
    }
}

fn add_children(idx: usize, start_idx: usize, pieces: &mut Vec<Piece>, rng: &mut LegacyRandom) {
    match pieces[idx].kind {
        Kind::Room => add_room_children(idx, pieces, rng),
        Kind::Corridor => add_corridor_children(idx, start_idx, pieces, rng),
        Kind::Crossing => add_crossing_children(idx, start_idx, pieces, rng),
        Kind::Stairs => add_stairs_children(idx, start_idx, pieces, rng),
    }
}

fn add_room_children(idx: usize, pieces: &mut Vec<Piece>, rng: &mut LegacyRandom) {
    let depth = pieces[idx].depth;
    let bb = pieces[idx].bb;
    let mut y_range = bb.y_span() - 3 - 1;
    if y_range <= 0 {
        y_range = 1;
    }
    // NORTH
    let mut k = 0;
    while k < bb.x_span() {
        k += rng.next_int(bb.x_span());
        if k + 3 > bb.x_span() {
            break;
        }
        let y = bb.min_y + rng.next_int(y_range) + 1;
        if let Some(ci) = generate_and_add(idx, pieces, rng, bb.min_x + k, y, bb.min_z - 1, Dir::North, depth) {
            let c = pieces[ci].bb;
            let r = pieces[idx].bb;
            pieces[idx].entrances.push(Bb::new(
                c.min_x, c.min_y, r.min_z, c.max_x, c.max_y, r.min_z + 1,
            ));
        }
        k += 4;
    }
    // SOUTH
    k = 0;
    while k < bb.x_span() {
        k += rng.next_int(bb.x_span());
        if k + 3 > bb.x_span() {
            break;
        }
        let y = bb.min_y + rng.next_int(y_range) + 1;
        if let Some(ci) = generate_and_add(idx, pieces, rng, bb.min_x + k, y, bb.max_z + 1, Dir::South, depth) {
            let c = pieces[ci].bb;
            let r = pieces[idx].bb;
            pieces[idx].entrances.push(Bb::new(
                c.min_x, c.min_y, r.max_z - 1, c.max_x, c.max_y, r.max_z,
            ));
        }
        k += 4;
    }
    // WEST
    k = 0;
    while k < bb.z_span() {
        k += rng.next_int(bb.z_span());
        if k + 3 > bb.z_span() {
            break;
        }
        let y = bb.min_y + rng.next_int(y_range) + 1;
        if let Some(ci) = generate_and_add(idx, pieces, rng, bb.min_x - 1, y, bb.min_z + k, Dir::West, depth) {
            let c = pieces[ci].bb;
            let r = pieces[idx].bb;
            pieces[idx].entrances.push(Bb::new(
                r.min_x, c.min_y, c.min_z, r.min_x + 1, c.max_y, c.max_z,
            ));
        }
        k += 4;
    }
    // EAST
    k = 0;
    while k < bb.z_span() {
        k += rng.next_int(bb.z_span());
        if k + 3 > bb.z_span() {
            break;
        }
        let y = bb.min_y + rng.next_int(y_range) + 1;
        if let Some(ci) = generate_and_add(idx, pieces, rng, bb.max_x + 1, y, bb.min_z + k, Dir::East, depth) {
            let c = pieces[ci].bb;
            let r = pieces[idx].bb;
            pieces[idx].entrances.push(Bb::new(
                r.max_x - 1, c.min_y, c.min_z, r.max_x, c.max_y, c.max_z,
            ));
        }
        k += 4;
    }
}

fn add_corridor_children(
    idx: usize,
    start_idx: usize,
    pieces: &mut Vec<Piece>,
    rng: &mut LegacyRandom,
) {
    let depth = pieces[idx].depth;
    let bb = pieces[idx].bb;
    let dir = pieces[idx].dir;
    let j = rng.next_int(4);
    let y = bb.min_y - 1 + rng.next_int(3);
    let (x, z, d) = match (dir, j) {
        (Dir::North, 0 | 1) => (bb.min_x, bb.min_z - 1, dir),
        (Dir::North, 2) => (bb.min_x - 1, bb.min_z, Dir::West),
        (Dir::North, _) => (bb.max_x + 1, bb.min_z, Dir::East),
        (Dir::South, 0 | 1) => (bb.min_x, bb.max_z + 1, dir),
        (Dir::South, 2) => (bb.min_x - 1, bb.max_z, Dir::West),
        (Dir::South, _) => (bb.max_x + 1, bb.max_z, Dir::East),
        (Dir::West, 0 | 1) => (bb.min_x - 1, bb.min_z, dir),
        (Dir::West, 2) => (bb.min_x, bb.min_z - 1, Dir::North),
        (Dir::West, _) => (bb.min_x, bb.max_z + 1, Dir::South),
        (Dir::East, 0 | 1) => (bb.max_x + 1, bb.min_z, dir),
        // javap: maxX - 3 (corridor is 3 wide; spawn at west edge of the 3-block strip)
        (Dir::East, 2) => (bb.max_x - 3, bb.min_z - 1, Dir::North),
        (Dir::East, _) => (bb.max_x - 3, bb.max_z + 1, Dir::South),
    };
    let _ = generate_and_add(start_idx, pieces, rng, x, y, z, d, depth);
    if depth < 8 {
        if dir.axis_z() {
            let mut z = bb.min_z + 3;
            while z + 3 <= bb.max_z {
                let n = rng.next_int(5);
                if n == 0 {
                    let _ = generate_and_add(
                        start_idx,
                        pieces,
                        rng,
                        bb.min_x - 1,
                        bb.min_y,
                        z,
                        Dir::West,
                        depth + 1,
                    );
                } else if n == 1 {
                    let _ = generate_and_add(
                        start_idx,
                        pieces,
                        rng,
                        bb.max_x + 1,
                        bb.min_y,
                        z,
                        Dir::East,
                        depth + 1,
                    );
                }
                z += 5;
            }
        } else {
            let mut x = bb.min_x + 3;
            while x + 3 <= bb.max_x {
                let n = rng.next_int(5);
                if n == 0 {
                    let _ = generate_and_add(
                        start_idx,
                        pieces,
                        rng,
                        x,
                        bb.min_y,
                        bb.min_z - 1,
                        Dir::North,
                        depth + 1,
                    );
                } else if n == 1 {
                    let _ = generate_and_add(
                        start_idx,
                        pieces,
                        rng,
                        x,
                        bb.min_y,
                        bb.max_z + 1,
                        Dir::South,
                        depth + 1,
                    );
                }
                x += 5;
            }
        }
    }
}

fn add_crossing_children(
    idx: usize,
    start_idx: usize,
    pieces: &mut Vec<Piece>,
    rng: &mut LegacyRandom,
) {
    let depth = pieces[idx].depth;
    let bb = pieces[idx].bb;
    // javap MineShaftCrossing.addChildren — switch on `direction` (incoming).
    match pieces[idx].dir {
        Dir::North => {
            let _ = generate_and_add(
                start_idx, pieces, rng, bb.min_x + 1, bb.min_y, bb.min_z - 1, Dir::North, depth,
            );
            let _ = generate_and_add(
                start_idx, pieces, rng, bb.min_x - 1, bb.min_y, bb.min_z + 1, Dir::West, depth,
            );
            let _ = generate_and_add(
                start_idx, pieces, rng, bb.max_x + 1, bb.min_y, bb.min_z + 1, Dir::East, depth,
            );
        }
        Dir::South => {
            let _ = generate_and_add(
                start_idx, pieces, rng, bb.min_x + 1, bb.min_y, bb.max_z + 1, Dir::South, depth,
            );
            let _ = generate_and_add(
                start_idx, pieces, rng, bb.min_x - 1, bb.min_y, bb.min_z + 1, Dir::West, depth,
            );
            let _ = generate_and_add(
                start_idx, pieces, rng, bb.max_x + 1, bb.min_y, bb.min_z + 1, Dir::East, depth,
            );
        }
        Dir::West => {
            let _ = generate_and_add(
                start_idx, pieces, rng, bb.min_x + 1, bb.min_y, bb.min_z - 1, Dir::North, depth,
            );
            let _ = generate_and_add(
                start_idx, pieces, rng, bb.min_x + 1, bb.min_y, bb.max_z + 1, Dir::South, depth,
            );
            let _ = generate_and_add(
                start_idx, pieces, rng, bb.min_x - 1, bb.min_y, bb.min_z + 1, Dir::West, depth,
            );
        }
        Dir::East => {
            let _ = generate_and_add(
                start_idx, pieces, rng, bb.min_x + 1, bb.min_y, bb.min_z - 1, Dir::North, depth,
            );
            let _ = generate_and_add(
                start_idx, pieces, rng, bb.min_x + 1, bb.min_y, bb.max_z + 1, Dir::South, depth,
            );
            let _ = generate_and_add(
                start_idx, pieces, rng, bb.max_x + 1, bb.min_y, bb.min_z + 1, Dir::East, depth,
            );
        }
    }
    // Two-floor extra exits: y = minY+3+1, each gated by nextBoolean.
    if pieces[idx].bb.y_span() > 3 {
        let y2 = pieces[idx].bb.min_y + 3 + 1;
        let bb = pieces[idx].bb;
        if rng.next_boolean() {
            let _ = generate_and_add(
                start_idx, pieces, rng, bb.min_x + 1, y2, bb.min_z - 1, Dir::North, depth,
            );
        }
        if rng.next_boolean() {
            let _ = generate_and_add(
                start_idx, pieces, rng, bb.min_x - 1, y2, bb.min_z + 1, Dir::West, depth,
            );
        }
        if rng.next_boolean() {
            let _ = generate_and_add(
                start_idx, pieces, rng, bb.max_x + 1, y2, bb.min_z + 1, Dir::East, depth,
            );
        }
        if rng.next_boolean() {
            let _ = generate_and_add(
                start_idx, pieces, rng, bb.min_x + 1, y2, bb.max_z + 1, Dir::South, depth,
            );
        }
    }
}

fn add_stairs_children(
    idx: usize,
    start_idx: usize,
    pieces: &mut Vec<Piece>,
    rng: &mut LegacyRandom,
) {
    let depth = pieces[idx].depth;
    let bb = pieces[idx].bb;
    match pieces[idx].dir {
        Dir::North => {
            let _ = generate_and_add(
                start_idx, pieces, rng, bb.min_x, bb.min_y, bb.min_z - 1, Dir::North, depth,
            );
        }
        Dir::South => {
            let _ = generate_and_add(
                start_idx, pieces, rng, bb.min_x, bb.min_y, bb.max_z + 1, Dir::South, depth,
            );
        }
        Dir::West => {
            let _ = generate_and_add(
                start_idx, pieces, rng, bb.min_x - 1, bb.min_y, bb.min_z, Dir::West, depth,
            );
        }
        Dir::East => {
            let _ = generate_and_add(
                start_idx, pieces, rng, bb.max_x + 1, bb.min_y, bb.min_z, Dir::East, depth,
            );
        }
    }
}
