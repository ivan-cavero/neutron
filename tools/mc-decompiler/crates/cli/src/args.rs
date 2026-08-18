use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "mc-decompiler", about = "Decompile Minecraft server JARs")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Verify Java and Vineflower are installed
    Setup,
    /// Decompile a server version
    Decompile {
        /// Version (e.g., "26.2", "1.21.4")
        version: String,
        /// Path to server.jar (optional if already downloaded)
        #[arg(short, long)]
        jar: Option<PathBuf>,
        /// Force re-decompile even if already done
        #[arg(long)]
        force: bool,
    },
    /// Download a server JAR from Mojang
    Download {
        /// Version (e.g., "26.2", "1.21.4")
        version: String,
        /// Output directory (default: jars/)
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    /// List decompiled versions
    List,
    /// Compare two versions
    Diff {
        from: String,
        to: String,
        #[arg(short, long)]
        class: Option<String>,
    },
    /// Search classes
    Search {
        query: String,
        #[arg(short, long)]
        version: Option<String>,
    },
    /// Show a specific class
    Show { version: String, class: String },
    /// Extract the vanilla worldgen datapack (JSON) from a server version and
    /// diff it against the crate data tree (version-bump data detection)
    ExtractData {
        /// Version (e.g., "26.2", "1.21.4")
        version: String,
        /// Path to server.jar (optional if already downloaded)
        #[arg(short, long)]
        jar: Option<PathBuf>,
        /// Output directory (default: output/<version>/datapack)
        #[arg(short, long)]
        output: Option<PathBuf>,
        /// Crate data tree to diff against
        /// (default: crates/neutron-worldgen/src/data/worldgen)
        #[arg(long)]
        target: Option<PathBuf>,
    },
}
