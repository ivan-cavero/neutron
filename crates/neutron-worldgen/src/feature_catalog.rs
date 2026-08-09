// Copyright (c) 2026 Neutron Contributors -- MIT License
//
// Data-driven feature step / index resolution from vanilla biome JSON.
//
// Minecraft `WorldgenRandom.setFeatureSeed(decorationSeed, featureIndex, step)`
// uses the **index within the biome's features[step] list**, not a global id.
// When Mojang reorders features or adds a biome, re-run extract-worldgen.ps1
// and this module picks up the new lists automatically.

use crate::datapack_fs;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::OnceLock;

/// Generation step indices (vanilla GenerationStep.Decoration ordinal).
pub mod step {
    pub const RAW_GENERATION: i32 = 0;
    pub const LAKES: i32 = 1;
    pub const LOCAL_MODIFICATIONS: i32 = 2;
    pub const UNDERGROUND_STRUCTURES: i32 = 3;
    pub const SURFACE_STRUCTURES: i32 = 4;
    pub const STRONGHOLDS: i32 = 5;
    pub const UNDERGROUND_ORES: i32 = 6;
    pub const UNDERGROUND_DECORATION: i32 = 7;
    pub const FLUID_SPRINGS: i32 = 8;
    pub const VEGETAL_DECORATION: i32 = 9;
    pub const TOP_LAYER_MODIFICATION: i32 = 10;
}

/// Feature index of `placed` inside biome `biome`'s step list, or None.
pub fn feature_index_in_biome(biome: &str, step: i32, placed: &str) -> Option<i32> {
    let lists = biome_feature_steps(biome)?;
    let step = step as usize;
    if step >= lists.len() {
        return None;
    }
    let want = strip_mc(placed);
    lists[step]
        .iter()
        .position(|f| strip_mc(f) == want)
        .map(|i| i as i32)
}

/// All placed feature ids for a biome at a step (empty if missing).
pub fn features_at_step(biome: &str, step: i32) -> Vec<String> {
    biome_feature_steps(biome)
        .and_then(|lists| lists.get(step as usize).cloned())
        .unwrap_or_default()
}

fn strip_mc(s: &str) -> &str {
    s.strip_prefix("minecraft:").unwrap_or(s)
}

fn biome_feature_steps(biome: &str) -> Option<&'static Vec<Vec<String>>> {
    let map = biome_features_cache();
    let key = strip_mc(biome);
    map.get(key)
}

fn biome_features_cache() -> &'static HashMap<String, Vec<Vec<String>>> {
    static CACHE: OnceLock<HashMap<String, Vec<Vec<String>>>> = OnceLock::new();
    CACHE.get_or_init(|| {
        let mut out = HashMap::new();
        // Prefer crate data; fall back to extract tree.
        let biome_dir = datapack_fs::worldgen_path("biome");
        let paths: Vec<std::path::PathBuf> = if biome_dir.is_dir() {
            std::fs::read_dir(&biome_dir)
                .into_iter()
                .flatten()
                .filter_map(|e| e.ok())
                .map(|e| e.path())
                .filter(|p| p.extension().and_then(|x| x.to_str()) == Some("json"))
                .collect()
        } else {
            Vec::new()
        };
        for path in paths {
            let stem = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_string();
            let Ok(text) = std::fs::read_to_string(&path) else {
                continue;
            };
            if let Some(steps) = parse_biome_features(&text) {
                out.insert(stem, steps);
            }
        }
        out
    })
}

fn parse_biome_features(text: &str) -> Option<Vec<Vec<String>>> {
    let v: Value = serde_json::from_str(text).ok()?;
    let arr = v.get("features")?.as_array()?;
    let mut steps = Vec::with_capacity(arr.len());
    for step in arr {
        let list = step
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|x| x.as_str().map(|s| s.to_string()))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        steps.push(list);
    }
    Some(steps)
}

/// Load a placed_feature JSON object.
pub fn load_placed_feature(id: &str) -> Option<Value> {
    let name = strip_mc(id);
    let text = datapack_fs::worldgen_json_with_fallback(&format!("placed_feature/{name}.json"))?;
    serde_json::from_str(&text).ok()
}

/// Load a configured_feature JSON object.
pub fn load_configured_feature(id: &str) -> Option<Value> {
    let name = strip_mc(id);
    let text =
        datapack_fs::worldgen_json_with_fallback(&format!("configured_feature/{name}.json"))?;
    serde_json::from_str(&text).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deep_dark_sculk_indices() {
        // deep_dark features[7] = [sculk_vein, sculk_patch_deep_dark]
        let vein = feature_index_in_biome("deep_dark", step::UNDERGROUND_DECORATION, "sculk_vein");
        let patch = feature_index_in_biome(
            "deep_dark",
            step::UNDERGROUND_DECORATION,
            "sculk_patch_deep_dark",
        );
        assert_eq!(vein, Some(0), "sculk_vein should be index 0 in step 7");
        assert_eq!(patch, Some(1), "sculk_patch_deep_dark should be index 1");
    }

    #[test]
    fn sculk_patch_config_loads() {
        let cfg = load_configured_feature("sculk_patch_deep_dark").expect("config");
        assert_eq!(cfg["type"], "minecraft:sculk_patch");
        assert_eq!(cfg["config"]["charge_count"], 10);
        assert_eq!(cfg["config"]["amount_per_charge"], 32);
        assert_eq!(cfg["config"]["spread_attempts"], 64);
    }
}
