//! Anvil / vanilla world storage.
//!
//! Reads and writes `.mca` regions, gzip `level.dat`, `session.lock`, and the
//! `world/` / `world_nether/` / `world_the_end/` directory layout.
//! Not yet wired into `neutron-server` (the join path caches encoded chunks
//! in memory only).
//!
//! Copyright (c) 2026 Neutron Contributors — MIT License

#![forbid(unsafe_code)]

pub mod error;
pub mod level;
pub mod nbt;
pub mod region;
pub mod session;
pub mod world;

// Re-exports for convenience.
pub use error::{WorldError, WorldResult};
pub use level::{Difficulty, GameMode, LevelDat};
pub use region::{parse_region_filename, region_path, Region, CHUNKS_PER_REGION, SECTOR_SIZE};
pub use session::SessionLock;
pub use world::{Dimension, World};
