// Copyright (c) 2026 Neutron Contributors -- MIT License
//
// TreeFeature subset for datapack tree configs:
//   - straight_trunk_placer
//   - blob_foliage_placer
//   - dark_oak_trunk_placer (2×2)
//   - fancy_trunk omitted → falls back to straight with taller height
//
// Algorithm follows TreeFeature.doPlace + StraightTrunkPlacer + BlobFoliagePlacer
// (CFR TreeFeature.java). Config loaded from configured_feature JSON.

use crate::feature_rng::FeatureRandom;
use crate::generator::{WORLD_BOTTOM, WORLD_TOP};
use crate::region_buf::RegionBuf;
use crate::surface::BlockId;
use serde_json::Value;

/// Place a tree from a configured_feature JSON object (`type: minecraft:tree`).
pub fn place_tree_from_config(
    rng: &mut FeatureRandom,
    region: &mut RegionBuf,
    x: i32,
    y: i32,
    z: i32,
    config: &Value,
) -> bool {
    let cfg = &config["config"];
    let trunk = &cfg["trunk_placer"];
    let foliage = &cfg["foliage_placer"];
    let trunk_ty = trunk["type"].as_str().unwrap_or("");
    let foliage_ty = foliage["type"].as_str().unwrap_or("");

    let log = block_from_provider(&cfg["trunk_provider"]).unwrap_or(BlockId::OakLog);
    let leaves = block_from_provider(&cfg["foliage_provider"]).unwrap_or(BlockId::OakLeaves);

    // Trunk height
    let tree_height = match trunk_ty {
        "minecraft:straight_trunk_placer" => {
            let base = trunk["base_height"].as_i64().unwrap_or(4) as i32;
            let a = trunk["height_rand_a"].as_i64().unwrap_or(2) as i32;
            let b = trunk["height_rand_b"].as_i64().unwrap_or(0) as i32;
            // StraightTrunkPlacer.getTreeHeight: base + rand(a+1) + rand(b+1)
            base + if a > 0 { rng.next_int(a + 1) } else { 0 }
                + if b > 0 { rng.next_int(b + 1) } else { 0 }
        }
        "minecraft:dark_oak_trunk_placer" => {
            let base = trunk["base_height"].as_i64().unwrap_or(6) as i32;
            let a = trunk["height_rand_a"].as_i64().unwrap_or(2) as i32;
            let b = trunk["height_rand_b"].as_i64().unwrap_or(1) as i32;
            base + if a > 0 { rng.next_int(a + 1) } else { 0 }
                + if b > 0 { rng.next_int(b + 1) } else { 0 }
        }
        "minecraft:fancy_trunk_placer" => {
            let base = trunk["base_height"].as_i64().unwrap_or(3) as i32;
            let a = trunk["height_rand_a"].as_i64().unwrap_or(11) as i32;
            let b = trunk["height_rand_b"].as_i64().unwrap_or(0) as i32;
            base + if a > 0 { rng.next_int(a + 1) } else { 0 }
                + if b > 0 { rng.next_int(b + 1) } else { 0 }
        }
        _ => 5 + rng.next_int(3),
    };

    // Foliage params
    let (foliage_height, foliage_radius, foliage_offset) = match foliage_ty {
        "minecraft:blob_foliage_placer" => {
            let h = foliage["height"].as_i64().unwrap_or(3) as i32;
            let r = foliage["radius"].as_i64().unwrap_or(2) as i32;
            let o = foliage["offset"].as_i64().unwrap_or(0) as i32;
            (h, r, o)
        }
        "minecraft:dark_oak_foliage_placer" => {
            let r = foliage["radius"].as_i64().unwrap_or(0) as i32;
            let o = foliage["offset"].as_i64().unwrap_or(0) as i32;
            (3, r.max(1) + 1, o)
        }
        _ => (3, 2, 0),
    };

    // Dirt under trunk
    if y > WORLD_BOTTOM {
        let below = region.get(x, y - 1, z);
        if matches!(below, BlockId::GrassBlock | BlockId::Podzol | BlockId::Dirt) {
            region.set(x, y - 1, z, BlockId::Dirt);
        }
    }

    // Free height check (simplified validTreePos column)
    let free_h = max_free_height(region, x, y, z, tree_height);
    if free_h < 3 {
        return false;
    }
    let h = free_h.min(tree_height);

    match trunk_ty {
        "minecraft:dark_oak_trunk_placer" => {
            place_dark_oak_trunk(region, x, y, z, h, log);
            place_blob_foliage(rng, region, x, y + h - 1 + foliage_offset, z, foliage_height, foliage_radius + 1, leaves);
            // 2x2 canopy offset
            place_blob_foliage(rng, region, x + 1, y + h - 1 + foliage_offset, z + 1, foliage_height, foliage_radius + 1, leaves);
        }
        _ => {
            // straight (+ fancy as taller straight for now — branch structure is separate CFR)
            for dy in 0..h {
                let yy = y + dy;
                if !valid_tree_pos(region.get(x, yy, z)) {
                    break;
                }
                region.set(x, yy, z, log);
            }
            // BlobFoliagePlacer.createFoliage at trunk top attachment
            let attach_y = y + h - 1 + foliage_offset;
            place_blob_foliage(rng, region, x, attach_y, z, foliage_height, foliage_radius, leaves);
        }
    }
    true
}

