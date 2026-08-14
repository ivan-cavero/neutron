//! Per-player TCP connection handling.
//!
//! Each connection has a reader task and a writer task. The reader task
//! reads packets from the TCP stream, decodes them, and processes them.
//! The writer task receives packet bytes from a channel and writes them
//! to the TCP stream.

use bytes::{Buf, BufMut, Bytes, BytesMut};
use neutron_protocol::codec::MinecraftCodec;
use neutron_protocol::types::{read_varint, write_varint};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::mpsc;

use crate::login::{handle_config_packet, handle_login_packet};
use crate::play::handle_play_packet;
use crate::protocol_data;
use crate::protocol_ids as pid;
use crate::server::SharedServer;

// ---------------------------------------------------------------------------
// Protocol state
// ---------------------------------------------------------------------------

/// Connection state in the protocol.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
    /// Waiting for the Handshake packet.
    Handshake,
    /// Server-list ping.
    Status,
    /// Login sequence (LoginStart → LoginFinished → Login Acknowledged).
    Login,
    /// 26.2 configuration (known packs, registries, tags).
    Configuration,
    /// Main gameplay.
    Play,
}

// ---------------------------------------------------------------------------
// Outgoing packet
// ---------------------------------------------------------------------------

/// An encoded packet ready to be written to the TCP stream.
#[derive(Debug, Clone)]
pub struct OutgoingPacket {
    pub data: Bytes,
}

// ---------------------------------------------------------------------------
// Connection handler
// ---------------------------------------------------------------------------

