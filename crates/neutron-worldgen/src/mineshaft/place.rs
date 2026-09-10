//! Mineshaft piece placement into the region (`postProcess`).
//!
//! Carving, supports and the `isInInvalidLocation` gate. RNG order mirrors
//! vanilla's `postProcess` draws: the decoration `WorldgenRandom` (Xoroshiro)
//! shared across pieces, reseeded per origin.

use super::pieces::{Bb, Dir, Kind, Piece};
use super::WORLD_MIN_Y;
use crate::feature_rng::FeatureRandom;
use crate::region_buf::RegionBuf;
use crate::surface::BlockId;
use crate::worldgen::WorldgenState;
use crate::biome_source::biome_id;

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
    clip: &super::pieces::Bb,
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
                if in_clip(clip, wx, wz) && can_replace(region.get(wx, wy, wz)) {
                    region.set(wx, wy, wz, block);
                }
            }
        }
    }
}

/// `maybePlaceCobWeb`: isInterior FIRST (no draw when false), then the roll,
/// then hasSturdyNeighbours(2) — vanilla short-circuit order.
fn maybe_place_cobweb(
    region: &mut RegionBuf,
    p: &Piece,
    clip: &super::pieces::Bb,
    rng: &mut FeatureRandom,
    probability: f32,
    x: i32,
    y: i32,
    z: i32,
) {
    let (wx, wy, wz) = world_pos(p, x, y, z);
    if region.index(wx, wy, wz).is_none() {
        return;
    }
    if !is_interior(region, clip, wx, wy, wz) {
        return;
    }
    if rng.next_f32() >= probability {
        return;
    }
    if !has_sturdy_neighbours(region, wx, wy, wz, 2) {
        return;
    }
    region.set(wx, wy, wz, BlockId::Cobweb);
}

/// `StructurePiece.isInterior`: pos (at y+1) inside chunkBB and below the
/// OCEAN_FLOOR_WG heightmap.
fn is_interior(region: &mut RegionBuf, clip: &super::pieces::Bb, wx: i32, wy: i32, wz: i32) -> bool {
    if wx < clip.min_x || wx > clip.max_x || wz < clip.min_z || wz > clip.max_z {
        return false;
    }
    wy + 1 < region.height_at(wx, wz)
}

fn has_sturdy_neighbours(region: &RegionBuf, wx: i32, wy: i32, wz: i32, count: u32) -> bool {
    const DIRS: [(i32, i32, i32); 6] = [
        (1, 0, 0),
        (-1, 0, 0),
        (0, 1, 0),
        (0, -1, 0),
        (0, 0, 1),
        (0, 0, -1),
    ];
    let mut sturdy = 0u32;
    for (dx, dy, dz) in DIRS {
        let b = region.get(wx + dx, wy + dy, wz + dz);
        if crate::multiface_spreader::is_face_sturdy_full(b) {
            sturdy += 1;
            if sturdy >= count {
                return true;
            }
        }
    }
    false
}

fn is_solid_render(b: BlockId) -> bool {
    // isSolidRender ~= full opaque cube: reject air/transparent/replacable.
    crate::multiface_spreader::is_face_sturdy_full(b)
}

/// `createChest`: draws nextLong (loot seed) only when the pos is inside
/// chunkBB and not already a chest. Chest block entity content lives in the
/// ref's block-entity NBT (not the block grid) — place the block only.
fn try_place_chest(
    region: &mut RegionBuf,
    p: &Piece,
    clip: &super::pieces::Bb,
    x: i32,
    y: i32,
    z: i32,
    rng: &mut FeatureRandom,
) {
    let (wx, wy, wz) = world_pos(p, x, y, z);
    if !in_clip(clip, wx, wz) || region.index(wx, wy, wz).is_none() {
        return;
    }
    if region.get(wx, wy, wz) == BlockId::Chest {
        return;
    }
    let _ = rng.next_long();
    region.set(wx, wy, wz, BlockId::Chest);
}

/// Vanilla clips every structure write to the chunk being decorated
/// (`chunkBB.isInside` in StructurePiece.placeBlock) — each cell is written
/// by exactly ONE origin: its own chunk's origin.
fn in_clip(clip: &super::pieces::Bb, wx: i32, wz: i32) -> bool {
    wx >= clip.min_x && wx <= clip.max_x && wz >= clip.min_z && wz <= clip.max_z
}

