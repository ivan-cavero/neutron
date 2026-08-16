use colored::Colorize;

pub fn run() -> anyhow::Result<()> {
    println!("\n{}\n", "mc-decompiler setup".bold().green());

    print!("  Checking Java... ");
    match check_java() {
        Ok(path) => println!("{} {}", "+".green().bold(), path),
        Err(e) => println!("{} {}", "!".red().bold(), e),
    }

    print!("  Checking Vineflower... ");
    match find_vineflower() {
        Ok(path) => println!("{} {}", "+".green().bold(), path),
        Err(e) => println!("{} {}", "!".red().bold(), e),
    }

    print!("  Checking output directory... ");
    let output_dir = std::path::Path::new("output");
    if output_dir.exists() {
        println!("{} {}", "+".green().bold(), output_dir.display());
    } else {
        std::fs::create_dir_all(output_dir)?;
        println!("{} {} (created)", "+".green().bold(), output_dir.display());
    }

    println!("\n{}", "Setup complete!".bold().green());
    Ok(())
}

fn check_java() -> Result<String, String> {
    if let Ok(java_home) = std::env::var("JAVA_HOME") {
        let java_path = if cfg!(target_os = "windows") {
            format!("{java_home}/bin/java.exe")
        } else {
            format!("{java_home}/bin/java")
        };
        if std::path::Path::new(&java_path).exists() {
            return Ok(java_path);
        }
    }
    let output = std::process::Command::new(if cfg!(target_os = "windows") {
        "where"
    } else {
        "which"
    })
    .arg("java")
    .output()
    .map_err(|e| format!("Failed: {e}"))?;
    if output.status.success() {
        let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
        return Ok(path.lines().next().unwrap_or(&path).to_string());
    }
    Err("Java not found. Set JAVA_HOME or install Java".to_string())
}

fn find_vineflower() -> Result<String, String> {
    let cwd = std::env::current_dir().unwrap_or_default();
    let possible = [
        "vendor/vineflower.jar",
        "../vendor/vineflower.jar",
        "../../vendor/vineflower.jar",
        "tools/mc-decompiler/vendor/vineflower.jar",
    ];
    for path in &possible {
        let full = cwd.join(path);
        if full.exists() {
            return Ok(full.to_string_lossy().to_string());
        }
    }
    Err("Vineflower not found in vendor/".to_string())
}
