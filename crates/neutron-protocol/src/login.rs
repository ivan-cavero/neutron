//! Login protocol packets for Minecraft 26.2.
//!
//! The login sequence is:
//! 1. Client sends **Handshake** (to transition to Login state)
//! 2. Client sends **LoginStart** (username + optional UUID)
//! 3. Server sends **EncryptionRequest** (if online-mode)
//! 4. Client sends **EncryptionResponse**
//! 5. Server sends **SetCompression** (optional)
//! 6. Server sends **LoginSuccess**
//!
//! Packet IDs are for Minecraft 26.2. They are hardcoded constants that can
//! be updated when the protocol version changes.

use bytes::{Buf, BufMut, Bytes, BytesMut};

use crate::error::{DecodeError, EncodeError};
use crate::packet::{Direction, PacketId, ProtocolState};
use crate::types::{read_string, read_uuid, read_varint, write_string, write_uuid, write_varint};

// ===========================================================================
// Serverbound Login Packets
// ===========================================================================

// ---------------------------------------------------------------------------
// Handshake
// ---------------------------------------------------------------------------

/// The very first packet sent by the client. Transitions the protocol state.
///
/// After this packet, the server changes state to either Login or Status.
#[derive(Debug, Clone, PartialEq)]
pub struct Handshake {
    /// Protocol version (e.g. 858 for 1.21.5 / 26.2 snapshot).
    pub protocol_version: i32,
    /// Server address (from the client's server list).
    pub server_address: String,
    /// Server port (from the client's server list).
    pub server_port: u16,
    /// Next state: 1 = Status, 2 = Login.
    pub next_state: i32,
}

impl PacketId for Handshake {
    const STATE: ProtocolState = ProtocolState::Handshake;
    const DIRECTION: Direction = Direction::Serverbound;
    const ID: u32 = 0x00;
}

impl Handshake {
    /// Decode a Handshake from the payload (after packet ID has been consumed).
    pub fn decode(payload: &mut Bytes) -> Result<Self, DecodeError> {
        let protocol_version = read_varint(payload)?;
        let server_address = read_string(payload)?;
        if payload.remaining() < 2 {
            return Err(DecodeError::InsufficientBytes {
                need: 2,
                have: payload.remaining(),
            });
        }
        let server_port = payload.get_u16();
        let next_state = read_varint(payload)?;
        Ok(Self {
            protocol_version,
            server_address,
            server_port,
            next_state,
        })
    }

