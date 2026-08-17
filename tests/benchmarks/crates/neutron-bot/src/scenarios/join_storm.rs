use super::ScenarioConfig;
use crate::client;
use crate::output::JoinStormResult;

pub fn run(config: &ScenarioConfig) -> JoinStormResult {
    let collector = client::launch_join_storm(&config.host, config.port, config.bot_count);

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