fn max_free_height(region: &RegionBuf, x: i32, y: i32, z: i32, max: i32) -> i32 {
    for dy in 0..=max {
        if y + dy >= WORLD_TOP {
            return dy.saturating_sub(1);
        }
        if !valid_tree_pos(region.get(x, y + dy, z)) {
            return dy.saturating_sub(1).max(0);
        }
    }
    max
}

fn valid_tree_pos(b: BlockId) -> bool {
    matches!(
        b,
        BlockId::Air
            | BlockId::OakLeaves
            | BlockId::DarkOakLeaves
            | BlockId::ShortGrass
            | BlockId::LeafLitter
            | BlockId::Snow
    )
}

/// BlobFoliagePlacer: layers of decreasing radius around attach point.
fn place_blob_foliage(
    rng: &mut FeatureRandom,
    region: &mut RegionBuf,
    cx: i32,
    cy: i32,
    cz: i32,
    height: i32,
    radius: i32,
    leaves: BlockId,
) {
    for dy in 0..height {
        let r = if dy == 0 || dy == height - 1 {
            radius - 1
        } else {
            radius
        }
        .max(0);
        for dx in -r..=r {
            for dz in -r..=r {
                // corners thinned
                if dx.abs() == r && dz.abs() == r && (rng.next_int(2) == 0 || r == 0) {
                    continue;
                }
                let x = cx + dx;
                let y = cy + dy;
                let z = cz + dz;
                if valid_tree_pos(region.get(x, y, z)) {
                    region.set(x, y, z, leaves);
                }
            }
        }
    }
}

fn place_dark_oak_trunk(region: &mut RegionBuf, x: i32, y: i32, z: i32, h: i32, log: BlockId) {
    for dy in 0..h {
        for (tx, tz) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
            let bx = x + tx;
            let by = y + dy;
            let bz = z + tz;
            if valid_tree_pos(region.get(bx, by, bz)) || matches!(region.get(bx, by, bz), BlockId::OakLog | BlockId::DarkOakLog) {
                region.set(bx, by, bz, log);
            }
        }
    }
}

fn block_from_provider(v: &Value) -> Option<BlockId> {
    // simple_state_provider or weighted — take first Name
    if let Some(name) = v
        .pointer("/state/Name")
        .and_then(|n| n.as_str())
        .or_else(|| {
            v.pointer("/entries/0/data/Name")
                .and_then(|n| n.as_str())
        })
    {
        return BlockId::from_name(name);
    }
    None
}