    /// Encode this Handshake into a buffer (without length framing).
    pub fn encode(&self, buf: &mut BytesMut) -> Result<(), EncodeError> {
        write_varint(buf, self.protocol_version)?;
        write_string(buf, &self.server_address)?;
        buf.put_u16(self.server_port);
        write_varint(buf, self.next_state)?;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// LoginStart
// ---------------------------------------------------------------------------

/// Sent by the client after the Handshake transitions to Login state.
#[derive(Debug, Clone, PartialEq)]
pub struct LoginStart {
    /// Player name (max 16 characters).
    pub name: String,
    /// Player UUID (optional in some versions).
    pub uuid: Option<uuid::Uuid>,
}

impl PacketId for LoginStart {
    const STATE: ProtocolState = ProtocolState::Login;
    const DIRECTION: Direction = Direction::Serverbound;
    const ID: u32 = 0x00;
}

impl LoginStart {
    pub fn decode(payload: &mut Bytes) -> Result<Self, DecodeError> {
        let name = read_string(payload)?;
        let uuid = if payload.has_remaining() {
            Some(read_uuid(payload)?)
        } else {
            None
        };
        Ok(Self { name, uuid })
    }

    pub fn encode(&self, buf: &mut BytesMut) -> Result<(), EncodeError> {
        write_string(buf, &self.name)?;
        if let Some(uuid) = &self.uuid {
            write_uuid(buf, uuid)?;
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// EncryptionResponse
// ---------------------------------------------------------------------------

/// Sent by the client in response to an EncryptionRequest.
#[derive(Debug, Clone, PartialEq)]
pub struct EncryptionResponse {
    /// Length of the encrypted secret key (256 bytes for RSA).
    pub shared_secret_length: i32,
    /// The encrypted shared secret.
    pub shared_secret: Bytes,
    /// Length of the encrypted verify token.
    pub verify_token_length: i32,
    /// The encrypted verify token (or salt+signature in newer versions).
    pub verify_token: Bytes,
}

impl PacketId for EncryptionResponse {
    const STATE: ProtocolState = ProtocolState::Login;
    const DIRECTION: Direction = Direction::Serverbound;
    const ID: u32 = 0x01;
}

impl EncryptionResponse {
    pub fn decode(payload: &mut Bytes) -> Result<Self, DecodeError> {
        let shared_secret_length = read_varint(payload)?;
        if shared_secret_length < 0 {
            return Err(DecodeError::Other("negative shared secret length".into()));
        }
        let shared_secret_len = shared_secret_length as usize;
        if payload.remaining() < shared_secret_len {
            return Err(DecodeError::InsufficientBytes {
                need: shared_secret_len,
                have: payload.remaining(),
            });
        }
        let shared_secret = payload.copy_to_bytes(shared_secret_len);
        let verify_token_length = read_varint(payload)?;
        if verify_token_length < 0 {
            return Err(DecodeError::Other("negative verify token length".into()));
        }
        let verify_token_len = verify_token_length as usize;
        if payload.remaining() < verify_token_len {
            return Err(DecodeError::InsufficientBytes {
                need: verify_token_len,
                have: payload.remaining(),
            });
        }
        let verify_token = payload.copy_to_bytes(verify_token_len);
        Ok(Self {
            shared_secret_length,
            shared_secret,
            verify_token_length,
            verify_token,
        })
    }

    pub fn encode(&self, buf: &mut BytesMut) -> Result<(), EncodeError> {
        write_varint(buf, self.shared_secret_length)?;
        buf.put_slice(&self.shared_secret);
        write_varint(buf, self.verify_token_length)?;
        buf.put_slice(&self.verify_token);
        Ok(())
    }
}

// ===========================================================================
// Clientbound Login Packets
// ===========================================================================

// ---------------------------------------------------------------------------
// EncryptionRequest
// ---------------------------------------------------------------------------

/// Sent by the server when online-mode is enabled.
#[derive(Debug, Clone, PartialEq)]
pub struct EncryptionRequest {
    /// Server ID (usually empty string).
    pub server_id: String,
    /// Length of the public key.
    pub public_key_length: i32,
    /// Server's DER-encoded public key.
    pub public_key: Bytes,
    /// Length of the verify token.
    pub verify_token_length: i32,
    /// Random verify token (16 bytes).
    pub verify_token: Bytes,
}

impl PacketId for EncryptionRequest {
    const STATE: ProtocolState = ProtocolState::Login;
    const DIRECTION: Direction = Direction::Clientbound;
    const ID: u32 = 0x00;
}

impl EncryptionRequest {
    pub fn decode(payload: &mut Bytes) -> Result<Self, DecodeError> {
        let server_id = read_string(payload)?;
        let public_key_length = read_varint(payload)?;
        if public_key_length < 0 {
            return Err(DecodeError::Other("negative public key length".into()));
        }
        let public_key_len = public_key_length as usize;
        if payload.remaining() < public_key_len {
            return Err(DecodeError::InsufficientBytes {
                need: public_key_len,
                have: payload.remaining(),
            });
        }
        let public_key = payload.copy_to_bytes(public_key_len);
        let verify_token_length = read_varint(payload)?;
        if verify_token_length < 0 {
            return Err(DecodeError::Other("negative verify token length".into()));
        }
        let verify_token_len = verify_token_length as usize;
        if payload.remaining() < verify_token_len {
            return Err(DecodeError::InsufficientBytes {
                need: verify_token_len,
                have: payload.remaining(),
            });
        }
        let verify_token = payload.copy_to_bytes(verify_token_len);
        Ok(Self {
            server_id,
            public_key_length,
            public_key,
            verify_token_length,
            verify_token,
        })
    }

    pub fn encode(&self, buf: &mut BytesMut) -> Result<(), EncodeError> {
        write_string(buf, &self.server_id)?;
        write_varint(buf, self.public_key_length)?;
        buf.put_slice(&self.public_key);
        write_varint(buf, self.verify_token_length)?;
        buf.put_slice(&self.verify_token);
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// SetCompression
// ---------------------------------------------------------------------------

/// Sent by the server to enable compression. Threshold is the minimum
/// uncompressed size (in bytes) at which compression is applied.
#[derive(Debug, Clone, PartialEq)]
pub struct SetCompression {
    /// Compression threshold. 0 = disabled, >0 = minimum size to compress.
    pub threshold: i32,
}

impl PacketId for SetCompression {
    const STATE: ProtocolState = ProtocolState::Login;
    const DIRECTION: Direction = Direction::Clientbound;
    const ID: u32 = 0x03;
}

impl SetCompression {
    pub fn decode(payload: &mut Bytes) -> Result<Self, DecodeError> {
        let threshold = read_varint(payload)?;
        Ok(Self { threshold })
    }

    pub fn encode(&self, buf: &mut BytesMut) -> Result<(), EncodeError> {
        write_varint(buf, self.threshold)?;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// LoginSuccess
// ---------------------------------------------------------------------------

/// Sent by the server after successful authentication.
#[derive(Debug, Clone, PartialEq)]
pub struct LoginSuccess {
    /// Player UUID.
    pub uuid: uuid::Uuid,
    /// Player name.
    pub username: String,
    /// Number of properties (skin data, etc.).
    pub num_properties: i32,
}

impl PacketId for LoginSuccess {
    const STATE: ProtocolState = ProtocolState::Login;
    const DIRECTION: Direction = Direction::Clientbound;
    const ID: u32 = 0x02;
}

impl LoginSuccess {
    pub fn decode(payload: &mut Bytes) -> Result<Self, DecodeError> {
        let uuid = read_uuid(payload)?;
        let username = read_string(payload)?;
        let num_properties = read_varint(payload)?;
        // We don't parse properties here — they'd need to be decoded separately
        Ok(Self {
            uuid,
            username,
            num_properties,
        })
    }

    pub fn encode(&self, buf: &mut BytesMut) -> Result<(), EncodeError> {
        write_uuid(buf, &self.uuid)?;
        write_string(buf, &self.username)?;
        write_varint(buf, self.num_properties)?;
        Ok(())
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_handshake_roundtrip() {
        let packet = Handshake {
            protocol_version: 858,
            server_address: "localhost".to_string(),
            server_port: 25565,
            next_state: 2,
        };
        let mut buf = BytesMut::new();
        packet.encode(&mut buf).unwrap();
        let decoded = Handshake::decode(&mut Bytes::copy_from_slice(&buf)).unwrap();
        assert_eq!(decoded.protocol_version, 858);
        assert_eq!(decoded.server_address, "localhost");
        assert_eq!(decoded.server_port, 25565);
        assert_eq!(decoded.next_state, 2);
    }

    #[test]
    fn test_login_start_roundtrip() {
        let packet = LoginStart {
            name: "Steve".to_string(),
            uuid: Some(uuid::Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap()),
        };
        let mut buf = BytesMut::new();
        packet.encode(&mut buf).unwrap();
        let decoded = LoginStart::decode(&mut Bytes::copy_from_slice(&buf)).unwrap();
        assert_eq!(decoded.name, "Steve");
        assert_eq!(decoded.uuid, packet.uuid);
    }

    #[test]
    fn test_login_start_no_uuid() {
        let packet = LoginStart {
            name: "Alex".to_string(),
            uuid: None,
        };
        let mut buf = BytesMut::new();
        packet.encode(&mut buf).unwrap();
        let decoded = LoginStart::decode(&mut Bytes::copy_from_slice(&buf)).unwrap();
        assert_eq!(decoded.name, "Alex");
        assert!(decoded.uuid.is_none());
    }

    #[test]
    fn test_set_compression_roundtrip() {
        let packet = SetCompression { threshold: 256 };
        let mut buf = BytesMut::new();
        packet.encode(&mut buf).unwrap();
        let decoded = SetCompression::decode(&mut Bytes::copy_from_slice(&buf)).unwrap();
        assert_eq!(decoded.threshold, 256);
    }

    #[test]
    fn test_login_success_roundtrip() {
        let packet = LoginSuccess {
            uuid: uuid::Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap(),
            username: "TestPlayer".to_string(),
            num_properties: 0,
        };
        let mut buf = BytesMut::new();
        packet.encode(&mut buf).unwrap();
        let decoded = LoginSuccess::decode(&mut Bytes::copy_from_slice(&buf)).unwrap();
        assert_eq!(decoded.uuid, packet.uuid);
        assert_eq!(decoded.username, "TestPlayer");
        assert_eq!(decoded.num_properties, 0);
    }

    #[test]
    fn test_encryption_request_roundtrip() {
        let key_data = vec![0xAA; 128];
        let token_data = vec![0xBB; 16];
        let packet = EncryptionRequest {
            server_id: String::new(),
            public_key_length: 128,
            public_key: Bytes::from(key_data.clone()),
            verify_token_length: 16,
            verify_token: Bytes::from(token_data.clone()),
        };
        let mut buf = BytesMut::new();
        packet.encode(&mut buf).unwrap();
        let decoded = EncryptionRequest::decode(&mut Bytes::copy_from_slice(&buf)).unwrap();
        assert_eq!(decoded.server_id, "");
        assert_eq!(decoded.public_key_length, 128);
        assert_eq!(&decoded.public_key[..], &key_data[..]);
        assert_eq!(decoded.verify_token_length, 16);
        assert_eq!(&decoded.verify_token[..], &token_data[..]);
    }

    #[test]
    fn test_encryption_response_roundtrip() {
        let secret = vec![0xCC; 256];
        let token = vec![0xDD; 128];
        let packet = EncryptionResponse {
            shared_secret_length: 256,
            shared_secret: Bytes::from(secret.clone()),
            verify_token_length: 128,
            verify_token: Bytes::from(token.clone()),
        };
        let mut buf = BytesMut::new();
        packet.encode(&mut buf).unwrap();
        let decoded = EncryptionResponse::decode(&mut Bytes::copy_from_slice(&buf)).unwrap();
        assert_eq!(decoded.shared_secret_length, 256);
        assert_eq!(&decoded.shared_secret[..], &secret[..]);
        assert_eq!(decoded.verify_token_length, 128);
        assert_eq!(&decoded.verify_token[..], &token[..]);
    }

    #[test]
    fn test_packet_ids() {
        assert_eq!(Handshake::ID, 0x00);
        assert_eq!(LoginStart::ID, 0x00);
        assert_eq!(EncryptionResponse::ID, 0x01);
        assert_eq!(EncryptionRequest::ID, 0x00);
        assert_eq!(SetCompression::ID, 0x03);
        assert_eq!(LoginSuccess::ID, 0x02);
    }

    #[test]
    fn test_packet_states() {
        assert_eq!(Handshake::STATE, ProtocolState::Handshake);
        assert_eq!(LoginStart::STATE, ProtocolState::Login);
        assert_eq!(EncryptionResponse::STATE, ProtocolState::Login);
        assert_eq!(EncryptionRequest::STATE, ProtocolState::Login);
        assert_eq!(SetCompression::STATE, ProtocolState::Login);
        assert_eq!(LoginSuccess::STATE, ProtocolState::Login);
    }
}
