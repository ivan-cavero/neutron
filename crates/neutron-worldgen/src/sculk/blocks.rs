//! Sculk block charge handlers — attemptUseCharge family, vein discharge.
use super::*;
use super::cursor::Cursor;
use super::gates::*;
use crate::feature_catalog;
use crate::feature_rng::FeatureRandom;
use crate::region_buf::RegionBuf;
use crate::surface::BlockId;
use crate::worldgen::WorldgenState;

/// SculkBlock/SculkVeinBlock use the interface default (veinSpreader.spreadAll).
/// Only DEFAULT (non-SculkBehaviour) branches on facings.
pub(super) fn attempt_spread_vein(
    region: &mut RegionBuf,
    faces: &mut FaceMap,
    x: i32,
    y: i32,
    z: i32,
    facings: Option<u8>,
    here: BlockId,
) -> bool {
    if is_sculk_behaviour(here) {
        return MultifaceSpreader::vein().spread_all(region, faces, x, y, z) > 0;
    }
    // SculkBehaviour$1.attemptSpreadVein (javap 26.2):
    //   facings == null            → sameSpaceSpreader.spreadAll
    //   facings.isEmpty() == true  → ifne → super (veinSpreader.spreadAll)
    //   facings.isEmpty() == false → regrow if air/water
    match facings {
        None => MultifaceSpreader::same_space().spread_all(region, faces, x, y, z) > 0,
        Some(0) => MultifaceSpreader::vein().spread_all(region, faces, x, y, z) > 0,
        Some(bits) => {
            if is_air_or_water(here) {
                MultifaceSpreader::regrow(region, faces, x, y, z, bits)
            } else {
                false
            }
        }
    }
}

pub(super) fn attempt_use_charge(
    rng: &mut FeatureRandom,
    region: &mut RegionBuf,
    faces: &mut FaceMap,
    c: &Cursor,
    here: BlockId,
    spread_veins: bool,
    ox: i32,
    oy: i32,
    oz: i32,
) -> i32 {
    let charge = c.charge;
    match here {
        BlockId::SculkVein => {
            // SculkVeinBlock.attemptUseCharge
            if spread_veins && attempt_place_sculk(rng, region, faces, c.x, c.y, c.z) {
                return charge - 1;
            }
            if rng.next_int(CHARGE_DECAY_RATE) == 0 {
                return ((charge as f32) * 0.5).floor() as i32;
            }
            charge
        }
        BlockId::Sculk => sculk_block_attempt_use_charge(rng, region, c, charge, ox, oy, oz),
        // Catalyst/sensor/shrieker do not implement SculkBehaviour → DEFAULT
        _ => {
            if c.decay_delay > 0 {
                charge
            } else {
                0
            }
        }
    }
}

/// SculkBlock.attemptUseCharge (CFR). Worldgen: noGrowthRadius=1, additionalDecay=10,
/// growthSpawnCost=50. extra_rare_growths is a separate patch path (config = 0).
fn sculk_block_attempt_use_charge(
    rng: &mut FeatureRandom,
    region: &mut RegionBuf,
    c: &Cursor,
    charge: i32,
    ox: i32,
    oy: i32,
    oz: i32,
) -> i32 {
    if charge == 0 || rng.next_int(CHARGE_DECAY_RATE) != 0 {
        return charge;
    }
    // closerThan(origin, noGrowthRadius=1) → distSqr < 1 → only the origin cell
    let dx = (c.x - ox) as f64;
    let dy = (c.y - oy) as f64;
    let dz = (c.z - oz) as f64;
    let is_close = dx * dx + dy * dy + dz * dz < 1.0;
    let can_g = can_place_growth(region, c.x, c.y, c.z);
    if std::env::var_os("NEUTRON_SCULK_PATCHES").is_some() {
        eprintln!(
            "sculk_use ({},{},{}) close={is_close} can_g={can_g} above={:?} ch={charge}",
            c.x,
            c.y,
            c.z,
            region.get(c.x, c.y + 1, c.z)
        );
    }
    if is_close || !can_g {
        if rng.next_int(ADDITIONAL_DECAY_RATE) != 0 {
            return charge;
        }
        let dec = if is_close {
            1
        } else {
            get_decay_penalty(c.x, c.y, c.z, ox, oy, oz, charge)
        };
        return charge - dec;
    }
    if rng.next_int(GROWTH_SPAWN_COST) < charge {
        // getRandomGrowthState: nextInt(11)==0 → shrieker, else sensor
        if rng.next_int(11) == 0 {
            region.set(c.x, c.y + 1, c.z, BlockId::SculkShrieker);
        } else {
            region.set(c.x, c.y + 1, c.z, BlockId::SculkSensor);
        }
    }
    (charge - GROWTH_SPAWN_COST).max(0)
}

