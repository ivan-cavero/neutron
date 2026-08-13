//! Login flow handler for Minecraft 26.2 with online-mode=false.
//!
//! Sequence:
//! 1. Receive LoginStart (username, uuid)
//! 2. Send LoginSuccess (uuid, username, num_properties=0)
//! 3. Transition to Play state
//! 4. Send JoinGame
//! 5. Send RegistryData (dimension_type, biome)
//! 6. Send SetDefaultSpawnPosition
//! 7. Send SynchronizePlayerPosition
//! 8. Send initial chunks
//!
//! Compression is skipped for simplicity (no SetCompression sent).

use bytes::{Buf, BufMut, Bytes, BytesMut};
use neutron_protocol::types::{read_string, read_uuid, write_varint};
use std::net::SocketAddr;

use tokio::sync::mpsc;

use crate::chunk_sender;
use crate::connection::{send_packet, OutgoingPacket};
use crate::server::SharedServer;
use neutron_protocol::codec::MinecraftCodec;

// ---------------------------------------------------------------------------
// Play-state packet IDs (used during login transition)
// ---------------------------------------------------------------------------
const PLAY_JOIN_GAME: u32 = 0x2B;
const PLAY_REGISTRY_DATA: u32 = 0x5D;
const PLAY_DEFAULT_SPAWN: u32 = 0x54;
const PLAY_SYNC_PLAYER_POS: u32 = 0x40;
const PLAY_PLAYER_ABILITIES: u32 = 0x36;
const PLAY_SET_CENTER_CHUNK: u32 = 0x50;

// ---------------------------------------------------------------------------
// Handle login packets
// ---------------------------------------------------------------------------

/// Handle a packet during the Login state.
///
/// Returns `Ok(true)` when the player has fully transitioned to Play state.
pub async fn handle_login_packet(
    server: &SharedServer,
    tx: &mpsc::Sender<OutgoingPacket>,
    codec: &mut MinecraftCodec,
    packet_id: u32,
    payload: &mut Bytes,
    addr: SocketAddr,
    player_uuid: &mut Option<uuid::Uuid>,
) -> anyhow::Result<bool> {
    match packet_id {
        0x00 => {
            // LoginStart
            handle_login_start(server, tx, codec, payload, addr, player_uuid).await
        }
        _ => {
            tracing::warn!(
                addr = %addr,
                packet_id,
                "unexpected packet in login state"
            );
            Ok(false)
        }
    }
}

// ---------------------------------------------------------------------------
// LoginStart
// ---------------------------------------------------------------------------

async fn handle_login_start(
    server: &SharedServer,
    tx: &mpsc::Sender<OutgoingPacket>,
    codec: &mut MinecraftCodec,
    payload: &mut Bytes,
    addr: SocketAddr,
    player_uuid: &mut Option<uuid::Uuid>,
) -> anyhow::Result<bool> {
    // Decode LoginStart.
    let name = read_string(payload)?;
    let uuid = if payload.has_remaining() {
        read_uuid(payload)?
    } else {
        // Generate a UUID from the username if not provided.
        uuid::Uuid::new_v4()
    };

    tracing::info!(
        addr = %addr,
        username = %name,
        uuid = %uuid,
        "login start"
    );

    // Register the player.
    let entity_id = server.register_player(uuid, name.clone()).await;
    *player_uuid = Some(uuid);

    // Send LoginSuccess (packet ID 0x02 in Login state).
    let mut login_success_buf = BytesMut::new();
    login_success_buf.put_slice(uuid.as_bytes());
    {
        let name_bytes = name.as_bytes();
        write_varint(&mut login_success_buf, name_bytes.len() as i32)?;
        login_success_buf.put_slice(name_bytes);
    }
    write_varint(&mut login_success_buf, 0)?; // num_properties = 0

    send_packet(tx, codec, 0x02, &login_success_buf).await?;
    tracing::debug!(addr = %addr, "sent LoginSuccess");

    // Transition to Play state and send the play-sequence packets.
    send_play_sequence(server, tx, codec, uuid, entity_id, addr).await?;

    // Mark player as playing.
    if let Some(player) = server.players.write().await.get_mut(&uuid) {
        player.is_playing = true;
    }

    Ok(true)
}

