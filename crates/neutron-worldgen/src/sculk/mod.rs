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
use serde_json::Value;
use std::collections::HashMap;
use std::sync::atomic::{AtomicI32, AtomicU32, Ordering};

pub static SCULK_TRIES: AtomicU32 = AtomicU32::new(0);
/// Env-gated per-write trace (`NEUTRON_SCULK_TRACE_W`): prints every
/// `RegionBuf::set` while enabled. Toggled around patch 0 of the traced origin.
pub static SET_TRACE: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);
pub(super) static LAST_CATALYST_ROLL: AtomicU32 = AtomicU32::new(0);
pub static SCULK_BIOME_OK: AtomicU32 = AtomicU32::new(0);
pub static SCULK_SPREAD_OK: AtomicU32 = AtomicU32::new(0);
pub static SCULK_PLACED: AtomicU32 = AtomicU32::new(0);
pub static SCULK_VEIN_PLACED: AtomicU32 = AtomicU32::new(0);

pub const SCULK_ENABLED: bool = true;

/// Diagnostic context for NEUTRON_SCULK_CURSOR_DRAWS (per-cursor draw log).
pub(super) static PATCH_I: AtomicI32 = AtomicI32::new(-1);
pub(super) static ATT_I: AtomicI32 = AtomicI32::new(-1);

pub(super) fn cursor_draws_on() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var_os("NEUTRON_SCULK_CURSOR_DRAWS").is_some())
}

pub(super) const DIRS: [(i32, i32, i32); 6] = MF_DIRS;

pub(super) const CHARGE_DECAY_RATE: i32 = 5;
pub(super) const ADDITIONAL_DECAY_RATE: i32 = 10;
pub(super) const GROWTH_SPAWN_COST: i32 = 50;
pub(super) const MAX_CURSORS: usize = 32;
pub(super) const WORLDGEN_MAX_DIST: f64 = 15.0;

pub(super) struct PatchConfig {
    pub(super) charge_count: i32,
    pub(super) amount_per_charge: i32,
    pub(super) spread_attempts: i32,
    pub(super) spread_rounds: i32,
    pub(super) growth_rounds: i32,
    pub(super) catalyst_chance: f32,
    pub(super) extra_rare_growths: i32,
    /// Raw JSON of `extra_rare_growths` (IntProvider). Sampled at the vanilla
    /// point in the RNG stream; ConstantInt consumes no draws.
    pub(super) extra_rare_growths_provider: Option<Value>,
    pub(super) patch_count: i32,
}

impl PatchConfig {
    pub(super) fn load() -> Self {
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
                    extra_rare_growths_provider: None,
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
            s.extra_rare_growths_provider = Some(c["extra_rare_growths"].clone());
        }
        s
    }
}

pub(super) struct VeinConfig {
    pub(super) count_min: i32,
    pub(super) count_max: i32,
    pub(super) search_range: i32,
    pub(super) chance_of_spreading: f32,
}

