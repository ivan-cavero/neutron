//! Server provisioning: download / list / status for the multi-version layout
//! `servers/<type>/<version>/<binary>` under the benchmarks workspace root.

use crate::types::ServerType;
use eyre::{Result, WrapErr};
use reqwest::Client;
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

const UA: &str = "neutron-bench/0.1.0 (https://github.com/ivan-cavero/neutron)";
const MANIFEST_URL: &str = "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";
/// PaperMC's legacy v2 API (`api.papermc.io/v2`) was sunset; the downloads
/// service now lives at fill.papermc.io/v3 (see docs.papermc.io/misc/downloads-service/).
const PAPER_API: &str = "https://fill.papermc.io/v3/projects";
const PUMPKIN_RELEASES: &str = "https://api.github.com/repos/Pumpkin-MC/Pumpkin/releases";

/// Local offline jar cache. Mirrors the managed layout: `<dir>/<type>/<version>/<binary>`.
fn fallback_dir() -> Option<PathBuf> {
    std::env::var_os("NEUTRON_BENCH_SERVERS_FALLBACK").map(PathBuf::from)
}

pub fn servers_dir() -> PathBuf {
    crate::ws_root().join("servers")
}

pub fn dest_path(server_type: ServerType, version: &str) -> PathBuf {
    servers_dir()
        .join(server_type.label())
        .join(version)
        .join(server_type.binary_name())
}

fn http_client() -> Result<Client> {
    Ok(Client::builder()
        .user_agent(UA)
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(600)) // 60 MB jars on slow links; bounded so we never hang
        .build()?)
}

/// Download `servers/<type>/<version>/<binary>`. When offline (or the network
/// fails), falls back to the local cache dir if the jar already exists there.
pub async fn download(server_type: ServerType, version: &str, offline: bool) -> Result<()> {
    let dest = dest_path(server_type, version);
    if dest.exists() {
        println!("Already present: {} — nothing to do.", dest.display());
        return Ok(());
    }

    if offline {
        return copy_from_fallback(server_type, version, &dest);
    }

    let client = http_client()?;
    match fetch_to(&client, server_type, version, &dest).await {
        Ok(()) => Ok(()),
        Err(net_err) => {
            eprintln!("Network download failed: {net_err:#}");
            copy_from_fallback(server_type, version, &dest).wrap_err_with(|| {
                format!(
                    "network download failed ({net_err:#}) and no usable local fallback jar"
                )
            })
        }
    }
}

async fn fetch_to(
    client: &Client,
    server_type: ServerType,
    version: &str,
    dest: &Path,
) -> Result<()> {
    let resolved = resolve_url(client, server_type, version).await?;
    println!(
        "Downloading {} {}...\n  URL: {}\n  size: {} bytes{}",
        server_type.label(),
        version,
        resolved.url,
        resolved.size.unwrap_or(0),
        resolved
            .sha
            .as_ref()
            .map(|s| format!(", sha: {s}"))
            .unwrap_or_default()
    );

    let bytes = client
        .get(&resolved.url)
        .send()
        .await
        .wrap_err("download request failed")?
        .error_for_status()
        .wrap_err("download returned an error status")?
        .bytes()
        .await
        .wrap_err("reading download body failed")?;

    if let Some(expected) = resolved.size {
        if bytes.len() as u64 != expected {
            eyre::bail!(
                "size mismatch: got {} bytes, expected {} (download truncated?)",
                bytes.len(),
                expected
            );
        }
    }
    verify_binary(server_type, &bytes)?;

    let parent = dest.parent().ok_or_else(|| eyre::anyhow!("no parent dir for {}", dest.display()))?;
    fs::create_dir_all(parent)?;
    fs::write(dest, &bytes)?;
    println!(
        "Saved {} ({} bytes, verified{})",
        dest.display(),
        bytes.len(),
        if server_type.is_java() { " zip/jar" } else { "" }
    );
    Ok(())
}

struct Resolved {
    url: String,
    size: Option<u64>,
    sha: Option<String>,
}

async fn resolve_url(client: &Client, server_type: ServerType, version: &str) -> Result<Resolved> {
    match server_type {
        ServerType::Vanilla => resolve_vanilla(client, version).await,
        ServerType::Paper | ServerType::Folia => resolve_paper(client, server_type, version).await,
        ServerType::Pumpkin => resolve_pumpkin(client).await,
    }
}

// --- Mojang (vanilla) ---

