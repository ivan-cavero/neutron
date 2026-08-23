//! Placement-modifier value sampling: count / int providers / heights.
use super::*;
use crate::feature_catalog;
use crate::feature_rng::FeatureRandom;
use crate::generator::{WORLD_BOTTOM, WORLD_TOP};
use crate::legacy_rng::LegacyRandom;
use crate::region_buf::RegionBuf;
use crate::sculk;
use crate::surface::BlockId;
use crate::worldgen::WorldgenState;
use serde_json::Value;


pub(super) fn placement_count(rng: &mut FeatureRandom, placed: &Value) -> i32 {
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

pub(super) fn sample_count_value(rng: &mut FeatureRandom, v: &Value) -> i32 {
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

pub(crate) fn sample_int_provider(rng: &mut FeatureRandom, v: &Value) -> i32 {
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

pub(crate) fn sample_height(rng: &mut FeatureRandom, height: &Value) -> i32 {
    let ty = height["type"].as_str().unwrap_or("minecraft:uniform");
    if ty.contains("uniform") {
        let min = resolve_anchor(&height["min_inclusive"]);
        let max = resolve_anchor(&height["max_inclusive"]);
        min + rng.next_int((max - min + 1).max(1))
    } else if ty.contains("trapezoid") {
        // TrapezoidHeight.sample (26.2): range = max-min;
        // if plateau >= range -> betweenInclusive(min,max);
        // else min + betweenInclusive(0, range-plateauStart)
        //          + betweenInclusive(0, plateauStart).
        let min = resolve_anchor(&height["min_inclusive"]);
        let max = resolve_anchor(&height["max_inclusive"]);
        if min > max {
            return min;
        }
        let plateau = height["plateau"].as_i64().unwrap_or(0) as i32;
        let range = max - min;
        if plateau >= range {
            return min + rng.next_int(range + 1);
        }
        let plateau_start = (range - plateau) / 2;
        let plateau_end = range - plateau_start;
        min + rng.next_int(plateau_end + 1) + rng.next_int(plateau_start + 1)
    } else if ty.contains("very_biased_to_bottom") {
        // VeryBiasedToBottomHeight.sample (decompiled 26.2):
        //   if max - min - inner + 1 <= 0 → min
        //   upper = nextInt(min + inner, max)
        //   biased = nextInt(min, upper - 1)
        //   return nextInt(min, biased - 1 + inner)
        let min = resolve_anchor(&height["min_inclusive"]);
        let max = resolve_anchor(&height["max_inclusive"]);
        let inner = height["inner"].as_i64().unwrap_or(1) as i32;
        if max - min - inner + 1 <= 0 {
            return min;
        }
        let upper = min + inner + rng.next_int((max - (min + inner) + 1).max(1));
        let biased = min + rng.next_int((upper - min).max(1));
        min + rng.next_int((biased - 1 + inner - min + 1).max(1))
    } else {
        64
    }
}

pub(super) fn resolve_anchor(v: &Value) -> i32 {
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


pub(crate) fn block_from_to_place(rng: &mut FeatureRandom, v: &Value) -> Option<BlockId> {
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
            // RandomizedIntStateProvider.getState: source.getState then
            // values.sample(random) for the int property (age on cave_vines).
            let block = block_from_to_place(rng, &v["source"]);
            let _ = sample_int_provider(rng, &v["values"]);
            block
        }
        _ => BlockId::from_name(v.pointer("/state/Name")?.as_str()?),
    }
}

