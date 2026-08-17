use super::ScenarioConfig;
use crate::client;
use crate::output::SpreadResult;

pub fn run(config: &ScenarioConfig) -> SpreadResult {
    let collector = client::launch_spread(&config.host, config.port, config.bot_count);

    let join_latencies = collector.get_latencies();
    let successful = collector.bots_spawned.load(std::sync::atomic::Ordering::SeqCst);
    let failed = collector.bots_failed.load(std::sync::atomic::Ordering::SeqCst);
    let chunks_total = collector.chunks_received.load(std::sync::atomic::Ordering::SeqCst);

    println!("  Spread: {} bots, {} chunks", successful, chunks_total);

    let chunk_loads = (0..successful)
        .map(|i| crate::output::SpreadChunkLoad {
            bot_index: i,
            target_x: (i as f64) * 1001.0,
            target_y: 65.0,
            target_z: 0.0,
            chunks_loaded: 0,
        })
        .collect();

    SpreadResult {
        total_bots: config.bot_count,
        successful,
        failed,
        join_latencies,
        spread_duration_ms: 0.0,
        chunk_loads,
        chunks_total,
        failure_details: Vec::new(),
    }
}
