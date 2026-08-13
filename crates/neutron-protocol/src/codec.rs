//! Length-delimited framing codec for Minecraft protocol packets.
//!
//! This module provides a codec that handles:
//! 1. VarInt length-prefixed framing
//! 2. Optional zlib compression (threshold-based)
//!
//! The codec works with `tokio::io::AsyncRead + AsyncWrite` and integrates
//! with `tokio_util::codec` (via `Framed`).

use bytes::{Buf, BufMut, Bytes, BytesMut};
use flate2::read::ZlibDecoder;
use flate2::write::ZlibEncoder;
use flate2::Compression;
use std::io::{Read, Write};
use tracing::{debug, trace};

use crate::error::{DecodeError, EncodeError, ProtocolError};
use crate::packet::RawPacket;
use crate::types::{read_varint, varint_size, write_varint};

// ---------------------------------------------------------------------------
// MinecraftCodec
// ---------------------------------------------------------------------------

/// A codec for Minecraft protocol framing.
///
/// Handles:
/// - VarInt length-prefixed framing
/// - Optional zlib compression (when data length >= threshold)
/// - Packet ID extraction
#[derive(Debug)]
pub struct MinecraftCodec {
    /// Compression threshold. Packets smaller than this are sent uncompressed.
    /// `None` means compression is disabled.
    compression_threshold: Option<i32>,
    /// Maximum packet size (default: 8 MiB).
    max_packet_size: usize,
}

impl MinecraftCodec {
    /// Create a new codec with compression disabled.
    pub fn new() -> Self {
        Self {
            compression_threshold: None,
            max_packet_size: 8 * 1024 * 1024,
        }
    }

    /// Create a codec with a specific compression threshold.
    ///
    /// When `threshold` is >= 0, packets with uncompressed size >= threshold
    /// will be compressed with zlib.
    pub fn with_compression(threshold: i32) -> Self {
        Self {
            compression_threshold: Some(threshold),
            max_packet_size: 8 * 1024 * 1024,
        }
    }

    /// Set the maximum allowed packet size.
    pub fn with_max_packet_size(mut self, max: usize) -> Self {
        self.max_packet_size = max;
        self
    }

    /// Enable or update the compression threshold.
    pub fn set_compression(&mut self, threshold: i32) {
        self.compression_threshold = Some(threshold);
    }

    /// Check if compression is enabled.
    pub fn compression_enabled(&self) -> bool {
        self.compression_threshold.is_some()
    }

    // -----------------------------------------------------------------------
    // Encoding (Server -> wire)
    // -----------------------------------------------------------------------

    /// Encode a packet into the output buffer with length framing.
    ///
    /// If compression is enabled and the uncompressed size >= threshold,
    /// the packet is compressed before writing.
    pub fn encode(
        &self,
        packet_id: u32,
        payload: &[u8],
        buf: &mut BytesMut,
    ) -> Result<(), ProtocolError> {
        match self.compression_threshold {
            None => self.encode_uncompressed(packet_id, payload, buf),
            Some(threshold) => self.encode_compressed(packet_id, payload, threshold, buf),
        }
    }

    fn encode_uncompressed(
        &self,
        packet_id: u32,
        payload: &[u8],
        buf: &mut BytesMut,
    ) -> Result<(), ProtocolError> {
        let id_size = varint_size(packet_id as i32);
        let packet_length = id_size + payload.len();

        if packet_length > self.max_packet_size {
            return Err(EncodeError::PacketTooLarge {
                size: packet_length,
                max: self.max_packet_size,
            }
            .into());
        }

        // Length prefix
        write_varint(buf, packet_length as i32)?;

        // Packet ID
        write_varint(buf, packet_id as i32)?;

        // Payload
        buf.put_slice(payload);

        trace!(
            packet_id = format!("0x{:02X}", packet_id),
            packet_length,
            "encoded uncompressed packet"
        );

        Ok(())
    }

