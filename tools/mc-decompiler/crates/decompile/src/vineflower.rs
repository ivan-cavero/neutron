use std::path::Path;
use std::process::Command;

use anyhow::Result;
use colored::Colorize;

pub fn decompile(classes_dir: &Path, output_dir: &Path, vineflower_jar: &Path) -> Result<()> {
    let java_path = find_java()?;
    println!("  {} {}", "Using Java:".dimmed(), java_path);
    println!("  {} {}", "Vineflower:".dimmed(), vineflower_jar.display());

    // Vineflower syntax: java -jar vineflower.jar --<option>=<value>... <source>... <destination>
    let output = Command::new(&java_path)
        .args([
            "-Xms512M",
            "-Xmx4G",
            "-jar",
            vineflower_jar.to_str().unwrap(),
            "--silent=true",
            "--renameillegalidents=true",
            // Source: the directory containing .class files
            classes_dir.to_str().unwrap(),
            // Destination: the output directory for .java files
            output_dir.to_str().unwrap(),
        ])
        .output()?;

    // Print stdout/stderr for debugging
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    if !stdout.is_empty() {
        println!("  Vineflower stdout: {stdout}");
    }
    if !stderr.is_empty() {
        println!("  Vineflower stderr: {stderr}");
    }

    if !output.status.success() {
        anyhow::bail!("Vineflower failed (exit code {:?})", output.status.code());
    }
    Ok(())
}

fn find_java() -> Result<String> {
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
    let output = Command::new(if cfg!(target_os = "windows") {
        "where"
    } else {
        "which"
    })
    .arg("java")
    .output()?;
    if output.status.success() {
        let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
        return Ok(path.lines().next().unwrap_or(&path).to_string());
    }
    anyhow::bail!("Java not found")
}
