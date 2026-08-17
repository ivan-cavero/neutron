// Copyright (c) 2026 Neutron Contributors — MIT License
//
// vanilla-hash: Extract chunk checksums from a Minecraft server for parity verification.
//
// Starts a vanilla/paper/folia server, lets it generate chunks around spawn,
// then reads the .mca region files and computes xxHash64 of each chunk's
// decompressed data. Outputs a JSON file suitable for comparison.

#![forbid(unsafe_code)]

mod compare;

use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use anyhow::{bail, Context, Result};
use chrono::Utc;
use clap::{Parser, Subcommand, ValueEnum};
use neutron_world::nbt::ussr_nbt::mutf8::MString;
use neutron_world::nbt::ussr_nbt::owned::{Compound, Nbt, Tag};
use neutron_world::nbt::{compound_get, read_nbt, write_nbt};
use neutron_world::{parse_region_filename, Region};
use xxhash_rust::xxh3::xxh3_64;

/// Extract chunk checksums from a Minecraft server for parity verification.
#[derive(Parser, Debug)]
#[command(name = "vanilla-hash", version, about)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Extract reference data from a running server.
    Extract {
        /// World seed to use.
        #[arg(long)]
        seed: i64,

        /// Server type to use.
        #[arg(long, value_enum, default_value_t = ServerType::Vanilla)]
        server: ServerType,

        /// Output JSON file path.
        #[arg(long)]
        output: PathBuf,

        /// Chunk radius around spawn to include (unused in read-all mode).
        #[arg(long, default_value_t = 4)]
        radius: i32,

        /// Base directory containing server jars (default: bench/servers).
        #[arg(long)]
        servers_dir: Option<PathBuf>,

        /// Timeout in seconds to wait for server startup.
        #[arg(long, default_value_t = 120)]
        startup_timeout: u64,

        /// Don't delete the temp server directory after extraction (for debugging).
        #[arg(long)]
        keep_tmp: bool,

        /// Path to the temp server directory (overrides auto-generated temp dir).
        #[arg(long)]
        tmp_dir: Option<PathBuf>,

        /// What to hash from each chunk.
        #[arg(long, value_enum, default_value_t = HashMode::Full)]
        hash_mode: HashMode,
    },

    /// Compare two reference data JSON files.
    Compare {
        /// Path to the first reference data JSON file (left/baseline).
        #[arg(long)]
        left: PathBuf,

        /// Path to the second reference data JSON file (right/test).
        #[arg(long)]
        right: PathBuf,
    },

    /// Extract reference data for multiple seeds × server types (batch).
    ///
    /// Writes one `<server>-<seed>-<mode>.json` per combination into the
    /// output directory. Replaces the old generate-all.sh/.ps1 scripts.
    ExtractAll {
        /// Seeds to extract (default: 12345 67890 11111 99999 42).
        #[arg(long)]
        seeds: Vec<i64>,

        /// Server types (repeatable; default: vanilla).
        #[arg(long, value_enum)]
        servers: Vec<ServerType>,

        /// Output directory (default: tools/vanilla-hash/hashes).
        #[arg(long)]
        output_dir: Option<PathBuf>,

        /// Base directory containing server jars (default: bench/servers).
        #[arg(long)]
        servers_dir: Option<PathBuf>,

        /// Timeout in seconds to wait for server startup.
        #[arg(long, default_value_t = 120)]
        startup_timeout: u64,

        /// What to hash from each chunk.
        #[arg(long, value_enum, default_value_t = HashMode::Blocks)]
        hash_mode: HashMode,
    },
}

#[derive(clap::ValueEnum, Clone, Debug, PartialEq)]
enum ServerType {
    Vanilla,
    Paper,
    Folia,
}

impl std::fmt::Display for ServerType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ServerType::Vanilla => write!(f, "vanilla"),
            ServerType::Paper => write!(f, "paper"),
            ServerType::Folia => write!(f, "folia"),
        }
    }
}

#[derive(ValueEnum, Clone, Debug, PartialEq)]
enum HashMode {
    /// Hash the full decompressed chunk NBT bytes.
    Full,
    /// Hash only block-state data from each section (deterministic, ignores lighting/timestamps).
    Blocks,
}

