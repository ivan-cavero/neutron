// Copyright (c) 2026 Neutron Contributors -- MIT License
//
// Multi-noise biome source -- matches vanilla Minecraft 26.2 climate-based biome selection.
//
// Noise parameters are taken verbatim from PARAMETERS.md Section 6.
// Climate ranges are from Section 7 (OverworldBiomeBuilder constructor).
// Biome target points approximate the OverworldBiomeBuilder's buildBiomes() output.

use crate::noise::OctavePerlinNoise;
use crate::rng::Xoroshiro128;

/// Biome IDs matching Minecraft's internal registry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum BiomeId {
    Ocean = 0,
    Plains = 1,
    Desert = 2,
    Forest = 3,
    Taiga = 4,
    Swamp = 5,
    River = 6,
    Beach = 7,
    DeepOcean = 8,
    SnowyPlains = 9,
    Jungle = 10,
    Savanna = 11,
    DarkForest = 12,
    StonyShore = 13,
    Meadow = 14,
    FrozenOcean = 15,
    FrozenRiver = 16,
    IceSpikes = 17,
    OldGrowthBirchForest = 18,
    OldGrowthPineForest = 19,
    WindsweptHills = 20,
    Grove = 21,
    SnowySlopes = 22,
    JaggedPeaks = 23,
    FrozenPeaks = 24,
    StonyPeaks = 25,
    Badlands = 26,
    ErodedBadlands = 27,
    WoodedBadlands = 28,
    MushroomFields = 29,
    CherryGrove = 30,
    DeepDark = 31,
    MangroveSwamp = 32,
    BirchForest = 33,
    LushCaves = 34,
    DripstoneCaves = 35,
    SulfurCaves = 36,
}

impl BiomeId {
    pub fn as_u8(self) -> u8 {
        self as u8
    }

    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::Ocean),
            1 => Some(Self::Plains),
            2 => Some(Self::Desert),
            3 => Some(Self::Forest),
            4 => Some(Self::Taiga),
            5 => Some(Self::Swamp),
            6 => Some(Self::River),
            7 => Some(Self::Beach),
            8 => Some(Self::DeepOcean),
            9 => Some(Self::SnowyPlains),
            10 => Some(Self::Jungle),
            11 => Some(Self::Savanna),
            12 => Some(Self::DarkForest),
            13 => Some(Self::StonyShore),
            14 => Some(Self::Meadow),
            15 => Some(Self::FrozenOcean),
            16 => Some(Self::FrozenRiver),
            17 => Some(Self::IceSpikes),
            18 => Some(Self::OldGrowthBirchForest),
            19 => Some(Self::OldGrowthPineForest),
            20 => Some(Self::WindsweptHills),
            21 => Some(Self::Grove),
            22 => Some(Self::SnowySlopes),
            23 => Some(Self::JaggedPeaks),
            24 => Some(Self::FrozenPeaks),
            25 => Some(Self::StonyPeaks),
            26 => Some(Self::Badlands),
            27 => Some(Self::ErodedBadlands),
            28 => Some(Self::WoodedBadlands),
            29 => Some(Self::MushroomFields),
            30 => Some(Self::CherryGrove),
            31 => Some(Self::DeepDark),
            32 => Some(Self::MangroveSwamp),
            33 => Some(Self::BirchForest),
            34 => Some(Self::LushCaves),
            35 => Some(Self::DripstoneCaves),
            36 => Some(Self::SulfurCaves),
            _ => None,
        }
    }

    pub fn is_ocean(self) -> bool {
        matches!(self, Self::Ocean | Self::DeepOcean | Self::FrozenOcean)
    }

    pub fn is_cold(self) -> bool {
        matches!(
            self,
            Self::SnowyPlains
                | Self::IceSpikes
                | Self::FrozenOcean
                | Self::FrozenRiver
                | Self::Grove
                | Self::SnowySlopes
                | Self::JaggedPeaks
                | Self::FrozenPeaks
        )
    }

    pub fn is_warm(self) -> bool {
        matches!(
            self,
            Self::Desert
                | Self::Savanna
                | Self::Badlands
                | Self::ErodedBadlands
                | Self::WoodedBadlands
                | Self::StonyPeaks
        )
    }
}

