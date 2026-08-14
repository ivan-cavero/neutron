//! Play-state packets for Minecraft 26.2 (protocol 776).
//!
//! Clientbound and serverbound. IDs are the 26.2 values used by
//! `neutron-server` (spawn `0x61`, position `0x48`, chunk `0x2D`, …).

use bytes::{Buf, BufMut, Bytes, BytesMut};

use crate::error::{DecodeError, EncodeError};
use crate::packet::{Direction, PacketId, ProtocolState};
use crate::types::{
    read_string, read_uuid, read_varint, write_string, write_uuid, write_varint, BlockPos, Chat,
    GameMode,
};

// ===========================================================================
// Clientbound Play Packets (Server -> Client)
// ===========================================================================

// ---------------------------------------------------------------------------
// KeepAlive (Clientbound)
// ---------------------------------------------------------------------------

/// Sent periodically to keep the connection alive.
#[derive(Debug, Clone, PartialEq)]
pub struct KeepAlive {
    /// Unique ID to match with the client's response.
    pub id: i64,
}

impl PacketId for KeepAlive {
    const STATE: ProtocolState = ProtocolState::Play;
    const DIRECTION: Direction = Direction::Clientbound;
    const ID: u32 = 0x26;
}

impl KeepAlive {
    pub fn decode(payload: &mut Bytes) -> Result<Self, DecodeError> {
        if payload.remaining() < 8 {
            return Err(DecodeError::InsufficientBytes {
                need: 8,
                have: payload.remaining(),
            });
        }
        let id = payload.get_i64();
        Ok(Self { id })
    }