impl std::fmt::Display for HashMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HashMode::Full => write!(f, "full"),
            HashMode::Blocks => write!(f, "blocks"),
        }
    }
}

/// Reference data output format.
#[derive(serde::Serialize)]
struct ReferenceData {
    seed: i64,
    server: String,
    version: String,
    generated_at: String,
    hash_mode: String,
    chunks: Vec<ChunkInfo>,
    total_chunks: usize,
}

#[derive(serde::Serialize)]
struct ChunkInfo {
    region_x: i32,
    region_z: i32,
    chunk_x: i32,
    chunk_z: i32,
    hash: String,
    size_bytes: usize,
}

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_target(false)
        .with_level(true)
        .init();

    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Extract {
            seed,
            server,
            output,
            radius,
            servers_dir,
            startup_timeout,
            keep_tmp,
            tmp_dir,
            hash_mode,
        }) => cmd_extract(
            seed,
            server,
            output,
            radius,
            servers_dir,
            startup_timeout,
            keep_tmp,
            tmp_dir,
            hash_mode,
        ),
        Some(Commands::Compare { left, right }) => cmd_compare(left, right),
        Some(Commands::ExtractAll {
            seeds,
            servers,
            output_dir,
            servers_dir,
            startup_timeout,
            hash_mode,
        }) => cmd_extract_all(
            seeds,
            servers,
            output_dir,
            servers_dir,
            startup_timeout,
            hash_mode,
        ),
        None => {
            // Default: show help
            Cli::parse_from(["vanilla-hash", "--help"]);
            Ok(())
        }
    }
}

/// Extract reference data from a server.
fn cmd_extract(
    seed: i64,
    server: ServerType,
    output: PathBuf,
    _radius: i32,
    servers_dir: Option<PathBuf>,
    startup_timeout: u64,
    keep_tmp: bool,
    tmp_dir: Option<PathBuf>,
    hash_mode: HashMode,
) -> Result<()> {
    tracing::info!(
        seed = seed,
        server = %server,
        output = %output.display(),
        "starting vanilla-hash extraction"
    );

    // Determine servers directory
    let repo_root = find_repo_root()?;
    let servers_dir = servers_dir.unwrap_or_else(|| repo_root.join("bench").join("servers"));

    // Create temp directory for server
    let tmp_dir = match &tmp_dir {
        Some(p) => {
            fs::create_dir_all(p)
                .with_context(|| format!("Failed to create tmp dir {}", p.display()))?;
            p.clone()
        }
        None => {
            let base = std::env::temp_dir().join(format!("vanilla-hash-{}", seed));
            if base.exists() {
                fs::remove_dir_all(&base)
                    .with_context(|| format!("Failed to clean old tmp dir {}", base.display()))?;
            }
            fs::create_dir_all(&base)
                .with_context(|| format!("Failed to create tmp dir {}", base.display()))?;
            base
        }
    };

    tracing::info!(tmp_dir = %tmp_dir.display(), "created temp server directory");

    // Set up server
    let server_jar = setup_server(&servers_dir, &server, &tmp_dir, seed)?;

    // Start server
    let mut server_proc = start_server(&server_jar, &tmp_dir)?;

    // Wait for startup
    wait_for_startup(&mut server_proc, startup_timeout)?;

    // Extra time for chunk generation
    tracing::info!("server started, waiting for chunk generation...");
    std::thread::sleep(Duration::from_secs(90));

    // Stop server gracefully
    stop_server(&mut server_proc)?;
    let _ = server_proc.wait();

    // Read .mca files and compute checksums
    // Modern Minecraft (1.18+) uses world/dimensions/minecraft/overworld/region/
    // Older versions use world/region/
    let modern_region_dir = tmp_dir
        .join("world")
        .join("dimensions")
        .join("minecraft")
        .join("overworld")
        .join("region");
    let legacy_region_dir = tmp_dir.join("world").join("region");

    let region_dir = if modern_region_dir.exists() {
        tracing::info!(
            path = %modern_region_dir.display(),
            "found modern region directory"
        );
        &modern_region_dir
    } else if legacy_region_dir.exists() {
        tracing::info!(
            path = %legacy_region_dir.display(),
            "found legacy region directory"
        );
        &legacy_region_dir
    } else {
        tracing::warn!("no region directory found in either modern or legacy path");
        bail!(
            "No region directory found. Checked:\n  {}\n  {}",
            modern_region_dir.display(),
            legacy_region_dir.display()
        );
    };

    tracing::info!(hash_mode = %hash_mode, "hash mode selected");
    let chunks = read_and_hash_regions(region_dir, &hash_mode)?;

    tracing::info!(chunk_count = chunks.len(), "extracted chunk checksums");

    // Determine server version
    let version = match &server {
        ServerType::Vanilla => "26.2",
        ServerType::Paper => "26.2",
        ServerType::Folia => "26.2",
    };

    let reference = ReferenceData {
        seed,
        server: server.to_string(),
        version: version.to_string(),
        generated_at: Utc::now().to_rfc3339(),
        hash_mode: hash_mode.to_string(),
        total_chunks: chunks.len(),
        chunks,
    };

    // Write output
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create output dir {}", parent.display()))?;
    }
    let json = serde_json::to_string_pretty(&reference)?;
    fs::write(&output, &json).with_context(|| format!("Failed to write {}", output.display()))?;

    tracing::info!(path = %output.display(), "saved reference data");

    // Cleanup
    if !keep_tmp {
        tracing::info!("cleaning up temp directory");
        fs::remove_dir_all(&tmp_dir)
            .with_context(|| format!("Failed to remove {}", tmp_dir.display()))?;
    } else {
        tracing::info!(path = %tmp_dir.display(), "keeping tmp dir for debugging");
    }

    println!(
        "Extracted {} chunks to {}",
        reference.total_chunks,
        output.display()
    );
    Ok(())
}

