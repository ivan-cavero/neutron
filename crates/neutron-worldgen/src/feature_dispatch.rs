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
/// For each origin chunk, seeds decoration RNG, then for each feature in every
/// biome that appears… simplified: runs features listed for `primary_biome`
/// with index-based seeds (vanilla uses per-chunk biome sampling per column —
/// full parity needs column biome; this uses biome at chunk center surface).
pub fn apply_step_region(
    region: &mut RegionBuf,
    state: &WorldgenState,
    gen_step: i32,
    primary_biome: &str,
) {
    let features = feature_catalog::features_at_step(primary_biome, gen_step);
    if features.is_empty() {
        return;
    }
    let level_seed = state.seed;
    let chunks = region.chunks;
    for czl in 0..chunks {
        for cxl in 0..chunks {
            let ox0 = region.origin_x + cxl * 16;
            let oz0 = region.origin_z + czl * 16;
            let cx = ox0 + 8;
            let cz = oz0 + 8;
            // Collect the biomes present in this origin chunk: the surface biome
            // plus underground cave biomes (lush_caves, dripstone, deep_dark, ...).
            // Vanilla places the *global* FeatureSorter list and lets each placed
            // feature's `biome` filter pick columns; we approximate by unioning
            // the feature lists of the biomes actually present.
            let sy = surface_y(region, cx, cz).unwrap_or(64);
            let mut biomes: Vec<String> = Vec::new();
            let mut push_if_new = |b: &str| {
                if !biomes.iter().any(|x| x == b) {
                    biomes.push(b.to_string());
                }
            };
            push_if_new(&biome_name_at(state, cx, sy, cz));
            let cols = [(0i32, 0i32), (7, 7), (-7, 7), (7, -7), (-7, -7)];
            for &y in &[0i32, 16, 32, 48, 64, 80, 96, 112, 128] {
                for &(dxc, dzc) in &cols {
                    push_if_new(&biome_name_at(state, cx + dxc, y, cz + dzc));
                }
            }
            // Union of feature lists, in global FeatureSorter index order.
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
            if list.is_empty() {
                // fall back to primary list if no biome matched
                place_feature_list(region, state, level_seed, ox0, oz0, gen_step, &features);
            } else {
                place_feature_list(region, state, level_seed, ox0, oz0, gen_step, &list);
            }
        }
    }
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
        place_placed_feature(&mut rng, region, state, ox0, oz0, placed_id);
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
    let Some(placed) = feature_catalog::load_placed_feature(placed_id) else {
        return;
    };
    let feature_ref = placed["feature"]
        .as_str()
        .map(|s| s.to_string())
        .or_else(|| {
            // inline feature
            None
        });

    // PlacedFeature.placeWithContext is a lazy stream: Count → InSquare →
    // filters → Feature.place. Each surviving position is placed *before*
    // the next InSquare nextInt (TreeFeature consumes a lot of RNG).
    // Collecting all xz first then placing desyncs every attempt after the first.
    let configured = if let Some(ref id) = feature_ref {
        feature_catalog::load_configured_feature(id)
    } else if placed["feature"].is_object() {
        Some(placed["feature"].clone())
    } else {
        None
    };
    let base_count = placement_count(rng, &placed);
    for _ in 0..base_count {
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
                        let ox = sample_int_provider(rng, &m["xz_spread"]);
                        let oz = sample_int_provider(rng, &m["xz_spread"]);
                        let oy = sample_int_provider(rng, &m["y_spread"]);
                        x += ox;
                        z += oz;
                        y += oy;
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
                            if py < WORLD_BOTTOM || py > WORLD_TOP {
                                break;
                            }
                            if !eval_block_predicate(region, x, py, z, allowed) {
                                break;
                            }
                        }
                        if found.is_none() && eval_block_predicate(region, x, py, z, target) {
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
                        let step_list =
                            feature_catalog::features_at_step(&bname, step_for_id(placed_id));
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
            continue;
        }
        if let Some(ref cfg) = configured {
            dispatch_configured(rng, region, state, x, y, z, cfg);
        } else if let Some(ref id) = feature_ref {
            // nested placed
            if let Some(inner) = feature_catalog::load_placed_feature(id) {
                if let Some(cid) = inner["feature"].as_str() {
                    if let Some(cfg) = feature_catalog::load_configured_feature(cid) {
                        dispatch_configured(rng, region, state, x, y, z, &cfg);
                    }
                }
            }
        }
    }
}

