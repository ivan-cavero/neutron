// Copyright (c) 2026 Neutron Contributors -- MIT License
//
// OreVeinifier matching Minecraft 26.2 `OreVeinifier.create`.

use crate::density::{DensityEnv, DF};
use crate::positional::PositionalRandomFactory;
use crate::surface::BlockId;

/// Place ore-vein blocks when aquifer returns solid default (None).
///
/// Returns `Some(block)` if this position is inside a vein, else `None`.
pub fn try_place_vein(
    x: i32,
    y: i32,
    z: i32,
    vein_toggle: &DF,
    vein_ridged: &DF,
    vein_gap: &DF,
    ore_random: PositionalRandomFactory,
    noises: &std::collections::HashMap<String, crate::noise::NormalNoise>,
) -> Option<BlockId> {
    let mut env = DensityEnv::new(x, y, z, noises);
    let toggle = crate::density::compute(vein_toggle, &mut env);
    // toggle > 0 → COPPER, else IRON
    let (ore, raw, filler, min_y, max_y) = if toggle > 0.0 {
        // COPPER: copper_ore, raw_copper, granite, 0..50
        (
            BlockId::CopperOre,
            BlockId::RawCopperBlock,
            BlockId::Granite,
            0,
            50,
        )
    } else {
        // IRON: deepslate_iron_ore, raw_iron, tuff, -60..-8
        (
            BlockId::DeepslateIronOre,
            BlockId::RawIronBlock,
            BlockId::Tuff,
            -60,
            -8,
        )
    };

    let abs_toggle = toggle.abs();
    let above_min = y - min_y;
    let below_max = max_y - y;
    if above_min < 0 || below_max < 0 {
        return None;
    }
    // edge roundoff: clampedMap(min(above_min, below_max), 0, 20, -0.2, 0)
    let edge = above_min.min(below_max) as f64;
    let edge_roundoff = clamped_map(edge, 0.0, 20.0, -0.2, 0.0);
    // VEININESS_THRESHOLD = 0.4
    if abs_toggle + edge_roundoff < 0.4 {
        return None;
    }

    let mut rng = ore_random.at(x, y, z);
    // VEIN_SOLIDNESS = 0.7 → skip if nextFloat() > 0.7
    if rng.next_f32() > 0.7 {
        return None;
    }

    // ridged must be < 0
    let ridged = crate::density::compute(vein_ridged, &mut env);
    if ridged >= 0.0 {
        return None;
    }

    // richness = clampedMap(absToggle, 0.4, 0.6, 0.1, 0.3)
    let richness = clamped_map(abs_toggle, 0.4, 0.6, 0.1, 0.3);
    if (rng.next_f32() as f64) < richness {
        let gap = crate::density::compute(vein_gap, &mut env);
        // SKIP_ORE_IF_GAP_NOISE_IS_BELOW = -0.3 → place ore if gap > -0.3
        if gap > -0.3 {
            // CHANCE_OF_RAW_ORE_BLOCK = 0.02
            if rng.next_f32() < 0.02 {
                return Some(raw);
            }
            return Some(ore);
        }
    }
    Some(filler)
}

#[inline]
fn clamped_map(v: f64, from_min: f64, from_max: f64, to_min: f64, to_max: f64) -> f64 {
    if v <= from_min {
        return to_min;
    }
    if v >= from_max {
        return to_max;
    }
    let t = (v - from_min) / (from_max - from_min);
    to_min + t * (to_max - to_min)
}
