//! Chunk Generation: N bots walk in straight lines, generating new chunks.
//!
//! Each bot walks along the X axis at walking speed (4.3 blocks/s) for the
//! test duration. Measures CPS (chunks per second) total and per-bot.

use super::ScenarioConfig;
use crate::client;
use crate::output::ChunkGenResult;

/// Run the chunk generation scenario.
///
/// 1. Launch all bots
/// 2. Each bot walks along X axis at walking speed
/// 3. Counts chunks received over the duration
/// 4. Reports total CPS and per-bot CPS
pub fn run(config: &ScenarioConfig) -> ChunkGenResult {
    let collector = client::launch_cps(
        &config.host,
        config.port,
        config.bot_count,
        config.duration.as_secs(),
    );

    let join_latencies = collector.get_latencies();
    let successful = collector.bots_spawned.load(std::sync::atomic::Ordering::SeqCst);
    let failed = collector.bots_failed.load(std::sync::atomic::Ordering::SeqCst);
    let total_chunks = collector.chunks_received.load(std::sync::atomic::Ordering::SeqCst);

    let duration_secs = config.duration.as_secs_f64();
    let cps_total = if duration_secs > 0.0 {
        total_chunks as f64 / duration_secs
    } else {
        0.0
    };
    let cps_per_bot = if successful > 0 {
        cps_total / successful as f64
    } else {
        0.0
    };

    // Distance each bot travels: 4.3 blocks/s * duration
    let distance_per_bot = 4.3 * duration_secs;

    println!(
        "  Chunk gen complete: {} bots, {} chunks, CPS={:.1} (total), {:.1} (per bot)",
        successful, total_chunks, cps_total, cps_per_bot
    );

    let per_bot = (0..successful)
        .map(|i| crate::output::BotChunkStats {
            bot_index: i,
            chunks_received: total_chunks / successful.max(1),
            distance_blocks: distance_per_bot,
            duration_secs,
        })
        .collect();

    ChunkGenResult {
        total_bots: config.bot_count,
        successful,
        failed,
        join_latencies,
        walk_duration_secs: duration_secs,
        total_chunks,
        cps_total,
        cps_per_bot,
        per_bot,
        failure_details: Vec::new(),
    }
}
