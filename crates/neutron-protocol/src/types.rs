//! Common Minecraft protocol types shared across packets.

use bytes::{Buf, BufMut, Bytes, BytesMut};
use std::fmt;

use crate::error::{DecodeError, EncodeError};

// ---------------------------------------------------------------------------
// VarInt / VarLong helpers
// ---------------------------------------------------------------------------

/// Maximum size of a VarInt in bytes (5).
pub const VARINT_MAX_BYTES: usize = 5;
/// Maximum size of a VarLong in bytes (10).
pub const VARINT_MAX_BYTES_LONG: usize = 10;

/// Encode a `i32` as a VarInt into the buffer.
///
/// # Errors
/// Returns `EncodeError::VarIntOutOfRange` if the value is outside the valid
/// VarInt range (-2^31 to 2^31 - 1, which is always valid for i32, but the
/// encoded form must fit in 5 bytes).
pub fn write_varint(buf: &mut BytesMut, value: i32) -> Result<(), EncodeError> {
    let mut val = value as u32;
    loop {
        let mut byte = (val & 0x7F) as u8;
        val >>= 7;
        if val != 0 {
            byte |= 0x80;
        }
        buf.put_u8(byte);
        if val == 0 {
            break;
        }
    }
    Ok(())
}

/// Read a VarInt from the buffer.
pub fn read_varint(buf: &mut Bytes) -> Result<i32, DecodeError> {
    let mut result: i32 = 0;
    let mut shift: u32 = 0;
    let mut byte: u8;
    loop {
        if !buf.has_remaining() {
            return Err(DecodeError::InvalidVarInt);
        }
        byte = buf.get_u8();
        result |= ((byte & 0x7F) as i32) << shift;
        if byte & 0x80 == 0 {
            break;
        }
        shift += 7;
        if shift >= 35 {
            return Err(DecodeError::InvalidVarInt);
        }
    }
    Ok(result)
}

/// Return the encoded size of a VarInt in bytes.
pub fn varint_size(value: i32) -> usize {
    let mut val = value as u32;
    let mut size = 0;
    loop {
        size += 1;
        val >>= 7;
        if val == 0 {
            break;
        }
    }
    size
}

/// Encode a `i64` as a VarLong into the buffer.
pub fn write_varlong(buf: &mut BytesMut, value: i64) -> Result<(), EncodeError> {
    let mut val = value as u64;
    loop {
        let mut byte = (val & 0x7F) as u8;
        val >>= 7;
        if val != 0 {
            byte |= 0x80;
        }
        buf.put_u8(byte);
        if val == 0 {
            break;
        }
    }
    Ok(())
}

/// Read a VarLong from the buffer.
pub fn read_varlong(buf: &mut Bytes) -> Result<i64, DecodeError> {
    let mut result: i64 = 0;
    let mut shift: u32 = 0;
    loop {
        if !buf.has_remaining() {
            return Err(DecodeError::InvalidVarLong);
        }
        let byte = buf.get_u8();
        result |= ((byte & 0x7F) as i64) << shift;
        if byte & 0x80 == 0 {
            break;
        }
        shift += 7;
        if shift >= 70 {
            return Err(DecodeError::InvalidVarLong);
        }
    }
    Ok(result)
}

// ---------------------------------------------------------------------------
// BlockPos
// ---------------------------------------------------------------------------

/// A position in the Minecraft world (block coordinates).
///
/// Encoded as a single `i64` with x in bits 64-38, z in bits 38-12, y in bits 12-0.
/// Each coordinate is a signed 26-bit value (range: -33,554,432 to 33,554,431).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BlockPos {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

impl BlockPos {
    /// Create a new block position.
    pub fn new(x: i32, y: i32, z: i32) -> Self {
        Self { x, y, z }
    }

    /// Encode this position as a packed `i64`.
    pub fn to_packed(self) -> i64 {
        ((self.x as i64 & 0x3FFFFFF) << 38)
            | ((self.z as i64 & 0x3FFFFFF) << 12)
            | (self.y as i64 & 0xFFF)
    }

    /// Decode a packed `i64` into a `BlockPos`.
    pub fn from_packed(val: i64) -> Result<Self, DecodeError> {
        let x = (val >> 38) as i32;
        let z = ((val << 26) >> 38) as i32;
        let y = ((val << 52) >> 52) as i32;
        Ok(Self { x, y, z })
    }
}

impl fmt::Display for BlockPos {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "({} {} {})", self.x, self.y, self.z)
    }
}

// ---------------------------------------------------------------------------
// Vec3f / Vec3d
// ---------------------------------------------------------------------------

/// A 3D vector with `f32` components (used for teleport, etc.).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vec3f {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Vec3f {
    pub fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }
}

/// A 3D vector with `f64` components (used for player position).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vec3d {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl Vec3d {
    pub fn new(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }
}

// ---------------------------------------------------------------------------
// Slot (inventory item)
// ---------------------------------------------------------------------------

