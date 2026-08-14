// Copyright (c) 2026 Neutron Contributors -- MIT License
//
// MineshaftStructure + MineshaftPieces for Minecraft 26.2.
//
// javap:
//   StructurePlacement.legacyProbabilityReducerWithDouble (legacy_type_3)
//   Structure.GenerationContext.makeRandom = LegacyRandom + setLargeFeatureSeed
//   MineshaftStructure.findGenerationPoint / generatePiecesAndAdjust
//   MineshaftPieces createRandomShaftPiece / generateAndAddPiece
//   MineShaftRoom / Corridor / Crossing / Stairs
// datapack: worldgen/structure_set/mineshafts.json (spacing 1, frequency 0.004)

use crate::legacy_rng::LegacyRandom;
use crate::region_buf::RegionBuf;
use crate::surface::BlockId;
use crate::worldgen::WorldgenState;
use crate::biome_source::biome_id;

const FREQUENCY: f64 = 0.004;
const MAGIC_START_Y: i32 = 50;
const MAX_DEPTH: i32 = 8;
const MAX_DIST: i32 = 80;
const SEA_LEVEL: i32 = 63;
const WORLD_MIN_Y: i32 = -64;
const SEARCH_RADIUS: i32 = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Dir {
    North,
    South,
    West,
    East,
}

impl Dir {
    fn axis_z(self) -> bool {
        matches!(self, Dir::North | Dir::South)
    }
}

#[derive(Clone, Copy, Debug)]
struct Bb {
    min_x: i32,
    min_y: i32,
    min_z: i32,
    max_x: i32,
    max_y: i32,
    max_z: i32,
}

impl Bb {
    fn new(a: i32, b: i32, c: i32, d: i32, e: i32, f: i32) -> Self {
        Self {
            min_x: a.min(d),
            min_y: b.min(e),
            min_z: c.min(f),
            max_x: a.max(d),
            max_y: b.max(e),
            max_z: c.max(f),
        }
    }

