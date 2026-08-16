use std::path::Path;

use anyhow::Result;
use colored::Colorize;
use indicatif::{ProgressBar, ProgressStyle};

use mc_decompiler_core::VersionMetadata;
use mc_decompiler_store::Store;

use crate::{inventory, vineflower};

/// Decompile a single Minecraft server version.
///
/// Creates `store.base_path()/<version>/src/` with decompiled Java source.
/// Intermediate files (.class, .extracted) are cleaned up after completion.
///
/// # Errors
/// Returns an error if the JAR is invalid, Vineflower fails, or I/O fails.
pub fn decompile_version(
    store: &Store,
    version: &str,
    jar_path: &Path,
    vineflower_jar: &Path,
) -> Result<()> {
    println!("\n{} {}", "Decompiling version:".bold().green(), version);

    if store.get_version(version)?.is_some() {
        anyhow::bail!("Version {version} is already decompiled. Use --force to re-decompile.");
    }
    if !jar_path.exists() {
        anyhow::bail!("JAR not found: {}", jar_path.display());
    }

    // 1. Resolve bundler JAR (Mojang wraps real server inside)
    println!("  {}", "Resolving JAR...".dimmed());
    let resolved_jar = inventory::resolve_server_jar(jar_path)?;
    let extracted_dir = if resolved_jar != jar_path {
        let dir = resolved_jar.parent().unwrap_or(Path::new(".")).to_path_buf();
        println!(
            "  {} {}",
            "Extracted inner server JAR:".dimmed(),
            resolved_jar.display()
        );
        Some(dir)
    } else {
        None
    };

    // 2. Hash
    println!("  {}", "Computing JAR hash...".dimmed());
    let hash = inventory::compute_sha256(&resolved_jar)?;
    println!("  {} {}", "SHA-256:".dimmed(), &hash[..16]);

    // 3. Inventory
    println!("  {}", "Listing classes...".dimmed());
    let classes = inventory::list_classes(&resolved_jar)?;
    println!("  {} {} classes", "Found:".dimmed(), classes.len());

    // 4. Create output dir, use system temp for intermediate .class files
    let src_dir = store.src_dir(version);
    std::fs::create_dir_all(&src_dir)?;
    let classes_dir = std::env::temp_dir().join(format!("mc-decompiler-{version}"));
    std::fs::create_dir_all(&classes_dir)?;

    // 5. Extract .class files to temp
    let pb = ProgressBar::new(classes.len() as u64);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} {msg}")
            .unwrap()
            .progress_chars("=>-"),
    );
    pb.set_message("Extracting classes");
    let class_count = inventory::extract_classes(&resolved_jar, &classes_dir)?;
    pb.set_position(u64::from(class_count));
    pb.finish_with_message("done");

    // 6. Decompile with Vineflower
    println!("  {}", "Running Vineflower...".dimmed());
    vineflower::decompile(&classes_dir, &src_dir, vineflower_jar)?;

    // 7. Cleanup intermediates
    let _ = std::fs::remove_dir_all(&classes_dir);
    if let Some(dir) = &extracted_dir {
        let _ = std::fs::remove_dir_all(dir);
    }

    // 8. Count lines
    println!("  {}", "Counting lines...".dimmed());
    let total_lines = count_java_lines(&src_dir)?;
    println!("  {} {} lines", "Total:".dimmed(), total_lines);

    // 9. Store metadata
    let metadata = VersionMetadata {
        id: version.to_string(),
        protocol: 0,
        jar_sha256: hash,
        class_count: class_count,
        total_lines,
    };
    store.add_version(&metadata)?;

    // 10. Index classes in SQLite
    println!("  {}", "Indexing classes...".dimmed());
    let pb = ProgressBar::new(classes.len() as u64);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} {msg}")
            .unwrap()
            .progress_chars("=>-"),
    );
    for class_name in &classes {
        let relative_path = format!("{}.java", class_name.replace('.', "/"));
        if src_dir.join(&relative_path).exists() {
            let info = mc_decompiler_core::ClassInfo::from_path(&src_dir, &relative_path)?;
            store.add_class(version, &info)?;
        }
        pb.inc(1);
    }
    pb.finish_with_message("done");

    println!(
        "\n{} {}",
        "[OK] Decompiled version:".bold().green(),
        version
    );
    println!("  {} {}", "Classes:".dimmed(), class_count);
    println!("  {} {} lines", "Source:".dimmed(), total_lines);
    println!(
        "  {} {}",
        "Location:".dimmed(),
        store.version_dir(version).display()
    );
    Ok(())
}

fn count_java_lines(dir: &Path) -> Result<u32> {
    let mut total: u32 = 0;
    if !dir.exists() {
        return Ok(0);
    }
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            total = total.saturating_add(count_java_lines(&path)?);
        } else if path
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("java"))
        {
            let lines = std::fs::read_to_string(&path)?.lines().count();
            total = total.saturating_add(u32::try_from(lines).unwrap_or(u32::MAX));
        }
    }
    Ok(total)
}
