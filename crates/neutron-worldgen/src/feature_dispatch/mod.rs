//! Feature placement dispatcher (biome lists + `placed_feature` JSON).
//!
//! Routes by configured_feature `type` to the Rust ports. Placement modifiers
//! implemented: count, in_square, height_range, heightmap (`OCEAN_FLOOR` /
//! `WORLD_SURFACE`), biome filter, random_offset.
//!
//! Copyright (c) 2026 Neutron Contributors -- MIT License
//   block_predicate_filter (air, matching_blocks offset — common cases)
//   noise_threshold_count (uses feature RNG as density stand-in until noise port)
//   rarity_filter
//
// Feature types:
//   sculk_patch, multiface_growth, tree, simple_block, random_selector,
//   ore (delegates to existing OreFeature path when called from ores step)

use crate::biome_source::{biome_id, biome_id_at_block};
#[allow(unused_imports)]
use std::collections::HashMap;
use crate::feature_catalog::{self, step};
use crate::feature_rng::FeatureRandom;
use crate::generator::{WORLD_BOTTOM, WORLD_TOP};
use crate::region_buf::RegionBuf;
use crate::surface::BlockId;
use crate::tree;
use crate::worldgen::WorldgenState;
use serde_json::Value;

/// Run decoration for one generation step across all origins in `region`.
///
/// Origin-major, center first (vanilla FEATURES order) with masking of the
/// not-yet-decorated origins.
pub fn apply_step_region(
    region: &mut RegionBuf,
    state: &WorldgenState,
    gen_step: i32,
    primary_biome: &str,
) {
    let order = crate::sculk::decoration_origin_order(region.chunks, region.origin_x, region.origin_z);
    for (pos, &(cxl, czl)) in order.iter().enumerate() {
        let ox0 = region.origin_x + cxl * 16;
        let oz0 = region.origin_z + czl * 16;
        apply_step_origin(
            region,
            state,
            gen_step,
            ox0,
            oz0,
            &order[pos + 1..],
            primary_biome,
        );
    }
}

/// Run one generation step for ONE chunk origin `(ox0, oz0)`.
///
/// `undecorated` are the origins after this one in the decoration order: their
/// feature output is masked to the terrain base for the duration of the pass
/// and restored afterwards (vanilla decorates each origin while the
/// not-yet-decorated neighbours are still at CARVERS).
///
/// The candidate feature list is the union of the feature lists of every biome
/// present in the 3×3 chunk neighbourhood of the origin (vanilla
/// `ChunkGenerator.applyBiomeDecoration` collects `section.getBiomes().getAll`
/// over `ChunkPos.rangeClosed(center, 1)`). For the center origin the 3×3
/// coincides with the region buffer; for edge origins the neighbourhood is
/// clamped to the buffer, which is an approximation (vanilla would read the
/// full 3×3 from the world).
pub(crate) fn apply_step_origin(
    region: &mut RegionBuf,
    state: &WorldgenState,
    gen_step: i32,
    ox0: i32,
    oz0: i32,
    undecorated: &[(i32, i32)],
    primary_biome: &str,
) {
    let features = feature_catalog::features_at_step(primary_biome, gen_step);
    if features.is_empty() {
        return;
    }
    let level_seed = state.seed;
    // Union of the biomes present in the sections of the 3×3 chunks around
    // this origin (clamped to the buffer), then the union of their feature
    // lists in global FeatureSorter index order.
    let biomes = origin_biome_union_memo(region, state, ox0, oz0);
    let mut merged: Vec<(i32, String)> = Vec::new();
    for b in &biomes {
        for f in feature_catalog::features_at_step(b, gen_step) {
            if let Some(idx) = feature_catalog::global_feature_index(gen_step, &f) {
                if !merged.iter().any(|(_, s)| s == &f) {
                    merged.push((idx, f));
                }
            }
        }
    }
    merged.sort_by_key(|(i, _)| *i);
    let list: Vec<String> = merged.into_iter().map(|(_, s)| s).collect();
    // Vanilla's WorldGenRegion.random is created fresh for THIS origin's
    // decoration pass (WorldGenRegion ctor seeds it at the origin min corner)
    // and survives across every feature of the pass.
    region.set_region_random(state.region_random(ox0, oz0));
    let saved: Vec<(i32, i32, i32, BlockId)> = if std::env::var_os("NEUTRON_DECO_NO_MASK").is_some() {
        // DIAGNOSTIC: skip the undecorated-origin masking entirely (every
        // origin sees all neighbours' feature output). Measures how much of
        // the tree/lush displacement is gate-visibility driven.
        Vec::new()
    } else {
        region.current_writer = crate::writers::MASK;
        let s = crate::sculk::mask_undecorated_output(region, undecorated, crate::sculk::FAMILY_ALL);
        region.current_writer = crate::writers::TERRAIN;
        s
    };
    if list.is_empty() {
        // fall back to primary list if no biome matched
        place_feature_list(region, state, level_seed, ox0, oz0, gen_step, &features);
    } else {
        place_feature_list(region, state, level_seed, ox0, oz0, gen_step, &list);
    }
    region.current_writer = crate::writers::MASK;
    crate::sculk::restore_masked(region, saved);
    region.current_writer = crate::writers::TERRAIN;
}

