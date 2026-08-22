//! configured_feature JSON parsing for trees.
use super::*;
use crate::surface::BlockId;
use serde_json::Value;

pub(super) fn parse_trunk_kind(ty: &str) -> TrunkKind {
    match ty {
        "minecraft:straight_trunk_placer" => TrunkKind::Straight,
        "minecraft:dark_oak_trunk_placer" => TrunkKind::DarkOak,
        "minecraft:fancy_trunk_placer" => TrunkKind::Fancy,
        _ => TrunkKind::Unknown,
    }
}

pub(super) fn parse_foliage_kind(foliage: &Value) -> FoliageKind {
    match foliage["type"].as_str().unwrap_or("") {
        "minecraft:blob_foliage_placer" => FoliageKind::Blob {
            height: foliage["height"].as_i64().unwrap_or(3) as i32,
        },
        "minecraft:fancy_foliage_placer" => FoliageKind::Fancy {
            height: foliage["height"].as_i64().unwrap_or(4) as i32,
        },
        "minecraft:dark_oak_foliage_placer" => FoliageKind::DarkOak,
        _ => FoliageKind::Unknown,
    }
}

pub(super) fn parse_int_provider(v: &Value, default: i32) -> IntProv {
    if v.is_null() {
        return IntProv::Constant(default);
    }
    if let Some(n) = v.as_i64() {
        return IntProv::Constant(n as i32);
    }
    if let Some(n) = v.get("value").and_then(|x| x.as_i64()) {
        return IntProv::Constant(n as i32);
    }
    if v.get("min_inclusive").is_some()
        || v["type"].as_str().is_some_and(|t| t.ends_with("uniform"))
    {
        let min = v["min_inclusive"].as_i64().unwrap_or(default as i64) as i32;
        let max = v["max_inclusive"].as_i64().unwrap_or(min as i64) as i32;
        return IntProv::Uniform { min, max };
    }
    IntProv::Constant(default)
}

pub(super) fn parse_feature_size(v: &Value) -> FeatureSizeCfg {
    let min_clipped = v["min_clipped_height"].as_i64().map(|n| n as i32);
    match v["type"].as_str().unwrap_or("") {
        "minecraft:three_layers_feature_size" => FeatureSizeCfg {
            kind: SizeKind::Three {
                limit: v["limit"].as_i64().unwrap_or(1) as i32,
                upper_limit: v["upper_limit"].as_i64().unwrap_or(1) as i32,
                lower: v["lower_size"].as_i64().unwrap_or(0) as i32,
                middle: v["middle_size"].as_i64().unwrap_or(1) as i32,
                upper: v["upper_size"].as_i64().unwrap_or(1) as i32,
            },
            min_clipped,
        },
        _ => FeatureSizeCfg {
            kind: SizeKind::Two {
                limit: v["limit"].as_i64().unwrap_or(1) as i32,
                lower: v["lower_size"].as_i64().unwrap_or(0) as i32,
                upper: v["upper_size"].as_i64().unwrap_or(1) as i32,
            },
            min_clipped,
        },
    }
}

pub(super) fn below_trunk_block(cfg: &Value) -> Option<BlockId> {
    let p = &cfg["below_trunk_provider"];
    if let Some(rules) = p["rules"].as_array() {
        for rule in rules {
            if let Some(b) = block_from_provider(&rule["then"]) {
                return Some(b);
            }
        }
    }
    block_from_provider(p)
}

pub(super) fn block_from_provider(v: &Value) -> Option<BlockId> {
    if let Some(name) = v
        .pointer("/state/Name")
        .and_then(|n| n.as_str())
        .or_else(|| v.pointer("/entries/0/data/Name").and_then(|n| n.as_str()))
    {
        return BlockId::from_name(name);
    }
    None
}



