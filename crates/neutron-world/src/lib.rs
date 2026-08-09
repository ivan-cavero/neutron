// Copyright (c) 2026 Neutron Contributors — MIT License
//
// neutron-world: Minecraft world storage for the Neutron server.
//
// Handles reading and writing of:
// - Anvil `.mca` region files (32x32 chunk regions)
// - `level.dat` (gzip-compressed NBT world metadata)
// - Vanilla directory structure (world/, world_nether/, world_the_end/)
// - `session.lock` (file-based PID lock for single-instance access)

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
