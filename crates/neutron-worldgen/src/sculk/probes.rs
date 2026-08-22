//! Diagnostic probes (parity harnesses drive these; not part of generation).
//!
//! RNG order mirrors the production entry points exactly so a probe replay
//! equals the in-generator pass for the same origin/seed.
use super::*;
use super::cursor::*;
use super::gates::*;
use super::place::*;
use crate::feature_catalog;
use crate::feature_rng::FeatureRandom;
use crate::region_buf::RegionBuf;
use crate::surface::BlockId;
use crate::worldgen::WorldgenState;

/// Place `sculk_vein` for one chunk origin (same seed path as `apply_sculk_region`).
/// Returns the face map so a following patch can see vein faces.
pub fn probe_apply_vein_origin(
    region: &mut RegionBuf,
    state: &WorldgenState,
    ox0: i32,
    oz0: i32,
) -> FaceMap {
    let vein_cfg = VeinConfig::load();
    let idx_vein =
        feature_catalog::global_feature_index(step::UNDERGROUND_DECORATION, "sculk_vein")
            .unwrap_or(0);
    let mut rng = FeatureRandom::new(state.seed);
    let dec = rng.set_decoration_seed(state.seed, ox0, oz0);
    rng.set_feature_seed(dec, idx_vein, step::UNDERGROUND_DECORATION);
    let mut faces = FaceMap::new();
    place_sculk_vein(&mut rng, region, state, &mut faces, ox0, oz0, &vein_cfg);
    faces
}

/// Run the first `sculk_patch` attempt of origin `(ox0,oz0)` with the live feature RNG.
/// `faces` should be the map produced by the preceding vein pass.
pub fn probe_real_first_patch(
    region: &mut RegionBuf,
    state: &WorldgenState,
    faces: &mut FaceMap,
    ox0: i32,
    oz0: i32,
) -> (i32, i32, i32, f32, u32) {
    let patch_cfg = PatchConfig::load();
    let idx_patch = feature_catalog::global_feature_index(
        step::UNDERGROUND_DECORATION,
        "sculk_patch_deep_dark",
    )
    .unwrap_or(1);
    let mut rng = FeatureRandom::new(state.seed);
    let dec = rng.set_decoration_seed(state.seed, ox0, oz0);
    rng.set_feature_seed(dec, idx_patch, step::UNDERGROUND_DECORATION);
    for _ in 0..patch_cfg.patch_count {
        let x = ox0 + rng.next_int(16);
        let z = oz0 + rng.next_int(16);
        let y = WORLD_BOTTOM + rng.next_int(256 - WORLD_BOTTOM + 1);
        if !is_deep_dark_at(state, x, y, z) {
            continue;
        }
        if !can_spread_from(region, x, y, z) {
            continue;
        }
        rng.reset_draw_count();
        run_patch(&mut rng, region, faces, x, y, z, &patch_cfg);
        return (
            x,
            y,
            z,
            LAST_CATALYST_ROLL.load(Ordering::Relaxed) as f32 / 1_000_000.0,
            rng.draw_count(),
        );
    }
    (0, 0, 0, 0.0, 0)
}

/// Run one worldgen patch at `origin` with `FeatureRandom::new(seed)`.
/// Returns (sculk, vein, growth, catalyst_roll, draw_count) in the whole region.
pub fn probe_run_patch(
    region: &mut RegionBuf,
    origin: (i32, i32, i32),
    seed: i64,
) -> (u32, u32, u32, f32, u32) {
    let cfg = PatchConfig {
        charge_count: 10,
        amount_per_charge: 32,
        spread_attempts: 64,
        spread_rounds: 1,
        growth_rounds: 0,
        catalyst_chance: 0.5,
        extra_rare_growths: 0,
        patch_count: 1,
    };
    let mut faces = FaceMap::new();
    let mut rng = FeatureRandom::new(seed);
    rng.reset_draw_count();
    run_patch(
        &mut rng, region, &mut faces, origin.0, origin.1, origin.2, &cfg,
    );
    let draws = rng.draw_count();
    let mut sculk = 0u32;
    let mut vein = 0u32;
    let mut growth = 0u32;
    for z in region.origin_z..region.origin_z + region.side {
        for y in crate::generator::WORLD_BOTTOM..crate::generator::WORLD_TOP {
            for x in region.origin_x..region.origin_x + region.side {
                match region.get(x, y, z) {
                    BlockId::Sculk => sculk += 1,
                    BlockId::SculkVein => vein += 1,
                    BlockId::SculkSensor | BlockId::SculkShrieker => growth += 1,
                    _ => {}
                }
            }
        }
    }
    (
        sculk,
        vein,
        growth,
        LAST_CATALYST_ROLL.load(Ordering::Relaxed) as f32 / 1_000_000.0,
        draws,
    )
}

