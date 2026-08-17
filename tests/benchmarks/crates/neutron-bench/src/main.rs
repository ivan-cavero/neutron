mod config;
mod diskio;
mod harness;
mod hardware;
mod history;
mod metrics;
mod provision;
mod rcon;
mod reporter;
mod server;
mod tps;
mod types;

use clap::{Parser, Subcommand};
use eyre::Result;
use std::path::PathBuf;

use types::{Scenario, ServerType, Size};

/// Benchmarks workspace root (`tests/benchmarks/`): `CARGO_MANIFEST_DIR` is
/// `crates/neutron-bench`, the workspace root is two levels up. Paths are anchored
/// here so the binary behaves the same regardless of the caller's cwd.
pub fn ws_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

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
    /// Provision and inspect server jars (multi-version layout servers/<type>/<version>/)
    Servers(ServersArgs),
    /// Inspect versioned report history (results/history/)
    History(HistoryArgs),
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

    /// Server version (resolves servers/<type>/<version>/server.jar;
    /// falls back to the legacy servers/<type>/server.jar layout)
    #[arg(long, default_value = "26.2")]
    version: String,

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

    /// Results output directory (relative to the benchmarks workspace root)
    #[arg(long, default_value = "results")]
    results_dir: String,

    /// Log output directory (relative to the benchmarks workspace root)
    #[arg(long, default_value = "logs")]
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

#[derive(Parser)]
struct HistoryArgs {
    #[command(subcommand)]
    command: HistoryCommand,
}

#[derive(Subcommand)]
enum HistoryCommand {
    /// List past runs sorted by time (newest first) with key metrics
    List,
}

#[derive(Parser)]
struct ServersArgs {
    #[command(subcommand)]
    command: ServersCommand,
}

#[derive(Subcommand)]
enum ServersCommand {
    /// Download a server jar into servers/<type>/<version>/server.jar
    Download {
        /// Server type
        #[arg(value_enum)]
        server: ServerType,
        /// Server version (e.g. 26.2)
        version: String,
        /// Skip the network; copy from the NEUTRON_BENCH_SERVERS_FALLBACK cache dir
        #[arg(long)]
        offline: bool,
    },
    /// List downloaded server jars
    List,
    /// Show presence/validity of server jars
    Status,
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
                    &args.version,
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
        Commands::Servers(args) => match args.command {
            ServersCommand::Download {
                server,
                version,
                offline,
            } => {
                provision::download(server, &version, offline).await?;
            }
            ServersCommand::List => provision::list()?,
            ServersCommand::Status => provision::status()?,
        },
        Commands::History(args) => match args.command {
            HistoryCommand::List => history::list()?,
        },
    }

    Ok(())
}
