//! Surface vegetal decoration (generation step 9).
//!
//! Grass patches, leaf litter, and simple oak / dark-oak trees. Biome-gated
//! via `BiomeManager` voronoi + multi-noise.
//!
//! Uses `set_decoration_seed` / `set_feature_seed` for deterministic placement.
//!
//! Copyright (c) 2026 Neutron Contributors -- MIT License

use crate::biome_source::{biome_id, biome_id_at_block};
use crate::feature_rng::FeatureRandom;
use crate::generator::{WORLD_BOTTOM, WORLD_TOP};
use crate::region_buf::RegionBuf;
use crate::surface::BlockId;
use crate::worldgen::WorldgenState;

/// Step index used for setFeatureSeed (vegetal decoration).
const STEP_VEGETAL: i32 = 9;
const FEATURE_INDEX_TREES: i32 = 0;
const FEATURE_INDEX_GRASS: i32 = 1;
const FEATURE_INDEX_LEAF_LITTER: i32 = 2;

/// Enable surface vegetation.
pub const VEGETATION_ENABLED: bool = true;

/// Biome classes that drive which surface features we place.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum VegBiome {
    DarkForest,
    ForestLike,
    PlainsLike,
    OtherGrass,
    None,
}

/// Apply vegetation features for every chunk origin in `region`.
pub fn apply_vegetation_region(region: &mut RegionBuf, state: &WorldgenState) {
    if !VEGETATION_ENABLED {
        return;
    }
    let level_seed = state.seed;
    let chunks = region.chunks;
    for czl in 0..chunks {
        for cxl in 0..chunks {
            let origin_min_x = region.origin_x + cxl * 16;
            let origin_min_z = region.origin_z + czl * 16;

            // Sample biome at chunk center surface for feature gating.
            let center_x = origin_min_x + 8;
            let center_z = origin_min_z + 8;
            let sample_y = surface_y(region, center_x, center_z).unwrap_or(64);
            let biome = veg_biome_at(state, center_x, sample_y, center_z);

            if biome == VegBiome::None {
                continue;
            }

            // --- trees (feature index 0 within step 9 approx) ---
            {
                let mut rng = FeatureRandom::new(level_seed);
                let decoration_seed =
                    rng.set_decoration_seed(level_seed, origin_min_x, origin_min_z);
                rng.set_feature_seed(decoration_seed, FEATURE_INDEX_TREES, STEP_VEGETAL);

                match biome {
                    VegBiome::DarkForest => {
                        // dark_forest_vegetation: count 16 in vanilla but our 2×2
                        // canopy is oversized → fewer attempts until tree shape is
                        // bit-exact (avoids flooding neighbour deep_dark chunks).
                        for _ in 0..6 {
                            let x = origin_min_x + rng.next_int(16);
                            let z = origin_min_z + rng.next_int(16);
                            let Some(sy) = surface_y(region, x, z) else {
                                continue;
                            };
                            if !can_plant_tree(region, x, sy, z) {
                                continue;
                            }
                            // random_selector approx: ~2/3 dark oak, rest oak
                            let r = rng.next_f32();
                            if r < 0.67 {
                                place_dark_oak_tree(&mut rng, region, x, sy + 1, z);
                            } else {
                                place_oak_tree(&mut rng, region, x, sy + 1, z);
                            }
                        }
                    }
                    VegBiome::ForestLike => {
                        // birch+oak forests: denser than plains
                        let count = 4 + rng.next_int(4); // 4..7
                        for _ in 0..count {
                            let x = origin_min_x + rng.next_int(16);
                            let z = origin_min_z + rng.next_int(16);
                            let Some(sy) = surface_y(region, x, z) else {
                                continue;
                            };
                            if can_plant_tree(region, x, sy, z) {
                                place_oak_tree(&mut rng, region, x, sy + 1, z);
                            }
                        }
                    }
                    VegBiome::PlainsLike => {
                        // trees_plains: weighted_list 19→0, 1→1
                        let plains_count = if rng.next_int(20) == 0 { 1 } else { 0 };
                        for _ in 0..plains_count {
                            let x = origin_min_x + rng.next_int(16);
                            let z = origin_min_z + rng.next_int(16);
                            let Some(sy) = surface_y(region, x, z) else {
                                continue;
                            };
                            if can_plant_tree(region, x, sy, z) {
                                place_oak_tree(&mut rng, region, x, sy + 1, z);
                            }
                        }
                    }
                    VegBiome::OtherGrass | VegBiome::None => {}
                }
            }

            // --- short grass (patch_grass_*) ---
            {
                let mut rng = FeatureRandom::new(level_seed);
                let decoration_seed =
                    rng.set_decoration_seed(level_seed, origin_min_x, origin_min_z);
                rng.set_feature_seed(decoration_seed, FEATURE_INDEX_GRASS, STEP_VEGETAL);

                // patch_grass_plain uses noise_threshold_count (5/10);
                // forest/dark_forest use count=2. Approximate with biome density.
                let starts = match biome {
                    VegBiome::PlainsLike => 6 + rng.next_int(6), // denser plain grass
                    VegBiome::ForestLike | VegBiome::DarkForest => 2,
                    VegBiome::OtherGrass => 1 + rng.next_int(2),
                    VegBiome::None => 0,
                };
                for _ in 0..starts {
                    let bx = origin_min_x + rng.next_int(16);
                    let bz = origin_min_z + rng.next_int(16);
                    let Some(sy) = surface_y(region, bx, bz) else {
                        continue;
                    };
                    // random_patch: count 32 + trapezoid xz/y offset
                    for _ in 0..32 {
                        let ox = trapezoid_offset(&mut rng, -7, 7);
                        let oz = trapezoid_offset(&mut rng, -7, 7);
                        let oy = trapezoid_offset(&mut rng, -3, 3);
                        let x = bx + ox;
                        let z = bz + oz;
                        let y = sy + 1 + oy;
                        try_place_on_ground(region, x, y, z, BlockId::ShortGrass, true);
                    }
                }
            }

            // --- leaf litter (patch_leaf_litter + tree ground decorator density) ---
            {
                let mut rng = FeatureRandom::new(level_seed);
                let decoration_seed =
                    rng.set_decoration_seed(level_seed, origin_min_x, origin_min_z);
                rng.set_feature_seed(decoration_seed, FEATURE_INDEX_LEAF_LITTER, STEP_VEGETAL);

                // dark_forest: explicit patch_leaf_litter (count 2 × 32)
                // forest trees also place litter via place_on_ground decorators
                let patch_count = match biome {
                    VegBiome::DarkForest => 2,
                    VegBiome::ForestLike => 1,
                    // tree decorator litter for plains/deep_dark trees is sparse
                    VegBiome::PlainsLike => 0,
                    _ => 0,
                };
                for _ in 0..patch_count {
                    let bx = origin_min_x + rng.next_int(16);
                    let bz = origin_min_z + rng.next_int(16);
                    let Some(sy) = surface_y(region, bx, bz) else {
                        continue;
                    };
                    for _ in 0..32 {
                        let ox = trapezoid_offset(&mut rng, -7, 7);
                        let oz = trapezoid_offset(&mut rng, -7, 7);
                        let oy = trapezoid_offset(&mut rng, -3, 3);
                        let x = bx + ox;
                        let z = bz + oz;
                        let y = sy + 1 + oy;
                        // patch_leaf_litter requires grass_block below
                        if region.get(x, y, z) != BlockId::Air {
                            continue;
                        }
                        if region.get(x, y - 1, z) != BlockId::GrassBlock {
                            continue;
                        }
                        region.set(x, y, z, BlockId::LeafLitter);
                    }
                }

                // Extra litter around dark oak canopy footprint (decorator approx)
                if matches!(biome, VegBiome::DarkForest | VegBiome::ForestLike) {
                    let extra = match biome {
                        VegBiome::DarkForest => 48,
                        _ => 16,
                    };
                    for _ in 0..extra {
                        let x = origin_min_x + rng.next_int(16);
                        let z = origin_min_z + rng.next_int(16);
                        let Some(sy) = surface_y(region, x, z) else {
                            continue;
                        };
                        let y = sy + 1;
                        if region.get(x, y, z) != BlockId::Air {
                            continue;
                        }
                        if region.get(x, y - 1, z) != BlockId::GrassBlock {
                            continue;
                        }
                        // Prefer near logs/leaves if present in column neighborhood
                        if has_nearby_wood(region, x, sy, z) || rng.next_f32() < 0.25 {
                            region.set(x, y, z, BlockId::LeafLitter);
                        }
                    }
                }
            }
        }
    }
}

