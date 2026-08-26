//! Minecraft 26.2 overworld generation.
//!
//! Pipeline: noise / aquifer → surface rules → carvers → structures →
//! placed features. Deterministic for a given seed. Not yet 1:1 with vanilla
//! (see `WORLDGEN.md`); checksums live in `tools/parity-check`.
//!
//! Density nodes are `Arc` so [`ChunkGenerator`] is `Send` and a worker pool
//! is possible. One worker is still enough for a single-player join.
//!
//! Copyright (c) 2026 Neutron Contributors -- MIT License

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod aquifer;
pub mod biome;
pub mod carvers;
pub mod datapack_data;
pub mod datapack_fs;
pub mod deco_util;
pub mod density;
pub mod feature_catalog;
pub mod feature_dispatch;
pub mod feature_ports;
pub mod feature_rng;
pub mod fossil_structures;
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
pub mod worldgen;
pub mod writers;

/// Compatibility alias for [`biome::manager`].
pub use biome::manager as biome_manager;
/// Compatibility alias for [`biome::params`].
pub use biome::params as biome_params;
/// Compatibility alias for [`biome::source`].
pub use biome::source as biome_source;

pub use biome_source::{find_biome, quantize_coord, ClimateTarget};
pub use density::{DensityEnv, DensityRegistry};
pub use generator::{ChunkGenerator, GeneratedChunk, NoiseCache, NoiseColumn};
pub use noise::{BlendedNoise, ImprovedNoise, NormalNoise, PerlinNoise};
pub use rng::Xoroshiro128;
pub use surface::BlockId;
pub use worldgen::{NoiseSet, Router, WorldgenState};
