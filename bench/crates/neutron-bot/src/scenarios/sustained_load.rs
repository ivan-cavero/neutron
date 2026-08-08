use super::ScenarioConfig;
use crate::client;
use crate::output::SustainedLoadResult;

pub fn run(config: &ScenarioConfig) -> SustainedLoadResult {
    println!("  Sustained load: {} bots for {}s...", config.bot_count, config.duration.as_secs());

    let collector = client::launch_sustained_load(
        &config.host, config.port, config.bot_count, config.duration.as_secs(),
    );

    let join_latencies = collector.get_latencies();
    let successful = collector.bots_spawned.load(std::sync::atomic::Ordering::SeqCst);
    let failed = collector.bots_failed.load(std::sync::atomic::Ordering::SeqCst);
    let ticks = collector.ticks_alive.load(std::sync::atomic::Ordering::SeqCst);

    println!("  Sustained load: {} bots, {} ticks", successful, ticks);

    SustainedLoadResult {
        total_bots: config.bot_count,
        successful,
        failed,
        join_latencies,
        duration_secs: config.duration.as_secs_f64(),
        ticks_total: ticks,
        failure_details: Vec::new(),
    }
}