fn step_for_id(_placed_id: &str) -> i32 {
    step::VEGETAL_DECORATION
}

fn placement_count(rng: &mut FeatureRandom, placed: &Value) -> i32 {
    let Some(mods) = placed["placement"].as_array() else {
        return 1;
    };
    // Sequential CountPlacement / NoiseThresholdCount compose as a product.
    // Sample each provider **once** (the previous double loop consumed RNG twice).
    let mut product = 1i32;
    let mut saw = false;
    for m in mods {
        let ty = m["type"].as_str().unwrap_or("");
        if ty == "minecraft:count" {
            product *= sample_count_value(rng, &m["count"]).max(1);
            saw = true;
        } else if ty == "minecraft:noise_threshold_count" {
            let below = m["below_noise"].as_i64().unwrap_or(5) as i32;
            let above = m["above_noise"].as_i64().unwrap_or(10) as i32;
            let n = rng.next_f64() * 2.0 - 1.0;
            let level = m["noise_level"].as_f64().unwrap_or(-0.8);
            product *= if n < level { below } else { above };
            saw = true;
        }
    }
    if saw {
        product.min(512)
    } else {
        1
    }
}

fn sample_count_value(rng: &mut FeatureRandom, v: &Value) -> i32 {
    if let Some(n) = v.as_i64() {
        return n as i32;
    }
    if let Some(obj) = v.as_object() {
        match obj.get("type").and_then(|t| t.as_str()) {
            Some("minecraft:uniform") => {
                let min = obj["min_inclusive"].as_i64().unwrap_or(0) as i32;
                let max = obj["max_inclusive"].as_i64().unwrap_or(min as i64) as i32;
                min + rng.next_int((max - min + 1).max(1))
            }
            Some("minecraft:weighted_list") => {
                let dist = obj.get("distribution").and_then(|d| d.as_array());
                let Some(dist) = dist else { return 1 };
                let total: i32 = dist
                    .iter()
                    .map(|e| e["weight"].as_i64().unwrap_or(1) as i32)
                    .sum();
                if total <= 0 {
                    return 0;
                }
                let mut r = rng.next_int(total);
                for e in dist {
                    let w = e["weight"].as_i64().unwrap_or(1) as i32;
                    if r < w {
                        return e["data"].as_i64().unwrap_or(0) as i32;
                    }
                    r -= w;
                }
                0
            }
            _ => sample_int_provider(rng, v),
        }
    } else {
        1
    }
}

fn sample_int_provider(rng: &mut FeatureRandom, v: &Value) -> i32 {
    if let Some(n) = v.as_i64() {
        return n as i32;
    }
    let Some(obj) = v.as_object() else {
        return 0;
    };
    match obj.get("type").and_then(|t| t.as_str()) {
        Some("minecraft:uniform") => {
            let min = obj
                .get("min_inclusive")
                .or_else(|| obj.get("value").and_then(|v| v.get("min_inclusive")))
                .and_then(|x| x.as_i64())
                .unwrap_or(0) as i32;
            let max = obj
                .get("max_inclusive")
                .or_else(|| obj.get("value").and_then(|v| v.get("max_inclusive")))
                .and_then(|x| x.as_i64())
                .unwrap_or(min as i64) as i32;
            min + rng.next_int((max - min + 1).max(1))
        }
        Some("minecraft:trapezoid") => {
            let min = obj["min"].as_i64().unwrap_or(0) as i32;
            let max = obj["max"].as_i64().unwrap_or(0) as i32;
            // average of two uniforms
            let a = min + rng.next_int((max - min + 1).max(1));
            let b = min + rng.next_int((max - min + 1).max(1));
            (a + b) / 2
        }
        Some("minecraft:constant") => obj["value"].as_i64().unwrap_or(0) as i32,
        Some("minecraft:weighted_list") => {
            let dist = obj.get("distribution").and_then(|d| d.as_array());
            let Some(dist) = dist else { return 0 };
            let total: i32 = dist
                .iter()
                .map(|e| e["weight"].as_i64().unwrap_or(1) as i32)
                .sum();
            if total <= 0 {
                return 0;
            }
            let mut r = rng.next_int(total);
            for e in dist {
                let w = e["weight"].as_i64().unwrap_or(1) as i32;
                if r < w {
                    return sample_int_provider(rng, &e["data"]);
                }
                r -= w;
            }
            0
        }
        _ => 0,
    }
}