/// Extract reference data for every seed × server combination, one JSON per pair.
fn cmd_extract_all(
    seeds: Vec<i64>,
    servers: Vec<ServerType>,
    output_dir: Option<PathBuf>,
    servers_dir: Option<PathBuf>,
    startup_timeout: u64,
    hash_mode: HashMode,
) -> Result<()> {
    let seeds = if seeds.is_empty() {
        vec![12345, 67890, 11111, 99999, 42]
    } else {
        seeds
    };
    let servers = if servers.is_empty() {
        vec![ServerType::Vanilla]
    } else {
        servers
    };

    let repo_root = find_repo_root()?;
    let servers_dir = servers_dir.unwrap_or_else(|| repo_root.join("bench").join("servers"));
    let output_dir = output_dir
        .unwrap_or_else(|| repo_root.join("tools").join("vanilla-hash").join("hashes"));
    fs::create_dir_all(&output_dir)
        .with_context(|| format!("Failed to create {}", output_dir.display()))?;

    println!("=== Reference Data Generation ===");
    println!("Seeds: {:?}", seeds);
    println!("Servers: {:?}", servers);
    println!("Output: {}", output_dir.display());
    println!();

    for server in &servers {
        for &seed in &seeds {
            let output = output_dir.join(format!("{}-{}-{}.json", server, seed, hash_mode));
            println!("--- Generating: server={} seed={} ---", server, seed);
            cmd_extract(
                seed,
                server.clone(),
                output,
                4, // radius (unused)
                Some(servers_dir.clone()),
                startup_timeout,
                false,
                None,
                hash_mode.clone(),
            )?;
            println!();
        }
    }
    Ok(())
}

/// Compare two reference data JSON files.
fn cmd_compare(left: PathBuf, right: PathBuf) -> Result<()> {
    let left_data = compare::load_reference_data(&left)?;
    let right_data = compare::load_reference_data(&right)?;

    println!(
        "Left:  {} ({} chunks, seed={}, server={})",
        left.display(),
        left_data.total_chunks,
        left_data.seed,
        left_data.server
    );
    println!(
        "Right: {} ({} chunks, seed={}, server={})",
        right.display(),
        right_data.total_chunks,
        right_data.seed,
        right_data.server
    );
    println!();

    let report = compare::compare(&left_data, &right_data);
    compare::print_report(&report);

    if report.different > 0 || report.missing_in_right > 0 || report.missing_in_left > 0 {
        std::process::exit(1);
    }

    println!("\nAll chunks match!");
    Ok(())
}

