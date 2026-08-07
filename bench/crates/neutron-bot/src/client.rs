//! Real azalea bot client for benchmark scenarios.
//!
//! Each bot runs in its own OS thread with its own tokio runtime,
//! following the pattern from the working azalea-join-bench.

use azalea::prelude::*;
use bevy_ecs::prelude::Component;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// Shared state for collecting results across bot threads.
pub struct BenchCollector {
    pub join_latencies: Mutex<Vec<f64>>,
    pub chunks_received: AtomicUsize,
    pub bots_spawned: AtomicUsize,
    pub bots_failed: AtomicUsize,
}

impl BenchCollector {
    pub fn new() -> Self {
        Self {
            join_latencies: Mutex::new(Vec::new()),
            chunks_received: AtomicUsize::new(0),
            bots_spawned: AtomicUsize::new(0),
            bots_failed: AtomicUsize::new(0),
        }
    }

    pub fn get_latencies(&self) -> Vec<f64> {
        let mut latencies = self.join_latencies.lock().unwrap().clone();
        latencies.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        latencies
    }
}

/// State passed to each azalea bot handler.
#[derive(Clone, Component)]
pub struct BotState {
    pub index: usize,
    pub t0_millis: u64,
    pub collector: Arc<BenchCollector>,
    /// Whether to disconnect immediately after spawn (for join benchmarks).
    pub disconnect_on_spawn: bool,
    /// Whether to walk and count chunks (for CPS benchmarks).
    pub walk_and_count_chunks: bool,
    /// Walk duration in ticks (for CPS benchmarks).
    pub walk_ticks: u64,
    /// Whether to spread to a far position (for spread benchmarks).
    pub spread_position: Option<(f64, f64, f64)>,
    /// Whether to move randomly in a small area (for movement benchmarks).
    pub movement_mode: bool,
}

impl Default for BotState {
    fn default() -> Self {
        Self {
            index: 0,
            t0_millis: 0,
            collector: Arc::new(BenchCollector::new()),
            disconnect_on_spawn: true,
            walk_and_count_chunks: false,
            walk_ticks: 0,
            spread_position: None,
            movement_mode: false,
        }
    }
}

/// Azalea event handler for benchmark bots.
async fn bench_handler(bot: Client, event: Event, mut state: BotState) -> eyre::Result<()> {
    match event {
        Event::Spawn => {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64;
            let join_ms = now.saturating_sub(state.t0_millis);

            println!(
                "  [bot-{}] spawned! Join latency: {}ms",
                state.index, join_ms
            );

            state.collector.join_latencies.lock().unwrap().push(join_ms as f64);
            state.collector.bots_spawned.fetch_add(1, Ordering::SeqCst);

            if state.disconnect_on_spawn {
                bot.disconnect();
                return Ok(());
            }

            // Spread mode: teleport to far position
            if let Some((x, y, z)) = state.spread_position {
                println!("  [bot-{}] spreading to ({:.0}, {:.0}, {:.0})", state.index, x, y, z);
                let flags = azalea_protocol::common::movements::MoveFlags::default();
                let pos = azalea_core::position::Vec3 { x, y, z };
                let packet = azalea_protocol::packets::game::s_move_player_pos::ServerboundMovePlayerPos { pos, flags };
                bot.write_packet(packet);
                // Wait a bit then disconnect
                tokio::time::sleep(Duration::from_secs(5)).await;
                bot.disconnect();
                return Ok(());
            }

            // Movement mode: move in a small area
            if state.movement_mode {
                println!("  [bot-{}] starting movement pattern", state.index);
                let mut x = 0.0;
                let mut z = 0.0;
                let mut direction = 0.0_f64;
                let speed = 0.2; // blocks per tick

                for tick in 0..(state.walk_ticks) {
                    // Change direction every 40 ticks (2 seconds)
                    if tick % 40 == 0 {
                        direction += std::f64::consts::FRAC_PI_4; // 45 degrees
                    }

                    x += direction.cos() * speed;
                    z += direction.sin() * speed;

                    // Keep within 50-block radius
                    let dist = (x * x + z * z).sqrt();
                    if dist > 50.0 {
                        x *= 50.0 / dist;
                        z *= 50.0 / dist;
                        direction += std::f64::consts::PI;
                    }

                    let flags = azalea_protocol::common::movements::MoveFlags::default();
                    let pos = azalea_core::position::Vec3 { x, y: 65.0, z };
                    let packet = azalea_protocol::packets::game::s_move_player_pos::ServerboundMovePlayerPos { pos, flags };
                    bot.write_packet(packet);

                    // Jump every 60 ticks (3 seconds)
                    if tick % 60 >= 55 {
                        let jump_pos = azalea_core::position::Vec3 { x, y: 65.5, z };
                        let jump_packet = azalea_protocol::packets::game::s_move_player_pos::ServerboundMovePlayerPos { pos: jump_pos, flags };
                        bot.write_packet(jump_packet);
                    }

                    tokio::time::sleep(Duration::from_millis(50)).await;
                }
                bot.disconnect();
                return Ok(());
            }

            // CPS mode: walk in straight line counting chunks
            if state.walk_and_count_chunks {
                println!("  [bot-{}] walking to generate chunks...", state.index);
                let mut x = 0.0;
                let speed = 0.2; // blocks per tick (4 blocks/second)

                for tick in 0..state.walk_ticks {
                    x += speed;
                    let flags = azalea_protocol::common::movements::MoveFlags::default();
                    let pos = azalea_core::position::Vec3 { x, y: 65.0, z: 0.0 };
                    let packet = azalea_protocol::packets::game::s_move_player_pos::ServerboundMovePlayerPos { pos, flags };
                    bot.write_packet(packet);

                    if tick % 200 == 0 {
                        let chunks = state.collector.chunks_received.load(Ordering::SeqCst);
                        println!(
                            "    [bot-{}] tick {}: {} chunks, x={:.1}",
                            state.index, tick, chunks, x
                        );
                    }
                    tokio::time::sleep(Duration::from_millis(50)).await;
                }
                bot.disconnect();
                return Ok(());
            }
        }
        Event::ReceiveChunk(_) => {
            state.collector.chunks_received.fetch_add(1, Ordering::SeqCst);
        }
        _ => {}
    }
    Ok(())
}

