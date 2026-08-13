// Copyright (c) 2026 Neutron Contributors -- MIT License
//
// Deep dark underground decoration (step 7) — vanilla-faithful port.
//
// Sources (re-sync: pwsh tools/vanilla-extract/extract-worldgen.ps1):
//   CFR decompiled:
//     SculkPatchFeature, SculkSpreader.ChargeCursor, SculkVeinBlock,
//     SculkBlock, SculkBehaviour.DEFAULT, MultifaceGrowthFeature
//   Datapack:
//     configured/placed_feature sculk_*, biome/deep_dark.json
//
// Rules: no wall-paint, no expand rings, no vertical seed rescue.
// Vein face bits are tracked; attemptPlaceSculk requires hasFace.

use crate::biome_source::biome_id_at_block;
use crate::feature_catalog::{self, step};
use crate::feature_rng::FeatureRandom;
use crate::generator::WORLD_BOTTOM;
use crate::multiface_spreader::{self, FaceMap, MultifaceSpreader, DIRS as MF_DIRS};
use crate::region_buf::RegionBuf;
use crate::surface::BlockId;
use crate::worldgen::WorldgenState;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};

pub static SCULK_TRIES: AtomicU32 = AtomicU32::new(0);
pub static SCULK_BIOME_OK: AtomicU32 = AtomicU32::new(0);
pub static SCULK_SPREAD_OK: AtomicU32 = AtomicU32::new(0);
pub static SCULK_PLACED: AtomicU32 = AtomicU32::new(0);
pub static SCULK_VEIN_PLACED: AtomicU32 = AtomicU32::new(0);

pub const SCULK_ENABLED: bool = true;

const DIRS: [(i32, i32, i32); 6] = MF_DIRS;

const CHARGE_DECAY_RATE: i32 = 5;
const ADDITIONAL_DECAY_RATE: i32 = 10;
const GROWTH_SPAWN_COST: i32 = 50;
const MAX_CURSORS: usize = 32;
const WORLDGEN_MAX_DIST: f64 = 15.0;

struct PatchConfig {
    charge_count: i32,
    amount_per_charge: i32,
    spread_attempts: i32,
    spread_rounds: i32,
    growth_rounds: i32,
    catalyst_chance: f32,
    patch_count: i32,
}

impl PatchConfig {
    fn load() -> Self {
        let mut patch_count = 256;
        if let Some(p) = feature_catalog::load_placed_feature("sculk_patch_deep_dark") {
            if let Some(arr) = p["placement"].as_array() {
                for m in arr {
                    if m["type"] == "minecraft:count" {
                        if let Some(n) = m["count"].as_i64() {
                            patch_count = n as i32;
                        }
                    }
                }
            }
        }
        let mut s = Self {
            charge_count: 10,
            amount_per_charge: 32,
            spread_attempts: 64,
            spread_rounds: 1,
            growth_rounds: 0,
            catalyst_chance: 0.5,
            patch_count,
        };
        if let Some(v) = feature_catalog::load_configured_feature("sculk_patch_deep_dark") {
            let c = &v["config"];
            s.charge_count = c["charge_count"].as_i64().unwrap_or(10) as i32;
            s.amount_per_charge = c["amount_per_charge"].as_i64().unwrap_or(32) as i32;
            s.spread_attempts = c["spread_attempts"].as_i64().unwrap_or(64) as i32;
            s.spread_rounds = c["spread_rounds"].as_i64().unwrap_or(1) as i32;
            s.growth_rounds = c["growth_rounds"].as_i64().unwrap_or(0) as i32;
            s.catalyst_chance = c["catalyst_chance"].as_f64().unwrap_or(0.5) as f32;
        }
        s
    }
}

struct VeinConfig {
    count_min: i32,
    count_max: i32,
    search_range: i32,
    chance_of_spreading: f32,
}