/// Walk up from the current directory to find the repo root.
fn find_repo_root() -> Result<PathBuf> {
    let mut dir = std::env::current_dir().context("Failed to get current directory")?;
    loop {
        if dir.join("Cargo.toml").exists() && dir.join("AGENTS.md").exists() {
            return Ok(dir);
        }
        dir = dir
            .parent()
            .context("Could not find repo root (reached filesystem root)")?
            .to_path_buf();
    }
}

/// Set up the server in the temp directory.
fn setup_server(
    servers_dir: &Path,
    server_type: &ServerType,
    tmp_dir: &Path,
    seed: i64,
) -> Result<PathBuf> {
    // Copy server jar
    let jar_source = servers_dir.join(format!("server-{}.jar", server_type));
    if !jar_source.exists() {
        bail!("Server jar not found at {}", jar_source.display());
    }
    let jar_dest = tmp_dir.join("server.jar");
    fs::copy(&jar_source, &jar_dest).with_context(|| {
        format!(
            "Failed to copy {} to {}",
            jar_source.display(),
            jar_dest.display()
        )
    })?;
    tracing::info!(source = %jar_source.display(), "copied server jar");

    // Accept EULA
    fs::write(tmp_dir.join("eula.txt"), "eula=true\n")?;

    // Create server.properties
    let props = format!(
        "level-seed={}\n\
         online-mode=false\n\
         view-distance=4\n\
         simulation-distance=4\n\
         level-name=world\n\
         gamemode=creative\n\
         spawn-protection=0\n\
         max-players=1\n",
        seed
    );
    fs::write(tmp_dir.join("server.properties"), props)?;

    // Create ops.json (empty) to avoid prompts
    fs::write(tmp_dir.join("ops.json"), "[]")?;

    tracing::info!(seed = seed, "configured server properties");
    Ok(jar_dest)
}