pub(super) fn get_decay_penalty(x: i32, y: i32, z: i32, ox: i32, oy: i32, oz: i32, charge: i32) -> i32 {
    // noGrowthRadius = 1; MAX_GROWTH_RATE_RADIUS = 24
    let no_growth_radius = 1i32;
    let dist_sqr = {
        let dx = (x - ox) as f64;
        let dy = (y - oy) as f64;
        let dz = (z - oz) as f64;
        dx * dx + dy * dy + dz * dz
    };
    let outer = (dist_sqr.sqrt() as f32) - (no_growth_radius as f32);
    let outer_sq = outer * outer;
    let max_reach_sq = {
        let r = 24 - no_growth_radius;
        r * r
    };
    let distance_factor = (outer_sq / (max_reach_sq as f32)).min(1.0);
    // Java (int)(float) truncates toward zero
    1.max((charge as f32 * distance_factor * 0.5) as i32)
}

/// SculkBlock.canPlaceGrowth: air/water above; at most 2 sensors/shriekers in ±4 x/z, y+0..2.
pub(super) fn can_place_growth(region: &RegionBuf, x: i32, y: i32, z: i32) -> bool {
    if !is_air_or_water(region.get(x, y + 1, z)) {
        return false;
    }
    let mut growth = 0i32;
    for dy in 0..=2 {
        for dz in -4..=4 {
            for dx in -4..=4 {
                let b = region.get(x + dx, y + dy, z + dz);
                if matches!(b, BlockId::SculkSensor | BlockId::SculkShrieker) {
                    growth += 1;
                    if growth > 2 {
                        return false;
                    }
                }
            }
        }
    }
    true
}

/// SculkVeinBlock.attemptPlaceSculk — requires hasFace toward replaceable.
pub(super) fn attempt_place_sculk(
    rng: &mut FeatureRandom,
    region: &mut RegionBuf,
    faces: &mut FaceMap,
    x: i32,
    y: i32,
    z: i32,
) -> bool {
    let mask = faces.get(&(x, y, z)).copied().unwrap_or(0);
    let order = multiface_spreader::all_shuffled(rng);
    for fi in order {
        // SculkVeinBlock.hasFace — no face bit means skip (mask 0 places nothing)
        if mask & (1u8 << fi) == 0 {
            continue;
        }
        let (dx, dy, dz) = DIRS[fi];
        let nx = x + dx;
        let ny = y + dy;
        let nz = z + dz;
        if !is_sculk_replaceable_world_gen(region.get(nx, ny, nz)) {
            continue;
        }
        region.set(nx, ny, nz, BlockId::Sculk);
        SCULK_PLACED.fetch_add(1, Ordering::Relaxed);
        // veinSpreader.spreadAll from the new SCULK (CFR attemptPlaceSculk)
        MultifaceSpreader::vein().spread_all(region, faces, nx, ny, nz);
        // Discharge adjacent veins (skip face toward support opposite = back to vein pos)
        let skip = opposite_dir(fi);
        for (vi, &(vx, vy, vz)) in DIRS.iter().enumerate() {
            if vi == skip {
                continue;
            }
            let px = nx + vx;
            let py = ny + vy;
            let pz = nz + vz;
            if region.get(px, py, pz) == BlockId::SculkVein {
                on_discharged(region, faces, px, py, pz);
            }
        }
        return true;
    }
    false
}

