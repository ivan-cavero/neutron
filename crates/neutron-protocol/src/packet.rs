//! Packet ID trait and registry for the Minecraft protocol.
//!
//! Packets are identified by a numeric ID that varies by protocol state
//! (Handshake, Login, Status, Play) and direction (Clientbound, Serverbound).

use bytes::{Buf, BufMut, Bytes, BytesMut};

use crate::error::{DecodeError, EncodeError};
use crate::types::read_varint;

// ---------------------------------------------------------------------------
// Protocol State
// ---------------------------------------------------------------------------

/// The protocol state determines which packets are valid.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ProtocolState {
    /// Initial state: client sends Handshake to transition.
    Handshake,
    /// Login sequence (encryption, compression, success).
    Login,
    /// Server list ping.
    Status,
    /// Main gameplay.
    Play,
}

impl ProtocolState {
    /// Return the state name for error messages.
    pub fn name(self) -> &'static str {
        match self {
            ProtocolState::Handshake => "Handshake",
            ProtocolState::Login => "Login",
            ProtocolState::Status => "Status",
            ProtocolState::Play => "Play",
        }
    }
}

// ---------------------------------------------------------------------------
// Direction
// ---------------------------------------------------------------------------

/// Packet direction relative to the server.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Direction {
    /// Sent from server to client.
    Clientbound,
    /// Sent from client to server.
    Serverbound,
}

// ---------------------------------------------------------------------------
// PacketId trait
// ---------------------------------------------------------------------------

/// Trait for types that represent a specific packet.
///
/// Each implementor knows its protocol state, direction, and numeric ID.
pub trait PacketId {
    /// The protocol state this packet belongs to.
    const STATE: ProtocolState;
    /// The direction this packet travels.
    const DIRECTION: Direction;
    /// The numeric packet ID (version-specific).
    const ID: u32;
}

// ---------------------------------------------------------------------------
// Packet registry
// ---------------------------------------------------------------------------

/// Registry mapping packet IDs to their names (for debugging/logging).
pub struct PacketRegistry {
    entries: Vec<PacketEntry>,
}

struct PacketEntry {
    state: ProtocolState,
    direction: Direction,
    id: u32,
    name: &'static str,
}

impl PacketRegistry {
    /// Create a new empty registry.
    pub fn new() -> Self {
        Self { entries: Vec::new() }
    }

    /// Register a packet type.
    pub fn register<S: PacketId>() -> Self {
        let mut reg = Self::new();
        reg.add::<S>();
        reg
    }

    /// Add a packet type to the registry.
    pub fn add<S: PacketId>(&mut self) {
        self.entries.push(PacketEntry {
            state: S::STATE,
            direction: S::DIRECTION,
            id: S::ID,
            name: std::any::type_name::<S>(),
        });
    }

    /// Look up a packet name by state, direction, and ID.
    pub fn name(&self, state: ProtocolState, direction: Direction, id: u32) -> Option<&'static str> {
        self.entries
            .iter()
            .find(|e| e.state == state && e.direction == direction && e.id == id)
            .map(|e| e.name)
    }
}

impl Default for PacketRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// RawPacket — a parsed but uninterpreted packet
// ---------------------------------------------------------------------------

/// A raw packet with its ID and payload bytes.
#[derive(Debug, Clone)]
pub struct RawPacket {
    /// The packet ID.
    pub id: u32,
    /// The payload bytes (everything after the packet ID VarInt).
    pub payload: Bytes,
}

/// Read the length-delimited frame: VarInt length + VarInt packet_id + payload.
///
/// Returns `None` if there aren't enough bytes for a complete frame yet.
pub fn read_raw_packet(buf: &mut Bytes) -> Result<Option<RawPacket>, DecodeError> {
    // We need at least 1 byte to peek
    if !buf.has_remaining() {
        return Ok(None);
    }

    // Try to read the packet length (VarInt)
    let mut peek_buf = buf.clone();
    let length = match read_varint(&mut peek_buf) {
        Ok(v) => v,
        Err(DecodeError::InvalidVarInt) => return Ok(None),
        Err(e) => return Err(e),
    };

    if length < 0 {
        return Err(DecodeError::Other("negative packet length".into()));
    }

    let length = length as usize;

    // Check if we have the full packet.
    // `length` is the number of bytes AFTER the length VarInt,
    // so we need varint_size(length) + length bytes in the buffer.
    let length_varint_size = varint_size(length as i32);
    if buf.remaining() < length_varint_size + length {
        return Ok(None);
    }

    // Consume the length VarInt
    let _ = read_varint(buf)?;

    // Read the packet ID
    let packet_id = read_varint(buf)? as u32;

    // Read the payload (remaining bytes of this packet)
    let overhead = varint_size(packet_id as i32);
    if length < overhead {
        return Err(DecodeError::Other("packet too short for header".into()));
    }
    let payload_len = length - overhead;
    if buf.remaining() < payload_len {
        return Err(DecodeError::InsufficientBytes {
            need: payload_len,
            have: buf.remaining(),
        });
    }
    let payload = buf.copy_to_bytes(payload_len);

    Ok(Some(RawPacket {
        id: packet_id,
        payload,
    }))
}

/// Write a raw packet with length-delimited framing.
pub fn write_raw_packet(buf: &mut BytesMut, id: u32, payload: &[u8]) -> Result<(), EncodeError> {
    // Calculate total size: packet_id varint + payload
    let id_size = varint_size(id as i32);
    let total_size = id_size + payload.len();

    // Write length
    write_varint(buf, total_size as i32)?;

    // Write packet ID
    write_varint(buf, id as i32)?;

    // Write payload
    buf.put_slice(payload);

    Ok(())
}

use crate::types::{write_varint, varint_size};

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::BytesMut;

    #[test]
    fn test_raw_packet_roundtrip() {
        let payload = vec![0x01, 0x02, 0x03, 0x04];
        let mut buf = BytesMut::new();
        write_raw_packet(&mut buf, 0x26, &payload).unwrap();

        let mut read_buf = Bytes::copy_from_slice(&buf);
        let packet = read_raw_packet(&mut read_buf).unwrap().unwrap();
        assert_eq!(packet.id, 0x26);
        assert_eq!(&packet.payload[..], &payload[..]);
    }

    #[test]
    fn test_raw_packet_incomplete() {
        // Write only a length prefix saying 10 bytes, but provide only 3
        let mut buf = BytesMut::new();
        write_varint(&mut buf, 10).unwrap();
        buf.put_slice(&[0x01, 0x02, 0x03]);

        let mut read_buf = Bytes::copy_from_slice(&buf);
        let result = read_raw_packet(&mut read_buf).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_raw_packet_empty_buf() {
        let mut buf = Bytes::new();
        let result = read_raw_packet(&mut buf).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_registry() {
        // Just test that the registry compiles and basic operations work
        let reg = PacketRegistry::new();
        assert!(reg.name(ProtocolState::Play, Direction::Clientbound, 0x26).is_none());
    }
}
