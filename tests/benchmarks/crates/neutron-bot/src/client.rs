//! Real azalea bot client for benchmark scenarios.
//!
//! Each bot runs in its own thread with its own tokio runtime.
//! Supports: join, movement, spread, chunk-gen, sustained-load.

use azalea::prelude::*;
use azalea::app::PluginGroup;
use bevy_ecs::prelude::Component;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// Shared state for collecting results across bot threads.
pub struct BenchCollector {
    pub join_latencies: Mutex<Vec<f64>>,
    pub chunks_received: AtomicUsize,
    pub ticks_alive: AtomicUsize,
    pub bots_spawned: AtomicUsize,
    pub bots_failed: AtomicUsize,
}

impl BenchCollector {
    pub fn new() -> Self {
        Self {
            join_latencies: Mutex::new(Vec::new()),
            chunks_received: AtomicUsize::new(0),
            ticks_alive: AtomicUsize::new(0),
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
    pub mode: BotMode,
}

#[derive(Clone, Debug)]
pub enum BotMode {
    /// Join and disconnect immediately (join-storm).
    JoinOnly,
    /// Join and walk in straight line (chunk-gen).
    WalkStraight { duration_secs: u64 },
    /// Join and move randomly in 50-block radius (movement).
    MoveRandom { duration_secs: u64 },
    /// Join and teleport to far position (spread).
    Spread { x: f64, y: f64, z: f64 },
    /// Join and stay idle for duration (sustained-load).
    Idle { duration_secs: u64 },
    /// Join and walk + move (stress-test with movement).
    StressMove { duration_secs: u64 },
}

impl Default for BotMode {
    fn default() -> Self {
        BotMode::JoinOnly
    }
}

impl Default for BotState {
    fn default() -> Self {
        Self {
            index: 0,
            t0_millis: 0,
            collector: Arc::new(BenchCollector::new()),
            mode: BotMode::JoinOnly,
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

            state.collector.join_latencies.lock().unwrap().push(join_ms as f64);
            state.collector.bots_spawned.fetch_add(1, Ordering::SeqCst);

            println!("  [bot-{}] spawned ({}ms)", state.index, join_ms);

            match &state.mode {
                BotMode::JoinOnly => {
                    bot.disconnect();
                }
                BotMode::Spread { x, y, z } => {
                    // Teleport to position
                    let flags = azalea_protocol::common::movements::MoveFlags::default();
                    let pos = azalea_core::position::Vec3 { x: *x, y: *y, z: *z };
                    let packet = azalea_protocol::packets::game::s_move_player_pos::ServerboundMovePlayerPos { pos, flags };
                    bot.write_packet(packet);
                    tokio::time::sleep(Duration::from_secs(5)).await;
                    bot.disconnect();
                }
                BotMode::Idle { duration_secs } => {
                    // Stay connected for duration
                    tokio::time::sleep(Duration::from_secs(*duration_secs)).await;
                    bot.disconnect();
                }
                BotMode::WalkStraight { duration_secs } => {
                    run_walk_straight(&bot, &state, *duration_secs).await;
                    bot.disconnect();
                }
                BotMode::MoveRandom { duration_secs } => {
                    run_move_random(&bot, &state, *duration_secs).await;
                    bot.disconnect();
                }
                BotMode::StressMove { duration_secs } => {
                    run_stress_move(&bot, &state, *duration_secs).await;
                    bot.disconnect();
                }
            }
        }
        Event::ReceiveChunk(_) => {
            state.collector.chunks_received.fetch_add(1, Ordering::SeqCst);
        }
        Event::Tick => {
            state.collector.ticks_alive.fetch_add(1, Ordering::SeqCst);
        }
        _ => {}
    }
    Ok(())
}

/// Walk straight along X axis at walking speed (4.3 blocks/s).
async fn run_walk_straight(bot: &Client, state: &BotState, duration_secs: u64) {
    let mut x = 0.0_f64;
    let speed = 0.2; // blocks per tick
    let total_ticks = duration_secs * 20;

    for tick in 0..total_ticks {
        x += speed;
        let flags = azalea_protocol::common::movements::MoveFlags::default();
        let pos = azalea_core::position::Vec3 { x, y: 65.0, z: 0.0 };
        let packet = azalea_protocol::packets::game::s_move_player_pos::ServerboundMovePlayerPos { pos, flags };
        bot.write_packet(packet);

        if tick % 200 == 0 {
            let chunks = state.collector.chunks_received.load(Ordering::SeqCst);
            println!("    [bot-{}] tick {}: {} chunks, x={:.1}", state.index, tick, chunks, x);
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

/// Move randomly in a 50-block radius around spawn.
async fn run_move_random(bot: &Client, state: &BotState, duration_secs: u64) {
    let mut x = 0.0_f64;
    let mut z = 0.0_f64;
    let mut direction = 0.0_f64;
    let speed = 0.2;
    let total_ticks = duration_secs * 20;

    for tick in 0..total_ticks {
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

        // Send movement packet
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

        if tick % 200 == 0 {
            let chunks = state.collector.chunks_received.load(Ordering::SeqCst);
            println!("    [bot-{}] tick {}: {} chunks", state.index, tick, chunks);
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

/// Stress test: walk in long lines, generate lots of chunks.
async fn run_stress_move(bot: &Client, state: &BotState, duration_secs: u64) {
    let mut x = 0.0_f64;
    let speed = 0.2;
    let total_ticks = duration_secs * 20;

    for tick in 0..total_ticks {
        x += speed;

        // Send movement packet
        let flags = azalea_protocol::common::movements::MoveFlags::default();
        let pos = azalea_core::position::Vec3 { x, y: 65.0, z: 0.0 };
        let packet = azalea_protocol::packets::game::s_move_player_pos::ServerboundMovePlayerPos { pos, flags };
        bot.write_packet(packet);

        // Jump every 20 ticks (1 second) for more chunk loading
        if tick % 20 >= 18 {
            let jump_pos = azalea_core::position::Vec3 { x, y: 65.5, z: 0.0 };
            let jump_packet = azalea_protocol::packets::game::s_move_player_pos::ServerboundMovePlayerPos { pos: jump_pos, flags };
            bot.write_packet(jump_packet);
        }

        if tick % 500 == 0 {
            let chunks = state.collector.chunks_received.load(Ordering::SeqCst);
            println!("    [bot-{}] tick {}: {} chunks, x={:.1}", state.index, tick, chunks, x);
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

/// Run a single bot in its own thread with its own tokio runtime.
fn run_bot_thread(host: &str, port: u16, state: BotState, timeout: Duration) {
    crate::init_logging();
    let host = host.to_string();
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async move {
        let account = Account::offline(&format!("bench-{}", state.index));
        let addr = format!("{}:{}", host, port);
        let _ = tokio::time::timeout(timeout, async {
            ClientBuilder::new_without_plugins()
                // bevy_log's LogPlugin installs the *global* logger/subscriber;
                // with one App per bot thread only the first can win (exit 101
                // panic on older bevy_log, broken bots on newer). We init the
                // global logger once in crate::init_logging() and disable the
                // plugin here.
                .add_plugins(azalea::DefaultPlugins.build().disable::<bevy_log::LogPlugin>())
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

/// Launch N bots with controlled concurrency.
pub fn launch_bots(
    host: &str,
    port: u16,
    count: usize,
    batch_size: usize,
    timeout: Duration,
    mode: BotMode,
) -> Arc<BenchCollector> {
    let collector = Arc::new(BenchCollector::new());
    let t0_millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;

    let batches: Vec<Vec<usize>> = (0..count)
        .collect::<Vec<_>>()
        .chunks(batch_size)
        .map(|chunk| chunk.to_vec())
        .collect();

    for (batch_idx, batch) in batches.iter().enumerate() {
        let mut handles = Vec::with_capacity(batch.len());

        for &i in batch {
            let state = BotState {
                index: i,
                t0_millis,
                collector: Arc::clone(&collector),
                mode: mode.clone(),
            };

            let host = host.to_string();
            let handle = std::thread::spawn(move || {
                run_bot_thread(&host, port, state, timeout);
            });
            handles.push(handle);

            std::thread::sleep(Duration::from_millis(10));
        }

        let deadline = Instant::now() + timeout + Duration::from_secs(5);
        for handle in handles {
            let remaining = deadline.duration_since(Instant::now());
            if remaining.is_zero() {
                break;
            }
            let _ = handle.join();
        }

        if batch_idx + 1 < batches.len() {
            std::thread::sleep(Duration::from_millis(100));
        }
    }

    collector
}

// ── Convenience functions for each scenario ────────────────────────────────

pub fn launch_join_storm(host: &str, port: u16, count: usize) -> Arc<BenchCollector> {
    let batch_size = if count > 200 { 50 } else if count > 50 { 25 } else { count };
    // MC 26.2's config phase (registries/tags) takes ~30s when bots join
    // concurrently; 10s killed every bot before Spawn fired (0/10 connected).
    launch_bots(host, port, count, batch_size, Duration::from_secs(90), BotMode::JoinOnly)
}

pub fn launch_distributed(host: &str, port: u16, count: usize) -> Arc<BenchCollector> {
    // Distributed: launch one bot per second
    let collector = Arc::new(BenchCollector::new());
    let t0_millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;

    let mut handles = Vec::with_capacity(count);
    for i in 0..count {
        let state = BotState {
            index: i,
            t0_millis,
            collector: Arc::clone(&collector),
            mode: BotMode::JoinOnly,
        };
        let host = host.to_string();
        let handle = std::thread::spawn(move || {
            run_bot_thread(&host, port, state, Duration::from_secs(90));
        });
        handles.push(handle);
        if i + 1 < count {
            std::thread::sleep(Duration::from_secs(1));
        }
    }

    let deadline = Instant::now() + Duration::from_secs(count as u64 * 2 + 30);
    for handle in handles {
        let remaining = deadline.duration_since(Instant::now());
        if remaining.is_zero() { break; }
        let _ = handle.join();
    }

    collector
}

pub fn launch_movement(host: &str, port: u16, count: usize, duration_secs: u64) -> Arc<BenchCollector> {
    let batch_size = if count > 200 { 50 } else if count > 50 { 25 } else { count };
    launch_bots(host, port, count, batch_size,
        Duration::from_secs(duration_secs + 30),
        BotMode::MoveRandom { duration_secs })
}

pub fn launch_spread(host: &str, port: u16, count: usize) -> Arc<BenchCollector> {
    let batch_size = if count > 200 { 50 } else if count > 50 { 25 } else { count };
    // Each bot gets a unique position
    let collector = Arc::new(BenchCollector::new());
    let t0_millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;

    let batches: Vec<Vec<usize>> = (0..count)
        .collect::<Vec<_>>()
        .chunks(batch_size)
        .map(|chunk| chunk.to_vec())
        .collect();

    for (batch_idx, batch) in batches.iter().enumerate() {
        let mut handles = Vec::with_capacity(batch.len());
        for &i in batch {
            let state = BotState {
                index: i,
                t0_millis,
                collector: Arc::clone(&collector),
                mode: BotMode::Spread {
                    x: (i as f64) * 1001.0,
                    y: 65.0,
                    z: 0.0,
                },
            };
            let host = host.to_string();
            let handle = std::thread::spawn(move || {
                run_bot_thread(&host, port, state, Duration::from_secs(90));
            });
            handles.push(handle);
            std::thread::sleep(Duration::from_millis(10));
        }

        let deadline = Instant::now() + Duration::from_secs(20);
        for handle in handles {
            let remaining = deadline.duration_since(Instant::now());
            if remaining.is_zero() { break; }
            let _ = handle.join();
        }

        if batch_idx + 1 < batches.len() {
            std::thread::sleep(Duration::from_millis(100));
        }
    }

    collector
}

pub fn launch_chunk_gen(host: &str, port: u16, count: usize, duration_secs: u64) -> Arc<BenchCollector> {
    let batch_size = if count > 200 { 50 } else if count > 50 { 25 } else { count };
    launch_bots(host, port, count, batch_size,
        Duration::from_secs(duration_secs + 30),
        BotMode::WalkStraight { duration_secs })
}

pub fn launch_sustained_load(host: &str, port: u16, count: usize, duration_secs: u64) -> Arc<BenchCollector> {
    let batch_size = if count > 200 { 50 } else if count > 50 { 25 } else { count };
    launch_bots(host, port, count, batch_size,
        Duration::from_secs(duration_secs + 30),
        BotMode::Idle { duration_secs })
}

pub fn launch_stress_test(host: &str, port: u16, count: usize, duration_secs: u64) -> Arc<BenchCollector> {
    let batch_size = if count > 200 { 50 } else if count > 50 { 25 } else { count };
    launch_bots(host, port, count, batch_size,
        Duration::from_secs(duration_secs + 30),
        BotMode::StressMove { duration_secs })
}