    fn encode_compressed(
        &self,
        packet_id: u32,
        payload: &[u8],
        threshold: i32,
        buf: &mut BytesMut,
    ) -> Result<(), ProtocolError> {
        let id_size = varint_size(packet_id as i32);
        let uncompressed_size = id_size + payload.len();

        if uncompressed_size > self.max_packet_size {
            return Err(EncodeError::PacketTooLarge {
                size: uncompressed_size,
                max: self.max_packet_size,
            }
            .into());
        }

        if (uncompressed_size as i32) < threshold {
            // Below threshold: send uncompressed, but with compression framing
            // (data length = 0 means "not compressed")
            let packet_length = varint_size(0) + id_size + payload.len();

            write_varint(buf, packet_length as i32)?;
            write_varint(buf, 0)?; // data length = 0 (uncompressed)
            write_varint(buf, packet_id as i32)?;
            buf.put_slice(payload);

            trace!(
                packet_id = format!("0x{:02X}", packet_id),
                uncompressed_size,
                "encoded packet below compression threshold"
            );
        } else {
            // Above threshold: compress
            let mut encoder = ZlibEncoder::new(Vec::new(), Compression::fast());

            // Write packet ID into compressed data
            let mut id_buf = BytesMut::new();
            write_varint(&mut id_buf, packet_id as i32)?;
            encoder.write_all(&id_buf)?;
            encoder.write_all(payload)?;

            let compressed = encoder.finish()?;

            let packet_length = varint_size(uncompressed_size as i32) + compressed.len();

            write_varint(buf, packet_length as i32)?;
            write_varint(buf, uncompressed_size as i32)?; // data length (uncompressed size)
            buf.put_slice(&compressed);

            debug!(
                packet_id = format!("0x{:02X}", packet_id),
                uncompressed_size,
                compressed_size = compressed.len(),
                "encoded compressed packet"
            );
        }

        Ok(())
    }

    // -----------------------------------------------------------------------
    // Decoding (wire -> packets)
    // -----------------------------------------------------------------------

    /// Try to decode one complete packet from the input buffer.
    ///
    /// Returns `Ok(None)` if there aren't enough bytes for a complete frame.
    pub fn decode(&self, buf: &mut Bytes) -> Result<Option<RawPacket>, ProtocolError> {
        if !buf.has_remaining() {
            return Ok(None);
        }

        match self.compression_threshold {
            None => self.decode_uncompressed(buf),
            Some(_) => self.decode_compressed(buf),
        }
    }

    fn decode_uncompressed(&self, buf: &mut Bytes) -> Result<Option<RawPacket>, ProtocolError> {
        // Read packet length
        let mut peek = buf.clone();
        let length = match read_varint(&mut peek) {
            Ok(v) => v,
            Err(DecodeError::InvalidVarInt) => return Ok(None),
            Err(e) => return Err(e.into()),
        };

        if length < 0 {
            return Err(DecodeError::Other("negative packet length".into()).into());
        }

        let length = length as usize;
        if length > self.max_packet_size {
            return Err(EncodeError::PacketTooLarge {
                size: length,
                max: self.max_packet_size,
            }
            .into());
        }

        // Check if we have the full frame.
        // `length` is the number of bytes AFTER the length VarInt,
        // so we need varint_size(length) + length bytes in the buffer.
        let length_varint_size = varint_size(length as i32);
        if buf.remaining() < length_varint_size + length {
            return Ok(None);
        }

        // Consume the length VarInt
        let _ = read_varint(buf)?;

        // Read packet ID
        let packet_id = read_varint(buf)? as u32;

        // Read payload
        let overhead = varint_size(packet_id as i32);
        if length < overhead {
            return Err(DecodeError::Other("packet too short for header".into()).into());
        }
        let payload_len = length - overhead;
        let payload = buf.copy_to_bytes(payload_len);

        trace!(
            packet_id = format!("0x{:02X}", packet_id),
            payload_len,
            "decoded uncompressed packet"
        );

        Ok(Some(RawPacket {
            id: packet_id,
            payload,
        }))
    }

