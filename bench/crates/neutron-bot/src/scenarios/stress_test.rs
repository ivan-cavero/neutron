use super::ScenarioConfig;
use crate::client;
use crate::output::StressTestResult;

pub fn run(config: &ScenarioConfig) -> StressTestResult {
    println!("  Stress test: {} bots moving for {}s...", config.bot_count, config.duration.as_secs());

    let collector = client::launch_stress_test(
        &config.host, config.port, config.bot_count, config.duration.as_secs(),
    );

    let join_latencies = collector.get_latencies();
    let successful = collector.bots_spawned.load(std::sync::atomic::Ordering::SeqCst);
    let failed = collector.bots_failed.load(std::sync::atomic::Ordering::SeqCst);
    let chunks_total = collector.chunks_received.load(std::sync::atomic::Ordering::SeqCst);
    let ticks = collector.ticks_alive.load(std::sync::atomic::Ordering::SeqCst);

    let cps = if config.duration.as_secs_f64() > 0.0 {
        chunks_total as f64 / config.duration.as_secs_f64()
    } else {
        0.0
    };

    println!("  Stress test: {} bots, {} chunks, CPS={:.1}", successful, chunks_total, cps);

    StressTestResult {
        total_bots: config.bot_count,
        successful,
        failed,
        join_latencies,
        duration_secs: config.duration.as_secs_f64(),
        total_chunks: chunks_total,
        cps,
        ticks_total: ticks,
        failure_details: Vec::new(),
    }
}