impl VeinConfig {
    fn load() -> Self {
        let mut count_min = 204;
        let mut count_max = 250;
        if let Some(p) = feature_catalog::load_placed_feature("sculk_vein") {
            if let Some(arr) = p["placement"].as_array() {
                for m in arr {
                    if m["type"] == "minecraft:count" {
                        if let Some(obj) = m["count"].as_object() {
                            if obj.get("type").and_then(|t| t.as_str()) == Some("minecraft:uniform")
                            {
                                count_min = obj["min_inclusive"].as_i64().unwrap_or(204) as i32;
                                count_max = obj["max_inclusive"].as_i64().unwrap_or(250) as i32;
                            }
                        }
                    }
                }
            }
        }
        let mut search_range = 20i32;
        let mut chance = 1.0f32;
        if let Some(c) = feature_catalog::load_configured_feature("sculk_vein") {
            let cfg = &c["config"];
            search_range = cfg["search_range"].as_i64().unwrap_or(20) as i32;
            chance = cfg["chance_of_spreading"].as_f64().unwrap_or(1.0) as f32;
        }
        Self {
            count_min,
            count_max,
            search_range,
            chance_of_spreading: chance,
        }
    }
}

/// Apply sculk_vein + sculk_patch for every chunk origin in the feature region.
pub fn apply_sculk_region(region: &mut RegionBuf, state: &WorldgenState) {
    if !SCULK_ENABLED {
        return;
    }
    let patch_cfg = PatchConfig::load();
    let vein_cfg = VeinConfig::load();
    let idx_vein =
        feature_catalog::global_feature_index(step::UNDERGROUND_DECORATION, "sculk_vein")
            .unwrap_or(0);
    let idx_patch = feature_catalog::global_feature_index(
        step::UNDERGROUND_DECORATION,
        "sculk_patch_deep_dark",
    )
    .unwrap_or(1);

    let level_seed = state.seed;
    let mut faces: FaceMap = HashMap::new();
    let chunks = region.chunks;
    for czl in 0..chunks {
        for cxl in 0..chunks {
            let ox0 = region.origin_x + cxl * 16;
            let oz0 = region.origin_z + czl * 16;

            let mut rng = FeatureRandom::new(level_seed);
            let dec = rng.set_decoration_seed(level_seed, ox0, oz0);
            rng.set_feature_seed(dec, idx_vein, step::UNDERGROUND_DECORATION);
            place_sculk_vein(&mut rng, region, state, &mut faces, ox0, oz0, &vein_cfg);

            let mut rng = FeatureRandom::new(level_seed);
            let dec = rng.set_decoration_seed(level_seed, ox0, oz0);
            rng.set_feature_seed(dec, idx_patch, step::UNDERGROUND_DECORATION);
            place_sculk_patch(&mut rng, region, state, &mut faces, ox0, oz0, &patch_cfg);
        }
    }
}

// ===================== MultifaceGrowthFeature (sculk_vein) =====================

/// validDirections order from MultifaceGrowthConfiguration:
/// ceiling UP, floor DOWN, then HORIZONTAL (N S W E) — then shuffled.
fn valid_growth_dirs(can_floor: bool, can_ceiling: bool, can_wall: bool) -> Vec<(i32, i32, i32)> {
    let mut v = Vec::with_capacity(6);
    // CFR adds UP first if ceiling, DOWN if floor, then HORIZONTAL
    if can_ceiling {
        v.push(DIRS[1]); // UP
    }
    if can_floor {
        v.push(DIRS[0]); // DOWN
    }
    if can_wall {
        v.push(DIRS[2]); // NORTH
        v.push(DIRS[3]); // SOUTH
        v.push(DIRS[4]); // WEST
        v.push(DIRS[5]); // EAST
    }
    v
}

fn shuffle_dirs_list(rng: &mut FeatureRandom, dirs: &[(i32, i32, i32)]) -> Vec<(i32, i32, i32)> {
    let mut d = dirs.to_vec();
    let mut i = d.len();
    while i > 1 {
        let j = rng.next_int(i as i32) as usize;
        d.swap(i - 1, j);
        i -= 1;
    }
    d
}