/// Handle a new TCP connection.
///
/// This spawns two async tasks:
/// - A reader task that decodes packets and processes them.
/// - A writer task that writes encoded packets to the TCP stream.
pub async fn handle_connection(
    stream: TcpStream,
    server: SharedServer,
    addr: std::net::SocketAddr,
) -> anyhow::Result<()> {
    tracing::info!(addr = %addr, "new connection");

    let (read_half, mut write_half) = stream.into_split();

    // Channel for outgoing packets (writer task reads from this).
    let (tx, mut rx) = mpsc::channel::<OutgoingPacket>(256);

    // Spawn writer task.
    let writer_handle = tokio::spawn(async move {
        while let Some(packet) = rx.recv().await {
            if let Err(e) = write_half.write_all(&packet.data).await {
                tracing::debug!(addr = %addr, error = %e, "write failed (client likely disconnected)");
                break;
            }
        }
        let _ = write_half.shutdown().await;
    });

    // Reader task (runs on this task).
    let result = run_reader(read_half, server, addr, tx).await;

    // When reader finishes, stop the writer.
    writer_handle.abort();

    match result {
        Ok(()) => tracing::info!(addr = %addr, "connection closed"),
        Err(e) => tracing::debug!(addr = %addr, error = %e, "connection error"),
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Reader loop
// ---------------------------------------------------------------------------

async fn run_reader(
    mut read_half: tokio::net::tcp::OwnedReadHalf,
    server: SharedServer,
    addr: std::net::SocketAddr,
    tx: mpsc::Sender<OutgoingPacket>,
) -> anyhow::Result<()> {
    let mut state = ConnectionState::Handshake;
    let mut codec = MinecraftCodec::new();
    let mut input_buf = BytesMut::with_capacity(8192);
    let mut player_uuid: Option<uuid::Uuid> = None;

    loop {
        // Read more data from TCP.
        let n = read_half.read_buf(&mut input_buf).await?;
        if n == 0 {
            // Client disconnected.
            break;
        }

        // Try to decode packets from the buffer.
        loop {
            let raw_bytes: Bytes = input_buf.split_to(input_buf.len()).into();
            let mut raw_buf = raw_bytes.clone();

            match codec.decode(&mut raw_buf) {
                Ok(Some(packet)) => {
                    // Process the packet.
                    let mut payload = packet.payload.clone();
                    let new_state = process_packet(
                        &server,
                        &tx,
                        &mut state,
                        &mut codec,
                        &mut player_uuid,
                        packet.id,
                        &mut payload,
                        addr,
                    )
                    .await?;

                    // Update state if login completed.
                    if new_state.is_some() {
                        state = new_state.unwrap();
                    }

                    // Put any remaining bytes back into the input buffer.
                    if raw_buf.has_remaining() {
                        let remaining = raw_buf.to_vec();
                        input_buf.clear();
                        input_buf.extend_from_slice(&remaining);
                    }
                }
                Ok(None) => {
                    // Incomplete frame — put all bytes back.
                    input_buf.clear();
                    input_buf.extend_from_slice(&raw_bytes);
                    break;
                }
                Err(e) => {
                    tracing::warn!(
                        addr = %addr,
                        error = %e,
                        "decode error"
                    );
                    break;
                }
            }
        }
    }

    // Clean up player state on disconnect.
    if let Some(uuid) = player_uuid {
        server.remove_player(&uuid).await;
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Packet processing
// ---------------------------------------------------------------------------

async fn process_packet(
    server: &SharedServer,
    tx: &mpsc::Sender<OutgoingPacket>,
    state: &mut ConnectionState,
    codec: &mut MinecraftCodec,
    player_uuid: &mut Option<uuid::Uuid>,
    packet_id: u32,
    payload: &mut Bytes,
    addr: std::net::SocketAddr,
) -> anyhow::Result<Option<ConnectionState>> {
    match state {
        ConnectionState::Handshake => {
            handle_handshake(server, tx, codec, packet_id, payload, addr).await
        }
        ConnectionState::Status => handle_status(server, tx, codec, packet_id, payload, addr).await,
        ConnectionState::Login => {
            let to_config =
                handle_login_packet(server, tx, codec, packet_id, payload, addr, player_uuid)
                    .await?;
            if to_config {
                Ok(Some(ConnectionState::Configuration))
            } else {
                Ok(None)
            }
        }
        ConnectionState::Configuration => {
            let to_play = handle_config_packet(
                server,
                tx,
                codec,
                packet_id,
                payload,
                addr,
                *player_uuid,
            )
            .await?;
            if to_play {
                Ok(Some(ConnectionState::Play))
            } else {
                Ok(None)
            }
        }
        ConnectionState::Play => {
            if let Some(uuid) = player_uuid {
                handle_play_packet(server, tx, uuid, packet_id, payload, addr).await?;
            }
            Ok(None)
        }
    }
}

// ---------------------------------------------------------------------------
// Handshake
// ---------------------------------------------------------------------------

async fn handle_handshake(
    _server: &SharedServer,
    _tx: &mpsc::Sender<OutgoingPacket>,
    _codec: &mut MinecraftCodec,
    packet_id: u32,
    payload: &mut Bytes,
    addr: std::net::SocketAddr,
) -> anyhow::Result<Option<ConnectionState>> {
    if packet_id != 0x00 {
        tracing::warn!(
            addr = %addr,
            packet_id,
            "expected Handshake (0x00), got unexpected packet"
        );
        return Ok(None);
    }

    // Decode Handshake.
    let protocol_version = read_varint(payload)?;
    let server_address = {
        let len = read_varint(payload)? as usize;
        let mut bytes = vec![0u8; len];
        payload.copy_to_slice(&mut bytes);
        String::from_utf8(bytes)?
    };
    let _server_port = {
        if payload.remaining() < 2 {
            return Err(anyhow::anyhow!("handshake too short for server_port"));
        }
        payload.get_u16()
    };
    let next_state = read_varint(payload)?;

    tracing::debug!(
        addr = %addr,
        protocol_version,
        server_address = %server_address,
        next_state,
        "handshake received"
    );

    match next_state {
        1 => Ok(Some(ConnectionState::Status)),
        2 => Ok(Some(ConnectionState::Login)),
        _ => {
            tracing::warn!(
                addr = %addr,
                next_state,
                "invalid next_state in handshake"
            );
            Ok(None)
        }
    }
}

async fn handle_status(
    server: &SharedServer,
    tx: &mpsc::Sender<OutgoingPacket>,
    codec: &mut MinecraftCodec,
    packet_id: u32,
    payload: &mut Bytes,
    addr: std::net::SocketAddr,
) -> anyhow::Result<Option<ConnectionState>> {
    match packet_id {
        pid::STATUS_REQUEST => {
            let online = server.player_count().await as i32;
            let motd = server
                .config
                .motd
                .replace('\\', "\\\\")
                .replace('"', "\\\"");
            let json = format!(
                r#"{{"version":{{"name":"26.2","protocol":{}}},"players":{{"max":{},"online":{}}},"description":{{"text":"{}"}},"enforcesSecureChat":false}}"#,
                protocol_data::PROTOCOL_VERSION,
                server.config.max_players,
                online,
                motd,
            );
            let mut buf = BytesMut::new();
            let bytes = json.as_bytes();
            write_varint(&mut buf, bytes.len() as i32)?;
            buf.put_slice(bytes);
            send_packet(tx, codec, pid::STATUS_RESPONSE, &buf).await?;
            tracing::debug!(addr = %addr, "sent status response");
            Ok(None)
        }
        pid::STATUS_PING => {
            let mut buf = BytesMut::new();
            if payload.remaining() >= 8 {
                buf.put_i64(payload.get_i64());
            } else {
                buf.put_i64(0);
            }
            send_packet(tx, codec, pid::STATUS_PONG, &buf).await?;
            Ok(None)
        }
        _ => Ok(None),
    }
}

// ---------------------------------------------------------------------------
// Encoding helpers
// ---------------------------------------------------------------------------

/// Encode and send a packet.
pub async fn send_packet(
    tx: &mpsc::Sender<OutgoingPacket>,
    codec: &MinecraftCodec,
    packet_id: u32,
    payload: &[u8],
) -> anyhow::Result<()> {
    let mut buf = BytesMut::with_capacity(payload.len() + 16);
    codec.encode(packet_id, payload, &mut buf)?;
    let data = Bytes::copy_from_slice(&buf);
    tx.send(OutgoingPacket { data }).await?;
    Ok(())
}
