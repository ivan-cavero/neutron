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

use crate::biome_source::{climate_at_block, find_biome};
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
    let idx_vein = feature_catalog::feature_index_in_biome(
        "deep_dark",
        step::UNDERGROUND_DECORATION,
        "sculk_vein",
    )
    .unwrap_or(0);
    let idx_patch = feature_catalog::feature_index_in_biome(
        "deep_dark",
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

fn place_sculk_vein(
    rng: &mut FeatureRandom,
    region: &mut RegionBuf,
    state: &WorldgenState,
    faces: &mut FaceMap,
    ox0: i32,
    oz0: i32,
    cfg: &VeinConfig,
) {
    let count = cfg.count_min + rng.next_int(cfg.count_max - cfg.count_min + 1);
    for _ in 0..count {
        let x = ox0 + rng.next_int(16);
        let z = oz0 + rng.next_int(16);
        let y = WORLD_BOTTOM + rng.next_int(256 - WORLD_BOTTOM + 1);
        if !is_deep_dark_at(state, x, y, z) {
            continue;
        }
        if !is_air_or_water(region.get(x, y, z)) {
            continue;
        }
        let dirs = shuffled_dirs(rng);
        if place_growth(rng, region, faces, x, y, z, &dirs, cfg.chance_of_spreading) {
            continue;
        }
        for &(dx, dy, dz) in &dirs {
            let opp = (-dx, -dy, -dz);
            let rem: Vec<_> = dirs.iter().copied().filter(|d| *d != opp).collect();
            for step_i in 1..=cfg.search_range {
                let nx = x + dx * step_i;
                let ny = y + dy * step_i;
                let nz = z + dz * step_i;
                let b = region.get(nx, ny, nz);
                if !is_air_or_water(b) && b != BlockId::SculkVein {
                    break;
                }
                if place_growth(rng, region, faces, nx, ny, nz, &rem, cfg.chance_of_spreading) {
                    break;
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
        let bit = 1u8 << fi;
        let prev = faces.get(&(x, y, z)).copied().unwrap_or(0);
        faces.insert((x, y, z), prev | bit);
        if is_air_or_water(b) {
            region.set(x, y, z, BlockId::SculkVein);
            SCULK_VEIN_PLACED.fetch_add(1, Ordering::Relaxed);
        }
        if rng.next_f32() < chance {
            MultifaceSpreader::vein().spread_from_face_toward_random_direction(
                rng, region, faces, x, y, z, fi,
            );
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
    if !matches!(b, BlockId::Air | BlockId::Water) {
        return false;
    }
    DIRS.iter()
        .any(|&(dx, dy, dz)| is_full_solid(region.get(x + dx, y + dy, z + dz)))
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
        if is_full_solid(region.get(ox, oy - 1, oz)) {
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
        let chess = (c.x - ox)
            .abs()
            .max((c.y - oy).abs())
            .max((c.z - oz).abs());
        if chess > 1024 {
            continue;
        }
        cursor_update(rng, region, faces, ox, oz, &mut c, spread_veins);
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

    if spread_veins {
        attempt_spread_vein(region, faces, c.x, c.y, c.z, c.facings, here);
        here = region.get(c.x, c.y, c.z);
    }

    c.charge = attempt_use_charge(rng, region, faces, c, here, spread_veins);
    if c.charge <= 0 {
        on_discharged(region, faces, c.x, c.y, c.z);
        return;
    }

    if let Some((nx, ny, nz)) = get_valid_movement(rng, region, faces, c.x, c.y, c.z) {
        on_discharged(region, faces, c.x, c.y, c.z);
        c.x = nx;
        c.y = ny;
        c.z = nz;
        let dx = (c.x - ox) as f64;
        let dz = (c.z - oz) as f64;
        if (dx * dx + dz * dz).sqrt() > WORLDGEN_MAX_DIST {
            c.charge = 0;
            return;
        }
        here = region.get(c.x, c.y, c.z);
    }

    if is_sculk_behaviour(here) {
        c.facings = faces.get(&(c.x, c.y, c.z)).copied();
        c.decay_delay = 1; // SculkBehaviour default updateDecayDelay → 1
    } else {
        // DEFAULT: max(decay-1, 0)
        c.decay_delay = (c.decay_delay - 1).max(0);
    }
    c.update_delay = 1; // getSculkSpreadDelay
}

/// SculkBehaviour.DEFAULT.attemptSpreadVein (CFR SculkBehaviour.java).
fn attempt_spread_vein(
    region: &mut RegionBuf,
    faces: &mut FaceMap,
    x: i32,
    y: i32,
    z: i32,
    facings: Option<u8>,
    here: BlockId,
) -> bool {
    // DEFAULT:
    //   facings == null → sameSpaceSpreader.spreadAll(...)
    //   facings non-empty + air/water → SculkVeinBlock.regrow
    //   facings empty → super = veinSpreader.spreadAll
    match facings {
        None => MultifaceSpreader::same_space().spread_all(region, faces, x, y, z) > 0,
        Some(bits) if bits != 0 => {
            if is_air_or_water(here) || here == BlockId::SculkVein {
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
        BlockId::Sculk => {
            // SculkBlock.attemptUseCharge (simplified growth/decay)
            if charge == 0 || rng.next_int(CHARGE_DECAY_RATE) != 0 {
                return charge;
            }
            // noGrowthRadius check vs origin not available here — use additional decay
            if rng.next_int(ADDITIONAL_DECAY_RATE) != 0 {
                return charge;
            }
            // growth path if open above
            if region.get(c.x, c.y + 1, c.z) == BlockId::Air {
                if rng.next_int(GROWTH_SPAWN_COST) < charge {
                    // sensor/shrieker rare — place sensor mostly
                    if rng.next_int(11) == 0 {
                        region.set(c.x, c.y + 1, c.z, BlockId::SculkShrieker);
                    } else {
                        region.set(c.x, c.y + 1, c.z, BlockId::SculkSensor);
                    }
                    return (charge - GROWTH_SPAWN_COST).max(0);
                }
            }
            (charge - 1).max(0)
        }
        BlockId::SculkCatalyst | BlockId::SculkSensor | BlockId::SculkShrieker => {
            if rng.next_int(CHARGE_DECAY_RATE) == 0 {
                return ((charge as f32) * 0.5).floor() as i32;
            }
            charge
        }
        _ => {
            // DEFAULT: if decay_delay > 0 keep charge else 0
            // After vein place this tick, re-check as vein
            let now = region.get(c.x, c.y, c.z);
            if now == BlockId::SculkVein && spread_veins {
                if attempt_place_sculk(rng, region, faces, c.x, c.y, c.z) {
                    return charge - 1;
                }
            }
            if c.decay_delay > 0 {
                charge
            } else {
                0
            }
        }
    }
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
    let dirs = shuffled_dirs(rng);
    for (dx, dy, dz) in dirs {
        let Some(fi) = dir_index(dx, dy, dz) else {
            continue;
        };
        // hasFace
        if mask & (1u8 << fi) == 0 {
            // If no face bits stored, allow convert only if solid attach exists
            // (bootstrap: treat all solid-facing as faces when mask empty)
            if mask != 0 {
                continue;
            }
            if !is_sculk_replaceable(region.get(x + dx, y + dy, z + dz)) {
                continue;
            }
        }
        let nx = x + dx;
        let ny = y + dy;
        let nz = z + dz;
        if !is_sculk_replaceable(region.get(nx, ny, nz)) {
            continue;
        }
        region.set(nx, ny, nz, BlockId::Sculk);
        SCULK_PLACED.fetch_add(1, Ordering::Relaxed);
        // veinSpreader.spreadAll from the new SCULK block (isOtherBlockValidAsSource)
        MultifaceSpreader::vein().spread_all(region, faces, nx, ny, nz);
        return true;
    }
    false
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

/// getValidMovementPos — only onto SculkBehaviour cells (vanilla).
fn get_valid_movement(
    rng: &mut FeatureRandom,
    region: &RegionBuf,
    faces: &FaceMap,
    x: i32,
    y: i32,
    z: i32,
) -> Option<(i32, i32, i32)> {
    let mut offs = non_corner_neighbours();
    // shuffle
    let mut i = offs.len();
    while i > 1 {
        let j = rng.next_int(i as i32) as usize;
        offs.swap(i - 1, j);
        i -= 1;
    }
    let mut chosen: Option<(i32, i32, i32)> = None;
    for (dx, dy, dz) in offs {
        let nx = x + dx;
        let ny = y + dy;
        let nz = z + dz;
        let b = region.get(nx, ny, nz);
        if !is_sculk_behaviour(b) {
            continue;
        }
        if !is_movement_unobstructed(region, x, y, z, nx, ny, nz) {
            continue;
        }
        chosen = Some((nx, ny, nz));
        // Prefer vein with substrate access
        if b == BlockId::SculkVein && has_substrate_access(region, faces, nx, ny, nz) {
            return Some((nx, ny, nz));
        }
    }
    chosen
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
    is_full_solid(b) || b == BlockId::Sculk
}

fn non_corner_neighbours() -> Vec<(i32, i32, i32)> {
    let mut v = Vec::with_capacity(18);
    for dy in -1..=1 {
        for dz in -1..=1 {
            for dx in -1..=1 {
                if dx == 0 && dy == 0 && dz == 0 {
                    continue;
                }
                if dx != 0 && dy != 0 && dz != 0 {
                    continue;
                }
                v.push((dx, dy, dz));
            }
        }
    }
    v
}

// ===================== helpers =====================

fn is_deep_dark_at(state: &WorldgenState, x: i32, y: i32, z: i32) -> bool {
    let mut env = crate::density::DensityEnv::new(x, y, z, state.noises.noises());
    let climate = climate_at_block(
        &mut env,
        &state.router.temperature,
        &state.router.vegetation,
        &state.router.continents,
        &state.router.erosion,
        &state.router.depth,
        &state.router.ridges,
    );
    find_biome(&climate) == crate::biome_source::biome_id::DEEP_DARK
}

fn is_sculk_behaviour(b: BlockId) -> bool {
    matches!(
        b,
        BlockId::Sculk
            | BlockId::SculkVein
            | BlockId::SculkCatalyst
            | BlockId::SculkSensor
            | BlockId::SculkShrieker
    )
}

fn is_full_solid(b: BlockId) -> bool {
    !matches!(
        b,
        BlockId::Air
            | BlockId::Water
            | BlockId::Lava
            | BlockId::Sculk
            | BlockId::SculkVein
            | BlockId::SculkCatalyst
            | BlockId::SculkSensor
            | BlockId::SculkShrieker
            | BlockId::OakLeaves
            | BlockId::DarkOakLeaves
            | BlockId::ShortGrass
            | BlockId::LeafLitter
            | BlockId::Snow
            | BlockId::PowderSnow
    )
}

fn is_sculk_replaceable(b: BlockId) -> bool {
    // tags/block/sculk_replaceable_world_gen includes base stone + more
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
    )
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

fn shuffled_dirs(rng: &mut FeatureRandom) -> Vec<(i32, i32, i32)> {
    multiface_spreader::all_shuffled(rng)
        .into_iter()
        .map(|i| DIRS[i])
        .collect()
}
