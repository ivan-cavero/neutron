use super::ScenarioConfig;
use crate::client;
use crate::output::MovementResult;

pub fn run(config: &ScenarioConfig) -> MovementResult {
    let collector = client::launch_movement(
        &config.host, config.port, config.bot_count, config.duration.as_secs(),
    );

    let join_latencies = collector.get_latencies();
    let successful = collector.bots_spawned.load(std::sync::atomic::Ordering::SeqCst);
    let failed = collector.bots_failed.load(std::sync::atomic::Ordering::SeqCst);
    let chunks_total = collector.chunks_received.load(std::sync::atomic::Ordering::SeqCst);
    let ticks = collector.ticks_alive.load(std::sync::atomic::Ordering::SeqCst);

    println!("  Movement: {} bots, {} chunks, {} ticks", successful, chunks_total, ticks);

    MovementResult {
        total_bots: config.bot_count,
        successful,
        failed,
        join_latencies,
        movement_duration_ms: config.duration.as_secs_f64() * 1000.0,
        chunks_per_bot: Vec::new(),
        chunks_total,
        ticks_total: ticks,
        failure_details: Vec::new(),
    }
}
