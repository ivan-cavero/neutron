// Copyright (c) 2026 Neutron Contributors -- MIT License
//
// Underground ore + stone blob features for overworld generation step 6.
//
// Ports:
// - `OreFeature.place` / `doPlace` (ellipsoid blob)
// - Placement modifiers: count, rarity_filter, in_square, height_range
// - `WorldgenRandom` decoration/feature seeding
//
// Applies the same ordered list of placed features that overworld biomes share
// for the underground ores step (see dark_forest biome features[6]).

use crate::feature_rng::FeatureRandom;
use crate::generator::{WORLD_BOTTOM, WORLD_TOP};
use crate::region_buf::RegionBuf;
use crate::surface::BlockId;

const STEP_UNDERGROUND_ORES: i32 = 6;
const PI: f32 = 3.1415927;

/// Apply underground ores for every chunk origin inside `region`.
///
/// Order: Z then X over the region chunk grid, then feature index — deterministic
/// and matches a full WorldGenRegion decoration pass over the area.
pub fn apply_underground_ores_region(region: &mut RegionBuf, level_seed: i64) {
    let chunks = region.chunks;
    for czl in 0..chunks {
        for cxl in 0..chunks {
            let origin_min_x = region.origin_x + cxl * 16;
            let origin_min_z = region.origin_z + czl * 16;
            let mut rng = FeatureRandom::new(level_seed);
            let decoration_seed =
                rng.set_decoration_seed(level_seed, origin_min_x, origin_min_z);
            for (feature_index, def) in OVERWORLD_ORES.iter().enumerate() {
                rng.set_feature_seed(
                    decoration_seed,
                    feature_index as i32,
                    STEP_UNDERGROUND_ORES,
                );
                place_feature(&mut rng, region, origin_min_x, origin_min_z, def);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Feature definitions (from datapack placed_feature + configured_feature)
// ---------------------------------------------------------------------------

#[derive(Clone, Copy)]
enum CountSpec {
    Fixed(i32),
    Uniform { min: i32, max: i32 },
    Rarity(i32),
}

#[derive(Clone, Copy)]
enum HeightSpec {
    Uniform {
        min: HeightAnchor,
        max: HeightAnchor,
    },
    Trapezoid {
        min: HeightAnchor,
        max: HeightAnchor,
    },
}

#[derive(Clone, Copy)]
enum HeightAnchor {
    Absolute(i32),
    AboveBottom(i32),
    BelowTop(i32),
}

impl HeightAnchor {
    fn resolve(self) -> i32 {
        match self {
            HeightAnchor::Absolute(y) => y,
            HeightAnchor::AboveBottom(n) => WORLD_BOTTOM + n,
            HeightAnchor::BelowTop(n) => (WORLD_TOP - 1) - n,
        }
    }
}

#[derive(Clone, Copy)]
struct OreDef {
    size: i32,
    discard_chance: f32,
    /// (stone_variant, deepslate_variant) — deepslate variant optional (None = same)
    stone_block: BlockId,
    deepslate_block: Option<BlockId>,
    /// What can be replaced
    target: TargetKind,
    count: CountSpec,
    height: HeightSpec,
}

#[derive(Clone, Copy)]
enum TargetKind {
    /// stone, granite, diorite, andesite
    StoneOre,
    /// stone + granite + diorite + andesite + tuff + deepslate
    BaseStone,
    /// deepslate + tuff only (unused for most; deepslate ores use dual target)
    DeepslateOre,
}

const OVERWORLD_ORES: &[OreDef] = &[
    // ore_dirt
    OreDef {
        size: 33,
        discard_chance: 0.0,
        stone_block: BlockId::Dirt,
        deepslate_block: None,
        target: TargetKind::BaseStone,
        count: CountSpec::Fixed(7),
        height: HeightSpec::Uniform {
            min: HeightAnchor::Absolute(0),
            max: HeightAnchor::Absolute(160),
        },
    },
    // ore_gravel
    OreDef {
        size: 33,
        discard_chance: 0.0,
        stone_block: BlockId::Gravel,
        deepslate_block: None,
        target: TargetKind::BaseStone,
        count: CountSpec::Fixed(14),
        height: HeightSpec::Uniform {
            min: HeightAnchor::AboveBottom(0),
            max: HeightAnchor::BelowTop(0),
        },
    },
    // ore_granite_upper (rarity 6)
    OreDef {
        size: 64,
        discard_chance: 0.0,
        stone_block: BlockId::Granite,
        deepslate_block: None,
        target: TargetKind::BaseStone,
        count: CountSpec::Rarity(6),
        height: HeightSpec::Uniform {
            min: HeightAnchor::Absolute(64),
            max: HeightAnchor::Absolute(128),
        },
    },
    // ore_granite_lower
    OreDef {
        size: 64,
        discard_chance: 0.0,
        stone_block: BlockId::Granite,
        deepslate_block: None,
        target: TargetKind::BaseStone,
        count: CountSpec::Fixed(2),
        height: HeightSpec::Uniform {
            min: HeightAnchor::Absolute(0),
            max: HeightAnchor::Absolute(60),
        },
    },
    // ore_diorite_upper
    OreDef {
        size: 64,
        discard_chance: 0.0,
        stone_block: BlockId::Diorite,
        deepslate_block: None,
        target: TargetKind::BaseStone,
        count: CountSpec::Rarity(6),
        height: HeightSpec::Uniform {
            min: HeightAnchor::Absolute(64),
            max: HeightAnchor::Absolute(128),
        },
    },
    // ore_diorite_lower
    OreDef {
        size: 64,
        discard_chance: 0.0,
        stone_block: BlockId::Diorite,
        deepslate_block: None,
        target: TargetKind::BaseStone,
        count: CountSpec::Fixed(2),
        height: HeightSpec::Uniform {
            min: HeightAnchor::Absolute(0),
            max: HeightAnchor::Absolute(60),
        },
    },
    // ore_andesite_upper
    OreDef {
        size: 64,
        discard_chance: 0.0,
        stone_block: BlockId::Andesite,
        deepslate_block: None,
        target: TargetKind::BaseStone,
        count: CountSpec::Rarity(6),
        height: HeightSpec::Uniform {
            min: HeightAnchor::Absolute(64),
            max: HeightAnchor::Absolute(128),
        },
    },
    // ore_andesite_lower
    OreDef {
        size: 64,
        discard_chance: 0.0,
        stone_block: BlockId::Andesite,
        deepslate_block: None,
        target: TargetKind::BaseStone,
        count: CountSpec::Fixed(2),
        height: HeightSpec::Uniform {
            min: HeightAnchor::Absolute(0),
            max: HeightAnchor::Absolute(60),
        },
    },
    // ore_tuff
    OreDef {
        size: 64,
        discard_chance: 0.0,
        stone_block: BlockId::Tuff,
        deepslate_block: None,
        target: TargetKind::BaseStone,
        count: CountSpec::Fixed(2),
        height: HeightSpec::Uniform {
            min: HeightAnchor::AboveBottom(0),
            max: HeightAnchor::Absolute(0),
        },
    },
    // ore_coal_upper
    OreDef {
        size: 17,
        discard_chance: 0.0,
        stone_block: BlockId::CoalOre,
        deepslate_block: Some(BlockId::DeepslateCoalOre),
        target: TargetKind::StoneOre,
        count: CountSpec::Fixed(30),
        height: HeightSpec::Uniform {
            min: HeightAnchor::Absolute(136),
            max: HeightAnchor::BelowTop(0),
        },
    },
    // ore_coal_lower (buried)
    OreDef {
        size: 17,
        discard_chance: 0.5,
        stone_block: BlockId::CoalOre,
        deepslate_block: Some(BlockId::DeepslateCoalOre),
        target: TargetKind::StoneOre,
        count: CountSpec::Fixed(20),
        height: HeightSpec::Trapezoid {
            min: HeightAnchor::Absolute(0),
            max: HeightAnchor::Absolute(192),
        },
    },
    // ore_iron_upper
    OreDef {
        size: 9,
        discard_chance: 0.0,
        stone_block: BlockId::IronOre,
        deepslate_block: Some(BlockId::DeepslateIronOre),
        target: TargetKind::StoneOre,
        count: CountSpec::Fixed(90),
        height: HeightSpec::Trapezoid {
            min: HeightAnchor::Absolute(80),
            max: HeightAnchor::Absolute(384),
        },
    },
    // ore_iron_middle
    OreDef {
        size: 9,
        discard_chance: 0.0,
        stone_block: BlockId::IronOre,
        deepslate_block: Some(BlockId::DeepslateIronOre),
        target: TargetKind::StoneOre,
        count: CountSpec::Fixed(10),
        height: HeightSpec::Trapezoid {
            min: HeightAnchor::Absolute(-24),
            max: HeightAnchor::Absolute(56),
        },
    },
    // ore_iron_small
    OreDef {
        size: 4,
        discard_chance: 0.0,
        stone_block: BlockId::IronOre,
        deepslate_block: Some(BlockId::DeepslateIronOre),
        target: TargetKind::StoneOre,
        count: CountSpec::Fixed(10),
        height: HeightSpec::Uniform {
            min: HeightAnchor::AboveBottom(0),
            max: HeightAnchor::Absolute(72),
        },
    },
    // ore_gold
    OreDef {
        size: 9,
        discard_chance: 0.5,
        stone_block: BlockId::GoldOre,
        deepslate_block: Some(BlockId::DeepslateGoldOre),
        target: TargetKind::StoneOre,
        count: CountSpec::Fixed(4),
        height: HeightSpec::Trapezoid {
            min: HeightAnchor::Absolute(-64),
            max: HeightAnchor::Absolute(32),
        },
    },
    // ore_gold_lower
    OreDef {
        size: 9,
        discard_chance: 0.5,
        stone_block: BlockId::GoldOre,
        deepslate_block: Some(BlockId::DeepslateGoldOre),
        target: TargetKind::StoneOre,
        count: CountSpec::Uniform { min: 0, max: 1 },
        height: HeightSpec::Uniform {
            min: HeightAnchor::Absolute(-64),
            max: HeightAnchor::Absolute(-48),
        },
    },
    // ore_redstone
    OreDef {
        size: 8,
        discard_chance: 0.0,
        stone_block: BlockId::RedstoneOre,
        deepslate_block: Some(BlockId::DeepslateRedstoneOre),
        target: TargetKind::StoneOre,
        count: CountSpec::Fixed(4),
        height: HeightSpec::Uniform {
            min: HeightAnchor::AboveBottom(0),
            max: HeightAnchor::Absolute(15),
        },
    },
    // ore_redstone_lower
    OreDef {
        size: 8,
        discard_chance: 0.0,
        stone_block: BlockId::RedstoneOre,
        deepslate_block: Some(BlockId::DeepslateRedstoneOre),
        target: TargetKind::StoneOre,
        count: CountSpec::Fixed(8),
        height: HeightSpec::Trapezoid {
            min: HeightAnchor::AboveBottom(-32),
            max: HeightAnchor::AboveBottom(32),
        },
    },
    // ore_diamond (small)
    OreDef {
        size: 4,
        discard_chance: 0.5,
        stone_block: BlockId::DiamondOre,
        deepslate_block: Some(BlockId::DeepslateDiamondOre),
        target: TargetKind::StoneOre,
        count: CountSpec::Fixed(7),
        height: HeightSpec::Trapezoid {
            min: HeightAnchor::AboveBottom(-80),
            max: HeightAnchor::AboveBottom(80),
        },
    },
    // ore_diamond_medium
    OreDef {
        size: 8,
        discard_chance: 0.5,
        stone_block: BlockId::DiamondOre,
        deepslate_block: Some(BlockId::DeepslateDiamondOre),
        target: TargetKind::StoneOre,
        count: CountSpec::Fixed(2),
        height: HeightSpec::Uniform {
            min: HeightAnchor::Absolute(-64),
            max: HeightAnchor::Absolute(-4),
        },
    },
    // ore_diamond_large (rarity 9)
    OreDef {
        size: 12,
        discard_chance: 0.7,
        stone_block: BlockId::DiamondOre,
        deepslate_block: Some(BlockId::DeepslateDiamondOre),
        target: TargetKind::StoneOre,
        count: CountSpec::Rarity(9),
        height: HeightSpec::Trapezoid {
            min: HeightAnchor::AboveBottom(-80),
            max: HeightAnchor::AboveBottom(80),
        },
    },
    // ore_diamond_buried
    OreDef {
        size: 8,
        discard_chance: 1.0,
        stone_block: BlockId::DiamondOre,
        deepslate_block: Some(BlockId::DeepslateDiamondOre),
        target: TargetKind::StoneOre,
        count: CountSpec::Fixed(4),
        height: HeightSpec::Trapezoid {
            min: HeightAnchor::AboveBottom(-80),
            max: HeightAnchor::AboveBottom(80),
        },
    },
    // ore_lapis
    OreDef {
        size: 7,
        discard_chance: 0.0,
        stone_block: BlockId::LapisOre,
        deepslate_block: Some(BlockId::DeepslateLapisOre),
        target: TargetKind::StoneOre,
        count: CountSpec::Fixed(2),
        height: HeightSpec::Trapezoid {
            min: HeightAnchor::Absolute(-32),
            max: HeightAnchor::Absolute(32),
        },
    },
    // ore_lapis_buried
    OreDef {
        size: 7,
        discard_chance: 1.0,
        stone_block: BlockId::LapisOre,
        deepslate_block: Some(BlockId::DeepslateLapisOre),
        target: TargetKind::StoneOre,
        count: CountSpec::Fixed(4),
        height: HeightSpec::Uniform {
            min: HeightAnchor::AboveBottom(0),
            max: HeightAnchor::Absolute(64),
        },
    },
    // ore_copper
    OreDef {
        size: 10,
        discard_chance: 0.0,
        stone_block: BlockId::CopperOre,
        deepslate_block: Some(BlockId::DeepslateCopperOre),
        target: TargetKind::StoneOre,
        count: CountSpec::Fixed(16),
        height: HeightSpec::Trapezoid {
            min: HeightAnchor::Absolute(-16),
            max: HeightAnchor::Absolute(112),
        },
    },
];

// ---------------------------------------------------------------------------
// Placement + OreFeature
// ---------------------------------------------------------------------------

fn place_feature(
    rng: &mut FeatureRandom,
    region: &mut RegionBuf,
    origin_min_x: i32,
    origin_min_z: i32,
    def: &OreDef,
) {
    let attempts = match def.count {
        CountSpec::Fixed(n) => n,
        CountSpec::Uniform { min, max } => {
            if max < min {
                0
            } else {
                min + rng.next_int(max - min + 1)
            }
        }
        CountSpec::Rarity(chance) => {
            if rng.next_int(chance) == 0 {
                1
            } else {
                0
            }
        }
    };

    for _ in 0..attempts {
        let lx = rng.next_int(16);
        let lz = rng.next_int(16);
        let x = origin_min_x + lx;
        let z = origin_min_z + lz;
        let y = sample_height(rng, def.height);
        if y < WORLD_BOTTOM || y >= WORLD_TOP {
            continue;
        }
        place_ore_blob(rng, region, x, y, z, def);
    }
}

fn sample_height(rng: &mut FeatureRandom, h: HeightSpec) -> i32 {
    match h {
        HeightSpec::Uniform { min, max } => {
            let lo = min.resolve();
            let hi = max.resolve();
            if hi <= lo {
                return lo;
            }
            // Mth.randomBetweenInclusive(random, lo, hi)
            lo + rng.next_int(hi - lo + 1)
        }
        HeightSpec::Trapezoid { min, max } => {
            // Vanilla TrapezoidHeight with plateau=0:
            //   range = max - min
            //   bottom = range / 2; top = range - bottom
            //   return min + randomBetweenInclusive(0, top) + randomBetweenInclusive(0, bottom)
            let lo = min.resolve();
            let hi = max.resolve();
            if hi <= lo {
                return lo;
            }
            let range = hi - lo;
            let bottom = range / 2;
            let top = range - bottom;
            lo + rng.next_int(top + 1) + rng.next_int(bottom + 1)
        }
    }
}

/// `OreFeature.place` + `doPlace` into a multi-chunk region.
fn place_ore_blob(
    rng: &mut FeatureRandom,
    region: &mut RegionBuf,
    ox: i32,
    oy: i32,
    oz: i32,
    def: &OreDef,
) {
    let size = def.size;
    if size <= 0 {
        return;
    }
    let angle = rng.next_f32() * PI;
    let f = size as f32 / 8.0;
    // Match Java: Math.sin((double)angle) after float angle
    let start_x = ox as f64 + (angle as f64).sin() * f as f64;
    let end_x = ox as f64 - (angle as f64).sin() * f as f64;
    let start_z = oz as f64 + (angle as f64).cos() * f as f64;
    let end_z = oz as f64 - (angle as f64).cos() * f as f64;
    let start_y = (oy + rng.next_int(3) - 2) as f64;
    let end_y = (oy + rng.next_int(3) - 2) as f64;

    // Sphere path samples
    let mut spheres = vec![0f64; (size as usize) * 4];
    for i in 0..size {
        let t = i as f64 / size as f64;
        let sx = lerp(t, start_x, end_x);
        let sy = lerp(t, start_y, end_y);
        let sz = lerp(t, start_z, end_z);
        let blip = rng.next_f64() * size as f64 / 16.0;
        // Java: ((Mth.sin(PI * t) + 1.0f) * blip + 1.0) / 2.0  — sin is float
        let sin_part = ((PI * t as f32).sin() + 1.0) as f64;
        let radius = (sin_part * blip + 1.0) / 2.0;
        let base = (i as usize) * 4;
        spheres[base] = sx;
        spheres[base + 1] = sy;
        spheres[base + 2] = sz;
        spheres[base + 3] = radius;
    }

    // Remove overlapping spheres (bytecode cull)
    for i in 0..size - 1 {
        let bi = (i as usize) * 4;
        if spheres[bi + 3] <= 0.0 {
            continue;
        }
        for j in (i + 1)..size {
            let bj = (j as usize) * 4;
            if spheres[bj + 3] <= 0.0 {
                continue;
            }
            let dx = spheres[bi] - spheres[bj];
            let dy = spheres[bi + 1] - spheres[bj + 1];
            let dz = spheres[bi + 2] - spheres[bj + 2];
            let dr = spheres[bi + 3] - spheres[bj + 3];
            if dr * dr > dx * dx + dy * dy + dz * dz {
                if dr > 0.0 {
                    spheres[bj + 3] = -1.0;
                } else {
                    spheres[bi + 3] = -1.0;
                }
            }
        }
    }

    let cell = (size as f32 / 16.0 * 2.0 + 1.0) / 2.0;
    let cell = cell.ceil() as i32;
    let start_block_x = ox - f.ceil() as i32 - cell;
    let start_block_y = oy - 2 - cell;
    let start_block_z = oz - f.ceil() as i32 - cell;
    let size_xz = 2 * (f.ceil() as i32 + cell);
    let size_y = 2 * (2 + cell);

    let mut bitset = vec![false; (size_xz * size_y * size_xz).max(1) as usize];

    for i in 0..size {
        let bi = (i as usize) * 4;
        let radius = spheres[bi + 3];
        if radius < 0.0 {
            continue;
        }
        let cx = spheres[bi];
        let cy = spheres[bi + 1];
        let cz = spheres[bi + 2];
        let min_x = floor(cx - radius).max(start_block_x);
        let min_y = floor(cy - radius).max(start_block_y);
        let min_z = floor(cz - radius).max(start_block_z);
        let max_x = floor(cx + radius).max(min_x);
        let max_y = floor(cy + radius).max(min_y);
        let max_z = floor(cz + radius).max(min_z);

        for x in min_x..=max_x {
            let dx = ((x as f64 + 0.5) - cx) / radius;
            if dx * dx >= 1.0 {
                continue;
            }
            for y in min_y..=max_y {
                if y < WORLD_BOTTOM || y >= WORLD_TOP {
                    continue;
                }
                let dy = ((y as f64 + 0.5) - cy) / radius;
                if dx * dx + dy * dy >= 1.0 {
                    continue;
                }
                for z in min_z..=max_z {
                    let dz = ((z as f64 + 0.5) - cz) / radius;
                    if dx * dx + dy * dy + dz * dz >= 1.0 {
                        continue;
                    }
                    let bit = ((x - start_block_x)
                        + (y - start_block_y) * size_xz
                        + (z - start_block_z) * size_xz * size_y) as usize;
                    if bit >= bitset.len() || bitset[bit] {
                        continue;
                    }
                    bitset[bit] = true;

                    if region.index(x, y, z).is_none() {
                        continue;
                    }
                    let existing = region.get(x, y, z);
                    let replacement = match target_match(existing, def) {
                        Some(b) => b,
                        None => continue,
                    };
                    if should_skip_air_exposure(region, x, y, z, def.discard_chance, rng) {
                        continue;
                    }
                    region.set(x, y, z, replacement);
                }
            }
        }
    }
}

fn target_match(existing: BlockId, def: &OreDef) -> Option<BlockId> {
    match def.target {
        TargetKind::StoneOre => {
            if is_stone_ore_replaceable(existing) {
                Some(def.stone_block)
            } else if is_deepslate_ore_replaceable(existing) {
                Some(def.deepslate_block.unwrap_or(def.stone_block))
            } else {
                None
            }
        }
        TargetKind::BaseStone => {
            if is_base_stone(existing) {
                Some(def.stone_block)
            } else {
                None
            }
        }
        TargetKind::DeepslateOre => {
            if is_deepslate_ore_replaceable(existing) {
                Some(def.deepslate_block.unwrap_or(def.stone_block))
            } else {
                None
            }
        }
    }
}

fn is_stone_ore_replaceable(b: BlockId) -> bool {
    matches!(
        b,
        BlockId::Stone | BlockId::Granite | BlockId::Diorite | BlockId::Andesite
    )
}

fn is_deepslate_ore_replaceable(b: BlockId) -> bool {
    matches!(b, BlockId::Deepslate | BlockId::Tuff)
}

fn is_base_stone(b: BlockId) -> bool {
    matches!(
        b,
        BlockId::Stone
            | BlockId::Granite
            | BlockId::Diorite
            | BlockId::Andesite
            | BlockId::Tuff
            | BlockId::Deepslate
    )
}

fn should_skip_air_exposure(
    region: &RegionBuf,
    x: i32,
    y: i32,
    z: i32,
    chance: f32,
    rng: &mut FeatureRandom,
) -> bool {
    if chance <= 0.0 {
        return false;
    }
    let exposed = neighbor_is_air(region, x + 1, y, z)
        || neighbor_is_air(region, x - 1, y, z)
        || neighbor_is_air(region, x, y + 1, z)
        || neighbor_is_air(region, x, y - 1, z)
        || neighbor_is_air(region, x, y, z + 1)
        || neighbor_is_air(region, x, y, z - 1);
    if !exposed {
        return false;
    }
    if chance >= 1.0 {
        return true;
    }
    rng.next_f32() < chance
}

fn neighbor_is_air(region: &RegionBuf, x: i32, y: i32, z: i32) -> bool {
    if y < WORLD_BOTTOM || y >= WORLD_TOP {
        return true;
    }
    if region.index(x, y, z).is_none() {
        return false;
    }
    let b = region.get(x, y, z);
    b.is_air() || b.is_fluid()
}

#[inline]
fn lerp(t: f64, a: f64, b: f64) -> f64 {
    a + t * (b - a)
}

#[inline]
fn floor(v: f64) -> i32 {
    v.floor() as i32
}