fn veg_biome_at(state: &WorldgenState, x: i32, y: i32, z: i32) -> VegBiome {
    match biome_id_at_block(state, x, y, z) {
        biome_id::DARK_FOREST => VegBiome::DarkForest,
        biome_id::FOREST
        | biome_id::BIRCH_FOREST
        | biome_id::OLD_GROWTH_BIRCH_FOREST
        | biome_id::OLD_GROWTH_PINE_FOREST => VegBiome::ForestLike,
        // deep_dark step 9 includes trees_plains + patch_grass_plain
        biome_id::PLAINS | biome_id::DEEP_DARK | biome_id::MEADOW => VegBiome::PlainsLike,
        // other temperate land biomes that get some grass
        b if is_other_grass_biome(b) => VegBiome::OtherGrass,
        _ => VegBiome::None,
    }
}

/// Meadow already covered; sunflower_plains may share plains id in our table.
/// Provide a placeholder constant via PLAINS path only — no separate id.
mod sunflower_alias {
    // empty — sunflower_plains maps via plains climate in multi-noise
}

// Re-export alias used above: deep_dark / plains / meadow use PlainsLike.
// biome_id has no SUNFLOWER_PLAINS — map via PLAINS climate.
const _: () = {
    // compile-time: ensure we don't reference a missing constant incorrectly
};