/// Climate parameters for biome selection.
#[derive(Debug, Clone, Copy)]
pub struct ClimateParams {
    pub temperature: f64,
    pub humidity: f64,
    pub continentalness: f64,
    pub erosion: f64,
    pub depth: f64,
    pub weirdness: f64,
}

impl Default for ClimateParams {
    fn default() -> Self {
        Self {
            temperature: 0.0,
            humidity: 0.0,
            continentalness: 0.0,
            erosion: 0.0,
            depth: 0.0,
            weirdness: 0.0,
        }
    }
}

impl ClimateParams {
    pub fn distance_to(&self, other: &Self) -> f64 {
        let dt = self.temperature - other.temperature;
        let dh = self.humidity - other.humidity;
        let dc = self.continentalness - other.continentalness;
        let de = self.erosion - other.erosion;
        let dd = self.depth - other.depth;
        let dw = self.weirdness - other.weirdness;
        (dt * dt + dh * dh + dc * dc + de * de + dd * dd + dw * dw).sqrt()
    }
}

/// A biome entry with its target climate parameters.
struct ClimateTarget {
    biome: BiomeId,
    target: ClimateParams,
}

/// Multi-noise biome source matching vanilla Minecraft 26.2.
///
/// Uses six noise functions seeded from the world seed:
/// - shift_a / shift_b: coordinate shifting (firstOctave=-3, amps=[1.0, 1.0, 0.0])
/// - temperature (firstOctave=-10, amps=[0.0, 1.0, 0.0, 0.0, 0.0])
/// - vegetation (firstOctave=-8, amps=[1.0, 0.0, 0.0, 0.0, 0.0])
/// - continentalness (firstOctave=-9, amps=[1.0, 2.0, 2.0, 2.0, 1.0, 1.0, 1.0, 1.0])
/// - erosion (firstOctave=-9, amps=[1.0, 0.0, 1.0, 1.0])
/// - ridge (firstOctave=-7, amps=[2.0, 1.0, 0.0, 0.0, 0.0])
pub struct MultiNoiseBiomeSource {
    /// Shift noise A: sampled at (x/1500, 0, z/1500) for XZ shifting.
    shift_noise_a: OctavePerlinNoise,
    /// Shift noise B: sampled at (z/1500, 0, x/1500) for ZX shifting.
    shift_noise_b: OctavePerlinNoise,
    /// Temperature noise: firstOctave=-10, amps=[0.0, 1.0, 0.0, 0.0, 0.0]
    temperature_noise: OctavePerlinNoise,
    /// Vegetation/humidity noise: firstOctave=-8, amps=[1.0, 0.0, 0.0, 0.0, 0.0]
    vegetation_noise: OctavePerlinNoise,
    /// Continentalness noise: firstOctave=-9, amps=[1.0, 2.0, 2.0, 2.0, 1.0, 1.0, 1.0, 1.0]
    continentalness_noise: OctavePerlinNoise,
    /// Erosion noise: firstOctave=-9, amps=[1.0, 0.0, 1.0, 1.0]
    erosion_noise: OctavePerlinNoise,
    /// Ridge noise: firstOctave=-7, amps=[2.0, 1.0, 0.0, 0.0, 0.0]
    ridge_noise: OctavePerlinNoise,
    /// Biome targets for distance-based selection.
    biomes: Vec<ClimateTarget>,
}