fn place_sculk_vein(
    rng: &mut FeatureRandom,
    region: &mut RegionBuf,
    state: &WorldgenState,
    faces: &mut FaceMap,
    ox0: i32,
    oz0: i32,
    cfg: &VeinConfig,
) {
    // sculk_vein config: floor+ceiling+wall all true
    let base_dirs = valid_growth_dirs(true, true, true);
    let count = cfg.count_min + rng.next_int(cfg.count_max - cfg.count_min + 1);
    for _ in 0..count {
        let x = ox0 + rng.next_int(16);
        let z = oz0 + rng.next_int(16);
        let y = WORLD_BOTTOM + rng.next_int(256 - WORLD_BOTTOM + 1);
        if !is_deep_dark_at(state, x, y, z) {
            continue;
        }
        // MultifaceGrowthFeature.place: origin must be air/water
        if !is_air_or_water(region.get(x, y, z)) {
            continue;
        }
        let search_dirs = shuffle_dirs_list(rng, &base_dirs);
        if place_growth(
            rng,
            region,
            faces,
            x,
            y,
            z,
            &search_dirs,
            cfg.chance_of_spreading,
        ) {
            continue;
        }
        // MultifaceGrowthFeature.place (bytecode 26.2): for each searchDirection,
        // loop search_range times with setWithOffset(origin, searchDirection) —
        // always origin±1, NOT multi-step accumulation (CFR matches javap).
        'search: for &(dx, dy, dz) in &search_dirs {
            let opp = (-dx, -dy, -dz);
            let placement_dirs = shuffle_dirs_list(
                rng,
                &base_dirs
                    .iter()
                    .copied()
                    .filter(|d| *d != opp)
                    .collect::<Vec<_>>(),
            );
            for _ in 0..cfg.search_range {
                let nx = x + dx;
                let ny = y + dy;
                let nz = z + dz;
                let b = region.get(nx, ny, nz);
                // solid (not vein) ends this search direction
                if !is_air_or_water(b) && b != BlockId::SculkVein {
                    break;
                }
                if place_growth(
                    rng,
                    region,
                    faces,
                    nx,
                    ny,
                    nz,
                    &placement_dirs,
                    cfg.chance_of_spreading,
                ) {
                    break 'search;
                }
            }
        }
    }
}

fn place_growth(
    rng: &mut FeatureRandom,
    region: &mut RegionBuf,
    faces: &mut FaceMap,
    x: i32,
    y: i32,
    z: i32,
    placement_dirs: &[(i32, i32, i32)],
    chance: f32,
) -> bool {
    let b = region.get(x, y, z);
    if !is_air_or_water(b) && b != BlockId::SculkVein {
        return false;
    }
    for &(dx, dy, dz) in placement_dirs {
        if !is_vein_placeable_on(region.get(x + dx, y + dy, z + dz)) {
            continue;
        }
        let Some(fi) = dir_index(dx, dy, dz) else {
            continue;
        };
        // getStateForPlacement == null → return false (do not try the next dir)
        let bit = 1u8 << fi;
        let prev = faces.get(&(x, y, z)).copied().unwrap_or(0);
        if b == BlockId::SculkVein && prev & bit != 0 {
            return false;
        }
        faces.insert((x, y, z), prev | bit);
        if is_air_or_water(b) {
            region.set(x, y, z, BlockId::SculkVein);
            SCULK_VEIN_PLACED.fetch_add(1, Ordering::Relaxed);
        }
        if rng.next_f32() < chance {
            MultifaceSpreader::vein()
                .spread_from_face_toward_random_direction(rng, region, faces, x, y, z, fi);
        }
        return true;
    }
    false
}

// ===================== SculkPatchFeature =====================

fn place_sculk_patch(
    rng: &mut FeatureRandom,
    region: &mut RegionBuf,
    state: &WorldgenState,
    faces: &mut FaceMap,
    ox0: i32,
    oz0: i32,
    cfg: &PatchConfig,
) {
    for _ in 0..cfg.patch_count {
        SCULK_TRIES.fetch_add(1, Ordering::Relaxed);
        let x = ox0 + rng.next_int(16);
        let z = oz0 + rng.next_int(16);
        let y = WORLD_BOTTOM + rng.next_int(256 - WORLD_BOTTOM + 1);
        if !is_deep_dark_at(state, x, y, z) {
            continue;
        }
        SCULK_BIOME_OK.fetch_add(1, Ordering::Relaxed);
        if !can_spread_from(region, x, y, z) {
            continue;
        }
        SCULK_SPREAD_OK.fetch_add(1, Ordering::Relaxed);
        run_patch(rng, region, faces, x, y, z, cfg);
    }
}

fn can_spread_from(region: &RegionBuf, x: i32, y: i32, z: i32) -> bool {
    let b = region.get(x, y, z);
    if is_sculk_behaviour(b) {
        return true;
    }
    // Vanilla: air OR water source; any neighbour with full collision shape.
    // SCULK is a full cube — must count (cascade after earlier patches).
    if !matches!(b, BlockId::Air | BlockId::Water) {
        return false;
    }
    DIRS.iter()
        .any(|&(dx, dy, dz)| is_collision_full_block(region.get(x + dx, y + dy, z + dz)))
}