fn sample_height(rng: &mut FeatureRandom, height: &Value) -> i32 {
    let ty = height["type"].as_str().unwrap_or("minecraft:uniform");
    if ty.contains("uniform") {
        let min = resolve_anchor(&height["min_inclusive"]);
        let max = resolve_anchor(&height["max_inclusive"]);
        min + rng.next_int((max - min + 1).max(1))
    } else if ty.contains("trapezoid") {
        let min = resolve_anchor(&height["min_inclusive"]);
        let max = resolve_anchor(&height["max_inclusive"]);
        let a = min + rng.next_int((max - min + 1).max(1));
        let b = min + rng.next_int((max - min + 1).max(1));
        (a + b) / 2
    } else {
        64
    }
}

fn resolve_anchor(v: &Value) -> i32 {
    if let Some(n) = v.get("absolute").and_then(|a| a.as_i64()) {
        return n as i32;
    }
    if let Some(n) = v.get("above_bottom").and_then(|a| a.as_i64()) {
        return WORLD_BOTTOM + n as i32;
    }
    if let Some(n) = v.get("below_top").and_then(|a| a.as_i64()) {
        return (WORLD_TOP - 1) - n as i32;
    }
    0
}

fn eval_block_predicate(region: &RegionBuf, x: i32, y: i32, z: i32, pred: &Value) -> bool {
    let ty = pred["type"].as_str().unwrap_or("");
    match ty {
        "minecraft:matching_block_tag" => {
            let tag = pred["tag"].as_str().unwrap_or("");
            let b = region.get(x, y, z);
            if tag.ends_with("air") {
                return b == BlockId::Air;
            }
            is_in_tag(b, tag)
        }
        "minecraft:solid" => {
            is_solid_block(region.get(x, y, z))
        }
        "minecraft:has_sturdy_face" => {
            // HasSturdyFacePredicate: the block at (origin + predicate offset)
            // must have a sturdy face in `direction`. We approximate "sturdy"
            // with is_solid_block (full solid blocks are sturdy on all faces).
            let off = pred["offset"].as_array();
            let ox = off.and_then(|a| a.first()).and_then(|v| v.as_i64()).unwrap_or(0) as i32;
            let oy = off.and_then(|a| a.get(1)).and_then(|v| v.as_i64()).unwrap_or(0) as i32;
            let oz = off.and_then(|a| a.get(2)).and_then(|v| v.as_i64()).unwrap_or(0) as i32;
            is_solid_block(region.get(x + ox, y + oy, z + oz))
        }
        "minecraft:not" => {
            !pred
                .get("predicate")
                .map(|p| eval_block_predicate(region, x, y, z, p))
                .unwrap_or(false)
        }
        "minecraft:matching_blocks" => {
            let ox = pred["offset"]
                .as_array()
                .and_then(|a| a.first())
                .and_then(|v| v.as_i64())
                .unwrap_or(0) as i32;
            let oy = pred["offset"]
                .as_array()
                .and_then(|a| a.get(1))
                .and_then(|v| v.as_i64())
                .unwrap_or(0) as i32;
            let oz = pred["offset"]
                .as_array()
                .and_then(|a| a.get(2))
                .and_then(|v| v.as_i64())
                .unwrap_or(0) as i32;
            let b = region.get(x + ox, y + oy, z + oz);
            if let Some(name) = pred["blocks"].as_str() {
                return BlockId::from_name(name) == Some(b);
            }
            if let Some(arr) = pred["blocks"].as_array() {
                return arr.iter().any(|n| {
                    n.as_str()
                        .and_then(BlockId::from_name)
                        .map(|id| id == b)
                        .unwrap_or(false)
                });
            }
            true
        }
        "minecraft:all_of" => {
            let Some(arr) = pred["predicates"].as_array() else {
                return true;
            };
            arr.iter().all(|p| eval_block_predicate(region, x, y, z, p))
        }
        "minecraft:any_of" => {
            let Some(arr) = pred["predicates"].as_array() else {
                return true;
            };
            arr.iter().any(|p| eval_block_predicate(region, x, y, z, p))
        }
        "minecraft:would_survive" => {
            // SaplingBlock.mayPlaceOn = BlockTags.DIRT (+ farmland).
            let below = region.get(x, y - 1, z);
            is_dirt_tag(below)
                && matches!(
                    region.get(x, y, z),
                    BlockId::Air | BlockId::ShortGrass | BlockId::LeafLitter
                )
        }
        "minecraft:true" => true,
        _ => true,
    }
}

