use colored::Colorize;
use mc_decompiler_store::Store;

pub fn run(query: &str, version: Option<&str>) -> anyhow::Result<()> {
    let store = Store::open("output")?;
    let search_version = match version {
        Some(v) => v.to_string(),
        None => {
            let versions = store.list_versions()?;
            versions.last().map(|v| v.id.clone()).unwrap_or_else(|| {
                println!("\n{}", "No versions decompiled.".yellow());
                std::process::exit(1);
            })
        }
    };

    println!(
        "\n{} '{}' in {}\n",
        "Searching for:".bold().green(),
        query,
        search_version
    );
    let classes = store.search_classes(&search_version, query)?;

    if classes.is_empty() {
        println!("  {}", "No classes found.".yellow());
        return Ok(());
    }

    println!("  {} classes found:\n", classes.len());
    for class in &classes {
        println!("  {} {}", "*".cyan(), class.fqn);
        println!(
            "    {} {} lines, {} methods",
            "Size:".dimmed(),
            class.line_count,
            class.method_count
        );
    }
    println!();
    Ok(())
}
