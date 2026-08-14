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
use crate::protocol_ids as pid;
use crate::server::SharedServer;

/// How many ticks between KeepAlive sends (30 seconds at 20 TPS).
const KEEPALIVE_INTERVAL: u64 = 600;

/// How many ticks before a player is considered timed out (no response).
const KEEPALIVE_TIMEOUT: u64 = 600;

/// Maximum already-ready chunks to send per tick per player.
const CHUNKS_PER_TICK: usize = 8;

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

        let tick = server
            .tick_count
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            + 1;

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
    _writer_tx: &mpsc::Sender<OutgoingPacket>,
    tick: u64,
) {
    let player_uuids: Vec<uuid::Uuid> = server.player_uuids().await;
    let codec = neutron_protocol::codec::MinecraftCodec::new();

    for uuid in &player_uuids {
        let playing = server
            .players
            .read()
            .await
            .get(uuid)
            .map(|p| p.is_playing)
            .unwrap_or(false);
        if !playing {
            continue;
        }

        let keepalive_id: i64 = tick as i64;

        let mut buf = bytes::BytesMut::with_capacity(8);
        buf.put_i64(keepalive_id);

        let Some(player_tx) = server.writer(uuid).await else {
            continue;
        };
        if let Err(e) = send_packet(&player_tx, &codec, pid::PLAY_KEEP_ALIVE, &buf).await {
            tracing::warn!(uuid = %uuid, error = %e, "failed to send keepalive");
            continue;
        }

        if let Some(player) = server.players.write().await.get_mut(uuid) {
            if player.keepalive_pending {
                tracing::warn!(
                    uuid = %uuid,
                    username = %player.username,
                    "player timed out (keepalive not responded)"
                );
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
                player.is_playing
                    && player.keepalive_pending
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

async fn send_pending_chunks(server: &SharedServer, _writer_tx: &mpsc::Sender<OutgoingPacket>) {
    let codec = neutron_protocol::codec::MinecraftCodec::new();

    let player_info: Vec<(uuid::Uuid, i32, i32)> = {
        let players = server.players.read().await;
        players
            .iter()
            .filter(|(_, p)| p.is_playing)
            .map(|(&uuid, p)| (uuid, p.chunk_x, p.chunk_z))
            .collect()
    };

    for (uuid, chunk_x, chunk_z) in player_info {
        let Some(player_tx) = server.writer(&uuid).await else {
            continue;
        };
        let view_dist = server.config.view_distance;
        let needed_chunks = chunk_sender::spiral_chunks(chunk_x, chunk_z, view_dist);

        let mut sent_this_tick = 0;
        let mut prefetch_budget = 16;
        let mut marked = Vec::new();

        for &(cx, cz) in &needed_chunks {
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

            let Some(encoded) = server.world.try_chunk(cx, cz) else {
                if prefetch_budget > 0 {
                    server.world.prefetch(cx, cz);
                    prefetch_budget -= 1;
                }
                continue;
            };
            if sent_this_tick >= CHUNKS_PER_TICK {
                continue;
            }

            let mut packet_buf = bytes::BytesMut::with_capacity(encoded.body.len() + 8);
            packet_buf.put_i32(cx);
            packet_buf.put_i32(cz);
            packet_buf.put_slice(&encoded.body);

            if send_packet(&player_tx, &codec, pid::PLAY_LEVEL_CHUNK, &packet_buf)
                .await
                .is_err()
            {
                break;
            }

            marked.push((cx, cz));
            sent_this_tick += 1;
        }

        if !marked.is_empty() {
            if let Some(player) = server.players.write().await.get_mut(&uuid) {
                for pos in marked {
                    player.sent_chunks.insert(pos);
                }
            }
        }
    }
}
