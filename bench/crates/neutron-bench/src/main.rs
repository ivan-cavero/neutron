mod config;
mod diskio;
mod harness;
mod hardware;
mod metrics;
mod reporter;
mod server;
mod tps;
mod types;

use clap::{Parser, Subcommand};
use eyre::Result;

use types::{Scenario, ServerType, Size};

#[derive(Parser)]
#[command(name = "neutron-bench")]
#[command(about = "Unified benchmark harness for Minecraft servers")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Run benchmarks
    Run(RunArgs),
    /// Compare benchmark results
    Compare(CompareArgs),
    /// Generate markdown report from JSON
    Report(ReportArgs),
}

#[derive(Parser)]
struct RunArgs {
    /// Server type to benchmark
    #[arg(short, long)]
    server: ServerType,

    /// Server size (number of bots)
    #[arg(short, long)]
    size: Size,

    /// Specific scenario to run (default: all)
    #[arg(short, long)]
    scenario: Option<Scenario>,

    /// Server host
    #[arg(long, default_value = "127.0.0.1")]
    host: String,

    /// Server port
    #[arg(long, default_value_t = 25565)]
    port: u16,

    /// Number of iterations per scenario
    #[arg(short, long, default_value_t = 5)]
    runs: u32,

    /// World seed
    #[arg(long, default_value = "1234567890123456789")]
    seed: String,

    /// Warmup duration in seconds
    #[arg(long, default_value_t = 60)]
    warmup_secs: u64,

    /// Scenario duration in seconds (for movement, spread, chunk-gen)
    #[arg(long, default_value_t = 60)]
    duration: u64,

    /// Results output directory
    #[arg(long, default_value = "bench/results")]
    results_dir: String,

    /// Log output directory
    #[arg(long, default_value = "bench/logs")]
    log_dir: String,
}

#[derive(Parser)]
struct CompareArgs {
    /// JSON files to compare
    files: Vec<String>,
}

#[derive(Parser)]
struct ReportArgs {
    /// JSON file to generate report from
    file: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Run(args) => {
            let scenarios = match args.scenario {
                Some(s) => vec![s],
                None => Scenario::all().to_vec(),
            };

            for scenario in &scenarios {
                println!(
                    "Running {} on {} ({} bots, {} iterations)...",
                    scenario.label(),
                    args.server.label(),
                    args.size.bot_count(),
                    args.runs
                );

                harness::run_scenario(
                    args.server,
                    args.size,
                    *scenario,
                    &args.host,
                    args.port,
                    args.runs,
                    &args.seed,
                    args.warmup_secs,
                    args.duration,
                    &args.results_dir,
                    &args.log_dir,
                )
                .await?;
            }

            println!("All benchmarks complete.");
        }
        Commands::Compare(args) => {
            reporter::compare(&args.files)?;
        }
        Commands::Report(args) => {
            reporter::generate_markdown(&args.file)?;
        }
    }

    Ok(())
}