fn is_dirt_tag(b: BlockId) -> bool {
    matches!(
        b,
        BlockId::Dirt
            | BlockId::GrassBlock
            | BlockId::Podzol
            | BlockId::CoarseDirt
            | BlockId::Mycelium
            | BlockId::Mud
            | BlockId::MossBlock
    )
}

/// `BlockState.isSolid()` approximation (mirrors `blocks_motion`).
fn is_solid_block(b: BlockId) -> bool {
    blocks_motion(b)
}

/// Membership in a block tag (subset used by lush/pale placement predicates).
fn is_in_tag(b: BlockId, tag: &str) -> bool {
    let t = tag.strip_prefix("#minecraft:").unwrap_or(tag);
    match t {
        "air" => b == BlockId::Air,
        "cave_vines" => matches!(b, BlockId::CaveVines | BlockId::CaveVinesPlant),
        "dirt" => matches!(
            b,
            BlockId::Dirt
                | BlockId::GrassBlock
                | BlockId::Podzol
                | BlockId::CoarseDirt
                | BlockId::Mycelium
                | BlockId::RootedDirt
        ),
        "moss_blocks" => matches!(b, BlockId::MossBlock | BlockId::PaleMossBlock),
        "grass_blocks" => b == BlockId::GrassBlock,
        "mud" => b == BlockId::Mud,
        "base_stone_overworld" => matches!(
            b,
            BlockId::Stone
                | BlockId::Granite
                | BlockId::Diorite
                | BlockId::Andesite
                | BlockId::Tuff
                | BlockId::Deepslate
        ),
        "moss_replaceable" => {
            is_in_tag(b, "#minecraft:base_stone_overworld")
                || is_in_tag(b, "#minecraft:cave_vines")
                || is_in_tag(b, "#minecraft:dirt")
                || is_in_tag(b, "#minecraft:mud")
                || is_in_tag(b, "#minecraft:moss_blocks")
                || is_in_tag(b, "#minecraft:grass_blocks")
        }
        "lush_ground_replaceable" => {
            is_in_tag(b, "#minecraft:moss_replaceable")
                || b == BlockId::Clay
                || b == BlockId::Gravel
                || b == BlockId::Sand
        }
        _ => false,
    }
}

/// Dispatch by configured_feature.type
fn dispatch_configured(
    rng: &mut FeatureRandom,
    region: &mut RegionBuf,
    state: &WorldgenState,
    x: i32,
    y: i32,
    z: i32,
    cfg: &Value,
) {
    let ty = cfg["type"].as_str().unwrap_or("");
    match ty {
        "minecraft:simple_block" => {
            if let Some(block) = block_from_to_place(rng, &cfg["config"]["to_place"]) {
                if region.get(x, y, z) == BlockId::Air {
                    region.set(x, y, z, block);
                }
            }
        }
        "minecraft:tree" => {
            tree::place_tree_from_config(rng, region, x, y, z, cfg);
        }
        "minecraft:random_selector" => {
            // weighted chance features then default
            if let Some(features) = cfg["config"]["features"].as_array() {
                for f in features {
                    let chance = f["chance"].as_f64().unwrap_or(0.0) as f32;
                    if rng.next_f32() < chance {
                        place_feature_ref(rng, region, state, x, y, z, &f["feature"]);
                        return;
                    }
                }
            }
            if let Some(def) = cfg["config"].get("default") {
                place_feature_ref(rng, region, state, x, y, z, def);
            }
        }
        "minecraft:sculk_patch" => {
            // handled by sculk module with proper seeds — skip here
        }
        "minecraft:multiface_growth" => {
            // sculk_vein handled by sculk module
        }
        "minecraft:ore" | "minecraft:scattered_ore" => {
            // step 6 ores still use features.rs batch
        }
        "minecraft:vegetation_patch" | "minecraft:waterlogged_vegetation_patch" => {
            place_vegetation_patch(rng, region, state, x, y, z, cfg);
        }
        "minecraft:block_column" => {
            place_block_column(rng, region, x, y, z, cfg);
        }
        "minecraft:simple_random_selector" => {
            if let Some(features) = cfg["config"]["features"].as_array() {
                if !features.is_empty() {
                    let idx = rng.next_int(features.len() as i32) as usize;
                    place_feature_ref(rng, region, state, x, y, z, &features[idx]["feature"]);
                }
            }
        }
        "minecraft:random_boolean_selector" => {
            let cfg = &cfg["config"];
            let feature = if rng.next_int(2) == 0 {
                &cfg["feature_true"]
            } else {
                &cfg["feature_false"]
            };
            place_feature_ref(rng, region, state, x, y, z, feature);
        }
        _ => {
            // unknown type — no-op (log in future)
        }
    }
}

