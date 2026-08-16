use colored::Colorize;
use mc_decompiler_store::Store;
use similar::{ChangeTag, TextDiff};

pub fn run(from_version: &str, to_version: &str, class_filter: Option<&str>) -> anyhow::Result<()> {
    println!(
        "\n{} {} -> {}\n",
        "Comparing:".bold().green(),
        from_version,
        to_version
    );

    let store = Store::open("output")?;
    let from_classes = store.get_classes(from_version)?;
    let to_classes = store.get_classes(to_version)?;

    let from_map: std::collections::HashMap<_, _> =
        from_classes.iter().map(|c| (c.fqn.as_str(), c)).collect();
    let to_map: std::collections::HashMap<_, _> =
        to_classes.iter().map(|c| (c.fqn.as_str(), c)).collect();

    let mut added = Vec::new();
    let mut removed = Vec::new();
    let mut modified = Vec::new();

    for (fqn, to_class) in &to_map {
        if let Some(from_class) = from_map.get(fqn) {
            let from_path = store.src_dir(from_version).join(&from_class.source_path);
            let to_path = store.src_dir(to_version).join(&to_class.source_path);
            if let (Ok(fc), Ok(tc)) = (
                std::fs::read_to_string(&from_path),
                std::fs::read_to_string(&to_path),
            ) {
                if fc != tc {
                    modified.push(fqn.to_string());
                }
            }
        } else {
            added.push(fqn.to_string());
        }
    }
    for fqn in from_map.keys() {
        if !to_map.contains_key(fqn) {
            removed.push(fqn.to_string());
        }
    }

    if let Some(filter) = class_filter {
        added.retain(|f| f.contains(filter));
        removed.retain(|f| f.contains(filter));
        modified.retain(|f| f.contains(filter));
    }

    println!("  {} {} added", "+".green().bold(), added.len());
    println!("  {} {} removed", "-".red().bold(), removed.len());
    println!("  {} {} modified", "~".yellow().bold(), modified.len());

    for fqn in &modified {
        let from_class = from_map.get(fqn.as_str()).unwrap();
        let to_class = to_map.get(fqn.as_str()).unwrap();
        let from_path = store.src_dir(from_version).join(&from_class.source_path);
        let to_path = store.src_dir(to_version).join(&to_class.source_path);
        if let (Ok(fc), Ok(tc)) = (
            std::fs::read_to_string(&from_path),
            std::fs::read_to_string(&to_path),
        ) {
            println!("\n  {} {}", "*".yellow(), fqn);
            let diff = TextDiff::from_lines(&fc, &tc);
            let mut n = 0;
            for change in diff.iter_all_changes() {
                let sign = match change.tag() {
                    ChangeTag::Delete => {
                        n += 1;
                        "-".red()
                    }
                    ChangeTag::Insert => {
                        n += 1;
                        "+".green()
                    }
                    ChangeTag::Equal => continue,
                };
                if n <= 10 {
                    print!("  {} {}", sign, change);
                }
            }
            if n > 10 {
                println!("  ... and {n} more");
            }
        }
    }
    println!();
    Ok(())
}