fn opposite_dir(fi: usize) -> usize {
    crate::deco_util::opposite(fi)
}

/// SculkVeinBlock.onDischarged with vanilla's STALE-state semantics
/// (ChargeCursor.update passes its start-of-tick snapshot): strip the
/// SNAPSHOT's faces toward current sculk neighbours, then setBlock the
/// stripped snapshot — wiping faces the live state gained mid-tick.
/// Non-empty result rewrites the cell as a vein even if the live mask
/// had extra faces; empty result turns it back to air.
/// Strip face bits whose neighbour cell is Sculk — the shared core of
/// SculkVeinBlock.onDischarged (live and snapshot variants).
fn strip_faces_toward_sculk(region: &RegionBuf, mut mask: u8, x: i32, y: i32, z: i32) -> u8 {
    for (i, &(dx, dy, dz)) in DIRS.iter().enumerate() {
        if mask & (1u8 << i) == 0 {
            continue;
        }
        if region.get(x + dx, y + dy, z + dz) == BlockId::Sculk {
            mask &= !(1u8 << i);
        }
    }
    mask
}

pub(super) fn on_discharged_snapshot(
    region: &mut RegionBuf,
    faces: &mut FaceMap,
    x: i32,
    y: i32,
    z: i32,
    snapshot_mask: u8,
) {
    if let Some(c) = crate::multiface_spreader::trace_coord() {
        if (x, y, z) == c {
            eprintln!("TRACE snapshot_discharge ({x},{y},{z}) snap={snapshot_mask}");
        }
    }
    let mask = strip_faces_toward_sculk(region, snapshot_mask, x, y, z);
    if mask == 0 {
        region.set(x, y, z, BlockId::Air);
        faces.remove(&(x, y, z));
    } else {
        region.set(x, y, z, BlockId::SculkVein);
        faces.insert((x, y, z), mask);
    }
}

pub(super) fn on_discharged(region: &mut RegionBuf, faces: &mut FaceMap, x: i32, y: i32, z: i32) {
    // SculkVeinBlock.onDischarged: strip faces toward sculk; clear if no faces
    if let Some(c) = crate::multiface_spreader::trace_coord() {
        if (x, y, z) == c {
            eprintln!(
                "TRACE live_discharge ({x},{y},{z}) mask={:?}",
                faces.get(&(x, y, z))
            );
        }
    }
    if region.get(x, y, z) != BlockId::SculkVein {
        return;
    }
    let mask = faces.get(&(x, y, z)).copied().unwrap_or(0);
    let mask = strip_faces_toward_sculk(region, mask, x, y, z);
    if mask == 0 {
        region.set(x, y, z, BlockId::Air);
        faces.remove(&(x, y, z));
    } else {
        faces.insert((x, y, z), mask);
    }
}

/// ChargeCursor.getValidMovementPos (CFR): single pass over shuffled non-corner
/// neighbours. Only SculkBehaviour cells; prefers hasSubstrateAccess (break),
/// else last SculkBehaviour found. No open-air walk (that is non-vanilla).
pub(super) fn get_valid_movement(
    rng: &mut FeatureRandom,
    region: &RegionBuf,
    faces: &FaceMap,
    x: i32,
    y: i32,
    z: i32,
) -> Option<(i32, i32, i32)> {
    let mut offs = non_corner_neighbours();
    let mut i = offs.len();
    while i > 1 {
        let j = rng.next_int(i as i32) as usize;
        offs.swap(i - 1, j);
        i -= 1;
    }

    let mut found: Option<(i32, i32, i32)> = None;
    for &(dx, dy, dz) in &offs {
        let nx = x + dx;
        let ny = y + dy;
        let nz = z + dz;
        if !is_sculk_behaviour(region.get(nx, ny, nz)) {
            continue;
        }
        if !is_movement_unobstructed(region, faces, x, y, z, nx, ny, nz) {
            continue;
        }
        found = Some((nx, ny, nz));
        if has_substrate_access(region, faces, nx, ny, nz) {
            break;
        }
    }
    found
}