/// Biomes present in the sections of the 3×3 chunk neighbourhood of origin
/// `(ox0, oz0)`, clamped to the region buffer (approximation for edge
/// origins — vanilla reads the full 3×3 from the world).
///
/// Memoized per `(seed, ox0, oz0)`: every generation step asks the same
/// question for the same origin, and one evaluation costs ~14k climate
/// lookups (3×3×24 sections × 16 quart cells).
fn origin_biome_union_memo(
    region: &RegionBuf,
    state: &WorldgenState,
    ox0: i32,
    oz0: i32,
) -> Vec<String> {
    thread_local! {
        static CACHE: std::cell::RefCell<HashMap<(i64, i32, i32), Vec<String>>> =
            std::cell::RefCell::new(HashMap::new());
    }
    CACHE.with(|c| {
        let key = (state.seed, ox0, oz0);
        if let Some(v) = c.borrow().get(&key) {
            return v.clone();
        }
        let v = origin_biome_union(region, state, ox0, oz0);
        c.borrow_mut().insert(key, v.clone());
        v
    })
}

/// Sampled on the same 4×4×24 quart grid that `generate_noise_and_surface`
/// stores (one Y quart per section at the section midpoint) via the noise
/// biome (no voronoi — mirrors vanilla `fillBiomesFromNoise`).
fn origin_biome_union(
    region: &RegionBuf,
    state: &WorldgenState,
    ox0: i32,
    oz0: i32,
) -> Vec<String> {
    let cxl = (ox0 - region.origin_x) / 16;
    let czl = (oz0 - region.origin_z) / 16;
    let mut fallback = 0u32;
    let mut names: Vec<String> = Vec::new();
    let mut push = |id: u8| {
        let n = biome_id_to_name(id);
        if !names.iter().any(|x| x == n) {
            names.push(n.to_string());
        }
    };
    for dz in -1..=1i32 {
        for dx in -1..=1i32 {
            let ncx = cxl + dx;
            let ncz = czl + dz;
            if ncx < 0 || ncz < 0 || ncx >= region.chunks || ncz >= region.chunks {
                continue;
            }
            let cx0 = region.origin_x + ncx * 16;
            let cz0 = region.origin_z + ncz * 16;
            for section in 0..24i32 {
                let base_y_q = (WORLD_BOTTOM + section * 16) >> 2;
                for sy4 in 0..4i32 {
                    for bz4 in 0..4i32 {
                        for bx4 in 0..4i32 {
                            let (qx, qy, qz) = (
                                cx0 / 4 + bx4,
                                base_y_q + sy4,
                                cz0 / 4 + bz4,
                            );
                            // Stored grid first (zero climate evals); the grid
                            // holds exactly `noise_biome_at_quart` values.
                            let stored = region.stored_noise_biome(qx, qy, qz);
                            if stored.is_none() {
                                fallback += 1;
                            }
                            let id = stored.unwrap_or_else(|| {
                                crate::biome_manager::noise_biome_at_quart(
                                    state, qx, qy, qz,
                                )
                            });
                            push(id);
                        }
                    }
                }
            }
        }
    }
    if std::env::var_os("NEUTRON_STEP_TIMING").is_some() {
        eprintln!("[union] origin ({ox0},{oz0}) fallback={fallback}");
    }
    names
}

fn place_feature_list(
    region: &mut RegionBuf,
    state: &WorldgenState,
    level_seed: i64,
    ox0: i32,
    oz0: i32,
    gen_step: i32,
    list: &[String],
) {
    let mut rng = FeatureRandom::new(level_seed);
    let decoration_seed = rng.set_decoration_seed(level_seed, ox0, oz0);
    // Vanilla places in increasing FeatureSorter global index.
    let mut indexed: Vec<(i32, &String)> = list
        .iter()
        .filter_map(|id| feature_catalog::global_feature_index(gen_step, id).map(|i| (i, id)))
        .collect();
    indexed.sort_by_key(|(i, _)| *i);
    for (global_index, placed_id) in indexed {
        rng.set_feature_seed(decoration_seed, global_index, gen_step);
        place_placed_feature_step(&mut rng, region, state, ox0, oz0, placed_id, gen_step);
    }
}

/// Place one placed_feature id (with placement modifiers) into the region.
pub fn place_placed_feature(
    rng: &mut FeatureRandom,
    region: &mut RegionBuf,
    state: &WorldgenState,
    origin_min_x: i32,
    origin_min_z: i32,
    placed_id: &str,
) {
    place_placed_feature_step(
        rng,
        region,
        state,
        origin_min_x,
        origin_min_z,
        placed_id,
        step::VEGETAL_DECORATION,
    );
}