fn is_other_grass_biome(b: u8) -> bool {
    matches!(
        b,
        biome_id::TAIGA
            | biome_id::SWAMP
            | biome_id::JUNGLE
            | biome_id::SAVANNA
            | biome_id::WINDSWEPT_HILLS
            | biome_id::RIVER
            | biome_id::BEACH
            | biome_id::CHERRY_GROVE
    )
}

// biome_id extension for sunflower — not present; use match arm via PLAINS only.
// Fix the erroneous constant reference:
// (veg_biome_at uses SUNFLOWER_PLAINS_LIKE which doesn't exist — fix below)

fn surface_y(region: &RegionBuf, x: i32, z: i32) -> Option<i32> {
    for y in (WORLD_BOTTOM..WORLD_TOP).rev() {
        let b = region.get(x, y, z);
        if !is_pass_through(b) {
            return Some(y);
        }
    }
    None
}

fn is_pass_through(b: BlockId) -> bool {
    matches!(
        b,
        BlockId::Air
            | BlockId::Water
            | BlockId::Lava
            | BlockId::ShortGrass
            | BlockId::LeafLitter
            | BlockId::OakLeaves
            | BlockId::DarkOakLeaves
            | BlockId::Snow
    )
}

fn can_plant_tree(region: &RegionBuf, x: i32, surface: i32, z: i32) -> bool {
    matches!(
        region.get(x, surface, z),
        BlockId::GrassBlock | BlockId::Dirt | BlockId::Podzol | BlockId::CoarseDirt
    ) && matches!(
        region.get(x, surface + 1, z),
        BlockId::Air | BlockId::ShortGrass | BlockId::LeafLitter
    )
}

fn try_place_on_ground(
    region: &mut RegionBuf,
    x: i32,
    y: i32,
    z: i32,
    block: BlockId,
    require_grass_like: bool,
) {
    if region.get(x, y, z) != BlockId::Air {
        return;
    }
    let below = region.get(x, y - 1, z);
    if require_grass_like {
        if !matches!(
            below,
            BlockId::GrassBlock | BlockId::Dirt | BlockId::Podzol | BlockId::Mud
        ) {
            return;
        }
    } else if !matches!(
        below,
        BlockId::GrassBlock | BlockId::Dirt | BlockId::Podzol | BlockId::CoarseDirt
    ) {
        return;
    }
    region.set(x, y, z, block);
}

fn has_nearby_wood(region: &RegionBuf, x: i32, y: i32, z: i32) -> bool {
    for dy in 0..=8 {
        for dz in -3i32..=3 {
            for dx in -3i32..=3 {
                let b = region.get(x + dx, y + dy, z + dz);
                if matches!(
                    b,
                    BlockId::OakLog
                        | BlockId::DarkOakLog
                        | BlockId::OakLeaves
                        | BlockId::DarkOakLeaves
                ) {
                    return true;
                }
            }
        }
    }
    false
}