/// Start the Java server process.
fn start_server(jar_path: &Path, work_dir: &Path) -> Result<Child> {
    tracing::info!("starting java server...");

    let child = Command::new("java")
        .arg("-Xms1G")
        .arg("-Xmx1G")
        .arg("-jar")
        .arg(jar_path)
        .arg("nogui")
        .current_dir(work_dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("Failed to start java process. Is Java installed?")?;

    Ok(child)
}

/// Wait for the server to finish starting by watching stdout for "Done (Xs)!".
fn wait_for_startup(server: &mut Child, timeout_secs: u64) -> Result<()> {
    let stdout = server.stdout.take().context("Server stdout not captured")?;

    let reader = BufReader::new(stdout);
    let start = Instant::now();
    let timeout = Duration::from_secs(timeout_secs);

    for line in reader.lines() {
        let line = line.context("Failed to read server stdout")?;
        tracing::debug!(line = %line, "server stdout");

        if line.contains("Done (") && line.contains(")!") {
            tracing::info!(elapsed = ?start.elapsed(), "server started successfully");
            return Ok(());
        }

        if start.elapsed() > timeout {
            bail!(
                "Server startup timed out after {}s. Last output may indicate the issue.",
                timeout_secs
            );
        }

        // Check if process exited unexpectedly
        if let Some(status) = server.try_wait().context("Failed to check server status")? {
            bail!("Server exited prematurely with status: {:?}", status);
        }
    }

    bail!("Server stdout closed without showing 'Done' message")
}

/// Stop the server by sending "stop" via its stdin.
fn stop_server(server: &mut Child) -> Result<()> {
    tracing::info!("stopping server...");

    if let Some(ref mut stdin) = server.stdin {
        writeln!(stdin, "stop").context("Failed to write 'stop' to server stdin")?;
    } else {
        tracing::warn!("no stdin available, killing server process");
        server.kill().context("Failed to kill server process")?;
    }

    Ok(())
}

/// Read all .mca files in the region directory and compute xxHash64 for each chunk.
fn read_and_hash_regions(region_dir: &Path, hash_mode: &HashMode) -> Result<Vec<ChunkInfo>> {
    if !region_dir.exists() {
        tracing::warn!("region directory does not exist: {}", region_dir.display());
        return Ok(Vec::new());
    }

    let mut chunks = Vec::new();
    let mut entries: Vec<_> = fs::read_dir(region_dir)
        .with_context(|| format!("Failed to read {}", region_dir.display()))?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "mca"))
        .collect();

    entries.sort_by_key(|e| e.file_name());

    for entry in &entries {
        let path = entry.path();
        let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

        let (rx, rz) = match parse_region_filename(filename) {
            Some(coords) => coords,
            None => {
                tracing::warn!(file = filename, "skipping non-region file");
                continue;
            }
        };

        tracing::debug!(file = filename, rx = rx, rz = rz, "reading region");

        let region = match Region::open(&path) {
            Ok(r) => r.with_coords(rx, rz),
            Err(e) => {
                tracing::warn!(file = filename, error = %e, "failed to read region, skipping");
                continue;
            }
        };

        for cz in 0..32i32 {
            for cx in 0..32i32 {
                match region.get_chunk(cx, cz) {
                    Ok(Some(data)) if !data.is_empty() => {
                        let hash = hash_chunk(&data, hash_mode);
                        let global_x = rx * 32 + cx;
                        let global_z = rz * 32 + cz;

                        chunks.push(ChunkInfo {
                            region_x: rx,
                            region_z: rz,
                            chunk_x: global_x,
                            chunk_z: global_z,
                            hash: format!("{:016x}", hash),
                            size_bytes: data.len(),
                        });
                    }
                    Ok(_) => {} // empty chunk, skip
                    Err(e) => {
                        tracing::warn!(
                            file = filename,
                            cx = cx,
                            cz = cz,
                            error = %e,
                            "failed to decompress chunk"
                        );
                    }
                }
            }
        }
    }

    // Sort by region then chunk coordinates for deterministic output
    chunks.sort_by(|a, b| {
        (a.region_x, a.region_z, a.chunk_x, a.chunk_z)
            .cmp(&(b.region_x, b.region_z, b.chunk_x, b.chunk_z))
    });

    Ok(chunks)
}

/// Hash a chunk's data according to the selected hash mode.
fn hash_chunk(data: &[u8], mode: &HashMode) -> u64 {
    match mode {
        HashMode::Full => xxh3_64(data),
        HashMode::Blocks => hash_chunk_blocks(data),
    }
}

/// Parse chunk NBT and hash only the deterministic block data (sections).
///
/// The chunk NBT structure contains non-deterministic fields like lighting data,
/// entity counts, and LastUpdate timestamps. This function extracts only the
/// sections array (which contains block_states and biomes) and hashes that.
fn hash_chunk_blocks(data: &[u8]) -> u64 {
    let nbt = match read_nbt(data) {
        Ok(n) => n,
        Err(e) => {
            tracing::warn!(error = %e, "failed to parse chunk NBT, falling back to full hash");
            return xxh3_64(data);
        }
    };

    // Extract the "sections" list from the root compound
    let sections_bytes = match extract_sections_bytes(&nbt.compound) {
        Some(bytes) => bytes,
        None => {
            tracing::debug!("no sections found in chunk, falling back to full hash");
            return xxh3_64(data);
        }
    };

    xxh3_64(&sections_bytes)
}

/// Extract the sections list as serializable bytes for deterministic hashing.
///
/// We re-serialize the sections array to bytes so the hash is stable across
/// different NBT serialization orders (the ussr-nbt library preserves order).
fn extract_sections_bytes(compound: &Compound) -> Option<Vec<u8>> {
    let sections = match compound_get(compound, "sections")? {
        Tag::List(list) => list,
        _ => return None,
    };

    // Build a minimal NBT containing just the sections list for hashing
    let mut sections_compound = Compound { tags: Vec::new() };
    sections_compound
        .tags
        .push((MString::from("sections"), Tag::List(sections.clone())));

    let root = Nbt {
        name: MString::new(),
        compound: sections_compound,
    };

    Some(write_nbt(&root))
}
