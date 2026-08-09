//! Play state packet handler.
//!
//! Handles incoming packets during the Play protocol state:
//! - KeepAliveResponse
//! - PlayerPosition
//! - PlayerPositionAndRotation
//! - PlayerRotation
//! - ChatCommand
//! - ClientStatus
//! - SetPlayerAbilities
//! - TeleportConfirm

use bytes::{Buf, BufMut, Bytes};
use neutron_protocol::types::{read_varint, write_varint};
use std::net::SocketAddr;
use tokio::sync::mpsc;

use crate::connection::send_packet;
use crate::server::SharedServer;

// ---------------------------------------------------------------------------
// Serverbound play packet IDs
// ---------------------------------------------------------------------------
const SB_KEEPALIVE_RESPONSE: u32 = 0x18;
const SB_PLAYER_POSITION: u32 = 0x17;
const SB_PLAYER_ROTATION: u32 = 0x19;
const SB_CHAT_COMMAND: u32 = 0x04;
const SB_CLIENT_STATUS: u32 = 0x07;
const SB_SET_PLAYER_ABILITIES: u32 = 0x1E;
const SB_TELEPORT_CONFIRM: u32 = 0x00;

// ---------------------------------------------------------------------------
// Handle play packets
// ---------------------------------------------------------------------------

pub async fn handle_play_packet(
    server: &SharedServer,
    tx: &mpsc::Sender<crate::connection::OutgoingPacket>,
    player_uuid: &uuid::Uuid,
    packet_id: u32,
    payload: &mut Bytes,
    addr: SocketAddr,
) -> anyhow::Result<()> {
    match packet_id {
        SB_KEEPALIVE_RESPONSE => {
            handle_keepalive_response(server, player_uuid, payload, addr).await
        }
        SB_PLAYER_POSITION => {
            handle_player_position(server, player_uuid, payload, addr).await
        }
        SB_PLAYER_ROTATION => {
            handle_player_rotation(server, player_uuid, payload, addr).await
        }
        SB_CHAT_COMMAND => {
            handle_chat_command(server, tx, player_uuid, payload, addr).await
        }
        SB_CLIENT_STATUS => {
            handle_client_status(server, tx, player_uuid, payload, addr).await
        }
        SB_SET_PLAYER_ABILITIES => {
            handle_set_player_abilities(player_uuid, payload, addr).await
        }
        SB_TELEPORT_CONFIRM => {
            // TeleportConfirm — acknowledge a teleport. Just ignore for now.
            Ok(())
        }
        _ => {
            // Unknown packet — ignore (don't kick, many packets are not yet handled).
            tracing::trace!(
                addr = %addr,
                packet_id = format!("0x{:02X}", packet_id),
                "ignoring unhandled play packet"
            );
            Ok(())
        }
    }
}

// ---------------------------------------------------------------------------
// KeepAliveResponse
// ---------------------------------------------------------------------------