/// [`place_placed_feature`] with an explicit generation step (the `minecraft:biome`
/// placement filter must check the feature list of the *actual* step — vanilla
/// `placeWithBiomeCheck` runs per step).
pub(crate) fn place_placed_feature_step(
    rng: &mut FeatureRandom,
    region: &mut RegionBuf,
    state: &WorldgenState,
    origin_min_x: i32,
    origin_min_z: i32,
    placed_id: &str,
    gen_step: i32,
) {
    let Some(placed) = feature_catalog::load_placed_feature(placed_id) else {
        return;
    };
    let feature_ref = placed["feature"].as_str().map(|s| s.to_string());

    // PlacedFeature.placeWithContext is a lazy stream: Count → InSquare →
    // filters → Feature.place. Each surviving position is placed *before*
    // the next InSquare nextInt (TreeFeature consumes a lot of RNG).
    // Collecting all xz first then placing desyncs every attempt after the first.
    let configured: Option<&'static Value> = if let Some(ref id) = feature_ref {
        feature_catalog::load_configured_feature(id)
    } else {
        placed.get("feature").filter(|v| v.is_object())
    };
    let base_count = placement_count(rng, &placed);
    static TRACE_TREES: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    let trace_trees = *TRACE_TREES.get_or_init(|| std::env::var_os("NEUTRON_TRACE_TREES").is_some());
    if trace_trees {
        eprintln!(
            "[trace] chunk=({origin_min_x},{origin_min_z}) placed={placed_id} count={base_count}"
        );
    }
    let mut draw_no = 0;
    for _ in 0..base_count {
        draw_no += 1;
        let mut x = origin_min_x;
        let mut z = origin_min_z;
        let mut y = 0i32;
        let mut ok = true;
        let mut has_xz = false;
        let mut has_y = false;

        if let Some(mods) = placed["placement"].as_array() {
            for m in mods {
                let ty = m["type"].as_str().unwrap_or("");
                match ty {
                    "minecraft:count" | "minecraft:count_on_every_layer" => {}
                    "minecraft:in_square" => {
                        x = origin_min_x + rng.next_int(16);
                        z = origin_min_z + rng.next_int(16);
                        has_xz = true;
                    }
                    "minecraft:height_range" => {
                        y = sample_height(rng, &m["height"]);
                        has_y = true;
                    }
                    "minecraft:heightmap" => {
                        if !has_xz {
                            x = origin_min_x + rng.next_int(16);
                            z = origin_min_z + rng.next_int(16);
                            has_xz = true;
                        }
                        // WorldGenRegion.getHeight = ChunkAccess.getHeight + 1
                        // = Heightmap.getFirstAvailable (one above highest opaque).
                        let kind = parse_heightmap_kind(m["heightmap"].as_str().unwrap_or(""));
                        if let Some(sy) = heightmap_top(region, x, z, kind) {
                            y = sy + 1;
                            has_y = true;
                        } else {
                            ok = false;
                        }
                    }
                    "minecraft:random_offset" => {
                        // Java RandomOffsetPlacement.getPositions samples in order
                        // scatterX (xz_spread), scatterY (y_spread), scatterZ (xz_spread).
                        let ox = sample_int_provider(rng, &m["xz_spread"]);
                        let oy = sample_int_provider(rng, &m["y_spread"]);
                        let oz = sample_int_provider(rng, &m["xz_spread"]);
                        x += ox;
                        y += oy;
                        z += oz;
                    }
                    "minecraft:environment_scan" => {
                        // EnvironmentScanPlacement: scan from current y in
                        // direction_of_search while allowed_search_condition holds
                        // (up to max_steps), stopping at the first target_condition
                        // match. No RNG consumed.
                        let dir = m["direction_of_search"].as_str().unwrap_or("down");
                        let max_steps = m["max_steps"].as_i64().unwrap_or(12) as i32;
                        let allowed = m.get("allowed_search_condition");
                        let target = &m["target_condition"];
                        let true_pred = serde_json::json!({"type":"minecraft:true"});
                        let allowed = allowed.unwrap_or(&true_pred);
                        let mut py = y;
                        let mut found = None;
                        // Vanilla EnvironmentScanPlacement.getPositions:
                        // leaving the build height returns Stream.empty()
                        // IMMEDIATELY — the final target re-check after the
                        // loop must not run on an out-of-world Y.
                        let mut out_of_world = false;
                        if !eval_block_predicate(region, x, py, z, allowed) {
                            ok = false;
                            break;
                        }
                        for _ in 0..max_steps {
                            if eval_block_predicate(region, x, py, z, target) {
                                found = Some(py);
                                break;
                            }
                            py += if dir == "down" { -1 } else { 1 };
                            if py < WORLD_BOTTOM || py >= WORLD_TOP {
                                out_of_world = true;
                                break;
                            }
                            if !eval_block_predicate(region, x, py, z, allowed) {
                                break;
                            }
                        }
                        // Tail check: reached by loop exhaustion or the
                        // `!allowed` break (vanilla still tests the target at
                        // that position once) but NOT after leaving the world.
                        if !out_of_world
                            && found.is_none()
                            && eval_block_predicate(region, x, py, z, target)
                        {
                            found = Some(py);
                        }
                        match found {
                            Some(fy) => {
                                y = fy;
                                has_y = true;
                            }
                            None => ok = false,
                        }
                    }
                    "minecraft:biome" => {
                        let bname = biome_name_at(state, x, y, z);
                        let step_list = feature_catalog::features_at_step(
                            &bname,
                            gen_step,                        );
                        let id = strip(placed_id);
                        if !step_list.iter().any(|f| strip(f) == id) {
                            ok = false;
                        }
                    }
                    "minecraft:block_predicate_filter" => {
                        if !eval_block_predicate(region, x, y, z, &m["predicate"]) {
                            ok = false;
                        }
                    }
                    "minecraft:surface_water_depth_filter" => {
                        // SurfaceWaterDepthFilter: WORLD_SURFACE - OCEAN_FLOOR <= max.
                        let max = m["max_water_depth"].as_i64().unwrap_or(0) as i32;
                        if column_water_depth(region, x, z) > max {
                            ok = false;
                        }
                    }
                    "minecraft:noise_threshold_count" => {
                        // already expanded into base_count via placement_count
                    }
                    "minecraft:rarity_filter" => {
                        // 26.2: nextFloat() < 1.0f / chance
                        let chance = m["chance"].as_i64().unwrap_or(1) as i32;
                        if chance <= 0 || rng.next_f32() >= 1.0 / chance as f32 {
                            ok = false;
                        }
                    }
                    "minecraft:surface_relative_threshold_filter" => {
                        // SurfaceRelativeThresholdFilter.shouldPlace: Java long
                        // add so omitted min=Integer.MIN_VALUE does not wrap.
                        // getHeight = Heightmap.getFirstAvailable = solid Y + 1.
                        let kind = parse_heightmap_kind(m["heightmap"].as_str().unwrap_or(""));
                        let min_inc = m["min_inclusive"].as_i64().unwrap_or(i32::MIN as i64);
                        let max_inc = m["max_inclusive"].as_i64().unwrap_or(i32::MAX as i64);
                        let surface = heightmap_top(region, x, z, kind)
                            .map(|s| s as i64 + 1)
                            .unwrap_or(i64::MIN / 4);
                        let yy = y as i64;
                        if !(surface + min_inc <= yy && yy <= surface + max_inc) {
                            ok = false;
                        }
                    }
                    _ => {}
                }
            }
        }
        if !has_xz {
            x = origin_min_x + rng.next_int(16);
            z = origin_min_z + rng.next_int(16);
        }
        if !has_y {
            y = heightmap_top(region, x, z, HeightmapKind::OceanFloor)
                .map(|s| s + 1)
                .unwrap_or(64);
        }
        if !ok {
            if trace_trees {
                eprintln!("[trace]   draw {draw_no} REJECT (x={x},z={z},y={y})");
            }
            continue;
        }
        let mut tree_placed = false;
        // NEUTRON_DECO_SKIP_TREE_DRAWS=N (diagnostic): reject the first N
        // draws at the feature gate (no tree RNG consumed) — simulates the
        // "draws rejected by decoration-time terrain" hypothesis for the
        // vanilla-stream derivation (deco_stream_probe).
        static SKIP_TREE_DRAWS: std::sync::OnceLock<Option<i32>> = std::sync::OnceLock::new();
        if let Some(skip) = *SKIP_TREE_DRAWS.get_or_init(|| {
            std::env::var("NEUTRON_DECO_SKIP_TREE_DRAWS")
                .ok()
                .and_then(|s| s.parse::<i32>().ok())
        }) {
            if draw_no <= skip {
                if trace_trees {
                    eprintln!("[trace]   draw {draw_no} SKIP (x={x},z={z},y={y})");
                }
                continue;
            }
        }
        if let Some(ref cfg) = configured {
            dispatch_configured(rng, region, Some(state), x, y, z, cfg, gen_step);
            tree_placed = true;
        } else if let Some(ref id) = feature_ref {
            // nested placed
            if let Some(inner) = feature_catalog::load_placed_feature(id) {
                if let Some(cid) = inner["feature"].as_str() {
                    if let Some(cfg) = feature_catalog::load_configured_feature(cid) {
                        dispatch_configured(rng, region, Some(state), x, y, z, &cfg, gen_step);
                    }
                }
            }
        }
        if trace_trees {
            eprintln!(
                "[trace]   draw {draw_no} ACCEPT x={x} z={z} y={y} tree_feature={tree_placed}"
            );
        }
    }
}

