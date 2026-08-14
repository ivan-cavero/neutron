//! Login → configuration → play for Minecraft 26.2 (protocol 776).
//!
//! Sequence (vanilla client):
//! 1. LoginStart → LoginFinished
//! 2. Login Acknowledged → Configuration
//! 3. Select Known Packs ↔ client reply
//! 4. Feature flags + Registry Data + Update Tags + Finish Configuration
//! 5. Finish ack → Play Login + spawn + chunks

use bytes::{Buf, BufMut, Bytes, BytesMut};
use neutron_protocol::codec::MinecraftCodec;
use neutron_protocol::types::{read_string, read_uuid, write_varint};
use std::net::SocketAddr;
use tokio::sync::mpsc;

use crate::connection::{send_packet, OutgoingPacket};
use crate::protocol_data;
use crate::protocol_ids as pid;
use crate::server::SharedServer;

/// How many chunks to send before the client is allowed to move (rest stream).
const INITIAL_CHUNK_RADIUS: i32 = 2;

// ---------------------------------------------------------------------------
// Login
// ---------------------------------------------------------------------------

/// Handle a packet during the Login state.
///
/// Returns `Ok(true)` when the connection should switch to Configuration
/// (Login Acknowledged received).
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
        pid::LOGIN_START => {
            handle_login_start(server, tx, codec, payload, addr, player_uuid).await?;
            Ok(false)
        }
        pid::LOGIN_ACKNOWLEDGED => {
            tracing::debug!(addr = %addr, "login acknowledged → configuration");
            send_known_packs(tx, codec, addr).await?;
            Ok(true)
        }
        _ => {
            tracing::warn!(addr = %addr, packet_id, "unexpected packet in login state");
            Ok(false)
        }
    }
}

async fn handle_login_start(
    server: &SharedServer,
    tx: &mpsc::Sender<OutgoingPacket>,
    codec: &mut MinecraftCodec,
    payload: &mut Bytes,
    addr: SocketAddr,
    player_uuid: &mut Option<uuid::Uuid>,
) -> anyhow::Result<()> {
    let name = read_string(payload)?;
    let uuid = if payload.has_remaining() {
        read_uuid(payload)?
    } else {
        uuid::Uuid::new_v4()
    };

    tracing::info!(addr = %addr, username = %name, uuid = %uuid, "login start");

    let _entity_id = server.register_player(uuid, name.clone()).await;
    *player_uuid = Some(uuid);
    server.register_writer(uuid, tx.clone()).await;

    // LoginFinished: GameProfile + session UUID.
    let mut buf = BytesMut::new();
    buf.put_slice(uuid.as_bytes());
    write_string(&mut buf, &name)?;
    write_varint(&mut buf, 0)?; // no properties
    buf.put_slice(uuid::Uuid::new_v4().as_bytes()); // session id

    send_packet(tx, codec, pid::LOGIN_FINISHED, &buf).await?;
    tracing::debug!(addr = %addr, "sent LoginFinished");
    Ok(())
}

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Handle a packet during Configuration.
///
/// Returns `Ok(true)` when the client acked Finish Configuration (enter Play).
pub async fn handle_config_packet(
    server: &SharedServer,
    tx: &mpsc::Sender<OutgoingPacket>,
    codec: &mut MinecraftCodec,
    packet_id: u32,
    payload: &mut Bytes,
    addr: SocketAddr,
    player_uuid: Option<uuid::Uuid>,
) -> anyhow::Result<bool> {
    match packet_id {
        pid::CFG_SB_SELECT_KNOWN_PACKS => {
            let _ = payload;
            tracing::debug!(addr = %addr, "client known packs received");
            send_configuration_body(tx, codec, addr).await?;
            Ok(false)
        }
        pid::CFG_SB_FINISH => {
            tracing::debug!(addr = %addr, "configuration finished → play");
            if let Some(uuid) = player_uuid {
                send_play_sequence(server, tx, codec, uuid, addr).await?;
            }
            Ok(true)
        }
        pid::CFG_SB_KEEP_ALIVE => Ok(false),
        _ => {
            tracing::trace!(addr = %addr, packet_id, "ignoring config packet");
            Ok(false)
        }
    }
}

