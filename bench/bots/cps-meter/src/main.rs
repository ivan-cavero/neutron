use std::time::{Duration, Instant};
use std::sync::atomic::{AtomicUsize, Ordering};
use azalea::prelude::*;
use azalea_protocol::packets::game::s_move_player_pos::ServerboundMovePlayerPos;
use azalea_core::position::Vec3;
use bevy_ecs::prelude::Component;
use clap::Parser;

static CHUNKS_RECEIVED: AtomicUsize = AtomicUsize::new(0);

#[derive(Parser, Debug)]
#[command(name = "cps-meter", about = "CPS measurement for Minecraft 26.2")]
struct Args {
    #[arg(long, default_value = "localhost")]
    host: String,
    #[arg(long, default_value_t = 25565)]
    port: u16,
    #[arg(long, default_value = "26.2")]
    version: String,
    #[arg(long, default_value_t = 30)]
    duration: u64,
    #[arg(long)]
    output: Option<String>,
}

#[derive(Clone, Component, Default)]
struct BotState {
    t0_millis: u64,
    spawned: bool,
}

async fn handle(bot: Client, event: Event, state: BotState) -> eyre::Result<()> {
    match event {
        Event::Spawn => {
            if !state.spawned {
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as u64;
                let join_ms = now.saturating_sub(state.t0_millis);
                println!("  Bot spawned! Join latency: {}ms", join_ms);
                println!("  Walking to generate chunks...");

                // Walk in a straight line at walking speed
                let mut x = 0.0;
                let speed = 0.2; // blocks per tick (4 blocks/second)

                for tick in 0..1000 { // ~50 seconds
                    x += speed;
                    let pos = Vec3 { x, y: 65.0, z: 0.0 }; // Walk at ground level
                    let flags = azalea_protocol::common::movements::MoveFlags::default();
                    let packet = ServerboundMovePlayerPos { pos, flags };
                    bot.write_packet(packet);

                    if tick % 100 == 0 {
                        let chunks = CHUNKS_RECEIVED.load(Ordering::SeqCst);
                        println!("  Tick {}: {} chunks, x={:.1}", tick, chunks, x);
                    }
                    tokio::time::sleep(Duration::from_millis(50)).await;
                }
            }
        }
        Event::ReceiveChunk(_) => {
            CHUNKS_RECEIVED.fetch_add(1, Ordering::SeqCst);
        }
        _ => {}
    }
    Ok(())
}

#[tokio::main]
async fn main() {
    let args = Args::parse();
    println!("cps-meter — starting");
    println!("  host   : {}", args.host);
    println!("  port   : {}", args.port);
    let start = Instant::now();
    let host = args.host.clone();
    let port = args.port;
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async move {
        let t0_millis = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        let state = BotState { t0_millis, spawned: false };
        let account = Account::offline("cps-bot");
        let addr = format!("{}:{}", host, port);
        ClientBuilder::new_without_plugins()
            .add_plugins(azalea::DefaultPlugins)
            .add_plugins(azalea::bot::BotPlugin)
            .add_plugins(azalea::pathfinder::PathfinderPlugin)
            .add_plugins(azalea::container::ContainerPlugin)
            .add_plugins(azalea::accept_resource_packs::AcceptResourcePacksPlugin)
            .add_plugins(azalea::tick_broadcast::TickBroadcastPlugin)
            .add_plugins(azalea::events::EventsPlugin)
            .set_handler(handle)
            .set_state(state)
            .start(account, addr.as_str())
            .await;
    });
    let elapsed = start.elapsed().as_secs_f64();
    let total_chunks = CHUNKS_RECEIVED.load(Ordering::SeqCst);
    let cps = if elapsed > 0.0 { total_chunks as f64 / elapsed } else { 0.0 };
    println!("\n=== CPS RESULTS ===");
    println!("  Duration: {:.1}s", elapsed);
    println!("  Total chunks: {}", total_chunks);
    println!("  CPS: {:.1}", cps);
    if let Some(path) = &args.output {
        let result = serde_json::json!({
            "test": "cps-meter",
            "config": { "host": args.host, "port": args.port },
            "results": { "duration_s": elapsed, "total_chunks": total_chunks, "cps": cps }
        });
        std::fs::write(path, serde_json::to_string_pretty(&result).unwrap()).expect("write failed");
        println!("  written to {}", path);
    }
}