    fn decode_compressed(&self, buf: &mut Bytes) -> Result<Option<RawPacket>, ProtocolError> {
        // Read total packet length
        let mut peek = buf.clone();
        let total_length = match read_varint(&mut peek) {
            Ok(v) => v,
            Err(DecodeError::InvalidVarInt) => return Ok(None),
            Err(e) => return Err(e.into()),
        };

        if total_length < 0 {
            return Err(DecodeError::Other("negative packet length".into()).into());
        }

        let total_length = total_length as usize;
        if total_length > self.max_packet_size {
            return Err(EncodeError::PacketTooLarge {
                size: total_length,
                max: self.max_packet_size,
            }
            .into());
        }

        // Check if we have the full frame.
        // `total_length` is the number of bytes AFTER the length VarInt,
        // so we need varint_size(total_length) + total_length bytes in the buffer.
        let total_length_varint_size = varint_size(total_length as i32);
        if buf.remaining() < total_length_varint_size + total_length {
            return Ok(None);
        }

        // Consume the length VarInt
        let _ = read_varint(buf)?;

        // Read data length (uncompressed size; 0 = not compressed)
        let data_length_raw = read_varint(buf)?;
        if data_length_raw < 0 {
            return Err(DecodeError::Other("negative data length".into()).into());
        }
        let data_length = data_length_raw as usize;

        // Guard against absurdly large data_length that would cause OOM
        if data_length > self.max_packet_size {
            return Err(EncodeError::PacketTooLarge {
                size: data_length,
                max: self.max_packet_size,
            }
            .into());
        }

        if data_length == 0 {
            // Not compressed: read packet ID + payload directly
            let packet_id = read_varint(buf)? as u32;
            let overhead = varint_size(0) + varint_size(packet_id as i32);
            if total_length < overhead {
                return Err(DecodeError::Other("packet too short for header".into()).into());
            }
            let payload_len = total_length - overhead;
            let payload = buf.copy_to_bytes(payload_len);

            trace!(
                packet_id = format!("0x{:02X}", packet_id),
                payload_len,
                "decoded uncompressed packet (compression frame)"
            );

            return Ok(Some(RawPacket {
                id: packet_id,
                payload,
            }));
        }

        // Compressed: decompress
        let data_length_varint_size = varint_size(data_length_raw);
        if total_length < data_length_varint_size {
            return Err(DecodeError::Other("packet too short for compressed data".into()).into());
        }
        let compressed_len = total_length - data_length_varint_size;
        if buf.remaining() < compressed_len {
            return Err(DecodeError::InsufficientBytes {
                need: compressed_len,
                have: buf.remaining(),
            }
            .into());
        }
        let compressed_data = buf.copy_to_bytes(compressed_len);

        let mut decoder = ZlibDecoder::new(&compressed_data[..]);
        let mut decompressed = Vec::with_capacity(data_length);
        decoder.read_to_end(&mut decompressed)?;

        if decompressed.len() != data_length {
            return Err(DecodeError::DecompressionFailed(format!(
                "expected {} bytes, got {}",
                data_length,
                decompressed.len()
            ))
            .into());
        }

        let mut decompressed_buf = Bytes::from(decompressed);
        let packet_id = read_varint(&mut decompressed_buf)? as u32;
        let payload = decompressed_buf;

        debug!(
            packet_id = format!("0x{:02X}", packet_id),
            compressed_size = compressed_len,
            decompressed_size = data_length,
            "decoded compressed packet"
        );

        Ok(Some(RawPacket {
            id: packet_id,
            payload,
        }))
    }
}

