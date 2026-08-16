use std::path::Path;

use colored::Colorize;

use mc_decompiler_decompile::mojang;

pub fn run(version: &str, output: Option<&Path>) -> anyhow::Result<()> {
    println!("\n{} {}", "Downloading version:".bold().green(), version);

    let output_dir = output.unwrap_or(Path::new("jars"));
    let jar_path = mojang::download_server(version, output_dir)?;

    println!(
        "\n{} {}",
        "[OK] Downloaded:".bold().green(),
        jar_path.display()
    );
    println!(
        "  {} mc-decompiler decompile {version} --jar {}",
        "Next:".dimmed(),
        jar_path.display()
    );
    Ok(())
}