/// One worldgen patch on a flat deepslate floor (y=9), origin (8,10,8), seed 1.
/// Returns (sculk, vein, sensor+shrieker, catalyst_roll, draw_count).
pub fn probe_flat_floor_patch() -> (u32, u32, u32, f32, u32) {
    let mut region = RegionBuf::new(0, 0, 1);
    for z in region.origin_z..region.origin_z + region.side {
        for x in region.origin_x..region.origin_x + region.side {
            region.set(x, 9, z, BlockId::Deepslate);
            region.set(x, 10, z, BlockId::Air);
        }
    }
    let cfg = PatchConfig {
        charge_count: 10,
        amount_per_charge: 32,
        spread_attempts: 64,
        spread_rounds: 1,
        growth_rounds: 0,
        catalyst_chance: 0.5,
        extra_rare_growths: 0,
        patch_count: 1,
    };
    let mut faces = FaceMap::new();
    let mut rng = FeatureRandom::new(1);
    rng.reset_draw_count();
    run_patch(&mut rng, &mut region, &mut faces, 8, 10, 8, &cfg);
    let draws = rng.draw_count();
    let mut sculk = 0u32;
    let mut vein = 0u32;
    let mut growth = 0u32;
    for z in region.origin_z..region.origin_z + region.side {
        for y in 8..=12 {
            for x in region.origin_x..region.origin_x + region.side {
                match region.get(x, y, z) {
                    BlockId::Sculk => sculk += 1,
                    BlockId::SculkVein => vein += 1,
                    BlockId::SculkSensor | BlockId::SculkShrieker => growth += 1,
                    _ => {}
                }
            }
        }
    }
    (
        sculk,
        vein,
        growth,
        LAST_CATALYST_ROLL.load(Ordering::Relaxed) as f32 / 1_000_000.0,
        draws,
    )
}

/// Record the `sculk_patch_deep_dark` position gate for one origin (same
/// draw order as the vein gate: x, z, then y).
pub fn probe_patch_gate_origin(
    ox0: i32,
    oz0: i32,
    level_seed: i64,
    feature_index: i32,
    state: &WorldgenState,
) -> Vec<(i32, i32, i32, u8)> {
    let patch_cfg = PatchConfig::load();
    let mut rng = FeatureRandom::new(level_seed);
    let dec = rng.set_decoration_seed(level_seed, ox0, oz0);
    rng.set_feature_seed(dec, feature_index, step::UNDERGROUND_DECORATION);
    let mut out = Vec::with_capacity(patch_cfg.patch_count as usize);
    for _ in 0..patch_cfg.patch_count {
        let x = ox0 + rng.next_int(16);
        let z = oz0 + rng.next_int(16);
        let y = WORLD_BOTTOM + rng.next_int(256 - WORLD_BOTTOM + 1);
        out.push((x, y, z, is_deep_dark_at(state, x, y, z) as u8));
    }
    out
}