struct Cursor {
    x: i32,
    y: i32,
    z: i32,
    charge: i32,
    decay_delay: i32,
    update_delay: i32,
    facings: Option<u8>,
}

fn run_patch(
    rng: &mut FeatureRandom,
    region: &mut RegionBuf,
    faces: &mut FaceMap,
    ox: i32,
    oy: i32,
    oz: i32,
    cfg: &PatchConfig,
) {
    let total = cfg.spread_rounds + cfg.growth_rounds;
    for round in 0..total {
        let mut cursors: Vec<Cursor> = Vec::new();
        for _ in 0..cfg.charge_count {
            let mut charge = cfg.amount_per_charge;
            while charge > 0 && cursors.len() < MAX_CURSORS {
                let cur = charge.min(1000);
                cursors.push(Cursor {
                    x: ox,
                    y: oy,
                    z: oz,
                    charge: cur,
                    decay_delay: 1,
                    update_delay: 0,
                    facings: None,
                });
                charge -= cur;
            }
        }
        let spread_veins = round < cfg.spread_rounds;
        for _ in 0..cfg.spread_attempts {
            update_cursors(rng, region, faces, ox, oy, oz, &mut cursors, spread_veins);
            if cursors.is_empty() {
                break;
            }
        }
    }

    if rng.next_f32() <= cfg.catalyst_chance {
        // isCollisionShapeFullBlock below
        if is_collision_full_block(region.get(ox, oy - 1, oz)) {
            region.set(ox, oy, oz, BlockId::SculkCatalyst);
            faces.remove(&(ox, oy, oz));
        }
    }
}

fn update_cursors(
    rng: &mut FeatureRandom,
    region: &mut RegionBuf,
    faces: &mut FaceMap,
    ox: i32,
    oy: i32,
    oz: i32,
    cursors: &mut Vec<Cursor>,
    spread_veins: bool,
) {
    let mut next: Vec<Cursor> = Vec::new();
    for mut c in cursors.drain(..) {
        let chess = (c.x - ox).abs().max((c.y - oy).abs()).max((c.z - oz).abs());
        if chess > 1024 {
            continue;
        }
        cursor_update(rng, region, faces, ox, oy, oz, &mut c, spread_veins);
        if c.charge > 0 {
            next.push(c);
        }
    }
    // Worldgen keeps multiple cursors at same pos (no merge when isWorldGeneration)
    if next.len() > MAX_CURSORS {
        next.truncate(MAX_CURSORS);
    }
    *cursors = next;
}

/// ChargeCursor.update
fn cursor_update(
    rng: &mut FeatureRandom,
    region: &mut RegionBuf,
    faces: &mut FaceMap,
    ox: i32,
    oy: i32,
    oz: i32,
    c: &mut Cursor,
    spread_veins: bool,
) {
    if c.charge <= 0 {
        return;
    }
    if c.update_delay > 0 {
        c.update_delay -= 1;
        return;
    }

    let mut here = region.get(c.x, c.y, c.z);
    // updateDecayDelay uses the behaviour from *before* the move
    // (and after attemptSpreadVein re-read if canChangeBlockStateOnSpread).
    let mut behaviour_is_sculk = is_sculk_behaviour(here);

    if spread_veins {
        if attempt_spread_vein(region, faces, c.x, c.y, c.z, c.facings, here) {
            // SculkBlock.canChangeBlockStateOnSpread == false; default is true.
            if here != BlockId::Sculk {
                here = region.get(c.x, c.y, c.z);
                behaviour_is_sculk = is_sculk_behaviour(here);
            }
        }
    }

    c.charge = attempt_use_charge(rng, region, faces, c, here, spread_veins, ox, oy, oz);
    if c.charge <= 0 {
        on_discharged(region, faces, c.x, c.y, c.z);
        return;
    }

    if let Some((nx, ny, nz)) = get_valid_movement(rng, region, faces, c.x, c.y, c.z) {
        on_discharged(region, faces, c.x, c.y, c.z);
        c.x = nx;
        c.y = ny;
        c.z = nz;
        // closerThan(Vec3i(originX, cursorY, originZ), 15) → distSqr < 225
        let dx = (c.x - ox) as f64;
        let dz = (c.z - oz) as f64;
        if dx * dx + dz * dz >= WORLDGEN_MAX_DIST * WORLDGEN_MAX_DIST {
            c.charge = 0;
            return;
        }
        here = region.get(c.x, c.y, c.z);
    }

    // MultifaceBlock.availableFaces: empty set on non-multiface SculkBehaviour (sculk).
    // Catalyst/sensor/shrieker are not SculkBehaviour — facings left unchanged.
    if here == BlockId::SculkVein {
        c.facings = Some(faces.get(&(c.x, c.y, c.z)).copied().unwrap_or(0));
    } else if here == BlockId::Sculk {
        c.facings = Some(0);
    }
    if behaviour_is_sculk {
        c.decay_delay = 1;
    } else {
        c.decay_delay = (c.decay_delay - 1).max(0);
    }
    c.update_delay = 1; // getSculkSpreadDelay
}

