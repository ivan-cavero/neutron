use colored::Colorize;
use mc_decompiler_store::Store;

pub fn run() -> anyhow::Result<()> {
    let store = Store::open("output")?;
    let versions = store.list_versions()?;

    if versions.is_empty() {
        println!("\n{}", "No versions decompiled yet.".yellow());
        println!("  Run: mc-decompiler download <version> && mc-decompiler decompile <version>");
        return Ok(());
    }

    println!("\n{}\n", "Decompiled versions:".bold().green());
    println!("  {:<12} {:>10} {:>12}", "Version", "Classes", "Lines");
    println!(
        "  {:<12} {:>10} {:>12}",
        "-".repeat(12),
        "-".repeat(10),
        "-".repeat(12)
    );

    for v in &versions {
        let lines = if v.total_lines >= 1_000_000 {
            format!("{:.1}M", v.total_lines as f64 / 1_000_000.0)
        } else if v.total_lines >= 1_000 {
            format!("{:.1}K", v.total_lines as f64 / 1_000.0)
        } else {
            v.total_lines.to_string()
        };
        println!("  {:<12} {:>10} {:>12}", v.id, v.class_count, lines);
    }
    println!();
    Ok(())
}