async fn send_known_packs(
    tx: &mpsc::Sender<OutgoingPacket>,
    codec: &MinecraftCodec,
    addr: SocketAddr,
) -> anyhow::Result<()> {
    let mut buf = BytesMut::new();
    write_varint(&mut buf, 1)?;
    write_string(&mut buf, protocol_data::KNOWN_PACK_NAMESPACE)?;
    write_string(&mut buf, protocol_data::KNOWN_PACK_ID)?;
    write_string(&mut buf, protocol_data::KNOWN_PACK_VERSION)?;
    send_packet(tx, codec, pid::CFG_SELECT_KNOWN_PACKS, &buf).await?;
    tracing::debug!(addr = %addr, "sent known packs (minecraft:core 26.2)");
    Ok(())
}

async fn send_configuration_body(
    tx: &mpsc::Sender<OutgoingPacket>,
    codec: &MinecraftCodec,
    addr: SocketAddr,
) -> anyhow::Result<()> {
    // Feature flags: minecraft:vanilla
    {
        let mut buf = BytesMut::new();
        write_varint(&mut buf, 1)?;
        write_string(&mut buf, "minecraft:vanilla")?;
        send_packet(tx, codec, pid::CFG_UPDATE_FEATURES, &buf).await?;
    }

    for &(registry, entries) in protocol_data::SYNC_REGISTRIES {
        let mut buf = BytesMut::new();
        write_string(&mut buf, registry)?;
        write_varint(&mut buf, entries.len() as i32)?;
        for entry in entries {
            write_string(&mut buf, entry)?;
            buf.put_u8(0); // has_data = false (sourced from known pack)
        }
        send_packet(tx, codec, pid::CFG_REGISTRY_DATA, &buf).await?;
    }

    send_update_tags(tx, codec).await?;

    send_packet(tx, codec, pid::CFG_FINISH, &[]).await?;
    tracing::debug!(addr = %addr, "sent configuration body + finish");
    Ok(())
}