/// `#minecraft:replaceable` subset reachable in the decoration buffer
/// (`BlockState.canBeReplaced` gate of createTopperWithSideChance).
fn can_be_replaced(b: BlockId) -> bool {
    matches!(
        b,
        BlockId::Air
            | BlockId::CaveAir
            | BlockId::Water
            | BlockId::Lava
            | BlockId::ShortGrass
            | BlockId::TallGrass
            | BlockId::LeafLitter
            | BlockId::Vine
            | BlockId::MossCarpet
            | BlockId::GlowLichen
            | BlockId::Snow
            | BlockId::PowderSnow
    )
}

/// `MossyCarpetBlock.placeAt(level, pos, level.getRandom(), 2)` — decompile
/// MossyCarpetBlock.java:143-155 + createTopperWithSideChance:166-192.
///
/// The region buffer stores block ids only, so the LOW/TALL side-face
/// properties are not modeled; what matters for parity and for RNG-stream
/// fidelity is (a) WHICH cells end up with pale_moss_carpet and (b) exactly
/// how many `nextBoolean` draws the topper consumes from WorldGenRegion.random
/// (one per surviving side face, NORTH/EAST/SOUTH/WEST order).
fn place_mossy_carpet(region: &mut RegionBuf, x: i32, y: i32, z: i32) {
    // canSurvive(BASE=true): below must be non-air.
    if region.get(x, y - 1, z).is_air() {
        return;
    }
    // setBlock(pos, getUpdatedState(default BASE layer)) — sides are LOW where
    // horizontal neighbours are full-cube faces; id unchanged.
    region.set(x, y, z, BlockId::PaleMossCarpet);

    // createTopperWithSideChance gate: `(!isCarpetAbove || !above.BASE) &&
    // (isCarpetAbove || above.canBeReplaced())`. The BASE/topper split lets
    // stacked carpets behave like vanilla: a BASE layer above blocks the
    // topper (and its dice), a topper layer above allows it.
    let above = region.get(x, y + 1, z);
    if above == BlockId::PaleMossCarpet {
        return;
    }
    if above != BlockId::PaleMossCarpetTopper && !can_be_replaced(above) {
        return;
    }
    // aboveState = getUpdatedState(BASE=false, pos.above(), createSides=true):
    // sides LOW where the neighbour OF THE ABOVE CELL is a full-cube face.
    let mut sides = [false; 4]; // Plane.HORIZONTAL: N(-Z), E(+X), S(+Z), W(-X)
    for (i, (dx, dz)) in [(0i32, -1i32), (1, 0), (0, 1), (-1, 0)].iter().enumerate() {
        if blocks_motion(region.get(x + dx, y + 1, z + dz)) {
            sides[i] = true;
        }
    }
    // One random.nextBoolean() per surviving side keeps or drops it; these
    // dice come from WorldGenRegion.random, NOT the decoration stream.
    let mut kept = false;
    for side in &mut sides {
        if *side && !region.with_region_random(|r| r.next_boolean()).unwrap_or(false) {
            *side = false;
        }
        kept |= *side;
    }
    // hasFaces(aboveState) && aboveState != previous ⇒ place the topper (a
    // BASE=false layer); the bottom layer's sides become TALL under it.
    if kept {
        region.set(x, y + 1, z, BlockId::PaleMossCarpetTopper);
    }
}