// ---------------------------------------------------------------------------
// Play transition sequence
// ---------------------------------------------------------------------------

async fn send_play_sequence(
    server: &SharedServer,
    tx: &mpsc::Sender<OutgoingPacket>,
    codec: &MinecraftCodec,
    uuid: uuid::Uuid,
    entity_id: i32,
    addr: SocketAddr,
) -> anyhow::Result<()> {
    let config = server.config.clone();

    // 1. JoinGame (0x2B)
    send_join_game(tx, codec, entity_id, &config, addr).await?;

    // 2. RegistryData (0x5D) for dimension_type and biome
    send_registry_data(tx, codec, addr).await?;

    // 3. SetDefaultSpawnPosition (0x54)
    send_default_spawn(tx, codec, addr).await?;

    // 4. Player Abilities (0x36) — creative mode, flying allowed
    send_player_abilities(tx, codec, addr).await?;

    // 5. Set Center Chunk (0x50)
    send_set_center_chunk(tx, codec, 0, 0, addr).await?;

    // 6. SynchronizePlayerPosition (0x40) — spawn at 0, 65, 0
    send_sync_player_position(tx, codec, addr).await?;

    // 7. Send initial chunks around spawn
    send_initial_chunks(server, tx, codec, uuid, addr).await?;

    // 8. Send system chat message: "Welcome!"
    send_system_chat(tx, codec, "Welcome to Neutron!", addr).await?;

    tracing::info!(addr = %addr, "play sequence complete");
    Ok(())
}

// ---------------------------------------------------------------------------
// JoinGame
// ---------------------------------------------------------------------------

async fn send_join_game(
    tx: &mpsc::Sender<OutgoingPacket>,
    codec: &MinecraftCodec,
    entity_id: i32,
    config: &crate::server::ServerConfig,
    addr: SocketAddr,
) -> anyhow::Result<()> {
    let mut buf = BytesMut::with_capacity(128);
    buf.put_i32(entity_id);
    buf.put_u8(0); // is_hardcore = false
    write_varint(&mut buf, 1)?; // dimension_count (1 = overworld only)
    write_varint(&mut buf, config.max_players)?;
    write_varint(&mut buf, config.view_distance)?;
    write_varint(&mut buf, config.view_distance)?; // simulation_distance = view_distance
    buf.put_u8(0); // reduced_debug_info = false
    buf.put_u8(0); // enable_respawn_screen = false
    buf.put_u8(0); // is_lan = false
    buf.put_u8(1); // game_mode = creative
    buf.put_i8(-1); // prev_game_mode = none

    // dimension_type (identifier string)
    let dim_type = "minecraft:overworld";
    let dim_type_bytes = dim_type.as_bytes();
    write_varint(&mut buf, dim_type_bytes.len() as i32)?;
    buf.put_slice(dim_type_bytes);

    // dimension_name (identifier string)
    let dim_name = "minecraft:overworld";
    let dim_name_bytes = dim_name.as_bytes();
    write_varint(&mut buf, dim_name_bytes.len() as i32)?;
    buf.put_slice(dim_name_bytes);

    buf.put_i64(0); // hashed_seed
    buf.put_u8(0); // is_flat = false (we set this to false for now)
    buf.put_u8(0); // has_death_location = false

    send_packet(tx, codec, PLAY_JOIN_GAME, &buf).await?;
    tracing::debug!(addr = %addr, "sent JoinGame");
    Ok(())
}

// ---------------------------------------------------------------------------
// RegistryData
// ---------------------------------------------------------------------------

async fn send_registry_data(
    tx: &mpsc::Sender<OutgoingPacket>,
    codec: &MinecraftCodec,
    addr: SocketAddr,
) -> anyhow::Result<()> {
    // 1. dimension_type registry
    send_dimension_type_registry(tx, codec, addr).await?;

    // 2. biome registry
    send_biome_registry(tx, codec, addr).await?;

    Ok(())
}