/// An inventory slot. `None` means empty.
pub type Slot = Option<SlotData>;

/// Data for a non-empty inventory slot.
#[derive(Debug, Clone, PartialEq)]
pub struct SlotData {
    pub item_id: i32,
    pub item_count: u8,
    pub nbt: Option<Bytes>,
}

/// Read a Slot from the buffer.
pub fn read_slot(buf: &mut Bytes) -> Result<Slot, DecodeError> {
    if !buf.has_remaining() {
        return Err(DecodeError::InsufficientBytes { need: 1, have: 0 });
    }
    let present = buf.get_u8() != 0;
    if !present {
        return Ok(None);
    }
    let item_id = read_varint(buf)?;
    if !buf.has_remaining() {
        return Err(DecodeError::InsufficientBytes { need: 1, have: 0 });
    }
    let item_count = buf.get_u8();
    // NBT is complex to parse fully; for now we read the raw bytes.
    // In production, use a proper NBT parser.
    let remaining = buf.remaining();
    if remaining > 0 {
        let nbt_bytes = buf.copy_to_bytes(remaining);
        Ok(Some(SlotData {
            item_id,
            item_count,
            nbt: Some(nbt_bytes),
        }))
    } else {
        Ok(Some(SlotData {
            item_id,
            item_count,
            nbt: None,
        }))
    }
}

