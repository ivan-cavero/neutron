//! Multi-noise biome source matching vanilla 26.2 `Climate.Sampler`.
//!
//! Uses quantized i64 arithmetic: `coord → (coord as f32 * 10000.0) as i64`.
//! Nearest-point search walks the packed table in [`crate::biome::params`].
//!
//! Copyright (c) 2026 Neutron Contributors -- MIT License

use crate::density::DensityEnv;

/// Quantize a climate coordinate: `(coord * 10000.0f32) as i64`.
///
/// This matches Java's `(long)(coord * 10000.0f)` exactly (truncation toward zero).
#[inline]
pub fn quantize_coord(coord: f64) -> i64 {
    let f = coord as f32;
    (f * 10000.0f32) as i64
}

/// Distance from a target value to an interval [min, max]:
/// `max(0, min - target, target - max)`
#[inline]
fn parameter_distance(min: i64, max: i64, target: i64) -> i64 {
    let above = target - max;
    if above > 0 {
        return above;
    }
    let below = min - target;
    below.max(0)
}

/// Squared distance from a point to a parameter interval (6 dimensions).
#[inline]
fn fitness(intervals: &[i64; 12], target: &[i64; 6]) -> i64 {
    let mut dist = 0i64;
    for i in 0..6 {
        let d = parameter_distance(intervals[i * 2], intervals[i * 2 + 1], target[i]);
        dist += d * d;
    }
    dist
}

/// The 44 known overworld biome IDs.
pub mod biome_id {
    pub const AIR: u8 = 0;
    pub const STONE: u8 = 1;
    pub const DIRT: u8 = 10;
    pub const COBBLESTONE: u8 = 20;
    pub const SAND: u8 = 24;
    pub const WATER: u8 = 50;
    pub const LAVA: u8 = 51;
    pub const PODZOL: u8 = 14;
    pub const TERRACOTTA: u8 = 59;

    // Biome parameter IDs (matching PARAMETER_FILE)
    pub const MUSHROOM_FIELDS: u8 = 29;
    pub const OCEAN: u8 = 0;
    pub const DEEP_OCEAN: u8 = 8;
    pub const FROZEN_OCEAN: u8 = 15;
    pub const DESERT: u8 = 2;
    pub const PLAINS: u8 = 1;
    pub const FOREST: u8 = 3;
    pub const TAIGA: u8 = 4;
    pub const SWAMP: u8 = 5;
    pub const RIVER: u8 = 6;
    pub const FROZEN_RIVER: u8 = 16;
    pub const BEACH: u8 = 7;
    pub const STONY_SHORE: u8 = 13;
    pub const SAVANNA: u8 = 11;
    pub const JUNGLE: u8 = 10;
    pub const SNOWY_PLAINS: u8 = 9;
    pub const SNOWY_SLOPES: u8 = 22;
    pub const JAGGED_PEAKS: u8 = 23;
    pub const FROZEN_PEAKS: u8 = 24;
    pub const STONY_PEAKS: u8 = 25;
    pub const GROVE: u8 = 21;
    pub const WINDSWEPT_HILLS: u8 = 20;
    pub const DARK_FOREST: u8 = 12;
    pub const MEADOW: u8 = 14;
    pub const ICE_SPIKES: u8 = 17;
    pub const OLD_GROWTH_PINE_FOREST: u8 = 19;
    pub const OLD_GROWTH_BIRCH_FOREST: u8 = 18;
    pub const BIRCH_FOREST: u8 = 33;
    pub const CHERRY_GROVE: u8 = 30;
    pub const BADLANDS: u8 = 26;
    pub const ERODED_BADLANDS: u8 = 27;
    pub const WOODED_BADLANDS: u8 = 28;
    pub const DRIPSTONE_CAVES: u8 = 35;
    pub const MANGROVE_SWAMP: u8 = 32;
    pub const DEEP_DARK: u8 = 31;
    /// Unique id — must not collide with OCEAN (0). Point 7591 in BIOME-SPEC.
    pub const LUSH_CAVES: u8 = 34;
    /// Unique id — point 7592 in BIOME-SPEC.
    pub const SULFUR_CAVES: u8 = 36;