async fn send_dimension_type_registry(
    tx: &mpsc::Sender<OutgoingPacket>,
    codec: &MinecraftCodec,
    addr: SocketAddr,
) -> anyhow::Result<()> {
    // RegistryData packet structure:
    // - String: registry identifier (e.g., "minecraft:dimension_type")
    // - Boolean: has_codec_data
    // - VarInt: number of entries
    // - For each entry:
    //   - String: entry identifier
    //   - Boolean: has_data (if true, NBT compound follows)
    // - String: "minecraft:dimension_type" (to signal codec follows)
    // - VarInt: 0 (codec size = 0, we don't send codec)
    // - VarInt: 0 (data size = 0, no data for the codec entry)

    let mut buf = BytesMut::with_capacity(512);

    // Registry identifier
    let reg_id = "minecraft:dimension_type";
    let reg_id_bytes = reg_id.as_bytes();
    write_varint(&mut buf, reg_id_bytes.len() as i32)?;
    buf.put_slice(reg_id_bytes);

    // has_codec_data = true
    buf.put_u8(1);

    // Number of entries = 1 (overworld)
    write_varint(&mut buf, 1)?;

    // Entry: "minecraft:overworld"
    let entry_id = "minecraft:overworld";
    let entry_id_bytes = entry_id.as_bytes();
    write_varint(&mut buf, entry_id_bytes.len() as i32)?;
    buf.put_slice(entry_id_bytes);

    // has_data = true
    buf.put_u8(1);

    // NBT data for overworld dimension type
    let nbt_data = build_overworld_dimension_nbt();
    buf.put_slice(&nbt_data);

    // Signal end of entries: send "minecraft:dimension_type" with no data
    let codec_entry = "minecraft:dimension_type";
    let codec_entry_bytes = codec_entry.as_bytes();
    write_varint(&mut buf, codec_entry_bytes.len() as i32)?;
    buf.put_slice(codec_entry_bytes);
    buf.put_u8(0); // has_data = false
    write_varint(&mut buf, 0)?; // codec size = 0
    write_varint(&mut buf, 0)?; // data size = 0

    send_packet(tx, codec, PLAY_REGISTRY_DATA, &buf).await?;
    tracing::debug!(addr = %addr, "sent dimension_type registry");
    Ok(())
}

async fn send_biome_registry(
    tx: &mpsc::Sender<OutgoingPacket>,
    codec: &MinecraftCodec,
    addr: SocketAddr,
) -> anyhow::Result<()> {
    let mut buf = BytesMut::with_capacity(256);

    // Registry identifier
    let reg_id = "minecraft:worldgen/biome";
    let reg_id_bytes = reg_id.as_bytes();
    write_varint(&mut buf, reg_id_bytes.len() as i32)?;
    buf.put_slice(reg_id_bytes);

    // has_codec_data = true
    buf.put_u8(1);

    // Number of entries = 1 (plains)
    write_varint(&mut buf, 1)?;

    // Entry: "minecraft:plains"
    let entry_id = "minecraft:plains";
    let entry_id_bytes = entry_id.as_bytes();
    write_varint(&mut buf, entry_id_bytes.len() as i32)?;
    buf.put_slice(entry_id_bytes);

    // has_data = true
    buf.put_u8(1);

    // NBT data for plains biome
    let nbt_data = build_plains_biome_nbt();
    buf.put_slice(&nbt_data);

    // Signal end of entries
    let codec_entry = "minecraft:worldgen/biome";
    let codec_entry_bytes = codec_entry.as_bytes();
    write_varint(&mut buf, codec_entry_bytes.len() as i32)?;
    buf.put_slice(codec_entry_bytes);
    buf.put_u8(0); // has_data = false
    write_varint(&mut buf, 0)?; // codec size = 0
    write_varint(&mut buf, 0)?; // data size = 0

    send_packet(tx, codec, PLAY_REGISTRY_DATA, &buf).await?;
    tracing::debug!(addr = %addr, "sent biome registry");
    Ok(())
}

// ---------------------------------------------------------------------------
// NBT builders for registry data
// ---------------------------------------------------------------------------

use ussr_nbt::mutf8::MString;
use ussr_nbt::owned::{Compound, Nbt, Tag};

