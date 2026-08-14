// Copyright (c) 2026 Neutron Contributors -- MIT License
//
// neutron-worldgen: Minecraft 26.2 world generation.
//
// Produces chunks that are identical to vanilla Minecraft when given the same seed.
// Verification is done against ground-truth dumps from `tools/java-probe`.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod aquifer;
pub mod biome_manager;
pub mod biome_params;
pub mod biome_source;
pub mod carvers;
pub mod datapack_data;
pub mod datapack_fs;
pub mod density;
pub mod feature_catalog;
pub mod feature_dispatch;
pub mod feature_rng;
pub mod features;
pub mod generator;
pub mod legacy_rng;
pub mod mineshaft;
pub mod multiface_spreader;
pub mod noise;
pub mod ore_vein;
pub mod positional;
pub mod region_buf;
pub mod rng;
pub mod sculk;
pub mod surface;
pub mod surface_rules;
pub mod tree;
pub mod vegetation;
pub mod worldgen;

pub use biome_source::{find_biome, quantize_coord, ClimateTarget};
pub use density::{DensityEnv, DensityRegistry};
pub use generator::{ChunkGenerator, GeneratedChunk, NoiseCache, NoiseColumn};
pub use noise::{BlendedNoise, ImprovedNoise, NormalNoise, PerlinNoise};
pub use rng::Xoroshiro128;
pub use surface::BlockId;
pub use worldgen::{NoiseSet, Router, WorldgenState};