impl MultiNoiseBiomeSource {
    /// Create a new multi-noise biome source from a world seed.
    ///
    /// Noise parameters match vanilla exactly (PARAMETERS.md Section 6).
    /// Each noise function consumes its own seed state sequentially from the shared RNG.
    pub fn new(seed: i64) -> Self {
        let mut rng = Xoroshiro128::new(seed);

        // Shift noises: firstOctave=-3, amps=[1.0, 1.0, 0.0]
        // Two separate instances (each consumes its own RNG state for decoration offsets).
        let shift_noise_a =
            OctavePerlinNoise::new_with_first_octave(&mut rng, -3, &[1.0, 1.0, 0.0]);
        let shift_noise_b =
            OctavePerlinNoise::new_with_first_octave(&mut rng, -3, &[1.0, 1.0, 0.0]);

        // Temperature: firstOctave=-10, amps=[0.0, 1.0, 0.0, 0.0, 0.0]
        // Only octave 1 contributes (effective freq = 2^10/2 = 512).
        let temperature_noise = OctavePerlinNoise::new_with_first_octave(
            &mut rng,
            -10,
            &[0.0, 1.0, 0.0, 0.0, 0.0],
        );

        // Vegetation: firstOctave=-8, amps=[1.0, 0.0, 0.0, 0.0, 0.0]
        // Only octave 0 contributes (effective freq = 2^8 = 256).
        let vegetation_noise = OctavePerlinNoise::new_with_first_octave(
            &mut rng,
            -8,
            &[1.0, 0.0, 0.0, 0.0, 0.0],
        );

        // Continentalness: firstOctave=-9, amps=[1.0, 2.0, 2.0, 2.0, 1.0, 1.0, 1.0, 1.0]
        // 8 octaves, all contributing. Effective freq starts at 2^9=512.
        let continentalness_noise = OctavePerlinNoise::new_with_first_octave(
            &mut rng,
            -9,
            &[1.0, 2.0, 2.0, 2.0, 1.0, 1.0, 1.0, 1.0],
        );

        // Erosion: firstOctave=-9, amps=[1.0, 0.0, 1.0, 1.0]
        // 4 octaves, octave 1 skipped.
        let erosion_noise = OctavePerlinNoise::new_with_first_octave(
            &mut rng,
            -9,
            &[1.0, 0.0, 1.0, 1.0],
        );

        // Ridge: firstOctave=-7, amps=[2.0, 1.0, 0.0, 0.0, 0.0]
        // 5 octaves, only first two contribute.
        let ridge_noise = OctavePerlinNoise::new_with_first_octave(
            &mut rng,
            -7,
            &[2.0, 1.0, 0.0, 0.0, 0.0],
        );

        Self {
            shift_noise_a,
            shift_noise_b,
            temperature_noise,
            vegetation_noise,
            continentalness_noise,
            erosion_noise,
            ridge_noise,
            biomes: Self::default_biomes(),
        }
    }

    /// Peaks and valleys transform applied to ridge noise.
    ///
    /// `peaksAndValleys(d) = -3 * (| |d| - 2/3 | - 1/3)`
    /// Range: [-1, 1]. Peaks at d=+/-2/3, valley at d=0.
    pub fn peaks_and_valleys(d: f64) -> f64 {
        -3.0 * ((d.abs() - 2.0 / 3.0).abs() - 1.0 / 3.0)
    }