/// Run a single bot in its own thread with its own tokio runtime.
///
/// This follows the pattern from the working azalea-join-bench.
fn run_bot_thread(
    host: &str,
    port: u16,
    state: BotState,
    timeout: Duration,
) {
    let host = host.to_string();
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async move {
        let account = Account::offline(&format!("bench-{}", state.index));
        let addr = format!("{}:{}", host, port);

        let _ = tokio::time::timeout(timeout, async {
            ClientBuilder::new_without_plugins()
                .add_plugins(azalea::DefaultPlugins)
                .add_plugins(azalea::bot::BotPlugin)
                .add_plugins(azalea::pathfinder::PathfinderPlugin)
                .add_plugins(azalea::container::ContainerPlugin)
                .add_plugins(azalea::accept_resource_packs::AcceptResourcePacksPlugin)
                .add_plugins(azalea::tick_broadcast::TickBroadcastPlugin)
                .add_plugins(azalea::events::EventsPlugin)
                .set_handler(bench_handler)
                .set_state(state)
                .start(account, addr.as_str())
                .await;
        }).await;
    });
}

/// Launch N bots for a join-storm scenario.
///
/// Each bot connects simultaneously, measures join latency, and disconnects.
pub fn launch_join_storm(
    host: &str,
    port: u16,
    count: usize,
    stagger_ms: u64,
) -> Arc<BenchCollector> {
    let collector = Arc::new(BenchCollector::new());
    let mut handles = Vec::with_capacity(count);
    let t0_millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;

    for i in 0..count {
        let state = BotState {
            index: i,
            t0_millis,
            collector: Arc::clone(&collector),
            disconnect_on_spawn: true,
            walk_and_count_chunks: false,
            walk_ticks: 0,
            spread_position: None,
            movement_mode: false,
        };

        let host = host.to_string();
        let handle = std::thread::spawn(move || {
            run_bot_thread(&host, port, state, Duration::from_secs(10));
        });
        handles.push(handle);

        if i + 1 < count {
            std::thread::sleep(Duration::from_millis(stagger_ms));
        }
    }

    // Wait for all bots with a global deadline
    let deadline = Instant::now() + Duration::from_secs(30);
    for handle in handles {
        let remaining = deadline.duration_since(Instant::now());
        if remaining.is_zero() {
            break;
        }
        let _ = handle.join();
    }

    collector
}