fn build_overworld_dimension_nbt() -> Vec<u8> {
    let mut compound = Compound { tags: Vec::new() };

    // Required fields for dimension type
    compound
        .tags
        .push((MString::from("piglin_safe"), Tag::Byte(0)));
    compound
        .tags
        .push((MString::from("has_skylight"), Tag::Byte(1)));
    compound
        .tags
        .push((MString::from("has_ceiling"), Tag::Byte(0)));
    compound
        .tags
        .push((MString::from("ultrawarm"), Tag::Byte(0)));
    compound.tags.push((MString::from("natural"), Tag::Byte(1)));
    compound
        .tags
        .push((MString::from("coordinate_scale"), Tag::Double(1.0)));
    compound
        .tags
        .push((MString::from("bed_works"), Tag::Byte(1)));
    compound
        .tags
        .push((MString::from("respawn_anchor_works"), Tag::Byte(0)));
    compound
        .tags
        .push((MString::from("has_raids"), Tag::Byte(1)));
    compound.tags.push((MString::from("min_y"), Tag::Int(-64)));
    compound.tags.push((MString::from("height"), Tag::Int(384)));
    compound
        .tags
        .push((MString::from("logical_height"), Tag::Int(384)));
    compound.tags.push((
        MString::from("infiniburn"),
        Tag::String(MString::from("#minecraft:infiniburn_overworld")),
    ));
    compound.tags.push((
        MString::from("effects"),
        Tag::String(MString::from("minecraft:overworld")),
    ));
    compound
        .tags
        .push((MString::from("ambient_light"), Tag::Float(0.0)));

    let nbt = Nbt {
        name: MString::new(),
        compound,
    };
    let mut buf = Vec::new();
    nbt.write(&mut buf).expect("NBT write should not fail");
    buf
}

fn build_plains_biome_nbt() -> Vec<u8> {
    let mut compound = Compound { tags: Vec::new() };

    compound
        .tags
        .push((MString::from("has_precipitation"), Tag::Byte(1)));
    compound
        .tags
        .push((MString::from("temperature"), Tag::Float(0.8)));
    compound
        .tags
        .push((MString::from("downfall"), Tag::Float(0.4)));

    // effects compound
    let mut effects = Compound { tags: Vec::new() };
    effects
        .tags
        .push((MString::from("sky_color"), Tag::Int(7907327)));
    effects
        .tags
        .push((MString::from("water_fog_color"), Tag::Int(329011)));
    effects
        .tags
        .push((MString::from("fog_color"), Tag::Int(12638463)));
    effects
        .tags
        .push((MString::from("water_color"), Tag::Int(4159204)));
    effects
        .tags
        .push((MString::from("grass_color"), Tag::Int(9286496)));
    compound
        .tags
        .push((MString::from("effects"), Tag::Compound(effects)));

    let nbt = Nbt {
        name: MString::new(),
        compound,
    };
    let mut buf = Vec::new();
    nbt.write(&mut buf).expect("NBT write should not fail");
    buf
}

// ---------------------------------------------------------------------------
// Default spawn position
// ---------------------------------------------------------------------------

async fn send_default_spawn(
    tx: &mpsc::Sender<OutgoingPacket>,
    codec: &MinecraftCodec,
    addr: SocketAddr,
) -> anyhow::Result<()> {
    let mut buf = BytesMut::with_capacity(16);
    // Spawn at (0, 65, 0) packed as i64.
    let spawn = neutron_protocol::types::BlockPos::new(0, 65, 0);
    buf.put_i64(spawn.to_packed());
    buf.put_f32(0.0); // angle

    send_packet(tx, codec, PLAY_DEFAULT_SPAWN, &buf).await?;
    tracing::debug!(addr = %addr, "sent default spawn position");
    Ok(())
}

// ---------------------------------------------------------------------------
// Player Abilities
// ---------------------------------------------------------------------------

async fn send_player_abilities(
    tx: &mpsc::Sender<OutgoingPacket>,
    codec: &MinecraftCodec,
    addr: SocketAddr,
) -> anyhow::Result<()> {
    let mut buf = BytesMut::with_capacity(9);
    // flags: 0x01 = invulnerable, 0x04 = allow flying
    buf.put_u8(0x01 | 0x04); // invulnerable + allow flying
    buf.put_f32(0.05); // flying speed
    buf.put_f32(0.1); // FOV modifier

    send_packet(tx, codec, PLAY_PLAYER_ABILITIES, &buf).await?;
    tracing::debug!(addr = %addr, "sent player abilities");
    Ok(())
}