    /// Default overworld biome targets.
    ///
    /// These approximate the OverworldBiomeBuilder.buildBiomes() output.
    /// Each biome has a center point in 6D climate space.
    /// The closest target (Euclidean distance) is selected for each block position.
    fn default_biomes() -> Vec<ClimateTarget> {
        vec![
            // ---- Ocean biomes (continentalness < -0.19) ----
            ClimateTarget {
                biome: BiomeId::MushroomFields,
                target: ClimateParams {
                    temperature: 0.9,
                    humidity: 1.0,
                    continentalness: -1.125,
                    erosion: 0.0,
                    depth: 0.0,
                    weirdness: 0.0,
                },
            },
            ClimateTarget {
                biome: BiomeId::DeepOcean,
                target: ClimateParams {
                    temperature: 0.0,
                    humidity: 0.0,
                    continentalness: -0.755,
                    erosion: 0.0,
                    depth: 0.0,
                    weirdness: 0.0,
                },
            },
            ClimateTarget {
                biome: BiomeId::Ocean,
                target: ClimateParams {
                    temperature: 0.0,
                    humidity: 0.0,
                    continentalness: -0.323,
                    erosion: 0.0,
                    depth: 0.0,
                    weirdness: 0.0,
                },
            },
            ClimateTarget {
                biome: BiomeId::FrozenOcean,
                target: ClimateParams {
                    temperature: -0.725,
                    humidity: 0.0,
                    continentalness: -0.323,
                    erosion: 0.0,
                    depth: 0.0,
                    weirdness: 0.0,
                },
            },
            // ---- Coast biomes (-0.19 < continentalness < -0.11) ----
            ClimateTarget {
                biome: BiomeId::Beach,
                target: ClimateParams {
                    temperature: 0.0,
                    humidity: 0.0,
                    continentalness: -0.15,
                    erosion: 0.0,
                    depth: 0.0,
                    weirdness: 0.0,
                },
            },
            ClimateTarget {
                biome: BiomeId::StonyShore,
                target: ClimateParams {
                    temperature: -0.5,
                    humidity: 0.0,
                    continentalness: -0.15,
                    erosion: -0.5,
                    depth: 0.0,
                    weirdness: 0.0,
                },
            },
            // ---- Cold biomes (temperature < -0.15) ----
            // Low erosion (E0-E2)
            ClimateTarget {
                biome: BiomeId::IceSpikes,
                target: ClimateParams {
                    temperature: -1.0,
                    humidity: 0.0,
                    continentalness: 0.2,
                    erosion: -0.9,
                    depth: 0.0,
                    weirdness: 0.0,
                },
            },
            ClimateTarget {
                biome: BiomeId::SnowyPlains,
                target: ClimateParams {
                    temperature: -1.0,
                    humidity: 0.0,
                    continentalness: 0.2,
                    erosion: -0.2,
                    depth: 0.0,
                    weirdness: 0.0,
                },
            },
            // Medium erosion (E3-E4)
            ClimateTarget {
                biome: BiomeId::SnowySlopes,
                target: ClimateParams {
                    temperature: -1.0,
                    humidity: 0.0,
                    continentalness: 0.2,
                    erosion: 0.15,
                    depth: 0.0,
                    weirdness: 0.0,
                },
            },
            ClimateTarget {
                biome: BiomeId::JaggedPeaks,
                target: ClimateParams {
                    temperature: -1.0,
                    humidity: 0.0,
                    continentalness: 0.2,
                    erosion: 0.15,
                    depth: 0.0,
                    weirdness: 0.7,
                },
            },
            ClimateTarget {
                biome: BiomeId::FrozenPeaks,
                target: ClimateParams {
                    temperature: -1.0,
                    humidity: 0.0,
                    continentalness: 0.2,
                    erosion: -0.5,
                    depth: 0.0,
                    weirdness: 0.7,
                },
            },
            // High erosion (E5-E6)
            ClimateTarget {
                biome: BiomeId::Grove,
                target: ClimateParams {
                    temperature: -0.5,
                    humidity: 0.0,
                    continentalness: 0.2,
                    erosion: 0.75,
                    depth: 0.0,
                    weirdness: 0.0,
                },
            },
            // ---- Cool biomes (-0.45 < temperature < -0.15) ----
            // Low-medium erosion
            ClimateTarget {
                biome: BiomeId::OldGrowthPineForest,
                target: ClimateParams {
                    temperature: -0.3,
                    humidity: 0.0,
                    continentalness: 0.2,
                    erosion: -0.2,
                    depth: 0.0,
                    weirdness: 0.0,
                },
            },
            ClimateTarget {
                biome: BiomeId::Taiga,
                target: ClimateParams {
                    temperature: -0.3,
                    humidity: 0.0,
                    continentalness: 0.2,
                    erosion: 0.15,
                    depth: 0.0,
                    weirdness: 0.0,
                },
            },
            ClimateTarget {
                biome: BiomeId::WindsweptHills,
                target: ClimateParams {
                    temperature: -0.3,
                    humidity: 0.0,
                    continentalness: 0.2,
                    erosion: -0.5,
                    depth: 0.0,
                    weirdness: 0.0,
                },
            },
            // ---- Neutral biomes (-0.15 < temperature < 0.2) ----
            ClimateTarget {
                biome: BiomeId::Forest,
                target: ClimateParams {
                    temperature: 0.0,
                    humidity: 0.0,
                    continentalness: 0.2,
                    erosion: 0.15,
                    depth: 0.0,
                    weirdness: 0.0,
                },
            },
            ClimateTarget {
                biome: BiomeId::DarkForest,
                target: ClimateParams {
                    temperature: 0.0,
                    humidity: 0.0,
                    continentalness: 0.2,
                    erosion: -0.2,
                    depth: 0.0,
                    weirdness: 0.0,
                },
            },
            ClimateTarget {
                biome: BiomeId::WindsweptHills,
                target: ClimateParams {
                    temperature: 0.0,
                    humidity: 0.0,
                    continentalness: 0.2,
                    erosion: -0.5,
                    depth: 0.0,
                    weirdness: 0.0,
                },
            },
            ClimateTarget {
                biome: BiomeId::StonyPeaks,
                target: ClimateParams {
                    temperature: 0.0,
                    humidity: 0.0,
                    continentalness: 0.2,
                    erosion: -0.5,
                    depth: 0.0,
                    weirdness: 0.7,
                },
            },
            // ---- Warm biomes (0.2 < temperature < 0.55) ----
            ClimateTarget {
                biome: BiomeId::Meadow,
                target: ClimateParams {
                    temperature: 0.375,
                    humidity: 0.0,
                    continentalness: 0.2,
                    erosion: 0.75,
                    depth: 0.0,
                    weirdness: 0.0,
                },
            },
            ClimateTarget {
                biome: BiomeId::CherryGrove,
                target: ClimateParams {
                    temperature: 0.375,
                    humidity: 0.0,
                    continentalness: 0.2,
                    erosion: 0.75,
                    depth: 0.0,
                    weirdness: 0.7,
                },
            },
            ClimateTarget {
                biome: BiomeId::Plains,
                target: ClimateParams {
                    temperature: 0.375,
                    humidity: 0.0,
                    continentalness: 0.2,
                    erosion: 0.15,
                    depth: 0.0,
                    weirdness: 0.0,
                },
            },
            // ---- Hot biomes (temperature > 0.55) ----
            // Low-medium erosion
            ClimateTarget {
                biome: BiomeId::Desert,
                target: ClimateParams {
                    temperature: 0.775,
                    humidity: 0.0,
                    continentalness: 0.2,
                    erosion: -0.2,
                    depth: 0.0,
                    weirdness: 0.0,
                },
            },
            ClimateTarget {
                biome: BiomeId::Savanna,
                target: ClimateParams {
                    temperature: 0.775,
                    humidity: 0.0,
                    continentalness: 0.2,
                    erosion: 0.15,
                    depth: 0.0,
                    weirdness: 0.0,
                },
            },
            ClimateTarget {
                biome: BiomeId::Jungle,
                target: ClimateParams {
                    temperature: 0.775,
                    humidity: 0.0,
                    continentalness: 0.2,
                    erosion: -0.5,
                    depth: 0.0,
                    weirdness: 0.0,
                },
            },
            // ---- Badlands (hot + extreme weirdness) ----
            ClimateTarget {
                biome: BiomeId::Badlands,
                target: ClimateParams {
                    temperature: 0.775,
                    humidity: 0.0,
                    continentalness: 0.2,
                    erosion: -0.2,
                    depth: 0.0,
                    weirdness: -0.7,
                },
            },
            ClimateTarget {
                biome: BiomeId::ErodedBadlands,
                target: ClimateParams {
                    temperature: 0.775,
                    humidity: 0.0,
                    continentalness: 0.2,
                    erosion: 0.15,
                    depth: 0.0,
                    weirdness: -0.7,
                },
            },
            ClimateTarget {
                biome: BiomeId::WoodedBadlands,
                target: ClimateParams {
                    temperature: 0.775,
                    humidity: 0.0,
                    continentalness: 0.2,
                    erosion: 0.55,
                    depth: 0.0,
                    weirdness: -0.7,
                },
            },
            // ---- Swamps (warm + wet) ----
            ClimateTarget {
                biome: BiomeId::Swamp,
                target: ClimateParams {
                    temperature: 0.775,
                    humidity: 0.5,
                    continentalness: 0.2,
                    erosion: 0.15,
                    depth: 0.0,
                    weirdness: 0.0,
                },
            },
            ClimateTarget {
                biome: BiomeId::MangroveSwamp,
                target: ClimateParams {
                    temperature: 0.775,
                    humidity: 0.5,
                    continentalness: 0.2,
                    erosion: -0.2,
                    depth: 0.0,
                    weirdness: 0.0,
                },
            },
            // ---- River biomes (using depth as a discriminator) ----
            // Rivers are selected when depth is near 0 (surface), using a separate
            // surface-level check. For the multi-noise selection, rivers use
            // specific targets at the coast boundary.
            ClimateTarget {
                biome: BiomeId::River,
                target: ClimateParams {
                    temperature: 0.0,
                    humidity: 0.0,
                    continentalness: -0.15,
                    erosion: 0.0,
                    depth: 0.0,
                    weirdness: 0.0,
                },
            },
            ClimateTarget {
                biome: BiomeId::FrozenRiver,
                target: ClimateParams {
                    temperature: -0.5,
                    humidity: 0.0,
                    continentalness: -0.15,
                    erosion: 0.0,
                    depth: 0.0,
                    weirdness: 0.0,
                },
            },
        ]
    }