#[derive(Deserialize)]
struct Manifest {
    versions: Vec<ManifestVersion>,
}
#[derive(Deserialize)]
struct ManifestVersion {
    id: String,
    url: String,
}
#[derive(Deserialize)]
struct VersionJson {
    downloads: VersionDownloads,
}
#[derive(Deserialize)]
struct VersionDownloads {
    server: ServerDownload,
}
#[derive(Deserialize)]
struct ServerDownload {
    url: String,
    sha1: String,
    size: u64,
}

async fn resolve_vanilla(client: &Client, version: &str) -> Result<Resolved> {
    let manifest: Manifest = client
        .get(MANIFEST_URL)
        .send()
        .await
        .wrap_err("fetching Mojang version manifest failed")?
        .error_for_status()?
        .json()
        .await?;
    let v = manifest
        .versions
        .iter()
        .find(|v| v.id == version)
        .ok_or_else(|| {
            eyre::anyhow!(
                "version '{version}' not found in the Mojang version manifest \
                 (https://piston-meta.mojang.com/mc/game/version_manifest_v2.json)"
            )
        })?;
    let vj: VersionJson = client
        .get(&v.url)
        .send()
        .await
        .wrap_err("fetching Mojang version metadata failed")?
        .error_for_status()?
        .json()
        .await?;
    Ok(Resolved {
        url: vj.downloads.server.url,
        size: Some(vj.downloads.server.size),
        sha: Some(vj.downloads.server.sha1),
    })
}

// --- PaperMC (paper / folia) ---

#[derive(Deserialize)]
struct PaperBuild {
    id: u64,
    channel: String,
    downloads: HashMap<String, PaperDownload>,
}
#[derive(Deserialize)]
struct PaperDownload {
    url: String,
    size: u64,
    #[serde(default)]
    checksums: PaperChecksums,
}
#[derive(Deserialize, Default)]
struct PaperChecksums {
    #[serde(default)]
    sha256: Option<String>,
}

async fn resolve_paper(client: &Client, server_type: ServerType, version: &str) -> Result<Resolved> {
    let project = match server_type {
        ServerType::Paper => "paper",
        ServerType::Folia => "folia",
        _ => unreachable!(),
    };
    let endpoint = format!("{PAPER_API}/{project}/versions/{version}/builds");
    let builds: Vec<PaperBuild> = client
        .get(&endpoint)
        .send()
        .await
        .wrap_err_with(|| format!("fetching {project} builds failed: {endpoint}"))?
        .error_for_status()?
        .json()
        .await?;
    let build = builds
        .iter()
        .find(|b| b.channel == "STABLE")
        .or_else(|| builds.first())
        .ok_or_else(|| eyre::anyhow!("no builds available for {project} {version}"))?;
    if build.channel != "STABLE" {
        println!(
            "  note: no STABLE build for {project} {version}; using {} build {}",
            build.channel, build.id
        );
    }
    let dl = build
        .downloads
        .get("server:default")
        .ok_or_else(|| eyre::anyhow!("no server:default download for {project} {version}"))?;
    Ok(Resolved {
        url: dl.url.clone(),
        size: Some(dl.size),
        sha: dl.checksums.sha256.clone(),
    })
}

// --- Pumpkin (GitHub releases) ---

#[derive(Deserialize)]
struct GhRelease {
    tag_name: String,
    assets: Vec<GhAsset>,
}
#[derive(Deserialize)]
struct GhAsset {
    name: String,
    size: u64,
    browser_download_url: String,
}

async fn resolve_pumpkin(client: &Client) -> Result<Resolved> {
    let releases: Vec<GhRelease> = client
        .get(PUMPKIN_RELEASES)
        .query(&[("per_page", "5")])
        .send()
        .await
        .wrap_err("fetching Pumpkin releases failed")?
        .error_for_status()?
        .json()
        .await?;
    let hit = releases
        .iter()
        .flat_map(|r| r.assets.iter().map(move |a| (r, a)))
        .find(|(_, a)| platform_matches(&a.name));
    match hit {
        Some((release, asset)) => {
            println!(
                "  pumpkin nightly {}: {}",
                release.tag_name, asset.name
            );
            Ok(Resolved {
                url: asset.browser_download_url.clone(),
                size: Some(asset.size),
                sha: None,
            })
        }
        None => eyre::bail!(
            "Pumpkin publishes no release binaries (the Pumpkin-MC/Pumpkin repo has no \
             releases; nightly binaries are GitHub Actions artifacts that require auth). \
             Build from source (https://github.com/Pumpkin-MC/Pumpkin) and place the \
             binary at servers/pumpkin/<version>/{}",
            if cfg!(target_os = "windows") { "pumpkin.exe" } else { "pumpkin" }
        ),
    }
}

