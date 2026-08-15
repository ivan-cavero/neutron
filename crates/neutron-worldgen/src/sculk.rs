//! Deep-dark underground decoration (generation step 7).
//!
//! Ports `SculkPatchFeature`, `ChargeCursor`, `SculkVeinBlock`, `SculkBlock`,
//! `SculkBehaviour.DEFAULT` and `MultifaceGrowthFeature`. Re-sync from CFR
//! after a Mojang drop (`extract-worldgen.ps1`).
//!
//! Datapack: `sculk_*` features + `biome/deep_dark.json`.
//! No wall-paint, no expand rings, no vertical seed rescue.
//! Vein face bits are tracked; `attemptPlaceSculk` requires `hasFace`.
//!
//! Copyright (c) 2026 Neutron Contributors -- MIT License

use crate::biome_source::biome_id_at_block;
use crate::feature_catalog::{self, step};
use crate::feature_rng::FeatureRandom;
use crate::generator::WORLD_BOTTOM;
use crate::multiface_spreader::{self, FaceMap, MultifaceSpreader, DIRS as MF_DIRS};
use crate::region_buf::RegionBuf;
use crate::surface::BlockId;
use crate::worldgen::WorldgenState;
use std::collections::HashMap;
use std::sync::atomic::{AtomicI32, AtomicU32, Ordering};

pub static SCULK_TRIES: AtomicU32 = AtomicU32::new(0);
static LAST_CATALYST_ROLL: AtomicU32 = AtomicU32::new(0);
pub static SCULK_BIOME_OK: AtomicU32 = AtomicU32::new(0);
pub static SCULK_SPREAD_OK: AtomicU32 = AtomicU32::new(0);
pub static SCULK_PLACED: AtomicU32 = AtomicU32::new(0);
pub static SCULK_VEIN_PLACED: AtomicU32 = AtomicU32::new(0);

pub const SCULK_ENABLED: bool = true;

/// Diagnostic context for NEUTRON_SCULK_CURSOR_DRAWS (per-cursor draw log).
static PATCH_I: AtomicI32 = AtomicI32::new(-1);
static ATT_I: AtomicI32 = AtomicI32::new(-1);

fn cursor_draws_on() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var_os("NEUTRON_SCULK_CURSOR_DRAWS").is_some())
}

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
    extra_rare_growths: i32,
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
            extra_rare_growths: 0,
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
            s.extra_rare_growths = c["extra_rare_growths"].as_i64().unwrap_or(0) as i32;
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
    // ChunkStatus.FEATURES: when the center is decorated, neighbours are still
    // at carvers (no sculk). Then each neighbour origin runs and can spill in.
    let origin_order = decoration_origin_order(region.chunks);
    for (pos, &(cxl, czl)) in origin_order.iter().enumerate() {
        let ox0 = region.origin_x + cxl * 16;
        let oz0 = region.origin_z + czl * 16;
        // Diagnostic: decorate only the center origin (cross-origin analysis).
        if std::env::var_os("NEUTRON_SCULK_ONE_ORIGIN").is_some() && (cxl, czl) != (1, 1) {
            continue;
        }

        // Vanilla decorates each origin while not-yet-decorated neighbour
        // chunks are still at CARVERS — their step<=6 output (ore blobs) is
        // not visible yet. Revert ore cells in those chunks for the duration
        // of this origin's vein+patch pass, then restore them.
        let saved = mask_undecorated_ores(region, &origin_order[pos + 1..]);

        let mut rng = FeatureRandom::new(level_seed);
        let dec = rng.set_decoration_seed(level_seed, ox0, oz0);
        if std::env::var_os("NEUTRON_SCULK_NO_VEIN").is_none() {
            rng.set_feature_seed(dec, idx_vein, step::UNDERGROUND_DECORATION);
            place_sculk_vein(&mut rng, region, state, &mut faces, ox0, oz0, &vein_cfg);
        }

        let mut rng = FeatureRandom::new(level_seed);
        let dec = rng.set_decoration_seed(level_seed, ox0, oz0);
        rng.set_feature_seed(dec, idx_patch, step::UNDERGROUND_DECORATION);
        place_sculk_patch(&mut rng, region, state, &mut faces, ox0, oz0, &patch_cfg);

        for (x, y, z, b) in saved {
            // Vanilla: a later origin's ore pass cannot replace sculk-family
            // blocks (not in stone_ore_replaceables), so sculk/veins spilled
            // onto masked cells during this pass must survive the restore.
            if matches!(
                region.get(x, y, z),
                BlockId::Sculk
                    | BlockId::SculkVein
                    | BlockId::SculkSensor
                    | BlockId::SculkShrieker
                    | BlockId::SculkCatalyst
            ) {
                continue;
            }
            region.set(x, y, z, b);
        }
    }
}