/// Write a Slot to the buffer.
pub fn write_slot(buf: &mut BytesMut, slot: &Slot) -> Result<(), EncodeError> {
    match slot {
        None => {
            buf.put_u8(0);
        }
        Some(data) => {
            buf.put_u8(1);
            write_varint(buf, data.item_id)?;
            buf.put_u8(data.item_count);
            if let Some(ref nbt) = data.nbt {
                buf.put_slice(nbt);
            }
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// GameMode
// ---------------------------------------------------------------------------

/// Minecraft game modes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum GameMode {
    Survival = 0,
    Creative = 1,
    Adventure = 2,
    Spectator = 3,
}

impl GameMode {
    /// Convert from raw protocol value.
    pub fn from_id(id: u8) -> Option<Self> {
        match id {
            0 => Some(Self::Survival),
            1 => Some(Self::Creative),
            2 => Some(Self::Adventure),
            3 => Some(Self::Spectator),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// Chat
// ---------------------------------------------------------------------------

/// A chat message, either plain text or JSON component.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Chat {
    /// Plain text message.
    Plain(String),
    /// JSON text component (serialized as a string).
    Json(String),
}

impl Chat {
    /// Encode the chat to a JSON string suitable for the protocol.
    pub fn to_json_string(&self) -> String {
        match self {
            Chat::Plain(text) => {
                // Escape the text and wrap in a simple JSON object
                let escaped = text
                    .replace('\\', "\\\\")
                    .replace('"', "\\\"")
                    .replace('\n', "\\n");
                format!("{{\"text\":\"{}\"}}", escaped)
            }
            Chat::Json(json) => json.clone(),
        }
    }

    /// Read a Chat from the buffer (length-prefixed string containing JSON).
    pub fn read_from(buf: &mut Bytes) -> Result<Self, DecodeError> {
        let len = read_varint(buf)? as usize;
        let bytes = read_bytes(buf, len)?;
        let s = String::from_utf8(bytes)?;
        Ok(Chat::Json(s))
    }

    /// Write a Chat to the buffer.
    pub fn write_to(&self, buf: &mut BytesMut) -> Result<(), EncodeError> {
        let json = self.to_json_string();
        let bytes = json.as_bytes();
        write_varint(buf, bytes.len() as i32)?;
        buf.put_slice(bytes);
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Angle
// ---------------------------------------------------------------------------

/// A yaw/pitch angle (0-255, maps to 0-360 degrees).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Angle(pub u8);

impl Angle {
    /// Convert to degrees (f32).
    pub fn to_degrees(self) -> f32 {
        (self.0 as f32) * (360.0 / 256.0)
    }

    /// Convert from degrees to protocol angle.
    pub fn from_degrees(degrees: f32) -> Self {
        Self((degrees * 256.0 / 360.0) as u8)
    }
}

// ---------------------------------------------------------------------------
// Helper functions for reading/writing common types
// ---------------------------------------------------------------------------

/// Read `n` bytes from the buffer.
pub fn read_bytes(buf: &mut Bytes, n: usize) -> Result<Vec<u8>, DecodeError> {
    if buf.remaining() < n {
        return Err(DecodeError::InsufficientBytes {
            need: n,
            have: buf.remaining(),
        });
    }
    let mut out = vec![0u8; n];
    buf.copy_to_slice(&mut out);
    Ok(out)
}

/// Read a length-prefixed string (VarInt length + UTF-8 bytes).
pub fn read_string(buf: &mut Bytes) -> Result<String, DecodeError> {
    let len = read_varint(buf)? as usize;
    if len > 32767 {
        return Err(DecodeError::StringTooLong { len, max: 32767 });
    }
    let bytes = read_bytes(buf, len)?;
    Ok(String::from_utf8(bytes)?)
}

/// Write a length-prefixed string.
pub fn write_string(buf: &mut BytesMut, s: &str) -> Result<(), EncodeError> {
    let bytes = s.as_bytes();
    if bytes.len() > 32767 {
        return Err(EncodeError::StringTooLong {
            len: bytes.len(),
            max: 32767,
        });
    }
    write_varint(buf, bytes.len() as i32)?;
    buf.put_slice(bytes);
    Ok(())
}

/// Read a UUID (16 bytes, most-significant first).
pub fn read_uuid(buf: &mut Bytes) -> Result<uuid::Uuid, DecodeError> {
    let bytes = read_bytes(buf, 16)?;
    Ok(uuid::Uuid::from_bytes(
        bytes.try_into().unwrap_or([0u8; 16]),
    ))
}

/// Write a UUID.
pub fn write_uuid(buf: &mut BytesMut, uuid: &uuid::Uuid) -> Result<(), EncodeError> {
    buf.put_slice(uuid.as_bytes());
    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::BytesMut;

    #[test]
    fn test_varint_roundtrip() {
        let values = [0, 1, 127, 128, 255, 25565, 2097151, -1, -2147483648];
        for &val in &values {
            let mut buf = BytesMut::new();
            write_varint(&mut buf, val).unwrap();
            let decoded = read_varint(&mut Bytes::copy_from_slice(&buf)).unwrap();
            assert_eq!(val, decoded, "VarInt roundtrip failed for {}", val);
        }
    }

    #[test]
    fn test_varint_encoded_size() {
        assert_eq!(varint_size(0), 1);
        assert_eq!(varint_size(127), 1);
        assert_eq!(varint_size(128), 2);
        assert_eq!(varint_size(25565), 3);
    }

    #[test]
    fn test_varlong_roundtrip() {
        let values: Vec<i64> = vec![0, 1, 127, 128, 255, 25565, -1, i64::MIN, i64::MAX];
        for val in values {
            let mut buf = BytesMut::new();
            write_varlong(&mut buf, val).unwrap();
            let decoded = read_varlong(&mut Bytes::copy_from_slice(&buf)).unwrap();
            assert_eq!(val, decoded, "VarLong roundtrip failed for {}", val);
        }
    }

    #[test]
    fn test_blockpos_roundtrip() {
        let positions = [
            BlockPos::new(0, 0, 0),
            BlockPos::new(100, 64, -200),
            BlockPos::new(-30000000, 256, 30000000),
        ];
        for pos in positions {
            let packed = pos.to_packed();
            let decoded = BlockPos::from_packed(packed).unwrap();
            assert_eq!(pos, decoded, "BlockPos roundtrip failed for {}", pos);
        }
    }

    #[test]
    fn test_slot_roundtrip() {
        // Empty slot
        let mut buf = BytesMut::new();
        write_slot(&mut buf, &None).unwrap();
        let decoded = read_slot(&mut Bytes::copy_from_slice(&buf)).unwrap();
        assert_eq!(decoded, None);

        // Non-empty slot
        let slot = Some(SlotData {
            item_id: 1,
            item_count: 64,
            nbt: None,
        });
        let mut buf = BytesMut::new();
        write_slot(&mut buf, &slot).unwrap();
        let decoded = read_slot(&mut Bytes::copy_from_slice(&buf)).unwrap();
        assert_eq!(decoded, slot);
    }

    #[test]
    fn test_chat_plain() {
        let chat = Chat::Plain("Hello World".to_string());
        let json = chat.to_json_string();
        assert!(json.contains("Hello World"));
        assert!(json.starts_with('{'));
    }

    #[test]
    fn test_gamemode() {
        assert_eq!(GameMode::from_id(0), Some(GameMode::Survival));
        assert_eq!(GameMode::from_id(1), Some(GameMode::Creative));
        assert_eq!(GameMode::from_id(2), Some(GameMode::Adventure));
        assert_eq!(GameMode::from_id(3), Some(GameMode::Spectator));
        assert_eq!(GameMode::from_id(4), None);
    }

    #[test]
    fn test_string_roundtrip() {
        let mut buf = BytesMut::new();
        write_string(&mut buf, "Hello, Minecraft!").unwrap();
        let decoded = read_string(&mut Bytes::copy_from_slice(&buf)).unwrap();
        assert_eq!(decoded, "Hello, Minecraft!");
    }

    #[test]
    fn test_angle() {
        let angle = Angle::from_degrees(0.0);
        let degrees = angle.to_degrees();
        assert!((degrees - 0.0).abs() < 2.0);

        let angle = Angle::from_degrees(90.0);
        assert_eq!(angle.0, 64);
        assert!((angle.to_degrees() - 90.0).abs() < 2.0);
    }
}
