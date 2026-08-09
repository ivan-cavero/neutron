//! Server tick loop.
//!
//! Runs at 20 TPS (50ms per tick). Each tick:
//! - Increments the global tick counter.
//! - Sends KeepAlive packets to all players (every 30s = 600 ticks).
//! - Checks for timed-out players.
//! - (Future: world simulation, entity ticks, etc.)

use bytes::BufMut;
use std::time::Duration;
use tokio::sync::mpsc;

use crate::chunk_sender;
use crate::connection::{send_packet, OutgoingPacket};
use crate::server::SharedServer;

/// How many ticks between KeepAlive sends (30 seconds at 20 TPS).
const KEEPALIVE_INTERVAL: u64 = 600;

/// How many ticks before a player is considered timed out (no response).
const KEEPALIVE_TIMEOUT: u64 = 600;

/// Maximum chunks to send per tick per player.
const CHUNKS_PER_TICK: usize = 5;

// ---------------------------------------------------------------------------
// Tick loop
// ---------------------------------------------------------------------------

/// Run the server tick loop at 20 TPS.
pub async fn run_tick_loop(
    server: SharedServer,
    writer_tx: mpsc::Sender<crate::connection::OutgoingPacket>,
) {
    let tick_duration = Duration::from_millis(50); // 20 TPS

    tracing::info!("tick loop started (20 TPS)");

    loop {
        tokio::time::sleep(tick_duration).await;

        let tick = server.tick_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;

        // Send KeepAlive every 600 ticks.
        if tick % KEEPALIVE_INTERVAL == 0 {
            send_keepalives(&server, &writer_tx, tick).await;
        }

        // Check for timed-out players.
        check_timeouts(&server, tick).await;

        // Send chunks to players who haven't received them all yet.
        send_pending_chunks(&server, &writer_tx).await;
    }
}

// ---------------------------------------------------------------------------
// KeepAlive
// ---------------------------------------------------------------------------

async fn send_keepalives(
    server: &SharedServer,
    writer_tx: &mpsc::Sender<OutgoingPacket>,
    tick: u64,
) {
    let player_uuids: Vec<uuid::Uuid> = server.player_uuids().await;
    let codec = neutron_protocol::codec::MinecraftCodec::new();

    for uuid in &player_uuids {
        let keepalive_id: i64 = tick as i64;

        // Build KeepAlive packet (0x26).
        let mut buf = bytes::BytesMut::with_capacity(8);
        buf.put_i64(keepalive_id);

        if let Err(e) = send_packet(writer_tx, &codec, 0x26, &buf).await {
            tracing::warn!(uuid = %uuid, error = %e, "failed to send keepalive");
            continue;
        }

        // Update player state.
        if let Some(player) = server.players.write().await.get_mut(uuid) {
            if player.keepalive_pending {
                // Previous keepalive was not acknowledged — player timed out.
                tracing::warn!(
                    uuid = %uuid,
                    username = %player.username,
                    "player timed out (keepalive not responded)"
                );
                // We'll let the connection handle disconnection via the writer channel.
            }
            player.last_keepalive_id = Some(keepalive_id);
            player.last_keepalive_tick = tick;
            player.keepalive_pending = true;
        }
    }
}

// ---------------------------------------------------------------------------
// Timeout check
// ---------------------------------------------------------------------------

async fn check_timeouts(server: &SharedServer, tick: u64) {
    let player_uuids: Vec<uuid::Uuid> = server.player_uuids().await;

    for uuid in &player_uuids {
        let should_kick = {
            if let Some(player) = server.players.read().await.get(uuid) {
                player.keepalive_pending
                    && tick.saturating_sub(player.last_keepalive_tick) > KEEPALIVE_TIMEOUT
            } else {
                false
            }
        };

        if should_kick {
            let username = server
                .players
                .read()
                .await
                .get(uuid)
                .map(|p| p.username.clone())
                .unwrap_or_default();
            tracing::warn!(
                uuid = %uuid,
                username = %username,
                "kicking player (keepalive timeout)"
            );
            server.remove_player(uuid).await;
        }
    }
}

// ---------------------------------------------------------------------------
// Chunk sending for players who need more chunks
// ---------------------------------------------------------------------------

async fn send_pending_chunks(
    server: &SharedServer,
    writer_tx: &mpsc::Sender<OutgoingPacket>,
) {
    let codec = neutron_protocol::codec::MinecraftCodec::new();
    let seed = server.config.seed;

    // Collect player info under read lock.
    let player_info: Vec<(uuid::Uuid, i32, i32, i32, i32)> = {
        let players = server.players.read().await;
        players
            .iter()
            .filter(|(_, p)| p.is_playing)
            .map(|(&uuid, p)| (uuid, p.chunk_x, p.chunk_z, p.entity_id, 0))
            .collect()
    };

    for (uuid, chunk_x, chunk_z, _entity_id, _) in player_info {
        let view_dist = server.config.view_distance;
        let needed_chunks = chunk_sender::spiral_chunks(chunk_x, chunk_z, view_dist);

        let mut sent_this_tick = 0;
        let mut chunks_to_remove = Vec::new();

        for &(cx, cz) in &needed_chunks {
            if sent_this_tick >= CHUNKS_PER_TICK {
                break;
            }

            // Check if this chunk was already sent.
            let already_sent = {
                let players = server.players.read().await;
                players
                    .get(&uuid)
                    .map(|p| p.sent_chunks.contains(&(cx, cz)))
                    .unwrap_or(true)
            };

            if already_sent {
                continue;
            }

            // Generate and send the chunk.
            let chunk_data = chunk_sender::build_worldgen_chunk(cx, cz, seed);
            let light_data = chunk_sender::build_full_light();

            let mut packet_buf =
                bytes::BytesMut::with_capacity(chunk_data.len() + light_data.len() + 64);
            packet_buf.put_i32(cx);
            packet_buf.put_i32(cz);
            neutron_protocol::types::write_varint(&mut packet_buf, chunk_data.len() as i32)
                .expect("varint write");
            packet_buf.put_slice(&chunk_data);
            neutron_protocol::types::write_varint(&mut packet_buf, light_data.len() as i32)
                .expect("varint write");
            packet_buf.put_slice(&light_data);

            if send_packet(writer_tx, &codec, 0x27, &packet_buf)
                .await
                .is_err()
            {
                break;
            }

            chunks_to_remove.push((cx, cz));
            sent_this_tick += 1;
        }

        // Mark chunks as sent.
        if !chunks_to_remove.is_empty() {
            if let Some(player) = server.players.write().await.get_mut(&uuid) {
                for (cx, cz) in chunks_to_remove {
                    player.sent_chunks.insert((cx, cz));
                }
            }
        }
    }
}