/// Ore-family blocks (step 6 output) that an undecorated neighbour would not
/// show yet. The revert base (deepslate below y=0, stone above) is
/// behaviourally equivalent for sculk: same sturdiness, same replaceable tags,
/// same vein-placeable set.
fn mask_undecorated_ores(
    region: &mut RegionBuf,
    undecorated: &[(i32, i32)],
) -> Vec<(i32, i32, i32, BlockId)> {
    let mut saved = Vec::new();
    for &(cxl, czl) in undecorated {
        let x0 = region.origin_x + cxl * 16;
        let z0 = region.origin_z + czl * 16;
        for y in WORLD_BOTTOM..crate::generator::WORLD_TOP {
            for z in z0..z0 + 16 {
                for x in x0..x0 + 16 {
                    let b = region.get(x, y, z);
                    if is_ore_family(b) {
                        saved.push((x, y, z, b));
                        region.set(
                            x,
                            y,
                            z,
                            if y < 0 {
                                BlockId::Deepslate
                            } else {
                                BlockId::Stone
                            },
                        );
                    }
                }
            }
        }
    }
    saved
}

fn is_ore_family(b: BlockId) -> bool {
    matches!(
        b,
        BlockId::CoalOre
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
    )
}

/// Center chunk first (vanilla FEATURES), then the other origins in x/z order.
/// NEUTRON_SCULK_ORIGIN_ORDER (diagnostic): `row`/`col` = plain scan with the
/// center in natural position; `center_row`/`center_col` = center first.
fn decoration_origin_order(chunks: i32) -> Vec<(i32, i32)> {
    let mid = chunks / 2;
    let mut out: Vec<(i32, i32)> = Vec::with_capacity((chunks * chunks) as usize);
    let order = std::env::var("NEUTRON_SCULK_ORIGIN_ORDER")
        .unwrap_or_else(|_| "center_row".into());
    match order.as_str() {
        "row" => {
            for czl in 0..chunks {
                for cxl in 0..chunks {
                    out.push((cxl, czl));
                }
            }
        }
        "col" => {
            for cxl in 0..chunks {
                for czl in 0..chunks {
                    out.push((cxl, czl));
                }
            }
        }
        "center_col" => {
            out.push((mid, mid));
            for cxl in 0..chunks {
                for czl in 0..chunks {
                    if cxl == mid && czl == mid {
                        continue;
                    }
                    out.push((cxl, czl));
                }
            }
        }
        _ => {
            out.push((mid, mid));
            for czl in 0..chunks {
                for cxl in 0..chunks {
                    if cxl == mid && czl == mid {
                        continue;
                    }
                    out.push((cxl, czl));
                }
            }
        }
    }
    out
}

// ===================== MultifaceGrowthFeature (sculk_vein) =====================

