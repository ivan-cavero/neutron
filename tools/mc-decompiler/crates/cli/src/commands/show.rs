use colored::Colorize;
use mc_decompiler_store::Store;

pub fn run(version: &str, class_fqn: &str) -> anyhow::Result<()> {
    let store = Store::open("output")?;
    let classes = store.get_classes(version)?;
    let class = classes.iter().find(|c| c.fqn == class_fqn);

    let class = match class {
        Some(c) => c,
        None => {
            println!(
                "\n{} '{}' not found in {}\n",
                "!".red().bold(),
                class_fqn,
                version
            );
            return Ok(());
        }
    };

    let src_path = store.src_dir(version).join(&class.source_path);
    match std::fs::read_to_string(&src_path) {
        Ok(content) => {
            println!("\n{} {} ({})", "Class:".bold().green(), class.fqn, version);
            println!("  {} {} lines\n", "Size:".dimmed(), class.line_count);
            println!("{}", "-".repeat(60));
            for (i, line) in content.lines().enumerate() {
                println!("{:>4} | {}", i + 1, line);
            }
            println!("{}", "-".repeat(60));
        }
        Err(e) => {
            println!(
                "\n{} Failed to read {}: {}\n",
                "!".red().bold(),
                src_path.display(),
                e
            );
        }
    }
    Ok(())
}