fn place_feature_ref(
    rng: &mut FeatureRandom,
    region: &mut RegionBuf,
    state: &WorldgenState,
    x: i32,
    y: i32,
    z: i32,
    v: &Value,
) {
    if let Some(id) = v.as_str() {
        // RandomSelector / WeightedPlacedFeature hold a *placed* feature id.
        // Prefer placed over configured so `would_survive` etc. actually run.
        // (`dark_oak_leaf_litter` exists as both.)
        if let Some(pl) = feature_catalog::load_placed_feature(id) {
            place_resolved_placed(rng, region, state, x, y, z, &pl);
            return;
        }
        if let Some(cfg) = feature_catalog::load_configured_feature(id) {
            dispatch_configured(rng, region, state, x, y, z, &cfg);
        }
        return;
    }
    if let Some(obj) = v.as_object() {
        if obj.get("placement").is_some() && obj.get("feature").is_some() {
            place_resolved_placed(rng, region, state, x, y, z, v);
        } else if let Some(fid) = obj.get("feature").and_then(|f| f.as_str()) {
            if let Some(cfg) = feature_catalog::load_configured_feature(fid) {
                dispatch_configured(rng, region, state, x, y, z, &cfg);
            }
        } else if obj.get("type").is_some() {
            dispatch_configured(rng, region, state, x, y, z, v);
        }
    }
}