/// Dispatch by configured_feature.type
pub(crate) fn dispatch_configured(
    rng: &mut FeatureRandom,
    region: &mut RegionBuf,
    state: Option<&WorldgenState>,
    x: i32,
    y: i32,
    z: i32,
    cfg: &Value,
    gen_step: i32,
) {
    let ty = cfg["type"].as_str().unwrap_or("");
    // Writer attribution: every dispatched feature stamps its family before
    // placing; recursive selectors (random_selector etc.) re-stamp with the
    // inner configured type for free precision.
    region.current_writer = crate::writers::for_configured_type(ty);
    match ty {
        "minecraft:simple_block" => {
            // SimpleBlockFeature.place (26.2): sample `to_place` first (the
            // weighted provider consumes RNG even when the attempt is later
            // rejected), gate on canSurvive, then per-block-class placement:
            //   TallGrass (DoublePlantBlock)  → below ∈ #dirt + air above,
            //                                    writes lower AND upper half
            //   PaleMossCarpet (MossyCarpet)  → MossyCarpetBlock.placeAt with
            //                                    topper dice from
            //                                    WorldGenRegion.random
            //   SmallDripleaf                 → waterlogged-capable double
            //                                    plant (existing port)
            //   everything else               → plain setBlock behind the
            //                                    historical air/water guard
            if let Some(block) = block_from_to_place(rng, &cfg["config"]["to_place"]) {
                match block {
                    BlockId::TallGrass => {
                        // TallGrassBlock.canSurvive = VegetationBlock
                        // .mayPlaceOn = below ∈ #dirt (farmland never occurs
                        // in worldgen). Whole placement fails when the cell
                        // above is occupied (DoublePlantBlock branch).
                        if is_in_tag(region.get(x, y - 1, z), "#minecraft:dirt")
                            && region.get(x, y + 1, z).is_air()
                        {
                            region.set(x, y, z, block);
                            region.set(x, y + 1, z, block);
                        }
                    }
                    BlockId::PaleMossCarpet => place_mossy_carpet(region, x, y, z),
                    BlockId::ShortGrass => {
                        // SimpleBlockFeature.place gates on Block.canSurvive:
                        // ShortGrass is TallGrassBlock class → VegetationBlock
                        // mayPlaceOn = below ∈ #supports_vegetation. Without the
                        // gate we plant grass on any air cell (incl. cave walls
                        // at y<0 where vanilla never does).
                        let cur = region.get(x, y, z);
                        if matches!(cur, BlockId::Air | BlockId::CaveAir)
                            && is_in_tag(
                                region.get(x, y - 1, z),
                                "#minecraft:supports_vegetation",
                            )
                        {
                            region.set(x, y, z, block);
                        }
                    }
                    b => {
                        let cur = region.get(x, y, z);
                        if matches!(cur, BlockId::Air | BlockId::CaveAir | BlockId::Water) {
                            if b == BlockId::SmallDripleaf {
                                if region.get(x, y + 1, z).is_air()
                                    && small_dripleaf_may_place_on(
                                        region.get(x, y - 1, z),
                                        cur,
                                    )
                                {
                                    region.set(x, y, z, b);
                                    region.set(x, y + 1, z, b);
                                }
                            } else {
                                region.set(x, y, z, b);
                            }
                        }
                    }
                }
            }
        }
        "minecraft:huge_red_mushroom" | "minecraft:huge_brown_mushroom" => {
            // AbstractHugeMushroomFeature port — the dark_forest_vegetation
            // selector and swamp/mushroom-field placed features route here.
            crate::feature_dispatch::place_huge_mushroom(
                rng,
                region,
                x,
                y,
                z,
                cfg,
                ty.ends_with("red"),
            );
        }
        "minecraft:tree" => {
            tree::place_tree_from_config(rng, region, state, x, y, z, cfg);
        }
        "minecraft:random_selector" => {
            // weighted chance features then default
            if let Some(features) = cfg["config"]["features"].as_array() {
                let trace_trees =
                    std::env::var_os("NEUTRON_TRACE_TREES").is_some();
                for f in features {
                    let chance = f["chance"].as_f64().unwrap_or(0.0) as f32;
                    let roll = rng.next_f32();
                    if trace_trees {
                        let name = f["feature"].as_str().unwrap_or("<inline>");
                        eprintln!("[selector] roll={roll:.4} chance={chance} -> {}",
                            if roll < chance { name } else { "next" });
                    }
                    if roll < chance {
                        place_feature_ref(rng, region, state, x, y, z, &f["feature"], gen_step);
                        return;
                    }
                }
            }
            if let Some(def) = cfg["config"].get("default") {
                place_feature_ref(rng, region, state, x, y, z, def, gen_step);
            }
        }
        "minecraft:sculk_patch" => {
            // handled by sculk module with proper seeds — skip here
        }
        "minecraft:multiface_growth" => {
            // sculk_vein stays in sculk.rs (step 7, dedicated seeds).
            place_multiface_growth(rng, region, x, y, z, cfg);
        }
        "minecraft:vines" => {
            place_vines(rng, region, x, y, z);
        }
        "minecraft:root_system" => {
            place_root_system(rng, region, state, x, y, z, cfg);
        }
        "minecraft:sea_pickle" => {
            // SeaPickleFeature.place (26.2): count attempts; x/z =
            // nextInt(8)-nextInt(8); y = OCEAN_FLOOR first-available; pickle
            // count = nextInt(4)+1 (state detail, draw consumed); place if
            // WATER + solid below (canSurvive).
            let count = cfg["config"]["count"].as_i64().unwrap_or(20) as i32;
            for _ in 0..count {
                let px = x + rng.next_int(8) - rng.next_int(8);
                let pz = z + rng.next_int(8) - rng.next_int(8);
                let Some(oy) = heightmap_top(region, px, pz, HeightmapKind::OceanFloor) else {
                    continue;
                };
                let py = oy + 1;
                let _pickles = rng.next_int(4) + 1;
                if region.get(px, py, pz) == BlockId::Water
                    && blocks_motion(region.get(px, py - 1, pz))
                {
                    region.set(px, py, pz, BlockId::SeaPickle);
                }
            }
        }
        "minecraft:seagrass" => {
            // SeagrassFeature.place: x/z = nextInt(8)-nextInt(8); y =
            // OCEAN_FLOOR; if WATER: tall = nextDouble < probability; place
            // seagrass (or tall + upper half) if canSurvive.
            let prob = cfg["config"]["probability"].as_f64().unwrap_or(0.0);
            let px = x + rng.next_int(8) - rng.next_int(8);
            let pz = z + rng.next_int(8) - rng.next_int(8);
            let Some(oy) = heightmap_top(region, px, pz, HeightmapKind::OceanFloor) else {
                return;
            };
            let py = oy + 1;
            if region.get(px, py, pz) == BlockId::Water {
                let is_tall = rng.next_f64() < prob;
                if blocks_motion(region.get(px, py - 1, pz)) {
                    if is_tall {
                        if region.get(px, py + 1, pz) == BlockId::Water {
                            region.set(px, py, pz, BlockId::TallSeagrass);
                            region.set(px, py + 1, pz, BlockId::TallSeagrass);
                        }
                    } else {
                        region.set(px, py, pz, BlockId::Seagrass);
                    }
                }
            }
        }
        "minecraft:kelp" => {
            // KelpFeature.place: y = OCEAN_FLOOR; if WATER: height =
            // 1 + nextInt(10); place kelp up the column (top kelp + plants).
            let Some(oy) = heightmap_top(region, x, z, HeightmapKind::OceanFloor) else {
                return;
            };
            let py = oy + 1;
            if region.get(x, py, z) == BlockId::Water {
                let height = 1 + rng.next_int(10);
                let mut kp = py;
                for h in 0..=height {
                    if region.get(x, kp, z) == BlockId::Water
                        && region.get(x, kp + 1, z) == BlockId::Water
                        && blocks_motion(region.get(x, kp - 1, z))
                    {
                        if h == height {
                            let _age = rng.next_int(4) + 20;
                            region.set(x, kp, z, BlockId::Kelp);
                        } else {
                            region.set(x, kp, z, BlockId::KelpPlant);
                        }
                    } else if h > 0 {
                        let below = kp - 1;
                        if blocks_motion(region.get(x, below - 1, z))
                            && region.get(x, below - 1, z) != BlockId::Kelp
                        {
                            let _age = rng.next_int(4) + 20;
                            region.set(x, below, z, BlockId::Kelp);
                        }
                        break;
                    }
                    kp += 1;
                }
            }
        }
        "minecraft:block_blob" => {
            // BlockBlobFeature.place (forest_rock): scan down until the block
            // below is placeable (forest_rock_can_place_on); 3 blobs of the
            // configured state (mossy_cobblestone).
            let Some(state) = cfg["config"]["state"]["Name"]
                .as_str()
                .and_then(BlockId::from_name)
            else {
                return;
            };
            let mut bx = x;
            let mut oy = y;
            let mut bz = z;
            while oy > WORLD_BOTTOM + 3
                && !is_in_tag(
                    region.get(bx, oy - 1, bz),
                    "#minecraft:forest_rock_can_place_on",
                )
            {
                oy -= 1;
            }
            if oy <= WORLD_BOTTOM + 3 {
                return;
            }
            for _ in 0..3 {
                let xr = rng.next_int(2);
                let yr = rng.next_int(2);
                let zr = rng.next_int(2);
                let tr = (xr + yr + zr) as f32 * 0.333 + 0.5;
                for dx in -xr..=xr {
                    for dy in -yr..=yr {
                        for dz in -zr..=zr {
                            let d2 = (dx * dx + dy * dy + dz * dz) as f32;
                            if d2 <= tr * tr {
                                region.set(bx + dx, oy + dy, bz + dz, state);
                            }
                        }
                    }
                }
                // origin.offset(-1 + nextInt(2), -nextInt(2), -1 + nextInt(2))
                bx += -1 + rng.next_int(2);
                oy -= rng.next_int(2);
                bz += -1 + rng.next_int(2);
            }
        }
        "minecraft:blue_ice" => {
            // BlueIceFeature.place (26.2): origin must be water at or below
            // sea level (63); a non-DOWN neighbor must be packed_ice;
            // place blue_ice at origin + 200 spread attempts.
            if y > 63 {
                return;
            }
            if region.get(x, y, z) != BlockId::Water && region.get(x, y - 1, z) != BlockId::Water {
                return;
            }
            let mut found = false;
            for &(dx, dy, dz) in &[(0, 1, 0), (0, 0, -1), (1, 0, 0), (0, 0, 1), (-1, 0, 0)] {
                if region.get(x + dx, y + dy, z + dz) == BlockId::PackedIce {
                    found = true;
                    break;
                }
            }
            if !found {
                return;
            }
            region.set(x, y, z, BlockId::BlueIce);
            for _ in 0..200 {
                let y_off = rng.next_int(5) - rng.next_int(6);
                let mut xz_diff = 3;
                if y_off < 2 {
                    xz_diff += y_off / 2;
                }
                if xz_diff >= 1 {
                    let bx = x + rng.next_int(xz_diff) - rng.next_int(xz_diff);
                    let bz = z + rng.next_int(xz_diff) - rng.next_int(xz_diff);
                    let by = y + y_off;
                    let b = region.get(bx, by, bz);
                    if b == BlockId::Air
                        || b == BlockId::CaveAir
                        || b == BlockId::Water
                        || b == BlockId::PackedIce
                        || b == BlockId::Ice
                    {
                        for &(dx, dy, dz) in &[
                            (0, 1, 0),
                            (0, -1, 0),
                            (0, 0, -1),
                            (1, 0, 0),
                            (0, 0, 1),
                            (-1, 0, 0),
                        ] {
                            if region.get(bx + dx, by + dy, bz + dz) == BlockId::BlueIce {
                                region.set(bx, by, bz, BlockId::BlueIce);
                                break;
                            }
                        }
                    }
                }
            }
        }
        "minecraft:ore" | "minecraft:scattered_ore" => {
            // Step 6 ores run via the features.rs batch (dedicated seeds).
            // Any other step (e.g. ore_infested at step 7) places generically.
            if gen_step != crate::feature_catalog::step::UNDERGROUND_ORES {
                crate::features::place_ore_from_config(rng, region, x, y, z, cfg);
            }
        }
        "minecraft:disk" => {
            // Step 6 disks (clay/mud/sand) run via the features.rs batch.
            // Other steps (e.g. ice_patch at step 4) place generically.
            if gen_step != crate::feature_catalog::step::UNDERGROUND_ORES {
                crate::features::place_disk_from_config(rng, region, x, y, z, cfg);
            }
        }
        "minecraft:desert_well" => {
            crate::feature_ports::place_desert_well(rng, region, x, y, z);
        }
        "minecraft:freeze_top_layer" => {
            if let Some(st) = state {
                crate::feature_ports::place_freeze_top_layer(region, st, x, y, z);
            }
        }
        "minecraft:spike" => {
            crate::feature_ports::place_spike(rng, region, x, y, z, cfg);
        }
        "minecraft:bamboo" => {
            crate::feature_ports::place_bamboo(rng, region, x, y, z, cfg);
        }
        "minecraft:monster_room" => {
            crate::feature_ports::place_monster_room(rng, region, x, y, z);
        }
        "minecraft:lake" => {
            crate::feature_ports::place_lake(rng, region, state, x, y, z, cfg);
        }
        "minecraft:sequence" => {
            crate::feature_ports::place_sequence(rng, region, state, x, y, z, cfg, gen_step);
        }
        "minecraft:speleothem_cluster" => {
            crate::feature_ports::place_speleothem_cluster(rng, region, x, y, z, cfg);
        }
        "minecraft:large_dripstone" => {
            crate::feature_ports::place_large_dripstone(rng, region, x, y, z, cfg);
        }
        "minecraft:iceberg" => {
            crate::feature_ports::place_iceberg(rng, region, x, z, cfg);
        }
        "minecraft:fossil" => {
            crate::feature_ports::place_fossil(rng, region, x, y, z, cfg);
        }
        "minecraft:geode" => {
            if let Some(st) = state {
                if std::env::var_os("NEUTRON_GEODE_TRACE").is_some() {
                    eprintln!("[geode] attempt at ({x},{y},{z})");
                }
                crate::feature_ports::place_geode(rng, region, st, x, y, z, cfg);
            }
        }
        "minecraft:vegetation_patch" | "minecraft:waterlogged_vegetation_patch" => {
            place_vegetation_patch(rng, region, state, x, y, z, cfg, gen_step);
        }
        "minecraft:spring_feature" => {
            // SpringFeature.place (spring_water / spring_lava, step 8).
            place_spring(rng, region, x, y, z, cfg);
        }
        "minecraft:block_column" => {
            place_block_column(rng, region, x, y, z, cfg);
        }
        "minecraft:simple_random_selector" => {
            if let Some(features) = cfg["config"]["features"].as_array() {
                if !features.is_empty() {
                    let idx = rng.next_int(features.len() as i32) as usize;
                    place_feature_ref(
                        rng,
                        region,
                        state,
                        x,
                        y,
                        z,
                        &features[idx]["feature"],
                        gen_step,
                    );
                }
            }
        }
        "minecraft:random_boolean_selector" => {
            // RandomBooleanSelectorFeature.place: `random.nextBoolean()`
            // (next(1) != 0), not `nextInt(2)`. Same xoroshiro consume count
            // but a different bit — lush_caves_clay true=dry clay, false=pool.
            let cfg = &cfg["config"];
            let feature = if rng.next_boolean() {
                &cfg["feature_true"]
            } else {
                &cfg["feature_false"]
            };
            place_feature_ref(rng, region, state, x, y, z, feature, gen_step);
        }
        _ => {
            // unknown type — no-op (log in future)
        }
    }
}