/// Record the `sculk_vein` position gate for one origin: the (x,y,z) the
/// feature RNG draws per attempt plus whether the deep_dark biome check
/// accepts it. Position draws never depend on placement, so this pass is
/// deterministic and lets the Java probe replay identical gate decisions.
pub fn probe_vein_gate_origin(
    ox0: i32,
    oz0: i32,
    level_seed: i64,
    feature_index: i32,
    state: &WorldgenState,
) -> Vec<(i32, i32, i32, u8)> {
    let vein_cfg = VeinConfig::load();
    let mut rng = FeatureRandom::new(level_seed);
    let dec = rng.set_decoration_seed(level_seed, ox0, oz0);
    rng.set_feature_seed(dec, feature_index, step::UNDERGROUND_DECORATION);
    let count = vein_cfg.count_min + rng.next_int(vein_cfg.count_max - vein_cfg.count_min + 1);
    let mut out = Vec::with_capacity(count as usize);
    for _ in 0..count {
        let x = ox0 + rng.next_int(16);
        let z = oz0 + rng.next_int(16);
        let y = WORLD_BOTTOM + rng.next_int(256 - WORLD_BOTTOM + 1);
        out.push((x, y, z, is_deep_dark_at(state, x, y, z) as u8));
    }
    out
}

/// Replay the vein feature with recorded gate decisions and a per-attempt
/// event log (`SOLID`/`PLACED x,y,z#mask`/`FAILED`), mirroring what the Java
/// probe logs. Also returns the final face map.
pub fn probe_vein_origin_traced(
    region: &mut RegionBuf,
    ox0: i32,
    oz0: i32,
    level_seed: i64,
    feature_index: i32,
    gate: &[(i32, i32, i32, u8)],
) -> (Vec<String>, FaceMap) {
    let vein_cfg = VeinConfig::load();
    let mut rng = FeatureRandom::new(level_seed);
    let dec = rng.set_decoration_seed(level_seed, ox0, oz0);
    rng.set_feature_seed(dec, feature_index, step::UNDERGROUND_DECORATION);
    let count = vein_cfg.count_min + rng.next_int(vein_cfg.count_max - vein_cfg.count_min + 1);
    debug_assert_eq!(count as usize, gate.len(), "gate list out of sync");
    let base_dirs = valid_growth_dirs(true, true, true);
    let mut faces: FaceMap = HashMap::new();
    let mut events = Vec::new();
    for &(x, y, z, ok) in gate {
        let _rx = rng.next_int(16);
        let _rz = rng.next_int(16);
        let _ry = rng.next_int(256 - WORLD_BOTTOM + 1);
        if ok == 0 {
            continue;
        }
        if !is_air_or_water(region.get(x, y, z)) {
            events.push(format!("SOLID {x},{y},{z}"));
            continue;
        }
        let search_dirs = shuffle_dirs_list(&mut rng, &base_dirs);
        if place_growth(
            &mut rng,
            region,
            &mut faces,
            x,
            y,
            z,
            &search_dirs,
            vein_cfg.chance_of_spreading,
        ) {
            let m = faces.get(&(x, y, z)).copied().unwrap_or(0);
            events.push(format!("PLACED {x},{y},{z}#{m}"));
            continue;
        }
        // search loop (mirrors place_sculk_vein_gated)
        'search: for &(dx, dy, dz) in &search_dirs {
            let opp = (-dx, -dy, -dz);
            let placement_dirs = shuffle_dirs_list(
                &mut rng,
                &base_dirs
                    .iter()
                    .copied()
                    .filter(|d| *d != opp)
                    .collect::<Vec<_>>(),
            );
            for _ in 0..vein_cfg.search_range {
                let nx = x + dx;
                let ny = y + dy;
                let nz = z + dz;
                let b = region.get(nx, ny, nz);
                if !is_air_or_water(b) && b != BlockId::SculkVein {
                    break;
                }
                if place_growth(
                    &mut rng,
                    region,
                    &mut faces,
                    nx,
                    ny,
                    nz,
                    &placement_dirs,
                    vein_cfg.chance_of_spreading,
                ) {
                    let m = faces.get(&(nx, ny, nz)).copied().unwrap_or(0);
                    events.push(format!("PLACED {nx},{ny},{nz}#{m}"));
                    break 'search;
                }
            }
        }
    }
    (events, faces)
}