async fn handle_keepalive_response(
    server: &SharedServer,
    player_uuid: &uuid::Uuid,
    payload: &mut Bytes,
    addr: SocketAddr,
) -> anyhow::Result<()> {
    if payload.remaining() < 8 {
        tracing::warn!(addr = %addr, "KeepAliveResponse too short");
        return Ok(());
    }
    let keepalive_id = payload.get_i64();

    if let Some(player) = server.players.write().await.get_mut(player_uuid) {
        if player.keepalive_pending && player.last_keepalive_id == Some(keepalive_id) {
            player.keepalive_pending = false;
            tracing::trace!(
                addr = %addr,
                keepalive_id,
                "keepalive acknowledged"
            );
        } else {
            tracing::warn!(
                addr = %addr,
                keepalive_id,
                expected = ?player.last_keepalive_id,
                "keepalive response mismatch"
            );
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// PlayerPosition
// ---------------------------------------------------------------------------

async fn handle_player_position(
    server: &SharedServer,
    player_uuid: &uuid::Uuid,
    payload: &mut Bytes,
    _addr: SocketAddr,
) -> anyhow::Result<()> {
    if payload.remaining() < 25 {
        return Ok(());
    }
    let x = payload.get_f64();
    let y = payload.get_f64();
    let z = payload.get_f64();
    let _on_ground = payload.get_u8() != 0;

    server.update_player_position(player_uuid, x, y, z).await;

    Ok(())
}

// ---------------------------------------------------------------------------
// PlayerPositionAndRotation
// ---------------------------------------------------------------------------

async fn handle_player_pos_and_rotation(
    server: &SharedServer,
    player_uuid: &uuid::Uuid,
    payload: &mut Bytes,
    _addr: SocketAddr,
) -> anyhow::Result<()> {
    if payload.remaining() < 33 {
        return Ok(());
    }
    let x = payload.get_f64();
    let y = payload.get_f64();
    let z = payload.get_f64();
    let yaw = payload.get_f32();
    let pitch = payload.get_f32();
    let _on_ground = payload.get_u8() != 0;

    server.update_player_position(player_uuid, x, y, z).await;
    server.update_player_rotation(player_uuid, yaw, pitch).await;

    Ok(())
}

// ---------------------------------------------------------------------------
// PlayerRotation
// ---------------------------------------------------------------------------

async fn handle_player_rotation(
    server: &SharedServer,
    player_uuid: &uuid::Uuid,
    payload: &mut Bytes,
    _addr: SocketAddr,
) -> anyhow::Result<()> {
    if payload.remaining() < 9 {
        return Ok(());
    }
    let yaw = payload.get_f32();
    let pitch = payload.get_f32();
    let _on_ground = payload.get_u8() != 0;

    server.update_player_rotation(player_uuid, yaw, pitch).await;

    Ok(())
}

// ---------------------------------------------------------------------------
// ChatCommand
// ---------------------------------------------------------------------------

async fn handle_chat_command(
    _server: &SharedServer,
    tx: &mpsc::Sender<crate::connection::OutgoingPacket>,
    _player_uuid: &uuid::Uuid,
    payload: &mut Bytes,
    addr: SocketAddr,
) -> anyhow::Result<()> {
    let command = {
        let len = read_varint(payload)? as usize;
        let mut bytes = vec![0u8; len];
        payload.copy_to_slice(&mut bytes);
        String::from_utf8(bytes)?
    };

    tracing::info!(addr = %addr, command = %command, "chat command");

    // For now, respond with a simple message.
    let response = format!("Command '{}' is not implemented yet.", command);
    let codec = neutron_protocol::codec::MinecraftCodec::new();
    let json = format!(r#"{{"text":"{}"}}"#, response.replace('"', "\\\""));
    let mut buf = bytes::BytesMut::with_capacity(json.len() + 8);
    let json_bytes = json.as_bytes();
    write_varint(&mut buf, json_bytes.len() as i32)?;
    buf.put_slice(json_bytes);
    buf.put_u8(0); // overlay = false

    // SystemChatMessage packet ID: 0x67
    send_packet(tx, &codec, 0x67, &buf).await?;

    Ok(())
}

// ---------------------------------------------------------------------------
// ClientStatus
// ---------------------------------------------------------------------------

async fn handle_client_status(
    _server: &SharedServer,
    _tx: &mpsc::Sender<crate::connection::OutgoingPacket>,
    _player_uuid: &uuid::Uuid,
    payload: &mut Bytes,
    addr: SocketAddr,
) -> anyhow::Result<()> {
    let action_id = read_varint(payload)?;
    tracing::debug!(addr = %addr, action_id, "client status");
    // action_id: 0 = perform respawn, 1 = request stats.
    // For now, just acknowledge.
    Ok(())
}

// ---------------------------------------------------------------------------
// SetPlayerAbilities
// ---------------------------------------------------------------------------

async fn handle_set_player_abilities(
    player_uuid: &uuid::Uuid,
    payload: &mut Bytes,
    addr: SocketAddr,
) -> anyhow::Result<()> {
    if payload.remaining() < 9 {
        return Ok(());
    }
    let flags = payload.get_u8();
    let flying_speed = payload.get_f32();
    let fov_modifier = payload.get_f32();

    tracing::debug!(
        addr = %addr,
        flags,
        flying_speed,
        fov_modifier,
        "set player abilities"
    );

    // We don't validate abilities yet; just acknowledge.
    let _ = player_uuid;
    Ok(())
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

// write_varint is imported from neutron_protocol::types and used directly.