    fn x_span(self) -> i32 {
        self.max_x - self.min_x + 1
    }
    fn y_span(self) -> i32 {
        self.max_y - self.min_y + 1
    }
    fn z_span(self) -> i32 {
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
enum Kind {
    Room,
    Corridor,
    Crossing,
    Stairs,
}

#[derive(Clone, Debug)]
struct Piece {
    kind: Kind,
    bb: Bb,
    depth: i32,
    dir: Dir,
    /// `StructurePiece.orientation` — None for Room/Crossing (NBT O=-1).
    orient: Option<Dir>,
    entrances: Vec<Bb>,
}

/// `legacy_type_3` = `legacyProbabilityReducerWithDouble`.
pub fn is_mineshaft_chunk(level_seed: i64, cx: i32, cz: i32) -> bool {
    let mut rng = LegacyRandom::new(0);
    rng.set_large_feature_seed(level_seed, cx, cz);
    rng.next_f64() < FREQUENCY
}

/// Generate mineshaft pieces that intersect `region` (starts in ±SEARCH_RADIUS).
pub fn apply_mineshafts_region(region: &mut RegionBuf, state: &WorldgenState) {
    let c0x = region.origin_x.div_euclid(16);
    let c0z = region.origin_z.div_euclid(16);
    let c1x = c0x + region.chunks - 1;
    let c1z = c0z + region.chunks - 1;
    for cz in (c0z - SEARCH_RADIUS)..=(c1z + SEARCH_RADIUS) {
        for cx in (c0x - SEARCH_RADIUS)..=(c1x + SEARCH_RADIUS) {
            if !is_mineshaft_chunk(state.seed, cx, cz) {
                continue;
            }
            let pieces = generate_start(state.seed, cx, cz);
            if pieces.is_empty() {
                continue;
            }
            // isValidBiome at stub (middleX, 50+dy, minZ) after vertical adjust.
            // dy is already applied inside generate_start; stub y = first room minY
            // after moveBelowSeaLevel ≈ MAGIC_START_Y + offset. Use room bb min.
            let stub_x = (cx << 4) + 8;
            let stub_z = cz << 4;
            let stub_y = pieces[0].bb.min_y;
            if biome_id::DEEP_DARK == crate::biome_source::biome_id_at_block(state, stub_x, stub_y, stub_z)
            {
                continue;
            }
            place_pieces(region, &pieces, state.seed);
        }
    }
}

fn generate_start(level_seed: i64, cx: i32, cz: i32) -> Vec<Piece> {
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

fn world_pos(p: &Piece, x: i32, y: i32, z: i32) -> (i32, i32, i32) {
    let Some(dir) = p.orient else {
        return (x, y, z);
    };
    let wy = y + p.bb.min_y;
    match dir {
        Dir::North => (p.bb.min_x + x, wy, p.bb.max_z - z),
        Dir::South => (p.bb.min_x + x, wy, p.bb.min_z + z),
        Dir::West => (p.bb.max_x - z, wy, p.bb.min_z + x),
        Dir::East => (p.bb.min_x + z, wy, p.bb.min_z + x),
    }
}

fn can_replace(b: BlockId) -> bool {
    // MineShaftPiece.canBeReplaced: anything except our own wood/fence/chain.
    !matches!(
        b,
        BlockId::OakPlanks | BlockId::OakFence | BlockId::OakLog | BlockId::DarkOakLog
    )
}

fn generate_box(
    region: &mut RegionBuf,
    p: &Piece,
    x0: i32,
    y0: i32,
    z0: i32,
    x1: i32,
    y1: i32,
    z1: i32,
    block: BlockId,
) {
    let (x0, x1) = (x0.min(x1), x0.max(x1));
    let (y0, y1) = (y0.min(y1), y0.max(y1));
    let (z0, z1) = (z0.min(z1), z0.max(z1));
    for y in y0..=y1 {
        for x in x0..=x1 {
            for z in z0..=z1 {
                let (wx, wy, wz) = world_pos(p, x, y, z);
                if region.index(wx, wy, wz).is_none() {
                    continue;
                }
                if can_replace(region.get(wx, wy, wz)) {
                    region.set(wx, wy, wz, block);
                }
            }
        }
    }
}

fn generate_maybe_box(
    region: &mut RegionBuf,
    p: &Piece,
    rng: &mut LegacyRandom,
    chance: f32,
    x0: i32,
    y0: i32,
    z0: i32,
    x1: i32,
    y1: i32,
    z1: i32,
    block: BlockId,
) {
    let (x0, x1) = (x0.min(x1), x0.max(x1));
    let (y0, y1) = (y0.min(y1), y0.max(y1));
    let (z0, z1) = (z0.min(z1), z0.max(z1));
    for y in y0..=y1 {
        for x in x0..=x1 {
            for z in z0..=z1 {
                if rng.next_f32() >= chance {
                    continue;
                }
                let (wx, wy, wz) = world_pos(p, x, y, z);
                if region.index(wx, wy, wz).is_none() {
                    continue;
                }
                if can_replace(region.get(wx, wy, wz)) {
                    region.set(wx, wy, wz, block);
                }
            }
        }
    }
}

fn get_block(region: &RegionBuf, p: &Piece, x: i32, y: i32, z: i32) -> BlockId {
    let (wx, wy, wz) = world_pos(p, x, y, z);
    region.get(wx, wy, wz)
}

fn is_supporting_box(region: &RegionBuf, p: &Piece, x0: i32, x1: i32, y: i32, z: i32) -> bool {
    for x in x0..=x1 {
        if get_block(region, p, x, y + 1, z).is_air() {
            return false;
        }
    }
    true
}

fn place_support(
    region: &mut RegionBuf,
    p: &Piece,
    rng: &mut LegacyRandom,
    x0: i32,
    y0: i32,
    z: i32,
    x1: i32,
    y1: i32,
) {
    if !is_supporting_box(region, p, x0, x1, y1, z) {
        return;
    }
    generate_box(region, p, x0, y0, z, x0, y1 - 1, z, BlockId::OakFence);
    generate_box(region, p, x1, y0, z, x1, y1 - 1, z, BlockId::OakFence);
    if rng.next_int(4) == 0 {
        generate_box(region, p, x0, y1, z, x0, y1, z, BlockId::OakPlanks);
        generate_box(region, p, x1, y1, z, x1, y1, z, BlockId::OakPlanks);
    } else {
        generate_box(region, p, x0, y1, z, x1, y1, z, BlockId::OakPlanks);
    }
}

fn set_planks_block(region: &mut RegionBuf, p: &Piece, x: i32, y: i32, z: i32) {
    let (wx, wy, wz) = world_pos(p, x, y, z);
    if region.index(wx, wy, wz).is_none() {
        return;
    }
    // isInterior ≈ the cell is air (open).
    if !region.get(wx, wy, wz).is_air() {
        return;
    }
    region.set(wx, wy, wz, BlockId::OakPlanks);
}

fn place_pieces(region: &mut RegionBuf, pieces: &[Piece], level_seed: i64) {
    let mut rng = LegacyRandom::new(0);
    rng.set_large_feature_seed(level_seed, 0, 0);
    for p in pieces {
        match p.kind {
            Kind::Room => {
                let top = (p.bb.min_y + 3).min(p.bb.max_y);
                generate_box(
                    region,
                    p,
                    p.bb.min_x,
                    p.bb.min_y + 1,
                    p.bb.min_z,
                    p.bb.max_x,
                    top,
                    p.bb.max_z,
                    BlockId::Air,
                );
                for e in &p.entrances {
                    generate_box(
                        region,
                        p,
                        e.min_x,
                        e.max_y - 2,
                        e.min_z,
                        e.max_x,
                        e.max_y,
                        e.max_z,
                        BlockId::Air,
                    );
                }
            }
            Kind::Corridor => {
                let nsec = if p.dir.axis_z() {
                    p.bb.z_span() / 5
                } else {
                    p.bb.x_span() / 5
                };
                let len = nsec * 5 - 1;
                generate_box(region, p, 0, 0, 0, 2, 1, len, BlockId::Air);
                generate_maybe_box(region, p, &mut rng, 0.8, 0, 2, 0, 2, 2, len, BlockId::Air);
                for sec in 0..nsec {
                    let z = 2 + sec * 5;
                    place_support(region, p, &mut rng, 0, 0, z, 2, 2);
                }
                for x in 0..=2 {
                    for z in 0..=len {
                        set_planks_block(region, p, x, -1, z);
                    }
                }
            }
            Kind::Crossing => {
                if p.bb.y_span() > 3 {
                    let y1 = p.bb.min_y + 3 - 1;
                    generate_box(
                        region, p,
                        p.bb.min_x + 1, p.bb.min_y, p.bb.min_z,
                        p.bb.max_x - 1, y1, p.bb.max_z,
                        BlockId::Air,
                    );
                    generate_box(
                        region, p,
                        p.bb.min_x, p.bb.min_y, p.bb.min_z + 1,
                        p.bb.max_x, y1, p.bb.max_z - 1,
                        BlockId::Air,
                    );
                    generate_box(
                        region, p,
                        p.bb.min_x + 1, p.bb.max_y - 2, p.bb.min_z,
                        p.bb.max_x - 1, p.bb.max_y, p.bb.max_z,
                        BlockId::Air,
                    );
                    generate_box(
                        region, p,
                        p.bb.min_x, p.bb.max_y - 2, p.bb.min_z + 1,
                        p.bb.max_x, p.bb.max_y, p.bb.max_z - 1,
                        BlockId::Air,
                    );
                } else {
                    generate_box(
                        region, p,
                        p.bb.min_x + 1, p.bb.min_y, p.bb.min_z,
                        p.bb.max_x - 1, p.bb.max_y, p.bb.max_z,
                        BlockId::Air,
                    );
                    generate_box(
                        region, p,
                        p.bb.min_x, p.bb.min_y, p.bb.min_z + 1,
                        p.bb.max_x, p.bb.max_y, p.bb.max_z - 1,
                        BlockId::Air,
                    );
                }
            }
            Kind::Stairs => {
                generate_box(region, p, 0, 5, 0, 2, 7, 1, BlockId::Air);
                generate_box(region, p, 0, 0, 7, 2, 2, 8, BlockId::Air);
                for i in 0..5 {
                    let z0 = 5 - i - if i < 4 { 1 } else { 0 };
                    generate_box(region, p, 0, z0, 2 + i, 2, 7 - i, 2 + i, BlockId::Air);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn start_4_minus_1_first_eight_bbs_match_vanilla_nbt() {
        let pieces = generate_start(12345, 4, -1);
        let van = [
            (66, -44, -14, 75, -39, -5),
            (73, -48, -23, 75, -41, -15),
            (73, -48, -43, 75, -46, -24),
            (76, -47, -44, 80, -45, -40),
            (77, -47, -59, 79, -45, -45),
            (77, -51, -68, 79, -44, -60),
            (76, -51, -73, 80, -49, -69),
            (77, -51, -83, 79, -49, -74),
        ];
        assert_eq!(pieces.len(), 121);
        for (i, &(x0, y0, z0, x1, y1, z1)) in van.iter().enumerate() {
            let b = pieces[i].bb;
            assert_eq!(
                (b.min_x, b.min_y, b.min_z, b.max_x, b.max_y, b.max_z),
                (x0, y0, z0, x1, y1, z1),
                "piece {i}"
            );
        }
    }

    #[test]
    fn seed_12345_has_start_at_4_minus_1() {
        // Vanilla chunk (6,-2) structures.References.mineshaft = ChunkPos(4,-1).
        assert!(
            is_mineshaft_chunk(12345, 4, -1),
            "legacy_type_3 must accept (4,-1) for seed 12345"
        );
    }

    #[test]
    fn dump_start_4_minus_1() {
        let pieces = generate_start(12345, 4, -1);
        assert_eq!(pieces.len(), 121);
        assert_eq!(pieces[0].bb.min_y, -44);
    }

    #[test]
    fn apply_carves_west_neighbor() {
        let g = crate::generator::ChunkGenerator::new(12345);
        let region = g.generate_ores_region(6, -2);
        let mut air = 0u32;
        let mut planks = 0u32;
        for y in -64..16 {
            for z in -32..-16 {
                for x in 80..96 {
                    match region.get(x, y, z) {
                        BlockId::Air => air += 1,
                        BlockId::OakPlanks => planks += 1,
                        _ => {}
                    }
                }
            }
        }
        eprintln!("(5,-2) y<16 air={air} planks={planks}");
        assert!(air > 0, "mineshaft must carve air into (5,-2)");
    }
}