/// Launch N bots for a distributed-join scenario.
///
/// Each bot connects one per second.
pub fn launch_distributed(
    host: &str,
    port: u16,
    count: usize,
    interval_secs: u64,
) -> Arc<BenchCollector> {
    let collector = Arc::new(BenchCollector::new());
    let mut handles = Vec::with_capacity(count);
    let t0_millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;

    for i in 0..count {
        let state = BotState {
            index: i,
            t0_millis,
            collector: Arc::clone(&collector),
            disconnect_on_spawn: true,
            walk_and_count_chunks: false,
            walk_ticks: 0,
            spread_position: None,
            movement_mode: false,
        };

        let host = host.to_string();
        let handle = std::thread::spawn(move || {
            run_bot_thread(&host, port, state, Duration::from_secs(10));
        });
        handles.push(handle);

        // Wait 1 second before next bot
        if i + 1 < count {
            std::thread::sleep(Duration::from_secs(interval_secs));
        }
    }

    // Wait for all bots
    let deadline = Instant::now() + Duration::from_secs(count as u64 * 2 + 10);
    for handle in handles {
        let remaining = deadline.duration_since(Instant::now());
        if remaining.is_zero() {
            break;
        }
        let _ = handle.join();
    }

    collector
}

/// Launch N bots for a CPS (chunk generation) scenario.
///
/// Each bot walks in a straight line, counting chunks received.
pub fn launch_cps(
    host: &str,
    port: u16,
    count: usize,
    duration_secs: u64,
) -> Arc<BenchCollector> {
    let collector = Arc::new(BenchCollector::new());
    let mut handles = Vec::with_capacity(count);
    let t0_millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    let walk_ticks = duration_secs * 20; // 20 TPS

    for i in 0..count {
        let state = BotState {
            index: i,
            t0_millis,
            collector: Arc::clone(&collector),
            disconnect_on_spawn: false,
            walk_and_count_chunks: true,
            walk_ticks,
            spread_position: None,
            movement_mode: false,
        };

        let host = host.to_string();
        let handle = std::thread::spawn(move || {
            run_bot_thread(&host, port, state, Duration::from_secs(duration_secs + 15));
        });
        handles.push(handle);

        // Small stagger to avoid thundering herd
        if i + 1 < count {
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    // Wait for all bots
    let deadline = Instant::now() + Duration::from_secs(duration_secs + 30);
    for handle in handles {
        let remaining = deadline.duration_since(Instant::now());
        if remaining.is_zero() {
            break;
        }
        let _ = handle.join();
    }

    collector
}

/// Launch N bots for a movement scenario.
///
/// Each bot moves randomly in a 50-block radius, alternating walk/jump.
pub fn launch_movement(
    host: &str,
    port: u16,
    count: usize,
    duration_secs: u64,
) -> Arc<BenchCollector> {
    let collector = Arc::new(BenchCollector::new());
    let mut handles = Vec::with_capacity(count);
    let t0_millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    let walk_ticks = duration_secs * 20;

    for i in 0..count {
        let state = BotState {
            index: i,
            t0_millis,
            collector: Arc::clone(&collector),
            disconnect_on_spawn: false,
            walk_and_count_chunks: false,
            walk_ticks,
            spread_position: None,
            movement_mode: true,
        };

        let host = host.to_string();
        let handle = std::thread::spawn(move || {
            run_bot_thread(&host, port, state, Duration::from_secs(duration_secs + 15));
        });
        handles.push(handle);

        if i + 1 < count {
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    let deadline = Instant::now() + Duration::from_secs(duration_secs + 30);
    for handle in handles {
        let remaining = deadline.duration_since(Instant::now());
        if remaining.is_zero() {
            break;
        }
        let _ = handle.join();
    }

    collector
}

/// Launch N bots for a spread scenario.
///
/// Each bot teleports to a unique far-away position (>1000 blocks apart).
pub fn launch_spread(
    host: &str,
    port: u16,
    count: usize,
) -> Arc<BenchCollector> {
    let collector = Arc::new(BenchCollector::new());
    let mut handles = Vec::with_capacity(count);
    let t0_millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;

    for i in 0..count {
        // Each bot gets a unique position, 1001 blocks apart along X axis
        let x = (i as f64) * 1001.0;
        let state = BotState {
            index: i,
            t0_millis,
            collector: Arc::clone(&collector),
            disconnect_on_spawn: false,
            walk_and_count_chunks: false,
            walk_ticks: 0,
            spread_position: Some((x, 65.0, 0.0)),
            movement_mode: false,
        };

        let host = host.to_string();
        let handle = std::thread::spawn(move || {
            run_bot_thread(&host, port, state, Duration::from_secs(15));
        });
        handles.push(handle);

        if i + 1 < count {
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    let deadline = Instant::now() + Duration::from_secs(count as u64 + 20);
    for handle in handles {
        let remaining = deadline.duration_since(Instant::now());
        if remaining.is_zero() {
            break;
        }
        let _ = handle.join();
    }

    collector
}
