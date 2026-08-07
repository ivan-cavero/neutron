//! Join Storm: N bots connect simultaneously (within 200ms total).
//!
//! Measures join latency from t0 to spawn for each bot.
//! Reports p50/p95/p99 percentiles.

use super::ScenarioConfig;
use crate::client;
use crate::output::JoinStormResult;

/// Run the join-storm scenario.
///
/// Launches `config.bot_count` bots with minimal stagger (<200ms total).
/// Returns aggregate join metrics.
pub fn run(config: &ScenarioConfig) -> JoinStormResult {
    let stagger_ms = if config.bot_count > 1 {
        (200.0 / config.bot_count as f64).max(1.0) as u64
    } else {
        0
    };

    let collector = client::launch_join_storm(
        &config.host,
        config.port,
        config.bot_count,
        stagger_ms,
    );

    let latencies = collector.get_latencies();
    let successful = collector.bots_spawned.load(std::sync::atomic::Ordering::SeqCst);
    let failed = collector.bots_failed.load(std::sync::atomic::Ordering::SeqCst);

    JoinStormResult {
        total_bots: config.bot_count,
        successful,
        failed,
        latencies,
        failure_details: Vec::new(),
    }
}
