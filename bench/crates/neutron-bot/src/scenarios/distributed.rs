use super::ScenarioConfig;
use crate::client;
use crate::output::{DistributedBotEntry, DistributedResult};

pub fn run(config: &ScenarioConfig) -> DistributedResult {
    let collector = client::launch_distributed(&config.host, config.port, config.bot_count);

    let latencies = collector.get_latencies();
    let successful = collector.bots_spawned.load(std::sync::atomic::Ordering::SeqCst);
    let failed = collector.bots_failed.load(std::sync::atomic::Ordering::SeqCst);

    let per_bot: Vec<DistributedBotEntry> = latencies
        .iter()
        .enumerate()
        .map(|(i, &latency)| {
            let launch_at_ms = (i as u64 * 1000) as f64;
            DistributedBotEntry {
                index: i,
                join_latency_ms: latency,
                queue_time_ms: latency - launch_at_ms,
                success: true,
            }
        })
        .collect();

    DistributedResult {
        total_bots: config.bot_count,
        launched: successful,
        successful,
        failed,
        per_bot,
        failure_details: Vec::new(),
        latencies,
    }
}