/// Apply *filter* modifiers of a placed feature at an already-chosen origin
/// (parent already did count / in_square / heightmap), then dispatch.
fn place_resolved_placed(
    rng: &mut FeatureRandom,
    region: &mut RegionBuf,
    state: &WorldgenState,
    x: i32,
    y: i32,
    z: i32,
    placed: &Value,
) {
    if let Some(mods) = placed["placement"].as_array() {
        for m in mods {
            let ty = m["type"].as_str().unwrap_or("");
            let ok = match ty {
                "minecraft:block_predicate_filter" => {
                    eval_block_predicate(region, x, y, z, &m["predicate"])
                }
                "minecraft:biome" => {
                    let bname = biome_name_at(state, x, y, z);
                    let id = placed["feature"]
                        .as_str()
                        .map(strip)
                        .unwrap_or("");
                    let list = feature_catalog::features_at_step(&bname, step::VEGETAL_DECORATION);
                    list.iter().any(|f| strip(f) == id)
                        || list.iter().any(|f| {
                            feature_catalog::load_placed_feature(f)
                                .and_then(|p| {
                                    p["feature"].as_str().map(|s| strip(s) == id)
                                })
                                .unwrap_or(false)
                        })
                }
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
    let configured = placed["feature"]
        .as_str()
        .and_then(feature_catalog::load_configured_feature)
        .or_else(|| {
            if placed["feature"].is_object() {
                Some(placed["feature"].clone())
            } else {
                None
            }
        });
    if let Some(cfg) = configured {
        dispatch_configured(rng, region, state, x, y, z, &cfg);
    }
}

fn block_from_to_place(rng: &mut FeatureRandom, v: &Value) -> Option<BlockId> {
    let ty = v["type"].as_str().unwrap_or("");
    match ty {
        "minecraft:simple_state_provider" => {
            BlockId::from_name(v["state"]["Name"].as_str().unwrap_or(""))
        }
        "minecraft:weighted_state_provider" => {
            let entries = v["entries"].as_array()?;
            let total: i32 = entries
                .iter()
                .map(|e| e["weight"].as_i64().unwrap_or(1) as i32)
                .sum();
            if total <= 0 {
                return None;
            }
            let mut r = rng.next_int(total);
            for e in entries {
                let w = e["weight"].as_i64().unwrap_or(1) as i32;
                if r < w {
                    return BlockId::from_name(e["data"]["Name"].as_str().unwrap_or(""));
                }
                r -= w;
            }
            None
        }
        "minecraft:randomized_int_state_provider" => {
            block_from_to_place(rng, &v["source"])
        }
        _ => BlockId::from_name(v.pointer("/state/Name")?.as_str()?),
    }
}

/// Port of `VegetationPatchFeature.place` (moss patches, pale moss patches).
fn place_vegetation_patch(
    rng: &mut FeatureRandom,
    region: &mut RegionBuf,
    state: &WorldgenState,
    x: i32,
    y: i32,
    z: i32,
    cfg: &Value,
) {
    let c = &cfg["config"];
    let surface = c["surface"].as_str().unwrap_or("floor");
    // inwards = surface direction (floor -> down, ceiling -> up).
    let (in_dx, in_dy, in_dz) = if surface == "ceiling" {
        (0, 1, 0)
    } else {
        (0, -1, 0)
    };
    let (out_dx, out_dy, out_dz) = (-in_dx, -in_dy, -in_dz);
    let vertical_range = c["vertical_range"].as_i64().unwrap_or(5) as i32;
    let extra_edge = c["extra_edge_column_chance"].as_f64().unwrap_or(0.0) as f32;
    let extra_bottom = c["extra_bottom_block_chance"].as_f64().unwrap_or(0.0) as f32;
    let depth_prov = &c["depth"];
    let ground_state = block_from_to_place(rng, &c["ground_state"]);
    let replaceable = c["replaceable"].as_str().unwrap_or("");
    let veg_chance = c["vegetation_chance"].as_f64().unwrap_or(0.0) as f32;
    let veg_feature = c["vegetation_feature"].clone();

    let xr = sample_int_provider(rng, &c["xz_radius"]).max(0) + 1;
    let zr = sample_int_provider(rng, &c["xz_radius"]).max(0) + 1;

    let mut surface_pts: Vec<(i32, i32, i32)> = Vec::new();

    for dx in -xr..=xr {
        let is_x_edge = dx == -xr || dx == xr;
        for dz in -zr..=zr {
            let is_z_edge = dz == -zr || dz == zr;
            let is_edge = is_x_edge || is_z_edge;
            let is_corner = is_x_edge && is_z_edge;
            let is_edge_not_corner = is_edge && !is_corner;
            if is_corner
                || (is_edge_not_corner && (extra_edge == 0.0 || rng.next_f32() > extra_edge))
            {
                continue;
            }
            let (mut px, mut py, mut pz) = (x + dx, y, z + dz);
            // Scan through air inwards.
            let mut off = 0;
            while region.get(px, py, pz) == BlockId::Air && off < vertical_range {
                px += in_dx;
                py += in_dy;
                pz += in_dz;
                off += 1;
            }
            // Scan back out through solid.
            off = 0;
            while region.get(px, py, pz) != BlockId::Air && off < vertical_range {
                px += out_dx;
                py += out_dy;
                pz += out_dz;
                off += 1;
            }
            let (bx, by, bz) = (px + in_dx, py + in_dy, pz + in_dz);
            if region.get(px, py, pz) != BlockId::Air {
                continue;
            }
            if !is_solid_block(region.get(bx, by, bz)) {
                continue;
            }
            let mut depth = sample_int_provider(rng, depth_prov).max(0);
            if extra_bottom > 0.0 && rng.next_f32() < extra_bottom {
                depth += 1;
            }
            // placeGround: replace replaceable blocks with ground_state.
            let (gx0, gy0, gz0) = (bx, by, bz);
            let (mut gx, mut gy, mut gz) = (bx, by, bz);
            let mut placed_any = false;
            let mut i = 0;
            while i < depth {
                let cur = region.get(gx, gy, gz);
                if let Some(st) = ground_state {
                    if st == cur {
                        i += 1;
                        continue;
                    }
                    if !is_in_tag(cur, replaceable) {
                        placed_any = i != 0;
                        break;
                    }
                    region.set(gx, gy, gz, st);
                    placed_any = true;
                }
                gx += in_dx;
                gy += in_dy;
                gz += in_dz;
                i += 1;
            }
            if placed_any {
                surface_pts.push((gx0, gy0, gz0));
            }
        }
    }

    // distributeVegetation
    for (sx, sy, sz) in surface_pts {
        if veg_chance > 0.0 && rng.next_f32() < veg_chance {
            let vx = sx + out_dx;
            let vy = sy + out_dy;
            let vz = sz + out_dz;
            place_feature_ref(rng, region, state, vx, vy, vz, &veg_feature);
        }
    }
}

/// Port of `BlockColumnFeature.place` (cave vines).
fn place_block_column(
    rng: &mut FeatureRandom,
    region: &mut RegionBuf,
    x: i32,
    y: i32,
    z: i32,
    cfg: &Value,
) {
    let c = &cfg["config"];
    let dir = c["direction"].as_str().unwrap_or("down");
    let (ddx, ddy, ddz) = match dir {
        "down" => (0, -1, 0),
        "up" => (0, 1, 0),
        "north" => (0, 0, -1),
        "south" => (0, 0, 1),
        "west" => (-1, 0, 0),
        "east" => (1, 0, 0),
        _ => (0, -1, 0),
    };
    let Some(layers) = c["layers"].as_array() else {
        return;
    };
    let mut heights: Vec<i32> = Vec::new();
    let mut total = 0;
    for layer in layers {
        let h = sample_int_provider(rng, &layer["height"]).max(0);
        heights.push(h);
        total += h;
    }
    if total == 0 {
        return;
    }
    // Find how far the column can extend before hitting a non-allowed block.
    let (mut nx, mut ny, mut nz) = (x + ddx, y + ddy, z + ddz);
    let mut truncate_at = total;
    for y in 0..total {
        if !eval_block_predicate(region, nx, ny, nz, &c["allowed_placement"]) {
            truncate_at = y;
            break;
        }
        nx += ddx;
        ny += ddy;
        nz += ddz;
    }
    // Truncate layer heights.
    let mut to_remove = total - truncate_at;
    let prioritize = c["prioritize_tip"].as_bool().unwrap_or(false);
    if prioritize {
        let mut i = 0;
        while i < heights.len() && to_remove > 0 {
            let r = heights[i].min(to_remove);
            heights[i] -= r;
            to_remove -= r;
            i += 1;
        }
    } else {
        let mut i = heights.len();
        while i > 0 && to_remove > 0 {
            i -= 1;
            let r = heights[i].min(to_remove);
            heights[i] -= r;
            to_remove -= r;
        }
    }
    // Place layers.
    let (mut pp_x, mut pp_y, mut pp_z) = (x, y, z);
    for (i, layer) in layers.iter().enumerate() {
        let count = heights[i];
        if count == 0 {
            continue;
        }
        for _ in 0..count {
            if let Some(st) = block_from_to_place(rng, &layer["provider"]) {
                region.set(pp_x, pp_y, pp_z, st);
            }
            pp_x += ddx;
            pp_y += ddy;
            pp_z += ddz;
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum HeightmapKind {
    /// `WORLD_SURFACE` / `WORLD_SURFACE_WG`: Heightmap.NOT_AIR.
    WorldSurface,
    /// `OCEAN_FLOOR` / `OCEAN_FLOOR_WG`: `BlockState.blocksMotion()`.
    OceanFloor,
    /// `MOTION_BLOCKING`: blocksMotion || !fluid.isEmpty.
    MotionBlocking,
    /// `MOTION_BLOCKING_NO_LEAVES`: (blocksMotion || fluid) && !LeavesBlock.
    MotionBlockingNoLeaves,
}

fn parse_heightmap_kind(name: &str) -> HeightmapKind {
    match name.strip_prefix("minecraft:").unwrap_or(name) {
        "world_surface" | "world_surface_wg" => HeightmapKind::WorldSurface,
        "ocean_floor" | "ocean_floor_wg" => HeightmapKind::OceanFloor,
        "motion_blocking" => HeightmapKind::MotionBlocking,
        "motion_blocking_no_leaves" => HeightmapKind::MotionBlockingNoLeaves,
        _ => HeightmapKind::OceanFloor,
    }
}

/// `BlockState.blocksMotion`: isSolid except cobweb / bamboo_sapling (not in palette).
fn blocks_motion(b: BlockId) -> bool {
    !matches!(
        b,
        BlockId::Air
            | BlockId::Water
            | BlockId::Lava
            | BlockId::ShortGrass
            | BlockId::LeafLitter
            | BlockId::SculkVein
            | BlockId::Snow
            | BlockId::PowderSnow
    )
}

fn is_leaves(b: BlockId) -> bool {
    matches!(
        b,
        BlockId::OakLeaves | BlockId::DarkOakLeaves | BlockId::PaleOakLeaves
    )
}

fn heightmap_opaque(b: BlockId, kind: HeightmapKind) -> bool {
    match kind {
        HeightmapKind::WorldSurface => !b.is_air(),
        HeightmapKind::OceanFloor => blocks_motion(b),
        HeightmapKind::MotionBlocking => blocks_motion(b) || b.is_fluid(),
        HeightmapKind::MotionBlockingNoLeaves => {
            (blocks_motion(b) || b.is_fluid()) && !is_leaves(b)
        }
    }
}

/// Highest Y whose block is opaque for `kind`, or None if the column is empty.
fn heightmap_top(region: &RegionBuf, x: i32, z: i32, kind: HeightmapKind) -> Option<i32> {
    for y in (WORLD_BOTTOM..WORLD_TOP).rev() {
        if heightmap_opaque(region.get(x, y, z), kind) {
            return Some(y);
        }
    }
    None
}

fn surface_y(region: &RegionBuf, x: i32, z: i32) -> Option<i32> {
    heightmap_top(region, x, z, HeightmapKind::OceanFloor)
}

/// `WORLD_SURFACE` first-available minus `OCEAN_FLOOR` first-available.
fn column_water_depth(region: &RegionBuf, x: i32, z: i32) -> i32 {
    let mut surface = None;
    let mut floor = None;
    for y in (WORLD_BOTTOM..WORLD_TOP).rev() {
        let b = region.get(x, y, z);
        if !b.is_air() && surface.is_none() {
            surface = Some(y + 1);
        }
        if !b.is_air() && !b.is_fluid() && floor.is_none() {
            floor = Some(y + 1);
            break;
        }
    }
    match (surface, floor) {
        (Some(s), Some(f)) => (s - f).max(0),
        _ => 0,
    }
}

fn biome_name_at(state: &WorldgenState, x: i32, y: i32, z: i32) -> String {
    biome_id_to_name(biome_id_at_block(state, x, y, z)).to_string()
}

pub(crate) fn biome_id_to_name(id: u8) -> &'static str {
    // Subset — extend as biome_source ids expand
    match id {
        x if x == biome_id::DEEP_DARK => "deep_dark",
        x if x == biome_id::DARK_FOREST => "dark_forest",
        x if x == biome_id::PALE_GARDEN => "pale_garden",
        x if x == biome_id::PLAINS => "plains",
        x if x == biome_id::FOREST => "forest",
        x if x == biome_id::BIRCH_FOREST => "birch_forest",
        x if x == biome_id::TAIGA => "taiga",
        x if x == biome_id::SWAMP => "swamp",
        x if x == biome_id::DESERT => "desert",
        x if x == biome_id::JUNGLE => "jungle",
        x if x == biome_id::SAVANNA => "savanna",
        x if x == biome_id::MEADOW => "meadow",
        x if x == biome_id::OLD_GROWTH_PINE_FOREST => "old_growth_pine_taiga",
        x if x == biome_id::OLD_GROWTH_BIRCH_FOREST => "old_growth_birch_forest",
        x if x == biome_id::DRIPSTONE_CAVES => "dripstone_caves",
        x if x == biome_id::LUSH_CAVES => "lush_caves",
        x if x == biome_id::SULFUR_CAVES => "sulfur_caves",
        x if x == biome_id::MANGROVE_SWAMP => "mangrove_swamp",
        x if x == biome_id::CHERRY_GROVE => "cherry_grove",
        x if x == biome_id::BADLANDS => "badlands",
        x if x == biome_id::ERODED_BADLANDS => "eroded_badlands",
        x if x == biome_id::WOODED_BADLANDS => "wooded_badlands",
        x if x == biome_id::GROVE => "grove",
        x if x == biome_id::JAGGED_PEAKS => "jagged_peaks",
        x if x == biome_id::STONY_PEAKS => "stony_peaks",
        x if x == biome_id::FROZEN_PEAKS => "frozen_peaks",
        x if x == biome_id::SNOWY_SLOPES => "snowy_slopes",
        x if x == biome_id::WINDSWEPT_HILLS => "windswept_hills",
        _ => "plains",
    }
}

fn strip(s: &str) -> &str {
    s.strip_prefix("minecraft:").unwrap_or(s)
}

#[cfg(test)]
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