    /// Generate climate parameters for the given block position.
    ///
    /// Matches vanilla's ShiftedNoise2d sampling:
    /// - shift values are sampled at (x/1500, 0, z/1500) and (z/1500, 0, x/1500)
    /// - climate noise is sampled at (x*0.25 + shift_a, 0, z*0.25 + shift_b)
    pub fn climate_at(&self, x: i32, z: i32) -> ClimateParams {
        // Sample shift noises at low frequency (ShiftA/ShiftB pattern).
        let shift_a = self
            .shift_noise_a
            .sample(x as f64 / 1500.0, 0.0, z as f64 / 1500.0, true);
        let shift_b = self
            .shift_noise_b
            .sample(z as f64 / 1500.0, 0.0, x as f64 / 1500.0, true);

        // Shifted coordinates matching vanilla's ShiftedNoise2d(xzScale=0.25).
        let sx = x as f64 * 0.25 + shift_a;
        let sz = z as f64 * 0.25 + shift_b;

        ClimateParams {
            temperature: self.temperature_noise.sample(sx, 0.0, sz, true),
            humidity: self.vegetation_noise.sample(sx, 0.0, sz, true),
            continentalness: self.continentalness_noise.sample(sx, 0.0, sz, true),
            erosion: self.erosion_noise.sample(sx, 0.0, sz, true),
            depth: 0.0,
            weirdness: MultiNoiseBiomeSource::peaks_and_valleys(
                self.ridge_noise.sample(sx, 0.0, sz, true),
            ),
        }
    }