fn generate_maybe_box(
    region: &mut RegionBuf,
    p: &Piece,
    clip: &super::pieces::Bb,
    rng: &mut FeatureRandom,
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
                if in_clip(clip, wx, wz) && can_replace(region.get(wx, wy, wz)) {
                    region.set(wx, wy, wz, block);
                }
            }
        }
    }
}

/// Port of `StructurePiece.generateUpperHalfSphere` used by mine rooms.
///
/// Vanilla accepts points whose squared normalized distance is at most 1.05;
/// keep that threshold and the room's replaceability rules identical to
/// `generate_box`.
fn generate_upper_half_sphere(
    region: &mut RegionBuf,
    p: &Piece,
    clip: &super::pieces::Bb,
    x0: i32,
    y0: i32,
    z0: i32,
    x1: i32,
    y1: i32,
    z1: i32,
) {
    let diag_x = (x1 - x0 + 1) as f32;
    let diag_y = (y1 - y0 + 1) as f32;
    let diag_z = (z1 - z0 + 1) as f32;
    let center_x = x0 as f32 + diag_x / 2.0;
    let center_z = z0 as f32 + diag_z / 2.0;

    for y in y0..=y1 {
        let normalized_y = (y - y0) as f32 / diag_y;
        for x in x0..=x1 {
            let normalized_x = (x as f32 - center_x) / (diag_x * 0.5);
            for z in z0..=z1 {
                let normalized_z = (z as f32 - center_z) / (diag_z * 0.5);
                let distance = normalized_x * normalized_x
                    + normalized_y * normalized_y
                    + normalized_z * normalized_z;
                if distance > 1.05 {
                    continue;
                }
                let (wx, wy, wz) = world_pos(p, x, y, z);
                if region.index(wx, wy, wz).is_some()
                    && in_clip(clip, wx, wz)
                    && can_replace(region.get(wx, wy, wz))
                {
                    // ponytail: vanilla writes CAVE_AIR here, but our piece
                    // layout still diverges from vanilla's — labeling these
                    // cave_air exposed the desync as a -0.01pp region loss
                    // (424242). Restore after mineshaft layout parity.
                    region.set(wx, wy, wz, BlockId::CaveAir); // MineshaftPieces air state = Blocks.CAVE_AIR
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
    clip: &super::pieces::Bb,
    rng: &mut FeatureRandom,
    x0: i32,
    y0: i32,
    z: i32,
    x1: i32,
    y1: i32,
) {
    if !is_supporting_box(region, p, x0, x1, y1, z) {
        return;
    }
    generate_box(region, p, clip, x0, y0, z, x0, y1 - 1, z, BlockId::OakFence);
    generate_box(region, p, clip, x1, y0, z, x1, y1 - 1, z, BlockId::OakFence);
    if rng.next_int(4) == 0 {
        generate_box(region, p, clip, x0, y1, z, x0, y1, z, BlockId::OakPlanks);
        generate_box(region, p, clip, x1, y1, z, x1, y1, z, BlockId::OakPlanks);
    } else {
        generate_box(region, p, clip, x0, y1, z, x1, y1, z, BlockId::OakPlanks);
        // Vanilla placeSupport else-branch: two wall-torch rolls (0.05 each,
        // SOUTH at z-1 / NORTH at z+1). These consume the RNG stream even
        // when the roll fails — skipping them desynced every later draw of
        // the corridor's support/cobweb/torch sequence.
        if rng.next_f32() < 0.05 {
            maybe_generate_block(region, p, clip, x0 + 1, y1, z - 1, BlockId::WallTorch);
        }
        if rng.next_f32() < 0.05 {
            maybe_generate_block(region, p, clip, x0 + 1, y1, z + 1, BlockId::WallTorch);
        }
    }
}

/// `StructurePiece.maybeGenerateBlock`: place only when the cell is
/// air/open (isInterior) and inside the region.
fn maybe_generate_block(
    region: &mut RegionBuf,
    p: &Piece,
    clip: &super::pieces::Bb,
    x: i32,
    y: i32,
    z: i32,
    block: BlockId,
) {
    let (wx, wy, wz) = world_pos(p, x, y, z);
    if region.index(wx, wy, wz).is_none() {
        return;
    }
    if !in_clip(clip, wx, wz) || !region.get(wx, wy, wz).is_air() {
        return;
    }
    region.set(wx, wy, wz, block);
}

fn set_planks_block(
    region: &mut RegionBuf,
    p: &Piece,
    clip: &super::pieces::Bb,
    x: i32,
    y: i32,
    z: i32,
) {
    let (wx, wy, wz) = world_pos(p, x, y, z);
    if region.index(wx, wy, wz).is_none() {
        return;
    }
    // isInterior ≈ the cell is air (open).
    if !in_clip(clip, wx, wz) || !region.get(wx, wy, wz).is_air() {
        return;
    }
    region.set(wx, wy, wz, BlockId::OakPlanks);
}

/// Piece RNG: vanilla passes the decoration `WorldgenRandom` (Xoroshiro) into
/// `postProcess`, reseeded per origin via
/// `setFeatureSeed(decorationSeed, structureIndex, step)` (ChunkGenerator
/// line 326 + 348). `decorationSeed = setDecorationSeed(seed, ox, oz)`;
/// mineshaft index within step 3 (UNDERGROUND_STRUCTURES) = 1
/// (buried_treasure=0, mineshaft=1, mineshaft_mesa=2, trail_ruins=3,
/// trial_chambers=4). The stream CONTINUES across pieces and starts of the
/// same origin (StructureStart.placeInChunk passes the same `random`).
pub(super) fn place_pieces(
    region: &mut RegionBuf,
    pieces: &[Piece],
    state: &WorldgenState,
    rng: &mut FeatureRandom,
    clip: &super::pieces::Bb,
) {
    for p in pieces {
        if is_in_invalid_location(region, state, p) {
            continue;
        }
        match p.kind {
            Kind::Room => {
                let top = (p.bb.min_y + 3).min(p.bb.max_y);
                generate_box(
                    region,
                    p,
                    clip,
                    p.bb.min_x,
                    p.bb.min_y + 1,
                    p.bb.min_z,
                    p.bb.max_x,
                    top,
                    p.bb.max_z,
                    BlockId::CaveAir,
                );
                for e in &p.entrances {
                    generate_box(
                        region,
                        p,
                        clip,
                        e.min_x,
                        e.max_y - 2,
                        e.min_z,
                        e.max_x,
                        e.max_y,
                        e.max_z,
                        BlockId::CaveAir,
                    );
                }
                generate_upper_half_sphere(
                    region,
                    p,
                    clip,
                    p.bb.min_x,
                    p.bb.min_y + 4,
                    p.bb.min_z,
                    p.bb.max_x,
                    p.bb.max_y,
                    p.bb.max_z,
                );
            }
            Kind::Corridor => {
                let nsec = if p.dir.axis_z() {
                    p.bb.z_span() / 5
                } else {
                    p.bb.x_span() / 5
                };
                let len = nsec * 5 - 1;
                generate_box(region, p, clip, 0, 0, 0, 2, 1, len, BlockId::CaveAir);
                generate_maybe_box(region, p, clip, rng, 0.8, 0, 2, 0, 2, 2, len, BlockId::CaveAir);
                if p.spider {
                    // spiderCorridor: generateMaybeBox(0.6, cobwebs, skipAir=false,
                    // hasToBeInside=true)
                    generate_maybe_box(
                        region,
                        p,
                        clip,
                        rng,
                        0.6,
                        0,
                        0,
                        0,
                        2,
                        1,
                        len,
                        BlockId::Cobweb,
                    );
                }
                for sec in 0..nsec {
                    let z = 2 + sec * 5;
                    place_support(region, p, clip, rng, 0, 0, z, 2, 2);
                    // 8 cobweb rolls per section — maybePlaceCobWeb short-circuits
                    // BEFORE the roll when !isInterior (chunkBB + heightmap gate).
                    maybe_place_cobweb(region, p, clip, rng, 0.1, 0, 2, z - 1);
                    maybe_place_cobweb(region, p, clip, rng, 0.1, 2, 2, z - 1);
                    maybe_place_cobweb(region, p, clip, rng, 0.1, 0, 2, z + 1);
                    maybe_place_cobweb(region, p, clip, rng, 0.1, 2, 2, z + 1);
                    maybe_place_cobweb(region, p, clip, rng, 0.05, 0, 2, z - 2);
                    maybe_place_cobweb(region, p, clip, rng, 0.05, 2, 2, z - 2);
                    maybe_place_cobweb(region, p, clip, rng, 0.05, 0, 2, z + 2);
                    maybe_place_cobweb(region, p, clip, rng, 0.05, 2, 2, z + 2);
                    // Two chest rolls (nextInt(100)); a hit draws nextLong ONLY when
                    // the chest pos is inside chunkBB and not already a chest.
                    if rng.next_int(100) == 0 {
                        try_place_chest(region, p, clip, 2, 0, z - 1, rng);
                    }
                    if rng.next_int(100) == 0 {
                        try_place_chest(region, p, clip, 0, 0, z + 1, rng);
                    }
                    // Spider-corridor spawner: one roll for the section z.
                    if p.spider {
                        let new_z = z - 1 + rng.next_int(3);
                        let (wx, wy, wz) = world_pos(p, 1, 0, new_z);
                        if in_clip(clip, wx, wz) && is_interior(region, clip, wx, wy, wz) {
                            region.set(wx, wy, wz, BlockId::Spawner);
                            // setEntityId → spawnPotentials.getRandom on a fresh
                            // spawner: empty selector returns Optional.empty()
                            // WITHOUT drawing (WeightedList.getRandom selector
                            // null-check) — zero RNG consumption.
                        }
                    }
                }
                for x in 0..=2 {
                    for z in 0..=len {
                        set_planks_block(region, p, clip, x, -1, z);
                    }
                }
                // placeDoubleLowerOrUpperSupport (RNG-free: fillPillarDownOrChainUp
                // only fires when the cell above is planks — geometry-gated).
                if p.rails {
                    for z in 0..=len {
                        let (fx, fy, fz) = world_pos(p, 1, -1, z);
                        if region.index(fx, fy, fz).is_none() {
                            continue;
                        }
                        let floor = region.get(fx, fy, fz);
                        let solid_render = is_solid_render(floor);
                        if !solid_render {
                            continue;
                        }
                        // probability 0.7 interior / 0.9 otherwise — the roll happens
                        // whenever the floor is solid, even when the rail cell is
                        // outside chunkBB (maybeGenerateBlock draws unconditionally).
                        let interior = is_interior(region, clip, fx, fy + 1, fz);
                        let prob = if interior { 0.7 } else { 0.9 };
                        let hit = rng.next_f32() < prob;
                        if hit {
                            maybe_generate_block(region, p, clip, 1, 0, z, BlockId::Rail);
                        }
                    }
                }
            }
            Kind::Crossing => {
                if p.bb.y_span() > 3 {
                    let y1 = p.bb.min_y + 3 - 1;
                    generate_box(
                        region, p, clip,
                        p.bb.min_x + 1, p.bb.min_y, p.bb.min_z,
                        p.bb.max_x - 1, y1, p.bb.max_z,
                        BlockId::CaveAir,
                    );
                    generate_box(
                        region, p, clip,
                        p.bb.min_x, p.bb.min_y, p.bb.min_z + 1,
                        p.bb.max_x, y1, p.bb.max_z - 1,
                        BlockId::CaveAir,
                    );
                    generate_box(
                        region, p, clip,
                        p.bb.min_x + 1, p.bb.max_y - 2, p.bb.min_z,
                        p.bb.max_x - 1, p.bb.max_y, p.bb.max_z,
                        BlockId::CaveAir,
                    );
                    generate_box(
                        region, p, clip,
                        p.bb.min_x, p.bb.max_y - 2, p.bb.min_z + 1,
                        p.bb.max_x, p.bb.max_y, p.bb.max_z - 1,
                        BlockId::CaveAir,
                    );
                } else {
                    generate_box(
                        region, p, clip,
                        p.bb.min_x + 1, p.bb.min_y, p.bb.min_z,
                        p.bb.max_x - 1, p.bb.max_y, p.bb.max_z,
                        BlockId::CaveAir,
                    );
                    generate_box(
                        region, p, clip,
                        p.bb.min_x, p.bb.min_y, p.bb.min_z + 1,
                        p.bb.max_x, p.bb.max_y, p.bb.max_z - 1,
                        BlockId::CaveAir,
                    );
                }
            }
            Kind::Stairs => {
                generate_box(region, p, clip, 0, 5, 0, 2, 7, 1, BlockId::CaveAir);
                generate_box(region, p, clip, 0, 0, 7, 2, 2, 8, BlockId::CaveAir);
                for i in 0..5 {
                    let z0 = 5 - i - if i < 4 { 1 } else { 0 };
                    generate_box(region, p, clip, 0, z0, 2 + i, 2, 7 - i, 2 + i, BlockId::CaveAir);
                }
            }
        }
    }
}

/// Port of `MineshaftPiece.isInInvalidLocation` for the generated region.
///
/// Vanilla skips a piece when its expanded bounding box touches a liquid or
/// belongs to the mineshaft-blocking `deep_dark` biome. The old implementation
/// only checked one approximate point of the structure start, which allowed
/// invalid pieces to carve air and subsequently changed sculk placement.
fn is_in_invalid_location(region: &RegionBuf, state: &WorldgenState, p: &Piece) -> bool {
    let x0 = (p.bb.min_x - 1).max(region.origin_x);
    let y0 = (p.bb.min_y - 1).max(WORLD_MIN_Y);
    let z0 = (p.bb.min_z - 1).max(region.origin_z);
    let x1 = (p.bb.max_x + 1).min(region.origin_x + region.side - 1);
    let y1 = (p.bb.max_y + 1).min(crate::generator::WORLD_TOP - 1);
    let z1 = (p.bb.max_z + 1).min(region.origin_z + region.side - 1);

    if x0 > x1 || y0 > y1 || z0 > z1 {
        return false;
    }

    let center_x = (x0 + x1) / 2;
    let center_y = (y0 + y1) / 2;
    let center_z = (z0 + z1) / 2;
    if biome_id::DEEP_DARK == crate::biome_source::biome_id_at_block(
        state, center_x, center_y, center_z,
    ) {
        return true;
    }

    for x in x0..=x1 {
        for z in z0..=z1 {
            if region.get(x, y0, z).is_fluid() || region.get(x, y1, z).is_fluid() {
                return true;
            }
        }
    }
    for x in x0..=x1 {
        for y in y0..=y1 {
            if region.get(x, y, z0).is_fluid() || region.get(x, y, z1).is_fluid() {
                return true;
            }
        }
    }
    for z in z0..=z1 {
        for y in y0..=y1 {
            if region.get(x0, y, z).is_fluid() || region.get(x1, y, z).is_fluid() {
                return true;
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::worldgen::WorldgenState;

    #[test]
    fn invalid_piece_rejects_boundary_liquid() {
        let state = WorldgenState::overworld(12345);
        let mut region = RegionBuf::new(0, 0, 0);
        let piece = Piece {
            kind: Kind::Room,
            bb: Bb::new(4, -10, 4, 6, -8, 6),
            depth: 0,
            dir: Dir::North,
            orient: None,
            entrances: Vec::new(),
            rails: false,
            spider: false,
        };
        region.set(4, -11, 4, BlockId::Water);
        assert!(is_in_invalid_location(&region, &state, &piece));
    }

    #[test]
    fn room_upper_half_sphere_uses_vanilla_threshold() {
        let mut region = RegionBuf::new(0, 0, 0);
        let piece = Piece {
            kind: Kind::Room,
            bb: Bb::new(2, 0, 2, 5, 5, 5),
            depth: 0,
            dir: Dir::North,
            orient: None,
            entrances: Vec::new(),
            rails: false,
            spider: false,
        };
        for y in 4..=5 {
            for x in 2..=5 {
                for z in 2..=5 {
                    region.set(x, y, z, BlockId::Deepslate);
                }
            }
        }

                let clip = Bb::new(0, -64, 0, 15, 319, 15);
        generate_upper_half_sphere(&mut region, &piece, &clip, 2, 4, 2, 5, 5, 5);

        assert_eq!(region.get(4, 4, 4), BlockId::CaveAir);
        assert_eq!(region.get(2, 5, 2), BlockId::Deepslate);
    }
}