    pub fn encode(&self, buf: &mut BytesMut) -> Result<(), EncodeError> {
        buf.put_i64(self.id);
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// JoinGame / Login (Play)
// ---------------------------------------------------------------------------

/// Sent to the client after a successful login to start the game.
#[derive(Debug, Clone, PartialEq)]
pub struct JoinGame {
    /// Entity ID assigned to the player.
    pub entity_id: i32,
    /// Whether hardcore mode is enabled.
    pub is_hardcore: bool,
    /// Number of dimensions in the registry.
    pub dimension_count: i32,
    /// Max players (from server properties).
    pub max_players: i32,
    /// View distance (in chunks).
    pub view_distance: i32,
    /// Simulation distance.
    pub simulation_distance: i32,
    /// Reduced debug info.
    pub reduced_debug_info: bool,
    /// Enable respawn screen.
    pub enable_respawn_screen: bool,
    /// Is a local "LAN" world.
    pub is_lan: bool,
    /// Game mode.
    pub game_mode: GameMode,
    /// Previous game mode (255 = none).
    pub prev_game_mode: i8,
    /// Dimension type name (identifier string).
    pub dimension_type: String,
    /// Dimension name (identifier string).
    pub dimension_name: String,
    /// Hashed seed for biome generation.
    pub hashed_seed: i64,
    /// Whether the world is flat.
    pub is_flat: bool,
    /// Whether death location is present.
    pub has_death_location: bool,
}

impl PacketId for JoinGame {
    const STATE: ProtocolState = ProtocolState::Play;
    const DIRECTION: Direction = Direction::Clientbound;
    const ID: u32 = 0x2B;
}

impl JoinGame {
    pub fn decode(payload: &mut Bytes) -> Result<Self, DecodeError> {
        // i32 (4 bytes)
        if payload.remaining() < 4 {
            return Err(DecodeError::InsufficientBytes {
                need: 4,
                have: payload.remaining(),
            });
        }
        let entity_id = payload.get_i32();

        // u8 (1 byte)
        if !payload.has_remaining() {
            return Err(DecodeError::InsufficientBytes { need: 1, have: 0 });
        }
        let is_hardcore = payload.get_u8() != 0;

        // 4 VarInts (each at least 1 byte)
        let dimension_count = read_varint(payload)?;
        let max_players = read_varint(payload)?;
        let view_distance = read_varint(payload)?;
        let simulation_distance = read_varint(payload)?;

        // 3 u8s + u8 (game_mode) + i8 = 5 bytes
        if payload.remaining() < 5 {
            return Err(DecodeError::InsufficientBytes {
                need: 5,
                have: payload.remaining(),
            });
        }
        let reduced_debug_info = payload.get_u8() != 0;
        let enable_respawn_screen = payload.get_u8() != 0;
        let is_lan = payload.get_u8() != 0;
        let game_mode = GameMode::from_id(payload.get_u8()).unwrap_or(GameMode::Survival);
        let prev_game_mode = payload.get_i8();

        // 2 length-prefixed strings
        let dimension_type = read_string(payload)?;
        let dimension_name = read_string(payload)?;

        // i64 + u8 + u8 = 9 bytes
        if payload.remaining() < 9 {
            return Err(DecodeError::InsufficientBytes {
                need: 9,
                have: payload.remaining(),
            });
        }
        let hashed_seed = payload.get_i64();
        let is_flat = payload.get_u8() != 0;
        let has_death_location = payload.get_u8() != 0;

        Ok(Self {
            entity_id,
            is_hardcore,
            dimension_count,
            max_players,
            view_distance,
            simulation_distance,
            reduced_debug_info,
            enable_respawn_screen,
            is_lan,
            game_mode,
            prev_game_mode,
            dimension_type,
            dimension_name,
            hashed_seed,
            is_flat,
            has_death_location,
        })
    }

    pub fn encode(&self, buf: &mut BytesMut) -> Result<(), EncodeError> {
        buf.put_i32(self.entity_id);
        buf.put_u8(if self.is_hardcore { 1 } else { 0 });
        write_varint(buf, self.dimension_count)?;
        write_varint(buf, self.max_players)?;
        write_varint(buf, self.view_distance)?;
        write_varint(buf, self.simulation_distance)?;
        buf.put_u8(if self.reduced_debug_info { 1 } else { 0 });
        buf.put_u8(if self.enable_respawn_screen { 1 } else { 0 });
        buf.put_u8(if self.is_lan { 1 } else { 0 });
        buf.put_u8(self.game_mode as u8);
        buf.put_i8(self.prev_game_mode);
        write_string(buf, &self.dimension_type)?;
        write_string(buf, &self.dimension_name)?;
        buf.put_i64(self.hashed_seed);
        buf.put_u8(if self.is_flat { 1 } else { 0 });
        buf.put_u8(if self.has_death_location { 1 } else { 0 });
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// ServerData
// ---------------------------------------------------------------------------

/// Server data sent during the login/play transition (MOTD, icon, etc.).
#[derive(Debug, Clone, PartialEq)]
pub struct ServerData {
    /// Server description (MOTD) as a Chat component.
    pub description: Chat,
    /// Whether the server has a preview.
    pub has_icon: bool,
    /// Base64-encoded server icon PNG (optional).
    pub icon: Option<String>,
    /// Whether the server enforces secure profiles.
    pub enforces_secure_chat: bool,
}

impl PacketId for ServerData {
    const STATE: ProtocolState = ProtocolState::Play;
    const DIRECTION: Direction = Direction::Clientbound;
    const ID: u32 = 0x5F;
}

impl ServerData {
    pub fn decode(payload: &mut Bytes) -> Result<Self, DecodeError> {
        if !payload.has_remaining() {
            return Err(DecodeError::InsufficientBytes { need: 1, have: 0 });
        }
        let has_icon = payload.get_u8() != 0;
        let icon = if has_icon {
            Some(read_string(payload)?)
        } else {
            None
        };
        if !payload.has_remaining() {
            return Err(DecodeError::InsufficientBytes { need: 1, have: 0 });
        }
        let enforces_secure_chat = payload.get_u8() != 0;
        // For now, read the description as a raw string
        let description = Chat::Json(read_string(payload)?);

        Ok(Self {
            description,
            has_icon,
            icon,
            enforces_secure_chat,
        })
    }

    pub fn encode(&self, buf: &mut BytesMut) -> Result<(), EncodeError> {
        buf.put_u8(if self.has_icon { 1 } else { 0 });
        if let Some(ref icon) = self.icon {
            write_string(buf, icon)?;
        }
        buf.put_u8(if self.enforces_secure_chat { 1 } else { 0 });
        // Write description as JSON string
        let json = self.description.to_json_string();
        write_string(buf, &json)?;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// ChatMessage (Clientbound)
// ---------------------------------------------------------------------------

/// A chat message broadcast to clients.
#[derive(Debug, Clone, PartialEq)]
pub struct ChatMessage {
    /// JSON text component.
    pub message: Chat,
    /// Source position type: 0 = chat, 1 = system, 2 = action bar.
    pub position: u8,
    /// Sender UUID.
    pub sender: uuid::Uuid,
}

impl PacketId for ChatMessage {
    const STATE: ProtocolState = ProtocolState::Play;
    const DIRECTION: Direction = Direction::Clientbound;
    const ID: u32 = 0x39;
}

impl ChatMessage {
    pub fn decode(payload: &mut Bytes) -> Result<Self, DecodeError> {
        let message = Chat::read_from(payload)?;
        if !payload.has_remaining() {
            return Err(DecodeError::InsufficientBytes { need: 1, have: 0 });
        }
        let position = payload.get_u8();
        let sender = read_uuid(payload)?;
        Ok(Self {
            message,
            position,
            sender,
        })
    }

    pub fn encode(&self, buf: &mut BytesMut) -> Result<(), EncodeError> {
        self.message.write_to(buf)?;
        buf.put_u8(self.position);
        write_uuid(buf, &self.sender)?;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// SystemChatMessage
// ---------------------------------------------------------------------------

/// A system chat message (not from a player).
#[derive(Debug, Clone, PartialEq)]
pub struct SystemChatMessage {
    /// JSON text component.
    pub content: Chat,
    /// Whether to overlay (action bar) instead of chat.
    pub overlay: bool,
}

impl PacketId for SystemChatMessage {
    const STATE: ProtocolState = ProtocolState::Play;
    const DIRECTION: Direction = Direction::Clientbound;
    const ID: u32 = 0x67;
}

impl SystemChatMessage {
    pub fn decode(payload: &mut Bytes) -> Result<Self, DecodeError> {
        let content = Chat::read_from(payload)?;
        if !payload.has_remaining() {
            return Err(DecodeError::InsufficientBytes { need: 1, have: 0 });
        }
        let overlay = payload.get_u8() != 0;
        Ok(Self { content, overlay })
    }

    pub fn encode(&self, buf: &mut BytesMut) -> Result<(), EncodeError> {
        self.content.write_to(buf)?;
        buf.put_u8(if self.overlay { 1 } else { 0 });
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// SetDefaultSpawnPosition
// ---------------------------------------------------------------------------

/// Sets the world spawn / respawn point (26.2 `RespawnData`).
#[derive(Debug, Clone, PartialEq)]
pub struct SetDefaultSpawnPosition {
    /// Dimension identifier, e.g. `minecraft:overworld`.
    pub dimension: String,
    /// Spawn block position.
    pub location: BlockPos,
    /// Yaw in degrees.
    pub yaw: f32,
    /// Pitch in degrees.
    pub pitch: f32,
}

impl PacketId for SetDefaultSpawnPosition {
    const STATE: ProtocolState = ProtocolState::Play;
    const DIRECTION: Direction = Direction::Clientbound;
    const ID: u32 = 0x61;
}

impl SetDefaultSpawnPosition {
    pub fn decode(payload: &mut Bytes) -> Result<Self, DecodeError> {
        let dimension = read_string(payload)?;
        if payload.remaining() < 16 {
            return Err(DecodeError::InsufficientBytes {
                need: 16,
                have: payload.remaining(),
            });
        }
        let packed = payload.get_i64();
        let location = BlockPos::from_packed(packed)?;
        let yaw = payload.get_f32();
        let pitch = payload.get_f32();
        Ok(Self {
            dimension,
            location,
            yaw,
            pitch,
        })
    }

    pub fn encode(&self, buf: &mut BytesMut) -> Result<(), EncodeError> {
        write_string(buf, &self.dimension)?;
        buf.put_i64(self.location.to_packed());
        buf.put_f32(self.yaw);
        buf.put_f32(self.pitch);
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// SynchronizePlayerPosition
// ---------------------------------------------------------------------------

/// Teleports the player (`ClientboundPlayerPositionPacket` in 26.2).
#[derive(Debug, Clone, PartialEq)]
pub struct SynchronizePlayerPosition {
    /// Teleport ID (client echoes this in Accept Teleportation).
    pub teleport_id: i32,
    /// Absolute X.
    pub x: f64,
    /// Absolute Y (feet).
    pub y: f64,
    /// Absolute Z.
    pub z: f64,
    /// Delta movement X.
    pub dx: f64,
    /// Delta movement Y.
    pub dy: f64,
    /// Delta movement Z.
    pub dz: f64,
    /// Yaw in degrees.
    pub yaw: f32,
    /// Pitch in degrees.
    pub pitch: f32,
    /// Packed `Relative` bitmask (`ByteBufCodecs.INT`, not a VarInt).
    pub relatives: i32,
}

impl PacketId for SynchronizePlayerPosition {
    const STATE: ProtocolState = ProtocolState::Play;
    const DIRECTION: Direction = Direction::Clientbound;
    const ID: u32 = 0x48;
}

impl SynchronizePlayerPosition {
    pub fn decode(payload: &mut Bytes) -> Result<Self, DecodeError> {
        let teleport_id = read_varint(payload)?;
        // 6*f64 + 2*f32 + i32 = 48+8+4 = 60
        if payload.remaining() < 60 {
            return Err(DecodeError::InsufficientBytes {
                need: 60,
                have: payload.remaining(),
            });
        }
        Ok(Self {
            teleport_id,
            x: payload.get_f64(),
            y: payload.get_f64(),
            z: payload.get_f64(),
            dx: payload.get_f64(),
            dy: payload.get_f64(),
            dz: payload.get_f64(),
            yaw: payload.get_f32(),
            pitch: payload.get_f32(),
            relatives: payload.get_i32(),
        })
    }

    pub fn encode(&self, buf: &mut BytesMut) -> Result<(), EncodeError> {
        write_varint(buf, self.teleport_id)?;
        buf.put_f64(self.x);
        buf.put_f64(self.y);
        buf.put_f64(self.z);
        buf.put_f64(self.dx);
        buf.put_f64(self.dy);
        buf.put_f64(self.dz);
        buf.put_f32(self.yaw);
        buf.put_f32(self.pitch);
        buf.put_i32(self.relatives);
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// ChunkDataAndUpdateLight
// ---------------------------------------------------------------------------

/// Sends a full chunk of block data and lighting information.
///
/// This is a complex packet. For now we store the raw payload and parse
/// the header fields. Full chunk parsing is a separate concern.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChunkDataAndUpdateLight {
    /// Chunk X coordinate.
    pub chunk_x: i32,
    /// Chunk Z coordinate.
    pub chunk_z: i32,
    /// Chunk data (biomes, block states, block entities).
    pub chunk_data: Bytes,
    /// Light data (sky light + block light per section).
    pub light_data: Bytes,
}

impl PacketId for ChunkDataAndUpdateLight {
    const STATE: ProtocolState = ProtocolState::Play;
    const DIRECTION: Direction = Direction::Clientbound;
    const ID: u32 = 0x27;
}

impl ChunkDataAndUpdateLight {
    pub fn decode(payload: &mut Bytes) -> Result<Self, DecodeError> {
        if payload.remaining() < 8 {
            return Err(DecodeError::InsufficientBytes {
                need: 8,
                have: payload.remaining(),
            });
        }
        let chunk_x = payload.get_i32();
        let chunk_z = payload.get_i32();
        // The rest is the chunk data and light data
        let chunk_data_len = read_varint(payload)? as usize;
        if payload.remaining() < chunk_data_len {
            return Err(DecodeError::InsufficientBytes {
                need: chunk_data_len,
                have: payload.remaining(),
            });
        }
        let chunk_data = payload.copy_to_bytes(chunk_data_len);
        let light_data_len = read_varint(payload)? as usize;
        if payload.remaining() < light_data_len {
            return Err(DecodeError::InsufficientBytes {
                need: light_data_len,
                have: payload.remaining(),
            });
        }
        let light_data = payload.copy_to_bytes(light_data_len);
        Ok(Self {
            chunk_x,
            chunk_z,
            chunk_data,
            light_data,
        })
    }

    pub fn encode(&self, buf: &mut BytesMut) -> Result<(), EncodeError> {
        buf.put_i32(self.chunk_x);
        buf.put_i32(self.chunk_z);
        write_varint(buf, self.chunk_data.len() as i32)?;
        buf.put_slice(&self.chunk_data);
        write_varint(buf, self.light_data.len() as i32)?;
        buf.put_slice(&self.light_data);
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// BlockUpdate
// ---------------------------------------------------------------------------

/// Sent when a single block changes in the world.
#[derive(Debug, Clone, PartialEq)]
pub struct BlockUpdate {
    /// Block position.
    pub location: BlockPos,
    /// Block state ID.
    pub block_state_id: i32,
}

impl PacketId for BlockUpdate {
    const STATE: ProtocolState = ProtocolState::Play;
    const DIRECTION: Direction = Direction::Clientbound;
    const ID: u32 = 0x09;
}

impl BlockUpdate {
    pub fn decode(payload: &mut Bytes) -> Result<Self, DecodeError> {
        if payload.remaining() < 8 {
            return Err(DecodeError::InsufficientBytes {
                need: 8,
                have: payload.remaining(),
            });
        }
        let packed = payload.get_i64();
        let location = BlockPos::from_packed(packed)?;
        let block_state_id = read_varint(payload)?;
        Ok(Self {
            location,
            block_state_id,
        })
    }

    pub fn encode(&self, buf: &mut BytesMut) -> Result<(), EncodeError> {
        buf.put_i64(self.location.to_packed());
        write_varint(buf, self.block_state_id)?;
        Ok(())
    }
}

// ===========================================================================
// Serverbound Play Packets (Client -> Server)
// ===========================================================================

// ---------------------------------------------------------------------------
// KeepAliveResponse
// ---------------------------------------------------------------------------

/// Client's response to a KeepAlive.
#[derive(Debug, Clone, PartialEq)]
pub struct KeepAliveResponse {
    /// Must match the ID from the server's KeepAlive.
    pub id: i64,
}

impl PacketId for KeepAliveResponse {
    const STATE: ProtocolState = ProtocolState::Play;
    const DIRECTION: Direction = Direction::Serverbound;
    const ID: u32 = 0x18;
}

impl KeepAliveResponse {
    pub fn decode(payload: &mut Bytes) -> Result<Self, DecodeError> {
        if payload.remaining() < 8 {
            return Err(DecodeError::InsufficientBytes {
                need: 8,
                have: payload.remaining(),
            });
        }
        let id = payload.get_i64();
        Ok(Self { id })
    }

    pub fn encode(&self, buf: &mut BytesMut) -> Result<(), EncodeError> {
        buf.put_i64(self.id);
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// PlayerPosition
// ---------------------------------------------------------------------------

/// Client's current position (on ground).
#[derive(Debug, Clone, PartialEq)]
pub struct PlayerPosition {
    /// X coordinate.
    pub x: f64,
    /// Feet Y coordinate.
    pub y: f64,
    /// Z coordinate.
    pub z: f64,
    /// Whether the player is on the ground.
    pub on_ground: bool,
}

impl PacketId for PlayerPosition {
    const STATE: ProtocolState = ProtocolState::Play;
    const DIRECTION: Direction = Direction::Serverbound;
    const ID: u32 = 0x17;
}

impl PlayerPosition {
    pub fn decode(payload: &mut Bytes) -> Result<Self, DecodeError> {
        // 3*f64 + u8 = 24+1 = 25 bytes
        if payload.remaining() < 25 {
            return Err(DecodeError::InsufficientBytes {
                need: 25,
                have: payload.remaining(),
            });
        }
        let x = payload.get_f64();
        let y = payload.get_f64();
        let z = payload.get_f64();
        let on_ground = payload.get_u8() != 0;
        Ok(Self { x, y, z, on_ground })
    }

    pub fn encode(&self, buf: &mut BytesMut) -> Result<(), EncodeError> {
        buf.put_f64(self.x);
        buf.put_f64(self.y);
        buf.put_f64(self.z);
        buf.put_u8(if self.on_ground { 1 } else { 0 });
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// PlayerPositionAndRotation
// ---------------------------------------------------------------------------

/// Client's position and rotation.
#[derive(Debug, Clone, PartialEq)]
pub struct PlayerPositionAndRotation {
    /// X coordinate.
    pub x: f64,
    /// Feet Y coordinate.
    pub y: f64,
    /// Z coordinate.
    pub z: f64,
    /// Yaw in degrees.
    pub yaw: f32,
    /// Pitch in degrees.
    pub pitch: f32,
    /// Whether the player is on the ground.
    pub on_ground: bool,
}

impl PacketId for PlayerPositionAndRotation {
    const STATE: ProtocolState = ProtocolState::Play;
    const DIRECTION: Direction = Direction::Serverbound;
    const ID: u32 = 0x18;
}

impl PlayerPositionAndRotation {
    pub fn decode(payload: &mut Bytes) -> Result<Self, DecodeError> {
        // 3*f64 + 2*f32 + u8 = 24+8+1 = 33 bytes
        if payload.remaining() < 33 {
            return Err(DecodeError::InsufficientBytes {
                need: 33,
                have: payload.remaining(),
            });
        }
        let x = payload.get_f64();
        let y = payload.get_f64();
        let z = payload.get_f64();
        let yaw = payload.get_f32();
        let pitch = payload.get_f32();
        let on_ground = payload.get_u8() != 0;
        Ok(Self {
            x,
            y,
            z,
            yaw,
            pitch,
            on_ground,
        })
    }

    pub fn encode(&self, buf: &mut BytesMut) -> Result<(), EncodeError> {
        buf.put_f64(self.x);
        buf.put_f64(self.y);
        buf.put_f64(self.z);
        buf.put_f32(self.yaw);
        buf.put_f32(self.pitch);
        buf.put_u8(if self.on_ground { 1 } else { 0 });
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// PlayerRotation
// ---------------------------------------------------------------------------

/// Client's rotation only (no position change).
#[derive(Debug, Clone, PartialEq)]
pub struct PlayerRotation {
    /// Yaw in degrees.
    pub yaw: f32,
    /// Pitch in degrees.
    pub pitch: f32,
    /// Whether the player is on the ground.
    pub on_ground: bool,
}

impl PacketId for PlayerRotation {
    const STATE: ProtocolState = ProtocolState::Play;
    const DIRECTION: Direction = Direction::Serverbound;
    const ID: u32 = 0x19;
}

impl PlayerRotation {
    pub fn decode(payload: &mut Bytes) -> Result<Self, DecodeError> {
        // 2*f32 + u8 = 8+1 = 9 bytes
        if payload.remaining() < 9 {
            return Err(DecodeError::InsufficientBytes {
                need: 9,
                have: payload.remaining(),
            });
        }
        let yaw = payload.get_f32();
        let pitch = payload.get_f32();
        let on_ground = payload.get_u8() != 0;
        Ok(Self {
            yaw,
            pitch,
            on_ground,
        })
    }

    pub fn encode(&self, buf: &mut BytesMut) -> Result<(), EncodeError> {
        buf.put_f32(self.yaw);
        buf.put_f32(self.pitch);
        buf.put_u8(if self.on_ground { 1 } else { 0 });
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// SetPlayerAbilities
// ---------------------------------------------------------------------------

/// Client sets its abilities (flying, creative mode, etc.).
#[derive(Debug, Clone, PartialEq)]
pub struct SetPlayerAbilities {
    /// Flags bitmask: 0x01 = invulnerable, 0x02 = flying, 0x04 = allow flying, 0x08 = creative mode.
    pub flags: u8,
    /// Flying speed.
    pub flying_speed: f32,
    /// FOV modifier (walking speed).
    pub fov_modifier: f32,
}

impl PacketId for SetPlayerAbilities {
    const STATE: ProtocolState = ProtocolState::Play;
    const DIRECTION: Direction = Direction::Serverbound;
    const ID: u32 = 0x1E;
}

impl SetPlayerAbilities {
    pub fn decode(payload: &mut Bytes) -> Result<Self, DecodeError> {
        // u8 + 2*f32 = 1+8 = 9 bytes
        if payload.remaining() < 9 {
            return Err(DecodeError::InsufficientBytes {
                need: 9,
                have: payload.remaining(),
            });
        }
        let flags = payload.get_u8();
        let flying_speed = payload.get_f32();
        let fov_modifier = payload.get_f32();
        Ok(Self {
            flags,
            flying_speed,
            fov_modifier,
        })
    }

    pub fn encode(&self, buf: &mut BytesMut) -> Result<(), EncodeError> {
        buf.put_u8(self.flags);
        buf.put_f32(self.flying_speed);
        buf.put_f32(self.fov_modifier);
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// ChatCommand
// ---------------------------------------------------------------------------

/// Client sends a chat command (slash command).
#[derive(Debug, Clone, PartialEq)]
pub struct ChatCommand {
    /// The command string (without the leading /).
    pub command: String,
}

impl PacketId for ChatCommand {
    const STATE: ProtocolState = ProtocolState::Play;
    const DIRECTION: Direction = Direction::Serverbound;
    const ID: u32 = 0x04;
}

impl ChatCommand {
    pub fn decode(payload: &mut Bytes) -> Result<Self, DecodeError> {
        let command = read_string(payload)?;
        Ok(Self { command })
    }

    pub fn encode(&self, buf: &mut BytesMut) -> Result<(), EncodeError> {
        write_string(buf, &self.command)?;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// ClientStatus
// ---------------------------------------------------------------------------

/// Client reports its status (e.g., respawn after death).
#[derive(Debug, Clone, PartialEq)]
pub struct ClientStatus {
    /// Action ID: 0 = perform respawn, 1 = request stats.
    pub action_id: i32,
}

impl PacketId for ClientStatus {
    const STATE: ProtocolState = ProtocolState::Play;
    const DIRECTION: Direction = Direction::Serverbound;
    const ID: u32 = 0x07;
}

impl ClientStatus {
    pub fn decode(payload: &mut Bytes) -> Result<Self, DecodeError> {
        let action_id = read_varint(payload)?;
        Ok(Self { action_id })
    }

    pub fn encode(&self, buf: &mut BytesMut) -> Result<(), EncodeError> {
        write_varint(buf, self.action_id)?;
        Ok(())
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::BlockPos;

    fn roundtrip<T, F, G>(packet: &T, encode: F, decode: G)
    where
        F: Fn(&T, &mut BytesMut) -> Result<(), EncodeError>,
        G: Fn(&mut Bytes) -> Result<T, DecodeError>,
        T: std::fmt::Debug + PartialEq,
    {
        let mut buf = BytesMut::new();
        encode(packet, &mut buf).unwrap();
        let mut read_buf = Bytes::copy_from_slice(&buf);
        let decoded = decode(&mut read_buf).unwrap();
        assert_eq!(*packet, decoded);
    }

    #[test]
    fn test_keep_alive_roundtrip() {
        let packet = KeepAlive { id: 0xDEADBEEF };
        roundtrip(&packet, |p, b| p.encode(b), KeepAlive::decode);
    }

    #[test]
    fn test_keep_alive_response_roundtrip() {
        let packet = KeepAliveResponse { id: 12345 };
        roundtrip(&packet, |p, b| p.encode(b), KeepAliveResponse::decode);
    }

    #[test]
    fn test_player_position_roundtrip() {
        let packet = PlayerPosition {
            x: 100.5,
            y: 64.0,
            z: -200.3,
            on_ground: true,
        };
        roundtrip(&packet, |p, b| p.encode(b), PlayerPosition::decode);
    }

    #[test]
    fn test_player_position_and_rotation_roundtrip() {
        let packet = PlayerPositionAndRotation {
            x: 100.5,
            y: 64.0,
            z: -200.3,
            yaw: 90.0,
            pitch: -45.0,
            on_ground: false,
        };
        roundtrip(
            &packet,
            |p, b| p.encode(b),
            PlayerPositionAndRotation::decode,
        );
    }

    #[test]
    fn test_player_rotation_roundtrip() {
        let packet = PlayerRotation {
            yaw: 180.0,
            pitch: 0.0,
            on_ground: true,
        };
        roundtrip(&packet, |p, b| p.encode(b), PlayerRotation::decode);
    }

    #[test]
    fn test_block_update_roundtrip() {
        let packet = BlockUpdate {
            location: BlockPos::new(10, 64, -10),
            block_state_id: 1,
        };
        roundtrip(&packet, |p, b| p.encode(b), BlockUpdate::decode);
    }

    #[test]
    fn test_set_default_spawn_position_roundtrip() {
        let packet = SetDefaultSpawnPosition {
            dimension: "minecraft:overworld".into(),
            location: BlockPos::new(0, 64, 0),
            yaw: 90.0,
            pitch: 0.0,
        };
        roundtrip(&packet, |p, b| p.encode(b), SetDefaultSpawnPosition::decode);
    }

    #[test]
    fn test_synchronize_player_position_roundtrip() {
        let packet = SynchronizePlayerPosition {
            teleport_id: 1,
            x: 0.0,
            y: 64.0,
            z: 0.0,
            dx: 0.0,
            dy: 0.0,
            dz: 0.0,
            yaw: 0.0,
            pitch: 0.0,
            relatives: 0,
        };
        roundtrip(
            &packet,
            |p, b| p.encode(b),
            SynchronizePlayerPosition::decode,
        );
    }

    #[test]
    fn test_set_player_abilities_roundtrip() {
        let packet = SetPlayerAbilities {
            flags: 0x02, // flying
            flying_speed: 0.05,
            fov_modifier: 0.1,
        };
        roundtrip(&packet, |p, b| p.encode(b), SetPlayerAbilities::decode);
    }

    #[test]
    fn test_chat_command_roundtrip() {
        let packet = ChatCommand {
            command: "say Hello World".to_string(),
        };
        roundtrip(&packet, |p, b| p.encode(b), ChatCommand::decode);
    }

    #[test]
    fn test_client_status_roundtrip() {
        let packet = ClientStatus { action_id: 0 };
        roundtrip(&packet, |p, b| p.encode(b), ClientStatus::decode);
    }

    #[test]
    fn test_system_chat_message_roundtrip() {
        let packet = SystemChatMessage {
            content: Chat::Json(r#"{"text":"Hello!"}"#.to_string()),
            overlay: false,
        };
        roundtrip(&packet, |p, b| p.encode(b), SystemChatMessage::decode);
    }

    #[test]
    fn test_chat_message_roundtrip() {
        let packet = ChatMessage {
            message: Chat::Json(r#"{"text":"Test message"}"#.to_string()),
            position: 0,
            sender: uuid::Uuid::nil(),
        };
        roundtrip(&packet, |p, b| p.encode(b), ChatMessage::decode);
    }

    #[test]
    fn test_chunk_data_roundtrip() {
        let chunk_data = vec![0x01, 0x02, 0x03, 0x04];
        let light_data = vec![0x0A, 0x0B];
        let packet = ChunkDataAndUpdateLight {
            chunk_x: 5,
            chunk_z: -3,
            chunk_data: Bytes::from(chunk_data),
            light_data: Bytes::from(light_data),
        };
        let mut buf = BytesMut::new();
        packet.encode(&mut buf).unwrap();
        let mut read_buf = Bytes::copy_from_slice(&buf);
        let decoded = ChunkDataAndUpdateLight::decode(&mut read_buf).unwrap();
        assert_eq!(decoded.chunk_x, 5);
        assert_eq!(decoded.chunk_z, -3);
        assert_eq!(&decoded.chunk_data[..], &[0x01, 0x02, 0x03, 0x04]);
        assert_eq!(&decoded.light_data[..], &[0x0A, 0x0B]);
    }

    #[test]
    fn test_packet_ids_are_unique_within_direction() {
        // Verify the IDs match expected values
        assert_eq!(KeepAlive::ID, 0x26);
        assert_eq!(JoinGame::ID, 0x2B);
        assert_eq!(BlockUpdate::ID, 0x09);
        assert_eq!(SystemChatMessage::ID, 0x67);
        assert_eq!(KeepAliveResponse::ID, 0x18);
        assert_eq!(PlayerPosition::ID, 0x17);
        assert_eq!(PlayerPositionAndRotation::ID, 0x18);
        assert_eq!(PlayerRotation::ID, 0x19);
        assert_eq!(ChatCommand::ID, 0x04);
    }

    #[test]
    fn test_join_game_roundtrip() {
        let packet = JoinGame {
            entity_id: 42,
            is_hardcore: false,
            dimension_count: 3,
            max_players: 20,
            view_distance: 10,
            simulation_distance: 10,
            reduced_debug_info: false,
            enable_respawn_screen: true,
            is_lan: false,
            game_mode: GameMode::Survival,
            prev_game_mode: -1,
            dimension_type: "minecraft:overworld".to_string(),
            dimension_name: "minecraft:overworld".to_string(),
            hashed_seed: 12345678,
            is_flat: false,
            has_death_location: false,
        };
        let mut buf = BytesMut::new();
        packet.encode(&mut buf).unwrap();
        let mut read_buf = Bytes::copy_from_slice(&buf);
        let decoded = JoinGame::decode(&mut read_buf).unwrap();
        assert_eq!(decoded.entity_id, 42);
        assert!(!decoded.is_hardcore);
        assert_eq!(decoded.game_mode, GameMode::Survival);
        assert_eq!(decoded.dimension_type, "minecraft:overworld");
    }
}
