use std::path::{Path, PathBuf};

use anyhow::Result;
use colored::Colorize;

const VERSION_MANIFEST_V2: &str =
    "https://launchermeta.mojang.com/mc/game/version_manifest_v2.json";

#[derive(serde::Deserialize)]
struct VersionManifestV2 {
    versions: Vec<VersionEntry>,
}

#[derive(serde::Deserialize)]
struct VersionEntry {
    id: String,
    url: String,
}

/// Get the download URL for a specific version by fetching the Mojang manifest.
pub fn get_server_url(version: &str) -> Result<String> {
    let resp: VersionManifestV2 = ureq::get(VERSION_MANIFEST_V2).call()?.into_json()?;

    let entry = resp
        .versions
        .iter()
        .find(|v| v.id == version)
        .ok_or_else(|| anyhow::anyhow!("Version '{version}' not found in Mojang manifest"))?;

    let version_json: serde_json::Value = ureq::get(&entry.url).call()?.into_json()?;

    let server_url = version_json["downloads"]["server"]["url"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("Server download not available for {version}"))?;

    Ok(server_url.to_string())
}

/// Download a server JAR from Mojang.
pub fn download_server(version: &str, output_dir: &Path) -> Result<PathBuf> {
    let url = get_server_url(version)?;
    let out_path = output_dir.join(format!("server-{version}.jar"));

    if out_path.exists() {
        println!(
            "  {} {}",
            "Already downloaded:".dimmed(),
            out_path.display()
        );
        return Ok(out_path);
    }

    std::fs::create_dir_all(output_dir)?;
    println!("  {} {url}", "Downloading:".dimmed());

    let response = ureq::get(&url).call()?;
    let mut reader = response.into_reader();
    let mut file = std::fs::File::create(&out_path)?;
    std::io::copy(&mut reader, &mut file)?;

    println!("  {} {}", "Saved:".dimmed(), out_path.display());
    Ok(out_path)
}

/// List all available versions from Mojang.
pub fn list_available_versions() -> Result<Vec<(String, String)>> {
    let resp: VersionManifestV2 = ureq::get(VERSION_MANIFEST_V2).call()?.into_json()?;

    let mut versions: Vec<(String, String)> = resp
        .versions
        .iter()
        .map(|v| (v.id.clone(), String::new()))
        .collect();
    versions.sort_by(|a, b| b.0.cmp(&a.0));
    Ok(versions)
}