impl VeinConfig {
    pub(super) fn load() -> Self {
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

/// Feature-output families that an undecorated neighbour chunk (still at
/// CARVERS) must not show while the current origin is being decorated.
///
/// Vanilla `ChunkStatus.FEATURES` requires the 3×3 neighbourhood of the
/// decorated center at CARVERS (ChunkPyramid.GENERATION_PYRAMID: FEATURES
/// needs CARVERS at radius 1), so the not-yet-decorated origins have no
/// feature output at all. In the single-buffer model the buffer accumulates
/// output origin by origin (center first); masking reverts feature-family
/// cells of the *undecorated* origins to the terrain base for the duration
/// of the current origin's pass, then restores them (last-writer-wins).
pub(crate) const FAMILY_ORES: u8 = 1 << 0;
pub(crate) const FAMILY_SCULK: u8 = 1 << 1;
pub(crate) const FAMILY_VEGETAL: u8 = 1 << 2;
/// FLUID_SPRINGS output. Neutron does not port step 8 yet, so no block is
/// produced under this family (aquifer/lava are terrain, never masked).
pub(crate) const FAMILY_SPRINGS: u8 = 1 << 3;
pub(crate) const FAMILY_ALL: u8 = FAMILY_ORES | FAMILY_SCULK | FAMILY_VEGETAL | FAMILY_SPRINGS;

/// Apply sculk_vein + sculk_patch for one chunk origin `(ox0, oz0)`.
///
/// `undecorated` are the origins after this one in the decoration order:
/// their feature output (ores/sculk/vegetal spilled by earlier origins) is
/// reverted to the terrain base for the duration of the vein+patch pass and
/// restored afterwards. `faces` is the region-wide face map so a later
/// origin's cursors see earlier origins' veins (vanilla reads vein faces
/// from the world block states).
pub(crate) fn apply_sculk_origin(
    region: &mut RegionBuf,
    state: &WorldgenState,
    ox0: i32,
    oz0: i32,
    undecorated: &[(i32, i32)],
    faces: &mut FaceMap,
) {
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

    // Diagnostic: decorate only the center origin (cross-origin analysis).
    if std::env::var_os("NEUTRON_SCULK_ONE_ORIGIN").is_some()
        && ((ox0 - region.origin_x) / 16 != 1 || (oz0 - region.origin_z) / 16 != 1)
    {
        return;
    }

    let level_seed = state.seed;
    // Vanilla decorates each origin while not-yet-decorated neighbour chunks
    // are still at CARVERS — their feature output (ores/sculk/vegetal spilled
    // by earlier origins) is not visible yet. Revert those cells for the
    // duration of this origin's patch pass, then restore them.
    let saved = mask_undecorated_output(region, undecorated, FAMILY_ALL);

    let mut rng = FeatureRandom::new(level_seed);
    let dec = rng.set_decoration_seed(level_seed, ox0, oz0);
    rng.set_feature_seed(dec, idx_patch, step::UNDERGROUND_DECORATION);
    place_sculk_patch(&mut rng, region, state, faces, ox0, oz0, &patch_cfg);

    restore_masked(region, saved);
}

/// Apply ONLY sculk_patch_deep_dark (catalyst + charge spreader) for one
/// origin. sculk_vein goes through the generic feature dispatch instead.
pub fn apply_sculk_patch_only(
    region: &mut RegionBuf,
    state: &WorldgenState,
    ox0: i32,
    oz0: i32,
    undecorated: &[(i32, i32)],
    faces: &mut FaceMap,
) {
    if !SCULK_ENABLED {
        return;
    }
    let patch_cfg = PatchConfig::load();
    let idx_patch = feature_catalog::global_feature_index(
        step::UNDERGROUND_DECORATION,
        "sculk_patch_deep_dark",
    )
    .unwrap_or(1);

    let level_seed = state.seed;
    let saved = mask_undecorated_output(region, undecorated, FAMILY_ALL);

    let mut rng = FeatureRandom::new(level_seed);
    let dec = rng.set_decoration_seed(level_seed, ox0, oz0);
    rng.set_feature_seed(dec, idx_patch, step::UNDERGROUND_DECORATION);
    place_sculk_patch(&mut rng, region, state, faces, ox0, oz0, &patch_cfg);

    restore_masked(region, saved);
}

/// Apply sculk_vein + sculk_patch for every chunk origin in the feature
/// region, origin-major center-first (the driver used by the standalone
/// `generate_ores_region`-style paths and diagnostics).
pub fn apply_sculk_region(region: &mut RegionBuf, state: &WorldgenState) {
    if !SCULK_ENABLED {
        return;
    }
    // ChunkStatus.FEATURES: when the center is decorated, neighbours are still
    // at carvers (no sculk). Then each neighbour origin runs and can spill in.
    let origin_order = decoration_origin_order(region.chunks, region.origin_x, region.origin_z);
    let mut faces: FaceMap = HashMap::new();
    for (pos, &(cxl, czl)) in origin_order.iter().enumerate() {
        let ox0 = region.origin_x + cxl * 16;
        let oz0 = region.origin_z + czl * 16;
        apply_sculk_origin(
            region,
            state,
            ox0,
            oz0,
            &origin_order[pos + 1..],
            &mut faces,
        );
    }
}

/// Restore cells masked by [`mask_undecorated_output`].
///
/// Vanilla: a later origin's ore pass cannot replace sculk-family blocks (not
/// in stone_ore_replaceables), so sculk/veins spilled onto masked cells during
/// this pass must survive the restore.
pub(crate) fn restore_masked(region: &mut RegionBuf, saved: Vec<(i32, i32, i32, BlockId)>) {
    for (x, y, z, b) in saved {
        if is_sculk_family(region.get(x, y, z)) {
            continue;
        }
        region.set(x, y, z, b);
    }
}

/// Feature-family blocks (step 6/7/9 output) that an undecorated neighbour
/// would not show yet. The revert base (deepslate below y=0, stone above) is
/// behaviourally equivalent for the ported steps: same sturdiness, same
/// replaceable tags, same vein-placeable set.
pub(crate) fn mask_undecorated_output(
    region: &mut RegionBuf,
    undecorated: &[(i32, i32)],
    families: u8,
) -> Vec<(i32, i32, i32, BlockId)> {
    let mut saved = Vec::new();
    for &(cxl, czl) in undecorated {
        let x0 = region.origin_x + cxl * 16;
        let z0 = region.origin_z + czl * 16;
        for y in WORLD_BOTTOM..crate::generator::WORLD_TOP {
            for z in z0..z0 + 16 {
                for x in x0..x0 + 16 {
                    let b = region.get(x, y, z);
                    let is_family = (families & FAMILY_ORES != 0 && is_ore_family(b))
                        || (families & FAMILY_SCULK != 0 && is_sculk_family(b))
                        || (families & FAMILY_VEGETAL != 0 && is_vegetal_family(b))
                        || (families & FAMILY_SPRINGS != 0 && is_spring_family(b));
                    if is_family {
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

fn is_sculk_family(b: BlockId) -> bool {
    matches!(
        b,
        BlockId::Sculk
            | BlockId::SculkVein
            | BlockId::SculkSensor
            | BlockId::SculkShrieker
            | BlockId::SculkCatalyst
    )
}

/// Vegetal-decoration output (step 9): logs, leaves, grass, saplings,
/// flowers, vines, moss carpets/blocks.
pub fn is_vegetal_family(b: BlockId) -> bool {
    matches!(
        b,
        BlockId::OakLog
            | BlockId::OakLeaves
            | BlockId::DarkOakLog
            | BlockId::DarkOakLeaves
            | BlockId::PaleOakLog
            | BlockId::PaleOakLeaves
            | BlockId::ShortGrass
            | BlockId::LeafLitter
            | BlockId::MossCarpet
            | BlockId::MossBlock
            | BlockId::PaleMossBlock
            | BlockId::PaleMossCarpet
            | BlockId::PaleHangingMoss
            | BlockId::CaveVines
            | BlockId::CaveVinesPlant
            | BlockId::Azalea
            | BlockId::FloweringAzalea
    )
}

/// FLUID_SPRINGS (step 8) output. Not ported yet — always empty. Kept so the
/// masking family set matches vanilla's step coverage once springs land.
fn is_spring_family(_b: BlockId) -> bool {
    false
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
pub(crate) fn decoration_origin_order(
    chunks: i32,
    origin_x: i32,
    origin_z: i32,
) -> Vec<(i32, i32)> {
    let mid = chunks / 2;
    let mut out: Vec<(i32, i32)> = Vec::with_capacity((chunks * chunks) as usize);
    let order =
        // Default = global ticket-wavefront approximation: refs pregenned with
        // concentric forceload squares centred on the world origin decorate
        // origins in ascending distance from (0,0), and cross-origin blob /
        // tree overwrites resolve last-writer-wins in that order (agentH:
        // granite/diorite/andesite swaps -76..-97% vs spawn-spiral). The old
        // per-buffer centre-first "spiral" remains available as an explicit
        // NEUTRON_SCULK_ORIGIN_ORDER=spiral for A/B measurement.
        std::env::var("NEUTRON_SCULK_ORIGIN_ORDER").unwrap_or_else(|_| "canonical_pregen".into());
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
        // B4 T3c (subagent-derived): for chunk (0,0) seed 424242, the vanilla
        // before-neighbors are (-1,0) and (0,-1) ONLY (their pale_moss patches
        // water-reject draws 1/4); the other 6 decorated after (0,0) (their
        // patches absent at draw time). Local coords (center mid,mid): (-1,0)
        // = (mid-1,mid), (0,-1) = (mid,mid-1). Order: the two before-neighbors,
        // then the center, then the rest (center THIRD - its scans see only the
        // confirmed before-neighbors' spillover, matching vanilla).
        "vanilla_spawn" => {
            out.push((mid - 1, mid));
            out.push((mid, mid - 1));
            out.push((mid, mid));
            for czl in 0..chunks {
                for cxl in 0..chunks {
                    if (cxl == mid - 1 && czl == mid)
                        || (cxl == mid && czl == mid - 1)
                        || (cxl == mid && czl == mid)
                    {
                        continue;
                    }
                    out.push((cxl, czl));
                }
            }
        }
        // EXPERIMENT: all 8 neighbors before the center (vanilla's spawn-area
        // decoration order appears to place neighbors first — the (0,0) tree
        // scans saw the (-1,0) tree spillover).
        "ring_first" => {
            for czl in 0..chunks {
                for cxl in 0..chunks {
                    if cxl == mid && czl == mid {
                        continue;
                    }
                    out.push((cxl, czl));
                }
            }
            out.push((mid, mid));
        }
        // EXPERIMENT: the setInitialSpawn square-spiral request order
        // (MinecraftServer.setInitialSpawn, offsets starting (0,0),(1,0),
        // (1,1),(0,1),(-1,1),(-1,0),(-1,-1),(0,-1),(1,-1), ...).
        "spiral" => {
            let spiral: [(i32, i32); 9] = [
                (0, 0),
                (1, 0),
                (1, 1),
                (0, 1),
                (-1, 1),
                (-1, 0),
                (-1, -1),
                (0, -1),
                (1, -1),
            ];
            for (dx, dz) in spiral {
                out.push((mid + dx, mid + dz));
            }
        }
        // Global ticket-wavefront approximation for refs pregenned with
        // concentric forceload squares centred on the WORLD ORIGIN
        // (`forceload add -128 -128 127 127` + outer ring): origins closer to
        // (0,0) reached FEATURES status earlier, so their feature spill-over
        // was already visible when later origins ran their gate checks.
        // Sort by squared distance of the origin CENTRE from the world
        // origin; ties broken x-then-z for determinism.
        "world_origin" => {
            let mut v: Vec<(i64, i32, i32)> = Vec::with_capacity((chunks * chunks) as usize);
            for czl in 0..chunks {
                for cxl in 0..chunks {
                    let wx = (origin_x + cxl * 16 + 8) as i64;
                    let wz = (origin_z + czl * 16 + 8) as i64;
                    v.push((wx * wx + wz * wz, cxl, czl));
                }
            }
            v.sort();
            out.extend(v.into_iter().map(|(_, cxl, czl)| (cxl, czl)));
        }
        // Faithful model of the CANONICAL ref pregen procedure
        // (`new-mc-version.sh`): ONE inner 16×16-chunk forceload square
        // covering chunks (-8..7)², settle, then FOUR outer-ring strip
        // commands in fixed order (west, east, south, north). Ticket
        // insertion order ≈ ChunkPos.rangeClosed iteration (x fastest, z
        // outer, from each command's min corner), which is what decoration
        // completion approximates. Cross-origin overwrites resolve
        // last-writer-wins along this sequence.
        "canonical_pregen" => {
            let mut v: Vec<(i32, i64, i32, i32)> = Vec::with_capacity((chunks * chunks) as usize);
            for czl in 0..chunks {
                for cxl in 0..chunks {
                    let cx = (origin_x >> 4) + cxl;
                    let cz = (origin_z >> 4) + czl;
                    let phase = if (-8..=7).contains(&cx) && (-8..=7).contains(&cz) {
                        0 // inner forceload square
                    } else if (-12..=-11).contains(&cx) {
                        1 // west strip
                    } else if (10..=11).contains(&cx) {
                        2 // east strip
                    } else if (-12..=-11).contains(&cz) {
                        3 // south strip
                    } else {
                        4 // north strip
                    };
                    // ChunkPos.rangeClosed cursor: x fastest within a command.
                    v.push((phase, (cz * 64 + cx) as i64, czl, cxl));
                }
            }
            v.sort();
            out.extend(v.into_iter().map(|(_, _, czl, cxl)| (cxl, czl)));
        }
        "custom" => {
            // NEUTRON_DECO_CUSTOM_ORDER="dx,dz;dx,dz;..." — explicit origin
            // order as neighbor offsets relative to the center (order search).
            let s = std::env::var("NEUTRON_DECO_CUSTOM_ORDER").unwrap_or_default();
            for p in s.split(';').filter(|p| !p.is_empty()) {
                let mut it = p.split(',');
                let dx: i32 = it.next().unwrap().parse().unwrap();
                let dz: i32 = it.next().unwrap().parse().unwrap();
                out.push((mid + dx, mid + dz));
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

mod blocks;
mod cursor;
mod gates;
mod place;
mod probes;

pub use gates::set_biome_gate_override;
pub use probes::{
    probe_apply_vein_origin, probe_flat_floor_patch, probe_patch_gate_origin,
    probe_real_first_patch, probe_run_patch, probe_vein_gate_origin, probe_vein_origin_traced,
};

use cursor::{run_patch, update_cursors};
use gates::is_deep_dark_at;
use place::{place_sculk_patch, place_sculk_vein};
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decoration_origin_order_default_world_origin() {
        // Default = `canonical_pregen`: faithful model of the canonical ref
        // pregen (inner forceload square (-8..7)² then west/east/south/north
        // ring strips, x-fastest within each command). agentH/agentI windows:
        // stone-blob swaps -76..-97% vs spawn-spiral; best of all presets on
        // worst chunks. spiral/world_origin remain selectable via env.
        let o = decoration_origin_order(3, 0, 0);
        assert_eq!(o.len(), 9);
        // A 3x3 buffer at origin (0,0) lies entirely inside the inner
        // forceload square ⇒ phase 0 for all origins ⇒ plain row-major
        // (z outer, x fastest), matching ChunkPos.rangeClosed insertion.
        assert_eq!(
            o,
            vec![
                (0, 0),
                (1, 0),
                (2, 0),
                (0, 1),
                (1, 1),
                (2, 1),
                (0, 2),
                (1, 2),
                (2, 2),
            ]
        );
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
                    extra_rare_growths_provider: None,
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