// ---------------------------------------------------------------------------
// Set Center Chunk
// ---------------------------------------------------------------------------

async fn send_set_center_chunk(
    tx: &mpsc::Sender<OutgoingPacket>,
    codec: &MinecraftCodec,
    chunk_x: i32,
    chunk_z: i32,
    addr: SocketAddr,
) -> anyhow::Result<()> {
    let mut buf = BytesMut::with_capacity(8);
    write_varint(&mut buf, chunk_x)?;
    write_varint(&mut buf, chunk_z)?;

    send_packet(tx, codec, PLAY_SET_CENTER_CHUNK, &buf).await?;
    tracing::debug!(addr = %addr, chunk_x, chunk_z, "sent set center chunk");
    Ok(())
}

// ---------------------------------------------------------------------------
// SynchronizePlayerPosition
// ---------------------------------------------------------------------------

async fn send_sync_player_position(
    tx: &mpsc::Sender<OutgoingPacket>,
    codec: &MinecraftCodec,
    addr: SocketAddr,
) -> anyhow::Result<()> {
    let mut buf = BytesMut::with_capacity(40);
    buf.put_f64(0.0); // x
    buf.put_f64(65.0); // y (above the grass at y=5)
    buf.put_f64(0.0); // z
    buf.put_f32(0.0); // yaw
    buf.put_f32(0.0); // pitch
    buf.put_u8(0); // flags = 0 (all absolute)
    write_varint(&mut buf, 0)?; // teleport_id = 0
    buf.put_u8(0); // dismount = false

    send_packet(tx, codec, PLAY_SYNC_PLAYER_POS, &buf).await?;
    tracing::debug!(addr = %addr, "sent sync player position");
    Ok(())
}

// ---------------------------------------------------------------------------
// System chat message
// ---------------------------------------------------------------------------

async fn send_system_chat(
    tx: &mpsc::Sender<OutgoingPacket>,
    codec: &MinecraftCodec,
    message: &str,
    addr: SocketAddr,
) -> anyhow::Result<()> {
    let json = format!(r#"{{"text":"{}"}}"#, message.replace('"', "\\\""));
    let mut buf = BytesMut::with_capacity(json.len() + 8);
    let json_bytes = json.as_bytes();
    write_varint(&mut buf, json_bytes.len() as i32)?;
    buf.put_slice(json_bytes);
    buf.put_u8(0); // overlay = false

    // SystemChatMessage packet ID: 0x67
    send_packet(tx, codec, 0x67, &buf).await?;
    tracing::debug!(addr = %addr, message, "sent system chat");
    Ok(())
}

// ---------------------------------------------------------------------------
// Initial chunks
// ---------------------------------------------------------------------------

async fn send_initial_chunks(
    server: &SharedServer,
    tx: &mpsc::Sender<OutgoingPacket>,
    codec: &MinecraftCodec,
    uuid: uuid::Uuid,
    addr: SocketAddr,
) -> anyhow::Result<()> {
    let config = server.config.clone();
    let view_dist = config.view_distance;
    let seed = config.seed;

    // Send chunks in a spiral pattern around spawn (0, 0).
    let chunks_to_send = chunk_sender::spiral_chunks(0, 0, view_dist);

    for (cx, cz) in &chunks_to_send {
        let chunk_data = chunk_sender::build_worldgen_chunk(*cx, *cz, seed);
        let light_data = chunk_sender::build_full_light();

        let mut packet_buf = BytesMut::with_capacity(chunk_data.len() + light_data.len() + 64);
        packet_buf.put_i32(*cx);
        packet_buf.put_i32(*cz);
        write_varint(&mut packet_buf, chunk_data.len() as i32)?;
        packet_buf.put_slice(&chunk_data);
        write_varint(&mut packet_buf, light_data.len() as i32)?;
        packet_buf.put_slice(&light_data);

        send_packet(tx, codec, 0x27, &packet_buf).await?;

        // Track sent chunks for this player.
        if let Some(player) = server.players.write().await.get_mut(&uuid) {
            player.sent_chunks.insert((*cx, *cz));
        }
    }

    tracing::debug!(
        addr = %addr,
        count = chunks_to_send.len(),
        "sent initial chunks"
    );
    Ok(())
}