impl Default for MinecraftCodec {
    fn default() -> Self {
        Self::new()
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_decode_uncompressed() {
        let codec = MinecraftCodec::new();
        let payload = vec![0x01, 0x02, 0x03];
        let mut buf = BytesMut::new();
        codec.encode(0x26, &payload, &mut buf).unwrap();

        let mut read_buf = Bytes::copy_from_slice(&buf);
        let packet = codec.decode(&mut read_buf).unwrap().unwrap();
        assert_eq!(packet.id, 0x26);
        assert_eq!(&packet.payload[..], &payload[..]);
    }

    #[test]
    fn test_encode_decode_compressed_above_threshold() {
        let codec = MinecraftCodec::with_compression(256);

        // Create a payload larger than the threshold
        let payload = vec![0xAA; 512];
        let mut buf = BytesMut::new();
        codec.encode(0x2B, &payload, &mut buf).unwrap();

        // Verify compressed data is smaller than uncompressed
        // (for repeated bytes, zlib compresses very well)
        assert!(buf.len() < 512 + 10); // rough check

        let mut read_buf = Bytes::copy_from_slice(&buf);
        let packet = codec.decode(&mut read_buf).unwrap().unwrap();
        assert_eq!(packet.id, 0x2B);
        assert_eq!(&packet.payload[..], &payload[..]);
    }

    #[test]
    fn test_encode_decode_compressed_below_threshold() {
        let codec = MinecraftCodec::with_compression(256);

        // Small payload (below threshold)
        let payload = vec![0x01, 0x02, 0x03];
        let mut buf = BytesMut::new();
        codec.encode(0x09, &payload, &mut buf).unwrap();

        let mut read_buf = Bytes::copy_from_slice(&buf);
        let packet = codec.decode(&mut read_buf).unwrap().unwrap();
        assert_eq!(packet.id, 0x09);
        assert_eq!(&packet.payload[..], &payload[..]);
    }

    #[test]
    fn test_multiple_packets_in_buffer() {
        let codec = MinecraftCodec::new();
        let mut buf = BytesMut::new();

        // Write two packets
        codec.encode(0x26, &[0x01], &mut buf).unwrap();
        codec.encode(0x2B, &[0x02, 0x03], &mut buf).unwrap();

        // Read them back
        let mut read_buf = Bytes::copy_from_slice(&buf);
        let p1 = codec.decode(&mut read_buf).unwrap().unwrap();
        assert_eq!(p1.id, 0x26);
        assert_eq!(&p1.payload[..], &[0x01]);

        let p2 = codec.decode(&mut read_buf).unwrap().unwrap();
        assert_eq!(p2.id, 0x2B);
        assert_eq!(&p2.payload[..], &[0x02, 0x03]);

        assert!(!read_buf.has_remaining());
    }

    #[test]
    fn test_partial_frame_returns_none() {
        let codec = MinecraftCodec::new();
        let mut buf = BytesMut::new();
        // Use a payload large enough that truncating to 3 bytes is incomplete
        let payload = vec![0x01; 20];
        codec.encode(0x26, &payload, &mut buf).unwrap();

        // Take only the first 3 bytes (VarInt length + partial)
        let mut read_buf = Bytes::copy_from_slice(&buf[..3]);
        let result = codec.decode(&mut read_buf).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_empty_buffer() {
        let codec = MinecraftCodec::new();
        let mut buf = Bytes::new();
        let result = codec.decode(&mut buf).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_max_packet_size() {
        let codec = MinecraftCodec::new().with_max_packet_size(100);
        let mut buf = BytesMut::new();

        // Small packet should work
        assert!(codec.encode(0x26, &[0x01], &mut buf).is_ok());

        // Large packet should fail
        let large_payload = vec![0u8; 200];
        let result = codec.encode(0x26, &large_payload, &mut buf);
        assert!(result.is_err());
    }

    #[test]
    fn test_set_compression_at_runtime() {
        let mut codec = MinecraftCodec::new();
        assert!(!codec.compression_enabled());

        codec.set_compression(256);
        assert!(codec.compression_enabled());

        // Now encode with compression
        let payload = vec![0xBB; 512];
        let mut buf = BytesMut::new();
        codec.encode(0x26, &payload, &mut buf).unwrap();

        let mut read_buf = Bytes::copy_from_slice(&buf);
        let packet = codec.decode(&mut read_buf).unwrap().unwrap();
        assert_eq!(packet.id, 0x26);
        assert_eq!(&packet.payload[..], &payload[..]);
    }

    #[test]
    fn test_large_compressed_packet() {
        let codec = MinecraftCodec::with_compression(128);
        // 1KB of repeated data
        let payload = vec![0x42; 1024];
        let mut buf = BytesMut::new();
        codec.encode(0x27, &payload, &mut buf).unwrap();

        let mut read_buf = Bytes::copy_from_slice(&buf);
        let packet = codec.decode(&mut read_buf).unwrap().unwrap();
        assert_eq!(packet.id, 0x27);
        assert_eq!(&packet.payload[..], &payload[..]);
    }
}
