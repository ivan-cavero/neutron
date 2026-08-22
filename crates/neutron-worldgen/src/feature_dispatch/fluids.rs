//! Fluid-side inline features: spring + block_column (+ cave vines plumbing).
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


/// Port of `SpringFeature.place` (step 8, FLUID_SPRINGS).
///
/// `SpringConfiguration`: state (water/lava), valid_blocks, requiresBlockBelow
/// (default true), rockCount (default 4), holeCount (default 1).
/// Place conditions (SpringFeature.java): the block ABOVE must be a valid
/// block; if requiresBlockBelow the block BELOW must be valid; the origin cell
/// must be air or a valid block; exactly `rockCount` of the five neighbours
/// (N/E/S/W/below) must be valid blocks and exactly `holeCount` must be air.
pub(super) fn place_spring(
    rng: &mut FeatureRandom,
    region: &mut RegionBuf,
    x: i32,
    y: i32,
    z: i32,
    cfg: &Value,
) {
    let _ = rng;
    let c = &cfg["config"];
    let valid = c["valid_blocks"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().and_then(BlockId::from_name))
                .collect::<Vec<BlockId>>()
        })
        .unwrap_or_default();
    let is_valid = |b: BlockId| valid.contains(&b);
    if !is_valid(region.get(x, y + 1, z)) {
        return;
    }
    if c["requires_block_below"].as_bool().unwrap_or(true) && !is_valid(region.get(x, y - 1, z)) {
        return;
    }
    let here = region.get(x, y, z);
    if !matches!(here, BlockId::Air) && !is_valid(here) {
        return;
    }
    let nb = [
        region.get(x - 1, y, z),
        region.get(x + 1, y, z),
        region.get(x, y, z - 1),
        region.get(x, y, z + 1),
        region.get(x, y - 1, z),
    ];
    let rock = nb.iter().filter(|b| is_valid(**b)).count() as i32;
    let holes = nb.iter().filter(|b| matches!(b, BlockId::Air)).count() as i32;
    let rock_count = c["rock_count"].as_i64().unwrap_or(4) as i32;
    let hole_count = c["hole_count"].as_i64().unwrap_or(1) as i32;
    if rock == rock_count && holes == hole_count {
        if let Some(st) = c["state"]["Name"].as_str().and_then(BlockId::from_name) {
            region.set(x, y, z, st);
        }
    }
}

/// Port of `BlockColumnFeature.place` (cave vines).
pub(super) fn place_block_column(
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