pub(crate) fn place_feature_ref(
    rng: &mut FeatureRandom,
    region: &mut RegionBuf,
    state: Option<&WorldgenState>,
    x: i32,
    y: i32,
    z: i32,
    v: &Value,
    gen_step: i32,
) {
    if let Some(id) = v.as_str() {
        // RandomSelector / WeightedPlacedFeature hold a *placed* feature id.
        // Prefer placed over configured so `would_survive` etc. actually run.
        // (`dark_oak_leaf_litter` exists as both.)
        if let Some(pl) = feature_catalog::load_placed_feature(id) {
            place_resolved_placed(rng, region, state, x, y, z, &pl, gen_step);
            return;
        }
        if let Some(cfg) = feature_catalog::load_configured_feature(id) {
            dispatch_configured(rng, region, state, x, y, z, &cfg, gen_step);
        }
        return;
    }
    if let Some(obj) = v.as_object() {
        if obj.get("placement").is_some() && obj.get("feature").is_some() {
            place_resolved_placed(rng, region, state, x, y, z, v, gen_step);
        } else if let Some(fid) = obj.get("feature").and_then(|f| f.as_str()) {
            if let Some(cfg) = feature_catalog::load_configured_feature(fid) {
                dispatch_configured(rng, region, state, x, y, z, &cfg, gen_step);
            }
        } else if obj.get("type").is_some() {
            dispatch_configured(rng, region, state, x, y, z, v, gen_step);
        }
    }
}