/// Approximate trapezoid IntProvider sample on [min, max] (plateau=0 → average of two uniforms).
fn trapezoid_offset(rng: &mut FeatureRandom, min: i32, max: i32) -> i32 {
    let a = min + rng.next_int(max - min + 1);
    let b = min + rng.next_int(max - min + 1);
    (a + b) / 2
}

fn place_oak_tree(rng: &mut FeatureRandom, region: &mut RegionBuf, x: i32, y: i32, z: i32) {
    // straight_trunk_placer base_height=4, height_rand_a=2 → 4..6
    let height = 4 + rng.next_int(3);
    for dy in 0..height {
        let yy = y + dy;
        if !matches!(
            region.get(x, yy, z),
            BlockId::Air
                | BlockId::OakLeaves
                | BlockId::DarkOakLeaves
                | BlockId::ShortGrass
                | BlockId::LeafLitter
        ) {
            return;
        }
        region.set(x, yy, z, BlockId::OakLog);
    }
    // blob_foliage radius=2, height=3, offset=0
    let top = y + height - 1;
    for dy in -2i32..=1 {
        for dz in -2i32..=2 {
            for dx in -2i32..=2 {
                // corner thinning like blob placer
                if dx.abs() == 2 && dz.abs() == 2 && rng.next_f32() < 0.5 {
                    continue;
                }
                let lx = x + dx;
                let ly = top + dy;
                let lz = z + dz;
                if matches!(
                    region.get(lx, ly, lz),
                    BlockId::Air | BlockId::ShortGrass | BlockId::LeafLitter
                ) {
                    region.set(lx, ly, lz, BlockId::OakLeaves);
                }
            }
        }
    }
    // place_on_ground leaf_litter decorator (sparse)
    for _ in 0..12 {
        let ox = rng.next_int(9) - 4;
        let oz = rng.next_int(9) - 4;
        let lx = x + ox;
        let lz = z + oz;
        if let Some(sy) = surface_y(region, lx, lz) {
            let ly = sy + 1;
            if region.get(lx, ly, lz) == BlockId::Air
                && region.get(lx, sy, lz) == BlockId::GrassBlock
            {
                region.set(lx, ly, lz, BlockId::LeafLitter);
            }
        }
    }
}

fn place_dark_oak_tree(rng: &mut FeatureRandom, region: &mut RegionBuf, x: i32, y: i32, z: i32) {
    // dark_oak_trunk_placer: 2×2 trunk, base_height=6, height_rand_a=2, height_rand_b=1
    let height = 6 + rng.next_int(3) + rng.next_int(2); // ~6..9
    for dy in 0..height {
        let yy = y + dy;
        for (tx, tz) in [(0i32, 0i32), (1, 0), (0, 1), (1, 1)] {
            let bx = x + tx;
            let bz = z + tz;
            if matches!(
                region.get(bx, yy, bz),
                BlockId::Air
                    | BlockId::DarkOakLeaves
                    | BlockId::OakLeaves
                    | BlockId::ShortGrass
                    | BlockId::LeafLitter
            ) {
                region.set(bx, yy, bz, BlockId::DarkOakLog);
            }
        }
    }
    // dark_oak canopy approx: dense top layers
    let top = y + height - 1;
    for dy in -2i32..=2 {
        for dz in -3i32..=3 {
            for dx in -3i32..=3 {
                if dx * dx + dz * dz + dy * dy > 14 && rng.next_f32() < 0.4 {
                    continue;
                }
                // offset into 2×2 trunk center
                let lx = x + dx;
                let ly = top + dy;
                let lz = z + dz;
                if matches!(
                    region.get(lx, ly, lz),
                    BlockId::Air | BlockId::ShortGrass | BlockId::LeafLitter
                ) {
                    region.set(lx, ly, lz, BlockId::DarkOakLeaves);
                }
            }
        }
    }
    // place_on_ground litter under canopy (tries ~96+150 in vanilla — scaled down)
    for _ in 0..40 {
        let ox = rng.next_int(9) - 4;
        let oz = rng.next_int(9) - 4;
        let lx = x + ox;
        let lz = z + oz;
        if let Some(sy) = surface_y(region, lx, lz) {
            let ly = sy + 1;
            if region.get(lx, ly, lz) == BlockId::Air
                && region.get(lx, sy, lz) == BlockId::GrassBlock
            {
                region.set(lx, ly, lz, BlockId::LeafLitter);
            }
        }
    }
}