/// validDirections order from MultifaceGrowthConfiguration:
/// ceiling UP, floor DOWN, then Direction.Plane.HORIZONTAL
/// (NORTH, EAST, SOUTH, WEST) — then Util.shuffledCopy.
fn valid_growth_dirs(can_floor: bool, can_ceiling: bool, can_wall: bool) -> Vec<(i32, i32, i32)> {
    let mut v = Vec::with_capacity(6);
    // javap MultifaceGrowthConfiguration.validDirections:
    //   UP if ceiling, DOWN if floor, then Plane.HORIZONTAL.
    if can_ceiling {
        v.push(DIRS[1]); // UP
    }
    if can_floor {
        v.push(DIRS[0]); // DOWN
    }
    if can_wall {
        v.push(DIRS[2]); // NORTH
        v.push(DIRS[5]); // EAST
        v.push(DIRS[3]); // SOUTH
        v.push(DIRS[4]); // WEST
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
    let gate = |x: i32, y: i32, z: i32| is_deep_dark_at(state, x, y, z);
    place_sculk_vein_gated(rng, region, faces, ox0, oz0, cfg, &gate);
}

/// MultifaceGrowthFeature driver with an injectable position gate
/// (vanilla: `minecraft:biome` placement modifier == deep_dark check).
fn place_sculk_vein_gated(
    rng: &mut FeatureRandom,
    region: &mut RegionBuf,
    faces: &mut FaceMap,
    ox0: i32,
    oz0: i32,
    cfg: &VeinConfig,
    gate: &dyn Fn(i32, i32, i32) -> bool,
) {
    // sculk_vein config: floor+ceiling+wall all true
    let base_dirs = valid_growth_dirs(true, true, true);
    let count = cfg.count_min + rng.next_int(cfg.count_max - cfg.count_min + 1);
    for _ in 0..count {
        let x = ox0 + rng.next_int(16);
        let z = oz0 + rng.next_int(16);
        let y = WORLD_BOTTOM + rng.next_int(256 - WORLD_BOTTOM + 1);
        if !gate(x, y, z) {
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
    let dump = std::env::var_os("NEUTRON_SCULK_PATCHES").is_some();
    for i in 0..cfg.patch_count {
        PATCH_I.store(i, Ordering::Relaxed);
        SCULK_TRIES.fetch_add(1, Ordering::Relaxed);
        let x = ox0 + rng.next_int(16);
        let z = oz0 + rng.next_int(16);
        let y = WORLD_BOTTOM + rng.next_int(256 - WORLD_BOTTOM + 1);
        if dump && ox0 == 96 && oz0 == -32 {
            eprintln!(
                "att o=({ox0},{oz0}) i={i} ({x},{y},{z}) biome={} here={:?}",
                is_deep_dark_at(state, x, y, z) as u8,
                region.get(x, y, z)
            );
        }
        let biome_ok = is_deep_dark_at(state, x, y, z);
        if !biome_ok {
            continue;
        }
        SCULK_BIOME_OK.fetch_add(1, Ordering::Relaxed);
        let here = region.get(x, y, z);
        let spread = can_spread_from(region, x, y, z);
        if !spread {
            if dump
                && y >= -40
                && y < -8
                && matches!(here, BlockId::Air | BlockId::Water)
            {
                let mut nbs = Vec::new();
                for &(dx, dy, dz) in &DIRS {
                    nbs.push(region.get(x + dx, y + dy, z + dz));
                }
                eprintln!(
                    "spread_fail o=({ox0},{oz0}) i={i} ({x},{y},{z}) here={here:?} nbs={nbs:?}"
                );
            }
            continue;
        }
        SCULK_SPREAD_OK.fetch_add(1, Ordering::Relaxed);
        if dump {
            rng.reset_draw_count();
            if ox0 == 96 && oz0 == -32 && i == 0 {
                eprint!("nbhd ({x},{y},{z}):");
                for dy in -1..=1 {
                    for dz in -1..=1 {
                        for dx in -1..=1 {
                            eprint!(
                                " ({},{},{})={:?}",
                                x + dx,
                                y + dy,
                                z + dz,
                                region.get(x + dx, y + dy, z + dz)
                            );
                        }
                    }
                }
                eprintln!();
            }
        }
        run_patch(rng, region, faces, x, y, z, cfg);
        if dump {
            eprintln!(
                "sculk_patch o=({ox0},{oz0}) i={i} ({x},{y},{z}) here={here:?} below={:?} draws={}",
                region.get(x, y - 1, z),
                rng.draw_count()
            );
            if ox0 == 96 && oz0 == -32 && i == 0 {
                let mut sc = 0u32;
                let mut vn = 0u32;
                let mut sn = 0u32;
                let mut sh = 0u32;
                let mut cat = 0u32;
                for dy in -16..=16 {
                    for dz in -16..=16 {
                        for dx in -16..=16 {
                            match region.get(x + dx, y + dy, z + dz) {
                                BlockId::Sculk => sc += 1,
                                BlockId::SculkVein => vn += 1,
                                BlockId::SculkSensor => {
                                    sn += 1;
                                    eprintln!("  sensor ({},{},{})", x + dx, y + dy, z + dz);
                                }
                                BlockId::SculkShrieker => {
                                    sh += 1;
                                    eprintln!("  shrieker ({},{},{})", x + dx, y + dy, z + dz);
                                }
                                BlockId::SculkCatalyst => cat += 1,
                                _ => {}
                            }
                        }
                    }
                }
                eprintln!(
                    "first_patch_r16 sculk={sc} vein={vn} sensor={sn} shrieker={sh} cat={cat}"
                );
            }
        }
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
        for attempt in 0..cfg.spread_attempts {
            ATT_I.store(attempt, Ordering::Relaxed);
            update_cursors(rng, region, faces, ox, oy, oz, &mut cursors, spread_veins);
            dump_tick_world_if_requested(region, faces, attempt);
            if cursor_draws_on() {
                for c in cursors.iter() {
                    eprintln!(
                        "CURA i={} att={} {},{},{} ch={} dec={} upd={} faces={}",
                        PATCH_I.load(Ordering::Relaxed),
                        attempt,
                        c.x,
                        c.y,
                        c.z,
                        c.charge,
                        c.decay_delay,
                        c.update_delay,
                        c.facings.map(|f| f as i32).unwrap_or(-1)
                    );
                }
            }
            if std::env::var_os("NEUTRON_SCULK_STEPS").is_some()
                && (attempt < 8
                || attempt == 15
                || attempt == 31
                || attempt == 63
                || std::env::var_os("NEUTRON_SCULK_ALL_TICKS").is_some())
            {
                let mut sc = 0u32;
                let mut vn = 0u32;
                for z in region.origin_z..region.origin_z + region.side {
                    for y in WORLD_BOTTOM..crate::generator::WORLD_TOP {
                        for x in region.origin_x..region.origin_x + region.side {
                            match region.get(x, y, z) {
                                BlockId::Sculk => sc += 1,
                                BlockId::SculkVein => vn += 1,
                                _ => {}
                            }
                        }
                    }
                }
                eprintln!(
                    "after {} cursors={} sculk={} vein={} nextBits={}",
                    attempt + 1,
                    cursors.len(),
                    sc,
                    vn,
                    rng.draw_count()
                );
                if attempt == 2 {
                    eprint!("after3 sculk:");
                    for z in region.origin_z..region.origin_z + region.side {
                        for y in (oy - 16)..=(oy + 16) {
                            for x in region.origin_x..region.origin_x + region.side {
                                if region.get(x, y, z) == BlockId::Sculk {
                                    eprint!(" {x},{y},{z}");
                                }
                            }
                        }
                    }
                    eprintln!();
                    eprint!("after3 vein:");
                    for z in region.origin_z..region.origin_z + region.side {
                        for y in (oy - 16)..=(oy + 16) {
                            for x in region.origin_x..region.origin_x + region.side {
                                if region.get(x, y, z) == BlockId::SculkVein {
                                    let m = faces.get(&(x, y, z)).copied().unwrap_or(0);
                                    eprint!(" {x},{y},{z}#{m}");
                                }
                            }
                        }
                    }
                    eprintln!();
                    for c in cursors.iter() {
                        eprintln!("  live {},{},{} ch={}", c.x, c.y, c.z, c.charge);
                    }
                }
                if attempt < 1 {
                    eprint!("after1 vein:");
                    for z in region.origin_z..region.origin_z + region.side {
                        for y in (oy - 16)..=(oy + 16) {
                            for x in region.origin_x..region.origin_x + region.side {
                                if region.get(x, y, z) == BlockId::SculkVein {
                                    let m = faces.get(&(x, y, z)).copied().unwrap_or(0);
                                    eprint!(" {x},{y},{z}#{m}");
                                }
                            }
                        }
                    }
                    eprintln!();
                    eprint!("after1 sculk:");
                    for z in region.origin_z..region.origin_z + region.side {
                        for y in (oy - 16)..=(oy + 16) {
                            for x in region.origin_x..region.origin_x + region.side {
                                if region.get(x, y, z) == BlockId::Sculk {
                                    eprint!(" {x},{y},{z}");
                                }
                            }
                        }
                    }
                    eprintln!();
                }
            }
            // Vanilla still executes all spread_attempts after the cursor
            // list becomes empty. There are no further RNG draws in that
            // case, but the differential harness needs all 64 snapshots.
            if cursors.is_empty()
                && std::env::var_os("NEUTRON_SCULK_TICK_DUMPS").is_none()
            {
                break;
            }
            if std::env::var_os("NEUTRON_SCULK_DUMP_PATCH").is_some()
                && (attempt == 63
                    || attempt == 37
                    || attempt == 5
                    || std::env::var_os("NEUTRON_SCULK_DUMP_ALL").is_some())
            {
                let mut cells: Vec<(i32, i32, i32, u8, &str)> = Vec::new();
                for z in region.origin_z..region.origin_z + region.side {
                    for y in WORLD_BOTTOM..crate::generator::WORLD_TOP {
                        for x in region.origin_x..region.origin_x + region.side {
                            match region.get(x, y, z) {
                                BlockId::Sculk => cells.push((x, y, z, 0, "sculk")),
                                BlockId::SculkVein => cells.push((
                                    x,
                                    y,
                                    z,
                                    faces.get(&(x, y, z)).copied().unwrap_or(0),
                                    "vein",
                                )),
                                _ => {}
                            }
                        }
                    }
                }
                cells.sort();
                eprintln!(
                    "PATCHEND a={} o=({ox},{oy},{oz}) sculk={} vein={}",
                    attempt,
                    cells.iter().filter(|c| c.4 == "sculk").count(),
                    cells.iter().filter(|c| c.4 == "vein").count()
                );
                for (x, y, z, m, k) in cells {
                    eprintln!("CELL {x},{y},{z} {k}#{m}");
                }
            }
        }
    }

    let dump = std::env::var_os("NEUTRON_SCULK_PATCHES").is_some();
    let roll = rng.next_f32();
    LAST_CATALYST_ROLL.store((roll * 1_000_000.0) as u32, Ordering::Relaxed);
    if dump {
        eprintln!(
            "catalyst_roll={roll:.6} <= {} ? {} below={:?}",
            cfg.catalyst_chance,
            roll <= cfg.catalyst_chance,
            region.get(ox, oy - 1, oz)
        );
    }
    if roll <= cfg.catalyst_chance {
        // isCollisionShapeFullBlock below
        if is_collision_full_block(region.get(ox, oy - 1, oz)) {
            region.set(ox, oy, oz, BlockId::SculkCatalyst);
            faces.remove(&(ox, oy, oz));
        }
    }
    // SculkPatchFeature.place: extraRareGrowths.sample then offset(nextInt(5)-2, 0, nextInt(5)-2).
    // deep_dark JSON is 0 (ConstantInt.sample — no RNG).
    let extra = cfg.extra_rare_growths;
    for _ in 0..extra {
        let px = ox + rng.next_int(5) - 2;
        let pz = oz + rng.next_int(5) - 2;
        if region.get(px, oy, pz) != BlockId::Air {
            continue;
        }
        if is_collision_full_block(region.get(px, oy - 1, pz)) {
            region.set(px, oy, pz, BlockId::SculkShrieker);
        }
    }
}

fn dump_tick_world_if_requested(region: &RegionBuf, faces: &FaceMap, attempt: i32) {
    if std::env::var_os("NEUTRON_SCULK_TICK_DUMPS").is_none() {
        return;
    }
    let selected = std::env::var("NEUTRON_SCULK_TICK_PATCH")
        .ok()
        .and_then(|s| s.parse::<i32>().ok());
    if selected.is_some_and(|i| i != PATCH_I.load(Ordering::Relaxed)) {
        return;
    }

    let mut cells = Vec::new();
    for z in region.origin_z..region.origin_z + region.side {
        for y in WORLD_BOTTOM..crate::generator::WORLD_TOP {
            for x in region.origin_x..region.origin_x + region.side {
                match region.get(x, y, z) {
                    BlockId::Sculk => cells.push(format!("{x},{y},{z} sculk#0")),
                    BlockId::SculkVein => {
                        let mask = faces.get(&(x, y, z)).copied().unwrap_or(0);
                        cells.push(format!("{x},{y},{z} vein#{mask}"));
                    }
                    _ => {}
                }
            }
        }
    }
    cells.sort_unstable();
    let dir = std::env::var_os("NEUTRON_SCULK_TICK_DUMPS_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_default();
    let path = dir.join(format!(
        "rust-tickfull-{}-{}.txt",
        PATCH_I.load(Ordering::Relaxed),
        attempt
    ));
    std::fs::write(path, cells.join("\n") + "\n").expect("write sculk tick dump");
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
    let log = cursor_draws_on();
    let mut next: Vec<Cursor> = Vec::new();
    for (idx, mut c) in cursors.drain(..).enumerate() {
        let chess = (c.x - ox).abs().max((c.y - oy).abs()).max((c.z - oz).abs());
        if chess > 1024 {
            continue;
        }
        let before = rng.draw_count();
        if log {
            eprintln!(
                "CDCB i={} att={} cur={} {},{},{} ch={} dec={} upd={} faces={}",
                PATCH_I.load(Ordering::Relaxed),
                ATT_I.load(Ordering::Relaxed),
                idx,
                c.x,
                c.y,
                c.z,
                c.charge,
                c.decay_delay,
                c.update_delay,
                c.facings.map(|f| f as i32).unwrap_or(-1)
            );
        }
        cursor_update(rng, region, faces, ox, oy, oz, &mut c, spread_veins);
        if log {
            eprintln!(
                "CDCE i={} att={} cur={} n={} ch={} pos={},{},{} faces={}",
                PATCH_I.load(Ordering::Relaxed),
                ATT_I.load(Ordering::Relaxed),
                idx,
                rng.draw_count() - before,
                c.charge,
                c.x,
                c.y,
                c.z,
                c.facings.map(|f| f as i32).unwrap_or(-1)
            );
        }
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
    if std::env::var_os("NEUTRON_SCULK_STEPS").is_some() {
        eprintln!(
            "  upd ({},{},{}) here={here:?} ch={} dec={} faces={:?}",
            c.x, c.y, c.z, c.charge, c.decay_delay, c.facings
        );
    }
    // updateDecayDelay uses the behaviour from *before* the move
    // (and after attemptSpreadVein re-read if canChangeBlockStateOnSpread).
    let mut behaviour_is_sculk = is_sculk_behaviour(here);

    // Vanilla keeps `currentState` as a BlockState SNAPSHOT read here (A),
    // refreshed only after attemptSpreadVein when canChangeBlockStateOnSpread
    // (SculkBlock overrides it to false; veins/DEFAULT keep true). Both
    // onDischarged calls and the final availableFaces use that SNAPSHOT, not
    // the live state — faces added mid-tick (e.g. by attemptPlaceSculk's
    // spreadAll from a new support) are WIPED by the discharge rewrite.
    let mut stale_vein_mask = if here == BlockId::SculkVein {
        faces.get(&(c.x, c.y, c.z)).copied().unwrap_or(0)
    } else {
        0
    };

    if spread_veins {
        if attempt_spread_vein(region, faces, c.x, c.y, c.z, c.facings, here) {
            // SculkBlock.canChangeBlockStateOnSpread == false; default is true.
            if here != BlockId::Sculk {
                here = region.get(c.x, c.y, c.z);
                behaviour_is_sculk = is_sculk_behaviour(here);
                stale_vein_mask = if here == BlockId::SculkVein {
                    faces.get(&(c.x, c.y, c.z)).copied().unwrap_or(0)
                } else {
                    0
                };
            }
        }
    }

    c.charge = attempt_use_charge(rng, region, faces, c, here, spread_veins, ox, oy, oz);
    if c.charge <= 0 {
        if here == BlockId::SculkVein {
            on_discharged_snapshot(region, faces, c.x, c.y, c.z, stale_vein_mask);
        }
        return;
    }

    let mut moved = false;
    if let Some((nx, ny, nz)) = get_valid_movement(rng, region, faces, c.x, c.y, c.z) {
        if here == BlockId::SculkVein {
            on_discharged_snapshot(region, faces, c.x, c.y, c.z, stale_vein_mask);
        }
        c.x = nx;
        c.y = ny;
        c.z = nz;
        moved = true;
        // closerThan(Vec3i(originX, cursorY, originZ), 15) → distSqr < 225
        let dx = (c.x - ox) as f64;
        let dz = (c.z - oz) as f64;
        if dx * dx + dz * dz >= WORLDGEN_MAX_DIST * WORLDGEN_MAX_DIST {
            c.charge = 0;
            return;
        }
        here = region.get(c.x, c.y, c.z);
    }

    // MultifaceBlock.availableFaces(currentState): when the cursor did NOT
    // move, currentState is still the (A/B) SNAPSHOT — faces added mid-tick
    // must not leak into cursor.facings. After a move it is a fresh read.
    // Empty set on non-multiface SculkBehaviour (sculk); catalyst/sensor/
    // shrieker are not SculkBehaviour — facings left unchanged.
    if here == BlockId::SculkVein {
        c.facings = Some(if moved {
            faces.get(&(c.x, c.y, c.z)).copied().unwrap_or(0)
        } else {
            stale_vein_mask
        });
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

/// SculkVeinBlock.onDischarged with vanilla's STALE-state semantics
/// (ChargeCursor.update passes its start-of-tick snapshot): strip the
/// SNAPSHOT's faces toward current sculk neighbours, then setBlock the
/// stripped snapshot — wiping faces the live state gained mid-tick.
/// Non-empty result rewrites the cell as a vein even if the live mask
/// had extra faces; empty result turns it back to air.
fn on_discharged_snapshot(
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
    let mut mask = snapshot_mask;
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
        region.set(x, y, z, BlockId::SculkVein);
        faces.insert((x, y, z), mask);
    }
}

fn on_discharged(region: &mut RegionBuf, faces: &mut FaceMap, x: i32, y: i32, z: i32) {
    // SculkVeinBlock.onDischarged: strip faces toward sculk; clear if no faces
    if let Some(c) = crate::multiface_spreader::trace_coord() {
        if (x, y, z) == c {
            eprintln!("TRACE live_discharge ({x},{y},{z}) mask={:?}", faces.get(&(x, y, z)));
        }
    }
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
fn is_face_sturdy_at(
    region: &RegionBuf,
    faces: &FaceMap,
    x: i32,
    y: i32,
    z: i32,
    face: usize,
) -> bool {
    match region.get(x, y, z) {
        BlockId::SculkVein => {
            faces.get(&(x, y, z)).copied().unwrap_or(0) & (1u8 << face) != 0
        }
        BlockId::SculkSensor | BlockId::SculkShrieker => face == 0 || face == 1,
        b => is_collision_full_block(b),
    }
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

/// Diagnostic override for the deep_dark biome gate (parity experiments feed
/// the vanilla chunk's real 3D biomes here). `None` → neutron's biome source.
static BIOME_GATE_OVERRIDE: std::sync::RwLock<Option<std::sync::Arc<dyn Fn(i32, i32, i32) -> bool + Send + Sync>>> =
    std::sync::RwLock::new(None);

/// Install a diagnostic deep_dark gate override (parity experiments only).
pub fn set_biome_gate_override(
    f: Option<std::sync::Arc<dyn Fn(i32, i32, i32) -> bool + Send + Sync>>,
) {
    *BIOME_GATE_OVERRIDE.write().unwrap() = f;
}

fn is_deep_dark_at(state: &WorldgenState, x: i32, y: i32, z: i32) -> bool {
    if let Some(f) = &*BIOME_GATE_OVERRIDE.read().unwrap() {
        return f(x, y, z);
    }
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
    run_patch(&mut rng, region, &mut faces, origin.0, origin.1, origin.2, &cfg);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decoration_origin_order_center_first() {
        let o = decoration_origin_order(3);
        assert_eq!(o[0], (1, 1));
        assert_eq!(o.len(), 9);
        assert!(o[1..].iter().all(|&p| p != (1, 1)));
    }

    #[test]
    fn flat_floor_matches_probe_sculk_patch() {
        let (sculk, vein, growth, roll, draws) = probe_flat_floor_patch();
        assert_eq!(sculk, 166, "ProbeSculkPatch sculk");
        assert_eq!(vein, 174, "ProbeSculkPatch vein");
        assert_eq!(growth, 0);
        assert_eq!(draws, 4735, "nextBits including catalyst nextFloat");
        assert!(
            (roll - 0.821367).abs() < 1e-5,
            "catalyst_roll={roll} ProbeSculkPatch=0.8213676"
        );
    }

    #[test]
    fn one_patch_on_flat_floor_converts_deepslate() {
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
            catalyst_chance: 0.0,
            extra_rare_growths: 0,
            patch_count: 1,
        };
        let mut faces = FaceMap::new();
        let mut rng = FeatureRandom::new(1);
        run_patch(&mut rng, &mut region, &mut faces, 8, 10, 8, &cfg);
        let mut sculk = 0u32;
        let mut vein = 0u32;
        for z in region.origin_z..region.origin_z + region.side {
            for x in region.origin_x..region.origin_x + region.side {
                match region.get(x, 9, z) {
                    BlockId::Sculk => sculk += 1,
                    _ => {}
                }
                if region.get(x, 10, z) == BlockId::SculkVein {
                    vein += 1;
                }
            }
        }
        assert!(
            sculk >= 50,
            "flat-floor patch should convert a disk of deepslate, sculk={sculk} vein={vein}"
        );
    }

    #[test]
    fn fisher_yates_18_matches_probe_seed() {
        let mut rng = FeatureRandom::new(12345);
        rng.set_seed(12345);
        let mut a: Vec<i32> = (0..18).collect();
        let mut i = a.len();
        while i > 1 {
            let j = rng.next_int(i as i32) as usize;
            a.swap(i - 1, j);
            i -= 1;
        }
        assert_eq!(
            a,
            vec![7, 6, 14, 12, 1, 16, 17, 10, 13, 2, 9, 5, 15, 4, 0, 3, 11, 8],
            "Util.shuffle 26.2 ProbeShuffle"
        );
        let mut rng = FeatureRandom::new(12345);
        rng.set_seed(12345);
        let dirs = crate::multiface_spreader::all_shuffled(&mut rng);
        // Direction.allShuffled: WEST UP SOUTH DOWN EAST NORTH
        assert_eq!(dirs, vec![4, 1, 3, 0, 5, 2]);
    }
}
