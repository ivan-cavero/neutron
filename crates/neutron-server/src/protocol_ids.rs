//! Packet IDs for Minecraft 26.2 (protocol 776).
//!
//! Source: `java -DbundlerMainClass=net.minecraft.data.Main -jar server.jar --reports`
//! → `generated/reports/packets.json`.

// ---------------------------------------------------------------------------
// Handshake / status / login
// ---------------------------------------------------------------------------

pub const HANDSHAKE: u32 = 0x00;

pub const STATUS_RESPONSE: u32 = 0x00;
pub const STATUS_PONG: u32 = 0x01;
pub const STATUS_REQUEST: u32 = 0x00;
pub const STATUS_PING: u32 = 0x01;

pub const LOGIN_DISCONNECT: u32 = 0x00;
pub const LOGIN_FINISHED: u32 = 0x02;
pub const LOGIN_START: u32 = 0x00;
pub const LOGIN_ACKNOWLEDGED: u32 = 0x03;

// ---------------------------------------------------------------------------
// Configuration (clientbound)
// ---------------------------------------------------------------------------

pub const CFG_FINISH: u32 = 0x03;
pub const CFG_KEEP_ALIVE: u32 = 0x04;
pub const CFG_REGISTRY_DATA: u32 = 0x07;
pub const CFG_UPDATE_FEATURES: u32 = 0x0C;
pub const CFG_UPDATE_TAGS: u32 = 0x0D;
pub const CFG_SELECT_KNOWN_PACKS: u32 = 0x0E;

// Configuration (serverbound)
pub const CFG_SB_FINISH: u32 = 0x03;
pub const CFG_SB_KEEP_ALIVE: u32 = 0x04;
pub const CFG_SB_SELECT_KNOWN_PACKS: u32 = 0x07;

// ---------------------------------------------------------------------------
// Play (clientbound)
// ---------------------------------------------------------------------------

pub const PLAY_KEEP_ALIVE: u32 = 0x2C;
pub const PLAY_LEVEL_CHUNK: u32 = 0x2D;
pub const PLAY_LOGIN: u32 = 0x31;
pub const PLAY_ABILITIES: u32 = 0x40;
pub const PLAY_POSITION: u32 = 0x48;
pub const PLAY_CENTER_CHUNK: u32 = 0x5E;
pub const PLAY_DEFAULT_SPAWN: u32 = 0x61;
pub const PLAY_SYSTEM_CHAT: u32 = 0x79;
pub const PLAY_GAME_EVENT: u32 = 0x26;
pub const PLAY_CHUNK_BATCH_START: u32 = 0x0C;
pub const PLAY_CHUNK_BATCH_FINISHED: u32 = 0x0B;

/// Game event 13: client starts waiting for level chunks.
pub const GAME_EVENT_CHUNKS_LOAD_START: u8 = 13;

// Play (serverbound)
pub const PLAY_SB_ACCEPT_TELEPORT: u32 = 0x00;
pub const PLAY_SB_CHAT_COMMAND: u32 = 0x07;
pub const PLAY_SB_CLIENT_COMMAND: u32 = 0x0C;
pub const PLAY_SB_KEEP_ALIVE: u32 = 0x1C;
pub const PLAY_SB_MOVE_POS: u32 = 0x1E;
pub const PLAY_SB_MOVE_POS_ROT: u32 = 0x1F;
pub const PLAY_SB_MOVE_ROT: u32 = 0x20;
pub const PLAY_SB_ABILITIES: u32 = 0x28;
pub const PLAY_SB_CHUNK_BATCH: u32 = 0x0B;