    // Ocean variants (previously collapsed into OCEAN).
    pub const DEEP_FROZEN_OCEAN: u8 = 37;
    pub const DEEP_COLD_OCEAN: u8 = 38;
    pub const COLD_OCEAN: u8 = 39;
    pub const DEEP_LUKEWARM_OCEAN: u8 = 40;
    pub const LUKEWARM_OCEAN: u8 = 41;
    pub const WARM_OCEAN: u8 = 42;
    // Land biomes previously merged or dropped by the lossy pack.
    pub const SNOWY_BEACH: u8 = 43;
    pub const WINDSWEPT_FOREST: u8 = 44;
    pub const WINDSWEPT_GRAVELLY_HILLS: u8 = 45;
    pub const WINDSWEPT_SAVANNA: u8 = 46;
    pub const SAVANNA_PLATEAU: u8 = 47;
    pub const SPARSE_JUNGLE: u8 = 48;
    pub const BAMBOO_JUNGLE: u8 = 49;
    pub const SUNFLOWER_PLAINS: u8 = 50;
    pub const FLOWER_FOREST: u8 = 51;
    pub const OLD_GROWTH_SPRUCE_TAIGA: u8 = 52;
    pub const SNOWY_TAIGA: u8 = 53;
    pub const PALE_GARDEN: u8 = 54;
}

/// Climate target: quantized values for the 6 climate dimensions.
#[derive(Clone, Copy)]
pub struct ClimateTarget {
    pub temperature: i64,
    pub humidity: i64,
    pub continentalness: i64,
    pub erosion: i64,
    pub depth: i64,
    pub weirdness: i64,
}

impl ClimateTarget {
    pub fn from_quantized(
        temperature: i64,
        humidity: i64,
        continentalness: i64,
        erosion: i64,
        depth: i64,
        weirdness: i64,
    ) -> Self {
        Self {
            temperature,
            humidity,
            continentalness,
            erosion,
            depth,
            weirdness,
        }
    }
}

/// `BiomeManager.getBiome(BlockPos)`: voronoi quart, then multi-noise at `quart << 2`.
#[inline]
pub fn biome_id_at_block(state: &crate::worldgen::WorldgenState, x: i32, y: i32, z: i32) -> u8 {
    crate::biome_manager::biome_id_at_block(state, x, y, z)
}

/// Find the biome ID for a climate target by brute-force nearest-point search.
pub fn find_biome(target: &ClimateTarget) -> u8 {
    let target_arr = [
        target.temperature,
        target.humidity,
        target.continentalness,
        target.erosion,
        target.depth,
        target.weirdness,
    ];
    let mut best_fitness = i64::MAX;
    let mut best_biome = biome_id::PLAINS; // default

    for point in crate::biome::params::iter() {
        let f = fitness(&point.intervals, &target_arr);
        if f < best_fitness {
            best_fitness = f;
            best_biome = point.biome;
        }
    }
    best_biome
}

/// Compute the climate target from density functions at block coordinates.
///
/// The6 functions are: temperature, humidity(vegetation), continentalness,
/// erosion, depth, and ridges (peaksAndValleys(ridges) → weirdness).
pub fn climate_at_block(
    env: &mut DensityEnv,
    temp_df: &crate::density::DF,
    hum_df: &crate::density::DF,
    cont_df: &crate::density::DF,
    eros_df: &crate::density::DF,
    depth_df: &crate::density::DF,
    ridge_df: &crate::density::DF,
) -> ClimateTarget {
    let temp_raw = crate::density::compute(temp_df, env);
    let hum_raw = crate::density::compute(hum_df, env);
    let cont_raw = crate::density::compute(cont_df, env);
    let eros_raw = crate::density::compute(eros_df, env);
    let depth_raw = crate::density::compute(depth_df, env);
    let ridge_raw = crate::density::compute(ridge_df, env);
    let t = quantize_coord(temp_raw);
    let h = quantize_coord(hum_raw);
    let c = quantize_coord(cont_raw);
    let e = quantize_coord(eros_raw);
    let d = quantize_coord(depth_raw);
    let w = quantize_coord(peaks_and_valleys(ridge_raw));
    ClimateTarget::from_quantized(t, h, c, e, d, w)
}

/// `peaksAndValleys(weirdness) = -3 * (| |d| - 2/3 | - 1/3)`.
#[inline]
pub fn peaks_and_valleys(d: f64) -> f64 {
    -3.0 * ((d.abs() - 2.0 / 3.0).abs() - 1.0 / 3.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quantize_coord_matches_java() {
        // Java: (long)(-0.93333334f * 10000.0f) = -9333
        assert_eq!(quantize_coord(-0.93333334), -9333);
        assert_eq!(quantize_coord(0.93333334), 9333);
        assert_eq!(quantize_coord(0.0), 0);
        assert_eq!(quantize_coord(1.0), 10000);
    }

    #[test]
    fn parameter_distance_matches() {
        assert_eq!(parameter_distance(0, 100, 50), 0); // inside
        assert_eq!(parameter_distance(0, 100, -10), 10); // below
        assert_eq!(parameter_distance(0, 100, 110), 10); // above
    }
}
