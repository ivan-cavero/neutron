//! Movement: N bots spawn, then move+jump in a small area (50-block radius).
//!
//! Each bot walks in a random direction, alternates walking/jumping every 2s.
//! Measures: TPS, chunks received, RAM impact.

use super::ScenarioConfig;
use crate::client;
use crate::output::MovementResult;

/// Run the movement scenario.
///
/// 1. Launch all bots (join storm first)
/// 2. Once spawned, each bot moves in a 50-block radius
/// 3. Alternates: walk 2s → jump 1s → turn → repeat
/// 4. Runs for `config.duration` after all bots have spawned
pub fn run(config: &ScenarioConfig) -> MovementResult {
    let collector = client::launch_movement(
        &config.host,
        config.port,
        config.bot_count,
        config.duration.as_secs(),
    );

    let join_latencies = collector.get_latencies();
    let successful = collector.bots_spawned.load(std::sync::atomic::Ordering::SeqCst);
    let failed = collector.bots_failed.load(std::sync::atomic::Ordering::SeqCst);
    let chunks_total = collector.chunks_received.load(std::sync::atomic::Ordering::SeqCst);

    println!(
        "  Movement complete: {} bots, {} total chunks received",
        successful, chunks_total
    );

    MovementResult {
        total_bots: config.bot_count,
        successful,
        failed,
        join_latencies,
        movement_duration_ms: config.duration.as_secs_f64() * 1000.0,
        chunks_per_bot: Vec::new(), // Per-bot chunk counts not tracked in this version
        chunks_total,
        failure_details: Vec::new(),
    }
}