/// SculkBlock/SculkVeinBlock use the interface default (veinSpreader.spreadAll).
/// Only DEFAULT (non-SculkBehaviour) branches on facings.
fn attempt_spread_vein(
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
    // SculkBehaviour.DEFAULT.attemptSpreadVein
    match facings {
        None => MultifaceSpreader::same_space().spread_all(region, faces, x, y, z) > 0,
        Some(bits) if bits != 0 => {
            if is_air_or_water(here) {
                MultifaceSpreader::regrow(region, faces, x, y, z, bits)
            } else {
                false
            }
        }
        Some(_) => MultifaceSpreader::vein().spread_all(region, faces, x, y, z) > 0,
    }
}

fn attempt_use_charge(
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
    if is_close || !can_place_growth(region, c.x, c.y, c.z) {
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

fn get_decay_penalty(x: i32, y: i32, z: i32, ox: i32, oy: i32, oz: i32, charge: i32) -> i32 {
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
fn can_place_growth(region: &RegionBuf, x: i32, y: i32, z: i32) -> bool {
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
fn attempt_place_sculk(
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
    match fi {
        0 => 1,
        1 => 0,
        2 => 3,
        3 => 2,
        4 => 5,
        5 => 4,
        _ => fi,
    }
}

fn on_discharged(region: &mut RegionBuf, faces: &mut FaceMap, x: i32, y: i32, z: i32) {
    // SculkVeinBlock.onDischarged: strip faces toward sculk; clear if no faces
    if region.get(x, y, z) != BlockId::SculkVein {
        return;
    }
    let mut mask = faces.get(&(x, y, z)).copied().unwrap_or(0);
    for (i, &(dx, dy, dz)) in DIRS.iter().enumerate() {
        if mask & (1u8 << i) == 0 {
            continue;
        }
        if region.get(x + dx, y + dy, z + dz) == BlockId::Sculk {
            mask &= !(1u8 << i);
        }
    }
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
fn get_valid_movement(
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
        if !is_movement_unobstructed(region, x, y, z, nx, ny, nz) {
            continue;
        }
        found = Some((nx, ny, nz));
        if has_substrate_access(region, faces, nx, ny, nz) {
            break;
        }
    }
    found
}

fn has_substrate_access(region: &RegionBuf, faces: &FaceMap, x: i32, y: i32, z: i32) -> bool {
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
    let free = |x: i32, y: i32, z: i32| !is_face_sturdy(region.get(x, y, z));
    if dx == 0 {
        return free(fx, fy + dy.signum(), fz) || free(fx, fy, fz + dz.signum());
    }
    if dy == 0 {
        return free(fx + dx.signum(), fy, fz) || free(fx, fy, fz + dz.signum());
    }
    free(fx + dx.signum(), fy, fz) || free(fx, fy + dy.signum(), fz)
}

fn is_face_sturdy(b: BlockId) -> bool {
    is_collision_full_block(b)
}

/// BlockPos.betweenClosed(-1,-1,-1)..(1,1,1): X fastest, Y mid, Z slowest;
/// drop corners (all nonzero) and origin. Matches ChargeCursor.NON_CORNER_NEIGHBOURS.
fn non_corner_neighbours() -> Vec<(i32, i32, i32)> {
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

// ===================== helpers =====================

fn is_deep_dark_at(state: &WorldgenState, x: i32, y: i32, z: i32) -> bool {
    biome_id_at_block(state, x, y, z) == crate::biome_source::biome_id::DEEP_DARK
}

/// Only SculkBlock and SculkVeinBlock implement SculkBehaviour.
/// Catalyst / sensor / shrieker do not (javap 26.2).
fn is_sculk_behaviour(b: BlockId) -> bool {
    matches!(b, BlockId::Sculk | BlockId::SculkVein)
}

/// Full collision cube (vanilla isCollisionShapeFullBlock). SCULK is solid;
/// veins/sensors/etc. are not.
fn is_collision_full_block(b: BlockId) -> bool {
    matches!(
        b,
        BlockId::Sculk
            | BlockId::SculkCatalyst
            | BlockId::Stone
            | BlockId::Granite
            | BlockId::Diorite
            | BlockId::Andesite
            | BlockId::Dirt
            | BlockId::CoarseDirt
            | BlockId::GrassBlock
            | BlockId::Podzol
            | BlockId::Mycelium
            | BlockId::Gravel
            | BlockId::Sand
            | BlockId::RedSand
            | BlockId::Clay
            | BlockId::Calcite
            | BlockId::Tuff
            | BlockId::Deepslate
            | BlockId::Sandstone
            | BlockId::RedSandstone
            | BlockId::Terracotta
            | BlockId::WhiteTerracotta
            | BlockId::OrangeTerracotta
            | BlockId::BrownTerracotta
            | BlockId::BlackTerracotta
            | BlockId::YellowTerracotta
            | BlockId::RedTerracotta
            | BlockId::LightGrayTerracotta
            | BlockId::Mud
            | BlockId::Sulfur
            | BlockId::Cinnabar
            | BlockId::Bedrock
            | BlockId::Cobblestone
            | BlockId::CoalOre
            | BlockId::IronOre
            | BlockId::CopperOre
            | BlockId::GoldOre
            | BlockId::RedstoneOre
            | BlockId::LapisOre
            | BlockId::DiamondOre
            | BlockId::DeepslateCoalOre
            | BlockId::DeepslateIronOre
            | BlockId::DeepslateCopperOre
            | BlockId::DeepslateGoldOre
            | BlockId::DeepslateRedstoneOre
            | BlockId::DeepslateLapisOre
            | BlockId::DeepslateDiamondOre
            | BlockId::RawIronBlock
            | BlockId::RawCopperBlock
            | BlockId::OakLog
            | BlockId::DarkOakLog
            | BlockId::MossBlock
            | BlockId::PackedIce
            | BlockId::BlueIce
            | BlockId::Ice
    )
}

/// tags/block/sculk_replaceable — NOT world_gen. Used by hasSubstrateAccess.
/// Ores are not in this tag (vanilla).
fn is_sculk_replaceable(b: BlockId) -> bool {
    matches!(
        b,
        BlockId::Stone
            | BlockId::Granite
            | BlockId::Diorite
            | BlockId::Andesite
            | BlockId::Dirt
            | BlockId::CoarseDirt
            | BlockId::GrassBlock
            | BlockId::Podzol
            | BlockId::Mycelium
            | BlockId::MossBlock
            | BlockId::Gravel
            | BlockId::Sand
            | BlockId::RedSand
            | BlockId::Clay
            | BlockId::Calcite
            | BlockId::Tuff
            | BlockId::Deepslate
            | BlockId::Sandstone
            | BlockId::RedSandstone
            | BlockId::Terracotta
            | BlockId::WhiteTerracotta
            | BlockId::OrangeTerracotta
            | BlockId::BrownTerracotta
            | BlockId::BlackTerracotta
            | BlockId::YellowTerracotta
            | BlockId::RedTerracotta
            | BlockId::LightGrayTerracotta
            | BlockId::Mud
            | BlockId::Sulfur
            | BlockId::Cinnabar
    )
}

/// tags/block/sculk_replaceable_world_gen = sculk_replaceable + deepslate bricks/tiles.
/// Those brick variants are not in BlockId; same set as the base tag here.
/// Used by worldgen SculkSpreader.replaceableBlocks() in attemptPlaceSculk.
fn is_sculk_replaceable_world_gen(b: BlockId) -> bool {
    is_sculk_replaceable(b)
}

fn is_air_or_water(b: BlockId) -> bool {
    matches!(b, BlockId::Air | BlockId::Water)
}

fn is_vein_placeable_on(b: BlockId) -> bool {
    matches!(
        b,
        BlockId::Stone
            | BlockId::Andesite
            | BlockId::Diorite
            | BlockId::Granite
            | BlockId::Calcite
            | BlockId::Tuff
            | BlockId::Deepslate
    )
}

fn dir_index(dx: i32, dy: i32, dz: i32) -> Option<usize> {
    DIRS.iter().position(|&d| d == (dx, dy, dz))
}
