use std::path::Path;

use anyhow::Result;
use colored::Colorize;

use mc_decompiler_decompile::datapack;

pub fn run(
    version: &str,
    jar: Option<&Path>,
    output: Option<&Path>,
    target: Option<&Path>,
) -> Result<()> {
    let jar_path = match jar {
        Some(p) => p.to_path_buf(),
        None => {
            let expected = std::path::Path::new("jars").join(format!("server-{version}.jar"));
            if expected.exists() {
                expected
            } else {
                anyhow::bail!(
                    "No JAR specified. Use --jar <path> or run `mc-decompiler download {version}` first"
                );
            }
        }
    };

    let out_dir = output
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| std::path::Path::new("output").join(version).join("datapack"));

    // Default diff target: the crate data tree relative to the repo root (cwd).
    // If it is not there, skip the diff with a notice instead of comparing
    // against an empty tree (which would report everything as jar-only).
    let (target_path, target_opt): (String, Option<std::path::PathBuf>) = match target {
        Some(t) => (t.display().to_string(), Some(t.to_path_buf())),
        None => {
            let def = std::path::Path::new("crates/neutron-worldgen/src/data/worldgen");
            if def.is_dir() {
                (def.display().to_string(), Some(def.to_path_buf()))
            } else {
                (
                    def.display().to_string(),
                    None,
                )
            }
        }
    };

    println!("\n{} {}", "Extracting worldgen data:".bold().green(), version);

    let (extract, diff) = datapack::report(&jar_path, &out_dir, target_opt.as_deref())?;
    println!(
        "  {} {} JSON files ({} bytes) -> {}",
        "Extracted:".dimmed(),
        extract.files,
        extract.bytes,
        out_dir.join("worldgen").display()
    );

    match diff {
        Some(summary) => {
            println!("  {} {}", "Semantic diff vs:".dimmed(), target_path);
            for line in datapack::render_diff_summary(&summary).lines() {
                println!("    {line}");
            }
            if summary.changed.is_empty() && summary.jar_only.is_empty() {
                println!("  {} crate tree is up to date with the JAR", "OK:".green());
            } else {
                println!(
                    "  {} data changes detected — port them (see D0-D4 run template)",
                    "WARNING:".yellow()
                );
            }
        }
        None => println!(
            "  {} (target `{}` not found — extraction only)",
            "Skipped diff:".dimmed(),
            target_path
        ),
    }
    Ok(())
}