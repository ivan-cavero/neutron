//! SequenceFeature + the inline placed-feature placement pipeline.
//!
//! NOTE: this pipeline mirrors feature_dispatch::place_placed_feature_step;
//! consolidating both into one implementation is pending (Paso 5 notes).
use super::*;
use crate::feature_catalog;
use crate::feature_dispatch;
use crate::feature_dispatch::*;
use crate::feature_rng::FeatureRandom;
use crate::generator::{WORLD_BOTTOM, WORLD_TOP};
use crate::legacy_rng::LegacyRandom;
use crate::region_buf::RegionBuf;
use crate::surface::BlockId;
use crate::worldgen::WorldgenState;
use serde_json::Value;
use crate::feature_dispatch::*;


// ---------------------------------------------------------------------------
// sequence
// ---------------------------------------------------------------------------

/// `SequenceFeature.place` (26.2): run each sub placed-feature (placement
/// modifiers included) at the origin, stop on first failure.
pub(crate) fn place_sequence(
    rng: &mut FeatureRandom,
    region: &mut RegionBuf,
    state: Option<&WorldgenState>,
    x: i32,
    y: i32,
    z: i32,
    cfg: &Value,
    gen_step: i32,
) {
    let Some(features) = cfg["config"]["features"].as_array() else {
        return;
    };
    for f in features {
        if !place_inline_placed(rng, region, state, x, y, z, f, gen_step) {
            return;
        }
    }
}

/// Run one inline placed-feature object (full placement pipeline) at `(x,y,z)`.
/// Returns whether the feature placed (SequenceFeature short-circuits).
fn place_inline_placed(
    rng: &mut FeatureRandom,
    region: &mut RegionBuf,
    state: Option<&WorldgenState>,
    x: i32,
    y: i32,
    z: i32,
    placed: &Value,
    gen_step: i32,
) -> bool {
    let base_count = placed["placement"]
        .as_array()
        .map(|mods| {
            let mut product = 1i32;
            for m in mods {
                if m["type"].as_str() == Some("minecraft:count") {
                    product *= sample_int_provider(rng, &m["count"]).max(1);
                }
            }
            product.min(512)
        })
        .unwrap_or(1);
    let mut placed_any = false;
    for _ in 0..base_count {
        let mut px = x;
        let mut py = y;
        let mut pz = z;
        let mut ok = true;
        if let Some(mods) = placed["placement"].as_array() {
            for m in mods {
                match m["type"].as_str().unwrap_or("") {
                    "minecraft:count" => {}
                    "minecraft:in_square" => {
                        px = x + rng.next_int(16);
                        pz = z + rng.next_int(16);
                    }
                    "minecraft:height_range" => {
                        py = sample_height(rng, &m["height"]);
                    }
                    "minecraft:heightmap" => {
                        let kind = feature_dispatch::parse_heightmap_kind(
                            m["heightmap"].as_str().unwrap_or(""),
                        );
                        if let Some(sy) = heightmap_top(region, px, pz, kind) {
                            py = sy + 1;
                        } else {
                            ok = false;
                        }
                    }
                    "minecraft:random_offset" => {
                        px += sample_int_provider(rng, &m["xz_spread"]);
                        py += sample_int_provider(rng, &m["y_spread"]);
                        pz += sample_int_provider(rng, &m["xz_spread"]);
                    }
                    "minecraft:environment_scan" => {
                        let dir = m["direction_of_search"].as_str().unwrap_or("down");
                        let max_steps = m["max_steps"].as_i64().unwrap_or(12) as i32;
                        let target = &m["target_condition"];
                        let true_pred = serde_json::json!({"type":"minecraft:true"});
                        let allowed = m.get("allowed_search_condition").unwrap_or(&true_pred);
                        if !eval_block_predicate(region, px, py, pz, allowed) {
                            ok = false;
                            break;
                        }
                        let mut found = None;
                        let mut spy = py;
                        for _ in 0..max_steps {
                            if eval_block_predicate(region, px, spy, pz, target) {
                                found = Some(spy);
                                break;
                            }
                            spy += if dir == "down" { -1 } else { 1 };
                            if spy < WORLD_BOTTOM || spy > WORLD_TOP {
                                break;
                            }
                            if !eval_block_predicate(region, px, spy, pz, allowed) {
                                break;
                            }
                        }
                        if found.is_none() && eval_block_predicate(region, px, spy, pz, target) {
                            found = Some(spy);
                        }
                        match found {
                            Some(fy) => py = fy,
                            None => ok = false,
                        }
                    }
                    "minecraft:block_predicate_filter" => {
                        if !eval_block_predicate(region, px, py, pz, &m["predicate"]) {
                            ok = false;
                        }
                    }
                    "minecraft:rarity_filter" => {
                        let chance = m["chance"].as_i64().unwrap_or(1) as i32;
                        if chance <= 0 || rng.next_f32() >= 1.0 / chance as f32 {
                            ok = false;
                        }
                    }
                    "minecraft:surface_water_depth_filter" => {
                        let max = m["max_water_depth"].as_i64().unwrap_or(0) as i32;
                        if feature_dispatch::column_water_depth(region, px, pz) > max {
                            ok = false;
                        }
                    }
                    _ => {}
                }
            }
        }
        if !ok {
            continue;
        }
        let mut placed_one = false;
        if let Some(fid) = placed["feature"].as_str() {
            if let Some(cfg) = feature_catalog::load_configured_feature(fid) {
                dispatch_configured(rng, region, state, px, py, pz, &cfg, gen_step);
                placed_one = true;
            }
        } else if placed["feature"].is_object() {
            dispatch_configured(rng, region, state, px, py, pz, &placed["feature"], gen_step);
            placed_one = true;
        }
        placed_any |= placed_one;
    }
    placed_any
}
