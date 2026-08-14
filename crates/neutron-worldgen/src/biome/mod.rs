//! Overworld multi-noise biomes for Minecraft 26.2.
//!
//! Three pieces, matching vanilla's split:
//! - [`params`] — packed climate parameter points (`OverworldBiomeBuilder`)
//! - [`source`] — `Climate.Sampler` + nearest-point search
//! - [`manager`] — `BiomeManager` 4-block voronoi fuzz + SHA-256 seed mix
//!
//! Public module aliases (`biome_source`, `biome_manager`, `biome_params`)
//! stay in `lib.rs` so existing `crate::biome_source::…` paths keep compiling.

pub mod manager;
pub mod params;
pub mod source;
