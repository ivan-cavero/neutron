//! Neutron: Minimal functional Minecraft 26.2 server binary.
//!
//! This server accepts TCP connections from vanilla 26.2 clients (online-mode=false)
//! and handles the full login/play handshake. Players join a flat creative world
//! and can walk around, receive chunk data, and stay connected.
//!
//! Usage:
//!   cargo run -p neutron-server -- [--port 25565] [--seed 0] [--view-distance 10]

mod chunk_sender;
mod connection;
mod login;
mod play;
mod server;
mod tick;

use clap::Parser;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::mpsc;

use server::{ServerConfig, ServerState};

// ---------------------------------------------------------------------------
// CLI arguments
// ---------------------------------------------------------------------------

#[derive(Parser, Debug)]
#[command(name = "neutron-server")]
#[command(about = "Neutron — a minimal Minecraft 26.2 server")]
struct Args {
    /// TCP port to listen on.
    #[arg(long, default_value_t = 25565)]
    port: u16,

    /// World seed.
    #[arg(long, default_value_t = 0)]
    seed: i64,

    /// Server MOTD (message of the day).
    #[arg(long, default_value = "A Neutron Server")]
    motd: String,

    /// Maximum number of players.
    #[arg(long, default_value_t = 20)]
    max_players: i32,

    /// View distance in chunks.
    #[arg(long, default_value_t = 10)]
    view_distance: i32,

    /// Whether to enforce online-mode (Mojang authentication).
    #[arg(long, default_value_t = false)]
    online_mode: bool,
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Parse CLI arguments.
    let args = Args::parse();

    // Initialize tracing (logging).
    tracing_subscriber::fmt()
        .with_target(false)
        .with_thread_ids(false)
        .init();

    // Build server config.
    let config = ServerConfig {
        port: args.port,
        seed: args.seed,
        motd: args.motd,
        max_players: args.max_players,
        view_distance: args.view_distance,
        online_mode: args.online_mode,
        compression_threshold: -1, // disabled for now
    };

    tracing::info!("starting Neutron server...");

    // Create shared server state.
    let server = Arc::new(ServerState::new(config.clone()));

    // Start tick loop in a background task.
    let tick_server = server.clone();
    let (tick_writer_tx, _tick_writer_rx) = mpsc::channel::<connection::OutgoingPacket>(256);
    tokio::spawn(async move {
        tick::run_tick_loop(tick_server, tick_writer_tx).await;
    });

    // Bind TCP listener.
    let listener = TcpListener::bind(format!("0.0.0.0:{}", config.port)).await?;
    tracing::info!(
        port = config.port,
        seed = config.seed,
        motd = %config.motd,
        max_players = config.max_players,
        view_distance = config.view_distance,
        online_mode = config.online_mode,
        "Neutron server listening"
    );

    // Calculate startup time (like vanilla: "Done (Xs)!").
    let startup_secs = server.start_time.elapsed().as_secs_f64();
    tracing::info!("Done ({:.1}s)!", startup_secs);

    // Accept loop.
    loop {
        let (stream, addr) = listener.accept().await?;
        tracing::debug!(addr = %addr, "accepted connection");

        let server = server.clone();
        tokio::spawn(async move {
            if let Err(e) = connection::handle_connection(stream, server, addr).await {
                tracing::debug!(addr = %addr, error = %e, "connection handler error");
            }
        });
    }
}