fn platform_matches(name: &str) -> bool {
    #[cfg(target_os = "windows")]
    {
        name.contains("Windows")
    }
    #[cfg(target_os = "linux")]
    {
        name.contains("Linux")
    }
    #[cfg(target_os = "macos")]
    {
        name.contains("macOS") || name.contains("Darwin")
    }
}

// --- verification ---

fn verify_binary(server_type: ServerType, bytes: &[u8]) -> Result<()> {
    if server_type.is_java() {
        if bytes.len() < 4 || &bytes[..4] != b"PK\x03\x04" {
            eyre::bail!("downloaded file is not a valid zip/jar (missing PK magic bytes)");
        }
    } else if bytes.is_empty() {
        eyre::bail!("downloaded file is empty");
    }
    Ok(())
}

fn copy_from_fallback(server_type: ServerType, version: &str, dest: &Path) -> Result<()> {
    let fb = fallback_dir().ok_or_else(|| {
        eyre::anyhow!(
            "NEUTRON_BENCH_SERVERS_FALLBACK is not set; set it to a local jar cache \
             (layout: <dir>/<type>/<version>/<binary>) to provision offline"
        )
    })?;
    let src = fb
        .join(server_type.label())
        .join(version)
        .join(server_type.binary_name());
    if !src.exists() {
        eyre::bail!(
            "local fallback jar not found: {} (expected layout: <fallback>/<type>/<version>/<binary>)",
            src.display()
        );
    }
    let parent = dest
        .parent()
        .ok_or_else(|| eyre::anyhow!("no parent dir for {}", dest.display()))?;
    fs::create_dir_all(parent)?;
    fs::copy(&src, dest)?;
    println!(
        "Copied from local fallback: {} -> {}",
        src.display(),
        dest.display()
    );
    Ok(())
}

// --- list / status ---

struct Entry {
    server: ServerType,
    version: Option<String>,
    path: PathBuf,
}

fn scan() -> Vec<Entry> {
    let base = servers_dir();
    let mut out = Vec::new();
    for st in [
        ServerType::Vanilla,
        ServerType::Paper,
        ServerType::Folia,
        ServerType::Pumpkin,
    ] {
        let dir = base.join(st.label());
        if let Ok(rd) = fs::read_dir(&dir) {
            let mut versions: Vec<String> = rd
                .filter_map(|e| e.ok())
                .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .collect();
            versions.sort();
            for v in versions {
                out.push(Entry {
                    server: st,
                    version: Some(v.clone()),
                    path: dir.join(&v).join(st.binary_name()),
                });
            }
        }
        // Legacy single-jar layout: servers/<type>/<binary>
        let legacy = dir.join(st.binary_name());
        if legacy.exists() {
            out.push(Entry {
                server: st,
                version: None,
                path: legacy,
            });
        }
    }
    out
}

pub fn list() -> Result<()> {
    let entries = scan();
    if entries.is_empty() {
        println!("No server jars found under {}.", servers_dir().display());
        println!("Download with: neutron-bench servers download <type> <version>");
        return Ok(());
    }
    println!("{:<8} {:<10} path", "type", "version");
    for e in &entries {
        let v = e.version.as_deref().unwrap_or("(legacy)");
        println!("{:<8} {:<10} {}", e.server.label(), v, e.path.display());
    }
    Ok(())
}

pub fn status() -> Result<()> {
    let entries = scan();
    if entries.is_empty() {
        println!("No server jars found under {}.", servers_dir().display());
        println!("Download with: neutron-bench servers download <type> <version>");
        return Ok(());
    }
    for e in &entries {
        let v = e.version.as_deref().unwrap_or("(legacy)");
        if e.path.exists() {
            let size = fs::metadata(&e.path).map(|m| m.len()).unwrap_or(0);
            let valid = is_zip(&e.path);
            println!(
                "{:<8} {:<10} OK      ({:>8} bytes{})  {}",
                e.server.label(),
                v,
                size,
                if e.server.is_java() {
                    if valid { ", zip" } else { ", INVALID (not a zip)" }
                } else {
                    ""
                },
                e.path.display()
            );
        } else if let Some(ver) = &e.version {
            println!(
                "{:<8} {:<10} MISSING — run: neutron-bench servers download {} {}",
                e.server.label(),
                ver,
                e.server.label(),
                ver
            );
        }
    }
    Ok(())
}

fn is_zip(path: &Path) -> bool {
    fs::read(path)
        .ok()
        .map(|b| b.len() >= 4 && &b[..4] == b"PK\x03\x04")
        .unwrap_or(false)
}