pub(super) fn has_substrate_access(region: &RegionBuf, faces: &FaceMap, x: i32, y: i32, z: i32) -> bool {
    if region.get(x, y, z) != BlockId::SculkVein {
        return false;
    }
    let mask = faces.get(&(x, y, z)).copied().unwrap_or(0);
    for (i, &(dx, dy, dz)) in DIRS.iter().enumerate() {
        if mask & (1u8 << i) == 0 {
            continue;
        }
        // SCULK_REPLACEABLE tag (not world_gen) for hasSubstrateAccess
        if is_sculk_replaceable(region.get(x + dx, y + dy, z + dz)) {
            return true;
        }
    }
    false
}

fn is_movement_unobstructed(
    region: &RegionBuf,
    faces: &FaceMap,
    fx: i32,
    fy: i32,
    fz: i32,
    tx: i32,
    ty: i32,
    tz: i32,
) -> bool {
    let dx = tx - fx;
    let dy = ty - fy;
    let dz = tz - fz;
    let manh = dx.abs() + dy.abs() + dz.abs();
    if manh == 1 {
        return true;
    }
    // ChargeCursor.isUnobstructed(from, direction):
    //   testPos = from.relative(direction);
    //   !getBlockState(testPos).isFaceSturdy(level, testPos, direction.opposite())
    // The sturdy check is on the intermediate cell's face POINTING BACK at
    // `from` (SupportType.FULL over getBlockSupportShape).
    let unobst = |dx: i32, dy: i32, dz: i32| -> bool {
        let x = fx + dx;
        let y = fy + dy;
        let z = fz + dz;
        let back = dir_index(-dx, -dy, -dz).expect("axis-aligned direction");
        !is_face_sturdy_at(region, faces, x, y, z, back)
    };
    if dx == 0 {
        return unobst(0, dy.signum(), 0) || unobst(0, 0, dz.signum());
    }
    if dy == 0 {
        return unobst(dx.signum(), 0, 0) || unobst(0, 0, dz.signum());
    }
    unobst(dx.signum(), 0, 0) || unobst(0, dy.signum(), 0)
}

/// `BlockState.isFaceSturdy(level, pos, direction, SupportType.FULL)` for the
/// blocks that can sit between a cursor and a diagonal target:
/// - full cubes (stone family, ores, SCULK, catalyst): sturdy on every face;
/// - sensor/shrieker: `Block.column(16.0, 0.0, 8.0)` — 16x16x8 column, so the
///   top and bottom faces are full 16x16 quads (sturdy UP/DOWN = face 1/0)
///   while the side faces are only 8/16 tall (not sturdy);
/// - vein: 16x16x1 plates, sturdy exactly on the faces it HAS.
pub(super) fn is_face_sturdy_at(
    region: &RegionBuf,
    faces: &FaceMap,
    x: i32,
    y: i32,
    z: i32,
    face: usize,
) -> bool {
    match region.get(x, y, z) {
        BlockId::SculkVein => faces.get(&(x, y, z)).copied().unwrap_or(0) & (1u8 << face) != 0,
        BlockId::SculkSensor | BlockId::SculkShrieker => face == 0 || face == 1,
        b => is_collision_full_block(b),
    }
}

/// BlockPos.betweenClosed(-1,-1,-1)..(1,1,1): X fastest, Y mid, Z slowest;
/// drop corners (all nonzero) and origin. Matches ChargeCursor.NON_CORNER_NEIGHBOURS.
pub(super) fn non_corner_neighbours() -> Vec<(i32, i32, i32)> {
    let mut v = Vec::with_capacity(18);
    for z in -1..=1 {
        for y in -1..=1 {
            for x in -1..=1 {
                if x == 0 && y == 0 && z == 0 {
                    continue;
                }
                if x != 0 && y != 0 && z != 0 {
                    continue;
                }
                v.push((x, y, z));
            }
        }
    }
    v
}