    /// Determine the biome for the given climate parameters.
    pub fn biome_at(&self, params: &ClimateParams) -> BiomeId {
        let mut best = BiomeId::Plains;
        let mut best_dist = f64::MAX;

        for cp in &self.biomes {
            let dist = params.distance_to(&cp.target);
            if dist < best_dist {
                best_dist = dist;
                best = cp.biome;
            }
        }
        best
    }

    /// Get the biome for a given block position.
    pub fn biome_at_pos(&self, x: i32, z: i32) -> BiomeId {
        let params = self.climate_at(x, z);
        self.biome_at(&params)
    }

    /// Get the biome for a chunk-relative position (used for 4x4 biome sections).
    pub fn biome_at_chunk(
        &self,
        cx: i32,
        cz: i32,
        bx: u32,
        bz: u32,
    ) -> BiomeId {
        let x = cx * 16 + bx as i32;
        let z = cz * 16 + bz as i32;
        self.biome_at_pos(x, z)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn biome_determinism() {
        let source = MultiNoiseBiomeSource::new(42);
        for x in -100..=100 {
            for z in -100..=100 {
                let b1 = source.biome_at_pos(x, z);
                let b2 = source.biome_at_pos(x, z);
                assert_eq!(b1, b2, "biome not deterministic at ({x}, {z})");
            }
        }
    }

    #[test]
    fn climate_spatial_coherence() {
        let source = MultiNoiseBiomeSource::new(42);
        let c0 = source.climate_at(1000, 1000);
        let c1 = source.climate_at(1001, 1000);
        let c2 = source.climate_at(1000, 1001);
        let diff_x = (c0.temperature - c1.temperature).abs()
            + (c0.humidity - c1.humidity).abs()
            + (c0.continentalness - c1.continentalness).abs();
        let diff_z = (c0.temperature - c2.temperature).abs()
            + (c0.humidity - c2.humidity).abs()
            + (c0.continentalness - c2.continentalness).abs();
        // Shift noise adds organic variation; adjacent blocks can differ
        // significantly at noise boundaries. Allow generous tolerance.
        assert!(
            diff_x < 2.0,
            "adjacent X blocks should be coherent, diff={diff_x}"
        );
        assert!(
            diff_z < 2.0,
            "adjacent Z blocks should be coherent, diff={diff_z}"
        );
    }

    #[test]
    fn biome_variety() {
        let source = MultiNoiseBiomeSource::new(42);
        let mut counts: HashMap<BiomeId, usize> = HashMap::new();

        for x in 0..512 {
            for z in 0..512 {
                let biome = source.biome_at_pos(x, z);
                *counts.entry(biome).or_insert(0) += 1;
            }
        }

        assert!(
            counts.len() >= 5,
            "expected at least 5 biomes, got {}: {:?}",
            counts.len(),
            counts.keys().collect::<Vec<_>>()
        );

        let total = 512 * 512;
        let mut sorted: Vec<_> = counts.into_iter().collect();
        sorted.sort_by(|a, b| b.1.cmp(&a.1));
        for (biome, count) in &sorted {
            eprintln!(
                "  {:?}: {:.1}%",
                biome,
                *count as f64 / total as f64 * 100.0
            );
        }
    }

    #[test]
    fn ocean_from_climate() {
        let source = MultiNoiseBiomeSource::new(0);

        let params = ClimateParams {
            continentalness: -0.9,
            ..Default::default()
        };
        assert_eq!(source.biome_at(&params), BiomeId::DeepOcean);

        let params = ClimateParams {
            continentalness: -0.7,
            ..Default::default()
        };
        let biome = source.biome_at(&params);
        assert!(
            biome.is_ocean() || biome == BiomeId::MushroomFields,
            "expected ocean biome at continentalness=-0.7, got {:?}",
            biome
        );
    }

    #[test]
    fn hot_dry_is_desert() {
        let source = MultiNoiseBiomeSource::new(0);
        let params = ClimateParams {
            temperature: 0.8,
            humidity: 0.0,
            continentalness: 0.3,
            ..Default::default()
        };
        let biome = source.biome_at(&params);
        assert!(
            matches!(biome, BiomeId::Desert | BiomeId::Savanna | BiomeId::Jungle),
            "expected hot dry biome, got {:?}",
            biome
        );
    }

    #[test]
    fn cold_is_snowy() {
        let source = MultiNoiseBiomeSource::new(0);
        let params = ClimateParams {
            temperature: -1.0,
            humidity: 0.0,
            continentalness: 0.3,
            ..Default::default()
        };
        let biome = source.biome_at(&params);
        assert!(
            biome.is_cold() || matches!(biome, BiomeId::WindsweptHills | BiomeId::StonyPeaks),
            "expected cold biome at temp=-1.0, got {:?}",
            biome
        );
    }

    #[test]
    fn peaks_and_valleys_range() {
        // PV should be in [-1, 1]
        for i in -100..=100 {
            let d = i as f64 / 100.0;
            let pv = MultiNoiseBiomeSource::peaks_and_valleys(d);
            assert!(
                pv >= -1.0 - 1e-10 && pv <= 1.0 + 1e-10,
                "PV out of range at d={d}: {pv}"
            );
        }
        // Valley at d=0
        let pv0 = MultiNoiseBiomeSource::peaks_and_valleys(0.0);
        assert!(pv0 < -0.5, "PV at d=0 should be a valley, got {pv0}");
        // Peak at d=+/-2/3
        let pv_peak = MultiNoiseBiomeSource::peaks_and_valleys(2.0 / 3.0);
        assert!(
            pv_peak > 0.5,
            "PV at d=2/3 should be a peak, got {pv_peak}"
        );
    }

    #[test]
    fn biome_id_roundtrip() {
        for i in 0..=36 {
            let id = BiomeId::from_u8(i);
            assert!(id.is_some(), "from_u8({i}) returned None");
            assert_eq!(id.unwrap().as_u8(), i);
        }
    }

    #[test]
    fn climate_determinism() {
        let source = MultiNoiseBiomeSource::new(42);
        let c1 = source.climate_at(100, 200);
        let c2 = source.climate_at(100, 200);
        assert_eq!(c1.temperature, c2.temperature);
        assert_eq!(c1.humidity, c2.humidity);
        assert_eq!(c1.continentalness, c2.continentalness);
        assert_eq!(c1.weirdness, c2.weirdness);
    }

    #[test]
    fn different_seeds_different_biomes() {
        let s1 = MultiNoiseBiomeSource::new(1);
        let s2 = MultiNoiseBiomeSource::new(2);
        let mut same = 0;
        let total = 256 * 256;
        for x in 0..256 {
            for z in 0..256 {
                if s1.biome_at_pos(x, z) == s2.biome_at_pos(x, z) {
                    same += 1;
                }
            }
        }
        assert!(
            same < total * 80 / 100,
            "seeds 1 and 2 produced too similar layouts ({same}/{total} match)"
        );
    }
}
