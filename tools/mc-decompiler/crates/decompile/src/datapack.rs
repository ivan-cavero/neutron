//! Worldgen datapack extraction + diff (the "version data detection" pipeline).
//!
//! Extracts `data/minecraft/worldgen/**/*.json` from a server JAR (bundler or
//! plain) into a normalized tree, then semantically diffs it against a target
//! tree (typically `crates/neutron-worldgen/src/data/worldgen`).
//!
//! This is the data-side counterpart of the code `diff` command: Mojang worldgen
//! changes land mostly as datapack JSON, so a version bump must be detected here
//! (added/removed/changed features, biomes, noise settings), not only in `.java`
//! sources.

use std::collections::BTreeMap;
use std::fs;
use std::io::{Cursor, Read};
use std::path::Path;

use anyhow::{Context, Result};
use zip::ZipArchive;

/// Open the real server JAR (resolving the Mojang bundler wrapper if present)
/// fully in memory — no `.extracted/` litter on disk.
fn open_server_jar(jar_path: &Path) -> Result<ZipArchive<Cursor<Vec<u8>>>> {
    let file = fs::File::open(jar_path).with_context(|| format!("open {}", jar_path.display()))?;
    let mut outer = ZipArchive::new(file)?;
    for i in 0..outer.len() {
        let name = outer.by_index(i)?.name().to_string();
        if name.starts_with("META-INF/versions/") && name.ends_with(".jar") {
            let mut entry = outer.by_index(i)?;
            let mut buf = Vec::with_capacity(entry.size() as usize);
            entry.read_to_end(&mut buf)?;
            return ZipArchive::new(Cursor::new(buf)).context("unzip inner server JAR");
        }
    }
    // Not a bundler: treat the JAR itself as the server JAR.
    let mut file = fs::File::open(jar_path)?;
    let mut buf = Vec::new();
    file.read_to_end(&mut buf)?;
    ZipArchive::new(Cursor::new(buf)).context("plain server JAR")
}

/// Result of [`extract_worldgen`].
#[derive(Debug, Default)]
pub struct ExtractSummary {
    /// Number of JSON files written under `out/worldgen/`.
    pub files: usize,
    /// Total uncompressed bytes read from the JAR.
    pub bytes: usize,
}

/// Result of [`diff_worldgen`].
#[derive(Debug, Default)]
pub struct DiffSummary {
    /// Files present in both trees with semantically identical JSON.
    pub matched: usize,
    /// Files present in both trees that differ (semantic).
    pub changed: Vec<String>,
    /// Files only in the extracted JAR tree (not ported into the target).
    pub jar_only: Vec<String>,
    /// Files only in the target tree (removed from the JAR?).
    pub crate_only: Vec<String>,
}

/// Extract `data/minecraft/worldgen/**` JSON entries from `jar_path` into
/// `out_dir/worldgen/`, canonicalizing each file (pretty-printed, trailing
/// newline) so diffs are readable and stable.
pub fn extract_worldgen(jar_path: &Path, out_dir: &Path) -> Result<ExtractSummary> {
    let mut jar = open_server_jar(jar_path)?;
    const PREFIX: &str = "data/minecraft/worldgen/";
    let mut summary = ExtractSummary::default();

    for i in 0..jar.len() {
        let mut entry = jar.by_index(i)?;
        let name = entry.name().to_string();
        if !name.starts_with(PREFIX) || !name.ends_with(".json") {
            continue;
        }
        let rel = &name[PREFIX.len()..];
        let mut content = String::new();
        entry
            .read_to_string(&mut content)
            .with_context(|| format!("read {name} from JAR"))?;
        let value: serde_json::Value = serde_json::from_str(&content)
            .with_context(|| format!("parse {name} (invalid JSON in JAR)"))?;
        let canonical = format!("{}\n", serde_json::to_string_pretty(&value)?);

        let dest = out_dir.join("worldgen").join(rel);
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&dest, canonical)?;
        summary.files += 1;
        summary.bytes += content.len();
    }
    Ok(summary)
}

/// Recursively collect every `*.json` file under `dir`, keyed by its path
/// relative to `dir` (forward slashes).
fn collect_json_tree(dir: &Path) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(current) = stack.pop() {
        let Ok(entries) = fs::read_dir(&current) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().and_then(|e| e.to_str()) == Some("json") {
                let rel = path
                    .strip_prefix(dir)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .replace('\\', "/");
                if let Ok(text) = fs::read_to_string(&path) {
                    out.insert(rel, text);
                }
            }
        }
    }
    out
}

/// Semantically diff two worldgen trees: files are equal when their parsed JSON
/// is equal (formatting-independent).
pub fn diff_worldgen(extracted: &Path, target: &Path) -> Result<DiffSummary> {
    let extracted_tree = collect_json_tree(extracted);
    let target_tree = collect_json_tree(target);
    let mut summary = DiffSummary::default();

    let mut seen = BTreeMap::new();
    for rel in extracted_tree.keys().chain(target_tree.keys()) {
        seen.entry(rel.clone()).or_insert(true);
    }

    for rel in seen.keys() {
        match (extracted_tree.get(rel), target_tree.get(rel)) {
            (Some(a), Some(b)) => {
                let va: serde_json::Value = serde_json::from_str(a).unwrap_or_default();
                let vb: serde_json::Value = serde_json::from_str(b).unwrap_or_default();
                if va == vb {
                    summary.matched += 1;
                } else {
                    summary.changed.push(rel.clone());
                }
            }
            (Some(_), None) => summary.jar_only.push(rel.clone()),
            (None, Some(_)) => summary.crate_only.push(rel.clone()),
            (None, None) => unreachable!(),
        }
    }
    Ok(summary)
}

/// Render a [`DiffSummary`] as the standard report block (stable format for
/// run-file evidence).
pub fn render_diff_summary(summary: &DiffSummary) -> String {
    let mut lines = Vec::new();
    lines.push(format!("MATCH      {}", summary.matched));
    lines.push(format!("CHANGED    {}  {}", summary.changed.len(), summary.changed.join(", ")));
    lines.push(format!("JAR-ONLY   {}  {}", summary.jar_only.len(), summary.jar_only.join(", ")));
    lines.push(format!("CRATE-ONLY {}  {}", summary.crate_only.len(), summary.crate_only.join(", ")));
    lines.join("\n")
}

/// Convenience for the CLI: extract then diff, printing the report to stdout.
pub fn report(
    jar_path: &Path,
    out_dir: &Path,
    target: Option<&Path>,
) -> Result<(ExtractSummary, Option<DiffSummary>)> {
    let extract = extract_worldgen(jar_path, out_dir)?;
    let diff = match target {
        Some(target) => Some(diff_worldgen(&out_dir.join("worldgen"), target)?),
        None => None,
    };
    Ok((extract, diff))
}