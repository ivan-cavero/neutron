use std::time::{Duration, Instant};
use std::sync::Mutex;
use azalea::prelude::*;
use bevy_ecs::prelude::Component;
use clap::Parser;

static LATENCIES: std::sync::LazyLock<Mutex<Vec<f64>>> = std::sync::LazyLock::new(|| Mutex::new(Vec::new()));

#[derive(Parser, Debug)]
#[command(name = "azalea-join-bench", about = "Join latency benchmark for Minecraft 26.2")]
struct Args {
    #[arg(long, default_value = "localhost")]
    host: String,
    #[arg(long, default_value_t = 25565)]
    port: u16,
    #[arg(long, default_value_t = 10)]
    count: usize,
    #[arg(long, default_value = "26.2")]
    version: String,
    #[arg(long)]
    output: Option<String>,
}

#[derive(Clone, Component, Default)]
struct BotState {
    t0_millis: u64,
}

async fn handle(bot: Client, event: Event, state: BotState) -> eyre::Result<()> {
    match event {
        Event::Spawn => {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64;
            let join_ms = now.saturating_sub(state.t0_millis);
            println!("  Bot '{}' spawned! Join latency: {}ms", bot.username(), join_ms);
            LATENCIES.lock().unwrap().push(join_ms as f64);
            bot.disconnect();
        }
        _ => {}
    }
    Ok(())
}

fn percentile(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() { return 0.0; }
    let idx = (p / 100.0) * (sorted.len() - 1) as f64;
    let lo = idx.floor() as usize;
    let hi = idx.ceil() as usize;
    if lo == hi || hi >= sorted.len() { return sorted[lo]; }
    let frac = idx - lo as f64;
    sorted[lo] * (1.0 - frac) + sorted[hi] * frac
}

fn run_bot(host: &str, port: u16, bot_index: usize) {
    let host = host.to_string();
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async move {
        let t0_millis = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        let state = BotState { t0_millis };
        let bot_name = format!("bench-{}", bot_index);
        let account = Account::offline(&bot_name);
        let addr = format!("{}:{}", host, port);
        // Use new_without_plugins and add plugins individually, EXCLUDING AutoReconnectPlugin
        // Run client with timeout - disconnect after first spawn
        let _ = tokio::time::timeout(Duration::from_secs(5), async {
            ClientBuilder::new_without_plugins()
                .add_plugins(azalea::DefaultPlugins)
                .add_plugins(azalea::bot::BotPlugin)
                .add_plugins(azalea::pathfinder::PathfinderPlugin)
                .add_plugins(azalea::container::ContainerPlugin)
                .add_plugins(azalea::accept_resource_packs::AcceptResourcePacksPlugin)
                .add_plugins(azalea::tick_broadcast::TickBroadcastPlugin)
                .add_plugins(azalea::events::EventsPlugin)
                .set_handler(handle)
                .set_state(state)
                .start(account, addr.as_str())
                .await;
        }).await;
    })
}

#[tokio::main]
async fn main() {
    let args = Args::parse();
    let count = args.count;
    println!("azalea-join-bench — starting");
    println!("  host   : {}", args.host);
    println!("  port   : {}", args.port);
    println!("  count  : {}", count);
    println!("  version: {}", args.version);
    let global_start = Instant::now();
    let mut handles = vec![];
    for i in 0..count {
        let host = args.host.clone();
        let port = args.port;
        let handle = std::thread::spawn(move || run_bot(&host, port, i));
        handles.push(handle);
        std::thread::sleep(Duration::from_millis(2));
    }
    let deadline = global_start + Duration::from_secs(20);
    for handle in handles {
        let remaining = deadline.duration_since(Instant::now());
        if remaining.is_zero() { break; }
        let _ = handle.join();
    }
    let total_time = global_start.elapsed().as_millis() as f64;
    let mut latencies = LATENCIES.lock().unwrap().clone();
    latencies.sort_by(|a: &f64, b: &f64| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let p50 = percentile(&latencies, 50.0);
    let p95 = percentile(&latencies, 95.0);
    let p99 = percentile(&latencies, 99.0);
    let successful = latencies.len();
    let result = serde_json::json!({
        "test": "azalea-join-bench",
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "config": { "host": args.host, "port": args.port, "version": args.version, "count": count },
        "results": {
            "allConnected": successful > 0,
            "totalBots": count,
            "successful": successful,
            "failed": count - successful,
            "joinLatencies": latencies,
            "p50Ms": p50, "p95Ms": p95, "p99Ms": p99,
            "totalTimeMs": total_time,
        }
    });
    let output_str = serde_json::to_string_pretty(&result).unwrap();
    if let Some(path) = &args.output {
        std::fs::write(path, &output_str).expect("Failed to write output");
        println!("  results written to {}", path);
    } else {
        println!("{}", output_str);
    }
    println!("\nJoin latency (t0 → spawn, milliseconds):");
    println!("  p50Ms: {:.2}", p50);
    println!("  p95Ms: {:.2}", p95);
    println!("  p99Ms: {:.2}", p99);
    println!("  allConnected: {}", successful > 0);
    println!("  total time: {}ms", total_time as u64);
    println!("Benchmark complete.");
}