async fn send_update_tags(
    tx: &mpsc::Sender<OutgoingPacket>,
    codec: &MinecraftCodec,
) -> anyhow::Result<()> {
    // Group TAGS by registry while preserving order.
    let mut grouped: Vec<(&str, Vec<(&str, &[i32])>)> = Vec::new();
    for &(reg, tag, ids) in protocol_data::TAGS {
        if let Some(last) = grouped.last_mut() {
            if last.0 == reg {
                last.1.push((tag, ids));
                continue;
            }
        }
        grouped.push((reg, vec![(tag, ids)]));
    }

    let mut buf = BytesMut::with_capacity(64 * 1024);
    write_varint(&mut buf, grouped.len() as i32)?;
    for (reg, tags) in grouped {
        write_string(&mut buf, reg)?;
        write_varint(&mut buf, tags.len() as i32)?;
        for (tag, ids) in tags {
            write_string(&mut buf, tag)?;
            write_varint(&mut buf, ids.len() as i32)?;
            for id in ids {
                write_varint(&mut buf, *id)?;
            }
        }
    }
    send_packet(tx, codec, pid::CFG_UPDATE_TAGS, &buf).await?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Play sequence
// ---------------------------------------------------------------------------

async fn send_play_sequence(
    server: &SharedServer,
    tx: &mpsc::Sender<OutgoingPacket>,
    codec: &MinecraftCodec,
    uuid: uuid::Uuid,
    addr: SocketAddr,
) -> anyhow::Result<()> {
    let config = server.config.clone();
    let (sx, sy, sz) = server.world.spawn_xyz_async().await;
    let spawn_y_block = sy.floor() as i32;

    {
        let mut players = server.players.write().await;
        if let Some(p) = players.get_mut(&uuid) {
            p.x = sx;
            p.y = sy;
            p.z = sz;
            p.chunk_x = 0;
            p.chunk_z = 0;
        }
    }

    let entity_id = server
        .players
        .read()
        .await
        .get(&uuid)
        .map(|p| p.entity_id)
        .unwrap_or(1);

    send_play_login(tx, codec, entity_id, &config, addr).await?;
    send_game_event(tx, codec, pid::GAME_EVENT_CHUNKS_LOAD_START, 0.0).await?;
    send_player_abilities(tx, codec, addr).await?;
    send_center_chunk(tx, codec, 0, 0, addr).await?;
    send_default_spawn(tx, codec, spawn_y_block, addr).await?;
    send_sync_position(tx, codec, sx, sy, sz, addr).await?;
    send_initial_chunks(server, tx, codec, uuid, addr).await?;

    send_system_chat(tx, codec, "Welcome to Neutron — live worldgen (not 1:1 yet).", addr)
        .await?;

    if let Some(player) = server.players.write().await.get_mut(&uuid) {
        player.is_playing = true;
    }

    tracing::info!(
        addr = %addr,
        spawn = format!("({:.1}, {:.1}, {:.1})", sx, sy, sz),
        "play sequence complete"
    );
    Ok(())
}

async fn send_play_login(
    tx: &mpsc::Sender<OutgoingPacket>,
    codec: &MinecraftCodec,
    entity_id: i32,
    config: &crate::server::ServerConfig,
    addr: SocketAddr,
) -> anyhow::Result<()> {
    let mut buf = BytesMut::with_capacity(128);
    buf.put_i32(entity_id);
    buf.put_u8(0); // hardcore
    write_varint(&mut buf, 1)?; // dimension names
    write_string(&mut buf, "minecraft:overworld")?;
    write_varint(&mut buf, config.max_players)?;
    write_varint(&mut buf, config.view_distance)?;
    write_varint(&mut buf, config.view_distance)?;
    buf.put_u8(0); // reduced debug
    buf.put_u8(1); // show death screen
    buf.put_u8(0); // do limited crafting

    // CommonPlayerSpawnInfo
    write_varint(&mut buf, 1)?; // Holder: dimension_type[0] = overworld
    write_string(&mut buf, "minecraft:overworld")?;
    buf.put_i64(config.seed);
    buf.put_u8(1); // creative
    buf.put_i8(-1); // previous
    buf.put_u8(0); // debug
    buf.put_u8(0); // flat
    buf.put_u8(0); // no death location
    write_varint(&mut buf, 0)?; // portal cooldown
    write_varint(&mut buf, 63)?; // sea level

    buf.put_u8(0); // online mode
    buf.put_u8(0); // enforces secure chat

    send_packet(tx, codec, pid::PLAY_LOGIN, &buf).await?;
    tracing::debug!(addr = %addr, "sent play login");
    Ok(())
}

async fn send_game_event(
    tx: &mpsc::Sender<OutgoingPacket>,
    codec: &MinecraftCodec,
    event: u8,
    value: f32,
) -> anyhow::Result<()> {
    let mut buf = BytesMut::with_capacity(5);
    buf.put_u8(event);
    buf.put_f32(value);
    send_packet(tx, codec, pid::PLAY_GAME_EVENT, &buf).await?;
    Ok(())
}

async fn send_player_abilities(
    tx: &mpsc::Sender<OutgoingPacket>,
    codec: &MinecraftCodec,
    addr: SocketAddr,
) -> anyhow::Result<()> {
    let mut buf = BytesMut::with_capacity(9);
    // invulnerable | allow flying | flying | creative
    buf.put_u8(0x01 | 0x02 | 0x04 | 0x08);
    buf.put_f32(0.05);
    buf.put_f32(0.1);
    send_packet(tx, codec, pid::PLAY_ABILITIES, &buf).await?;
    tracing::debug!(addr = %addr, "sent player abilities");
    Ok(())
}

async fn send_center_chunk(
    tx: &mpsc::Sender<OutgoingPacket>,
    codec: &MinecraftCodec,
    chunk_x: i32,
    chunk_z: i32,
    addr: SocketAddr,
) -> anyhow::Result<()> {
    let mut buf = BytesMut::with_capacity(8);
    write_varint(&mut buf, chunk_x)?;
    write_varint(&mut buf, chunk_z)?;
    send_packet(tx, codec, pid::PLAY_CENTER_CHUNK, &buf).await?;
    tracing::debug!(addr = %addr, chunk_x, chunk_z, "sent set center chunk");
    Ok(())
}

async fn send_default_spawn(
    tx: &mpsc::Sender<OutgoingPacket>,
    codec: &MinecraftCodec,
    y: i32,
    addr: SocketAddr,
) -> anyhow::Result<()> {
    let mut buf = BytesMut::with_capacity(16);
    let spawn = neutron_protocol::types::BlockPos::new(0, y, 0);
    buf.put_i64(spawn.to_packed());
    buf.put_f32(0.0);
    send_packet(tx, codec, pid::PLAY_DEFAULT_SPAWN, &buf).await?;
    tracing::debug!(addr = %addr, y, "sent default spawn");
    Ok(())
}

async fn send_sync_position(
    tx: &mpsc::Sender<OutgoingPacket>,
    codec: &MinecraftCodec,
    x: f64,
    y: f64,
    z: f64,
    addr: SocketAddr,
) -> anyhow::Result<()> {
    let mut buf = BytesMut::with_capacity(64);
    write_varint(&mut buf, 1)?; // teleport id
    buf.put_f64(x);
    buf.put_f64(y);
    buf.put_f64(z);
    buf.put_f64(0.0);
    buf.put_f64(0.0);
    buf.put_f64(0.0);
    buf.put_f32(0.0);
    buf.put_f32(0.0);
    write_varint(&mut buf, 0)?; // no relative flags
    send_packet(tx, codec, pid::PLAY_POSITION, &buf).await?;
    tracing::debug!(addr = %addr, x, y, z, "sent sync player position");
    Ok(())
}

async fn send_system_chat(
    tx: &mpsc::Sender<OutgoingPacket>,
    codec: &MinecraftCodec,
    message: &str,
    addr: SocketAddr,
) -> anyhow::Result<()> {
    let mut buf = BytesMut::new();
    // Network NBT: unnamed TAG_String (type 8 + utm-16be length + mutf8).
    buf.put_u8(8);
    let bytes = message.as_bytes();
    buf.put_u16(bytes.len() as u16);
    buf.put_slice(bytes);
    buf.put_u8(0); // overlay = false
    send_packet(tx, codec, pid::PLAY_SYSTEM_CHAT, &buf).await?;
    tracing::debug!(addr = %addr, message, "sent system chat");
    Ok(())
}

async fn send_initial_chunks(
    server: &SharedServer,
    tx: &mpsc::Sender<OutgoingPacket>,
    codec: &MinecraftCodec,
    uuid: uuid::Uuid,
    addr: SocketAddr,
) -> anyhow::Result<()> {
    let chunks = crate::chunk_sender::spiral_chunks(0, 0, INITIAL_CHUNK_RADIUS);
    send_packet(tx, codec, pid::PLAY_CHUNK_BATCH_START, &[]).await?;

    for (cx, cz) in &chunks {
        let encoded = server.world.chunk_async(*cx, *cz).await;
        let mut packet_buf = BytesMut::with_capacity(encoded.body.len() + 8);
        packet_buf.put_i32(*cx);
        packet_buf.put_i32(*cz);
        packet_buf.put_slice(&encoded.body);
        send_packet(tx, codec, pid::PLAY_LEVEL_CHUNK, &packet_buf).await?;

        if let Some(player) = server.players.write().await.get_mut(&uuid) {
            player.sent_chunks.insert((*cx, *cz));
        }
    }

    let mut done = BytesMut::new();
    write_varint(&mut done, chunks.len() as i32)?;
    send_packet(tx, codec, pid::PLAY_CHUNK_BATCH_FINISHED, &done).await?;

    tracing::info!(
        addr = %addr,
        count = chunks.len(),
        radius = INITIAL_CHUNK_RADIUS,
        "sent initial chunks"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn write_string(buf: &mut BytesMut, s: &str) -> anyhow::Result<()> {
    let bytes = s.as_bytes();
    write_varint(buf, bytes.len() as i32)?;
    buf.put_slice(bytes);
    Ok(())
}
