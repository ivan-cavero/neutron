// Copyright (c) 2026 Neutron Contributors — MIT License
//
// WorldError types for neutron-world.

use std::path::PathBuf;

/// Errors that can occur during world storage operations.
#[derive(Debug, thiserror::Error)]
pub enum WorldError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("NBT error: {0}")]
    Nbt(String),

    #[error("invalid region file: {reason}")]
    InvalidRegion { reason: String },

    #[error("chunk ({cx}, {cz}) not found in region")]
    ChunkNotFound { cx: i32, cz: i32 },

    #[error("invalid chunk coordinates ({cx}, {cz}) for region")]
    InvalidChunkCoords { cx: i32, cz: i32 },

    #[error("invalid compression type: {0}")]
    InvalidCompression(u8),

    #[error("invalid region offset: {0}")]
    InvalidOffset(u32),

    #[error("chunk too large: {size} bytes (max {max})")]
    ChunkTooLarge { size: usize, max: usize },

    #[error("session lock held by PID {pid} at {path}")]
    SessionLockHeld { pid: u32, path: PathBuf },

    #[error("session lock file corrupted: {path}")]
    SessionLockCorrupted { path: PathBuf },

    #[error("level.dat missing field: {field}")]
    MissingField { field: String },

    #[error("level.dat parse error: {0}")]
    LevelDatParse(String),

    #[error("world directory not found: {0}")]
    WorldNotFound(PathBuf),

    #[error("invalid world structure: {reason}")]
    InvalidWorld { reason: String },
}

/// Result type alias for world operations.
pub type WorldResult<T> = Result<T, WorldError>;