/// Apply *filter* modifiers of a placed feature at an already-chosen origin
/// (parent already did count / in_square / heightmap), then dispatch.
fn place_resolved_placed(
    rng: &mut FeatureRandom,
    region: &mut RegionBuf,
    state: Option<&WorldgenState>,
    x: i32,
    y: i32,
    z: i32,
    placed: &Value,
    gen_step: i32,
) {
    if let Some(mods) = placed["placement"].as_array() {
        for m in mods {
            let ty = m["type"].as_str().unwrap_or("");
            let ok = match ty {
                "minecraft:block_predicate_filter" => {
                    eval_block_predicate(region, x, y, z, &m["predicate"])
                }
                "minecraft:biome" => match state {
                    Some(st) => {
                        let bname = biome_name_at(st, x, y, z);
                        let id = placed["feature"].as_str().map(strip).unwrap_or("");
                        let list =
                            feature_catalog::features_at_step(&bname, gen_step);
                        list.iter().any(|f| strip(f) == id)
                            || list.iter().any(|f| {
                                feature_catalog::load_placed_feature(f)
                                    .and_then(|p| p["feature"].as_str().map(|s| strip(s) == id))
                                    .unwrap_or(false)
                            })
                    }
                    None => true,
                },
                "minecraft:rarity_filter" => {
                    let chance = m["chance"].as_i64().unwrap_or(1) as i32;
                    chance > 0 && rng.next_f32() < 1.0 / chance as f32
                }
                "minecraft:surface_water_depth_filter" => {
                    let max = m["max_water_depth"].as_i64().unwrap_or(0) as i32;
                    column_water_depth(region, x, z) <= max
                }
                _ => true, // count / in_square / height* already applied by parent
            };
            if !ok {
                return;
            }
        }
    }
    if let Some(cfg) = placed["feature"]
        .as_str()
        .and_then(feature_catalog::load_configured_feature)
    {
        dispatch_configured(rng, region, state, x, y, z, cfg, gen_step);
    } else if let Some(feat) = placed.get("feature").filter(|v| v.is_object()) {
        dispatch_configured(rng, region, state, x, y, z, feat, gen_step);
    }
}

