//! # neutron-protocol
//!
//! Minecraft 26.2 protocol implementation for the Neutron server.
//!
//! This crate provides:
//! - Packet type definitions for Login and Play protocol states
//! - A length-delimited framing codec with optional zlib compression
//! - VarInt/VarLong encoding/decoding
//! - Common protocol types (BlockPos, Slot, Chat, etc.)
//!
//! # Design Principles
//!
//! - `#![forbid(unsafe_code)]` — no unsafe code anywhere
//! - Error handling via `thiserror` — no `unwrap()` or `expect()` in library code
//! - Zero-copy where possible using `bytes::Bytes`
//! - Packet IDs are version-specific constants (26.2 for now)

#![forbid(unsafe_code)]

pub mod codec;
pub mod error;
pub mod login;
pub mod packet;
pub mod play;
pub mod types;

// Re-exports for convenience
pub use codec::MinecraftCodec;
pub use error::{DecodeError, EncodeError, ProtocolError};
pub use packet::{Direction, ProtocolState, RawPacket};
pub use types::{
    read_varint, write_varint, Angle, BlockPos, Chat, GameMode, Slot, SlotData, Vec3d, Vec3f,
};