mod fluids;
mod predicates;
mod sampling;
mod vegetation;

pub(crate) use fluids::*;
pub use predicates::biome_id_to_name;
pub(crate) use predicates::*;
pub(crate) use sampling::*;
pub(crate) use vegetation::*;

use fluids::{place_block_column, place_spring};

#[allow(unused_imports)]
use sampling::{placement_count, resolve_anchor, sample_count_value};
mod tests {
    use super::*;

    #[test]
    fn ocean_floor_includes_leaves_world_surface_includes_plants() {
        assert!(blocks_motion(BlockId::DarkOakLeaves));
        assert!(blocks_motion(BlockId::OakLeaves));
        assert!(blocks_motion(BlockId::DarkOakLog));
        assert!(blocks_motion(BlockId::GrassBlock));
        assert!(!blocks_motion(BlockId::ShortGrass));
        assert!(!blocks_motion(BlockId::LeafLitter));
        assert!(!blocks_motion(BlockId::Air));
        assert!(!blocks_motion(BlockId::Water));
        assert!(heightmap_opaque(
            BlockId::DarkOakLeaves,
            HeightmapKind::OceanFloor
        ));
        assert!(!heightmap_opaque(
            BlockId::DarkOakLeaves,
            HeightmapKind::MotionBlockingNoLeaves
        ));
        assert!(heightmap_opaque(
            BlockId::ShortGrass,
            HeightmapKind::WorldSurface
        ));
        assert!(!heightmap_opaque(
            BlockId::ShortGrass,
            HeightmapKind::OceanFloor
        ));
        assert_eq!(
            parse_heightmap_kind("minecraft:ocean_floor"),
            HeightmapKind::OceanFloor
        );
    }

    #[test]
    fn ocean_floor_top_is_canopy_not_dirt_under_leaves() {
        let mut region = RegionBuf::new(0, 0, 0);
        region.set(4, 63, 4, BlockId::GrassBlock);
        region.set(4, 70, 4, BlockId::DarkOakLeaves);
        assert_eq!(
            heightmap_top(&region, 4, 4, HeightmapKind::OceanFloor),
            Some(70)
        );
        assert_eq!(
            heightmap_top(&region, 4, 4, HeightmapKind::WorldSurface),
            Some(70)
        );
        assert_eq!(
            heightmap_top(&region, 4, 4, HeightmapKind::MotionBlockingNoLeaves),
            Some(63)
        );
        // HeightmapPlacement y = getHeight = firstAvailable = solid + 1.
        assert_eq!(
            heightmap_top(&region, 4, 4, HeightmapKind::OceanFloor).map(|s| s + 1),
            Some(71)
        );
    }
}

