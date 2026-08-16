use std::path::{Path, PathBuf};

use anyhow::Result;
use sha2::{Digest, Sha256};

/// Resolve the actual server JAR from a Mojang bundler.
/// Mojang JARs contain the real server at `META-INF/versions/<ver>/server-<ver>.jar`.
pub fn resolve_server_jar(jar_path: &Path) -> Result<PathBuf> {
    let file = std::fs::File::open(jar_path)?;
    let mut archive = zip::ZipArchive::new(file)?;

    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)?;
        let name = entry.name().to_string();
        if name.starts_with("META-INF/versions/")
            && name.ends_with(".jar")
            && name.contains("server")
        {
            let out_dir = jar_path
                .parent()
                .unwrap_or(Path::new("."))
                .join(".extracted");
            std::fs::create_dir_all(&out_dir)?;
            let out_path = out_dir.join("server.jar");
            let mut out_file = std::fs::File::create(&out_path)?;
            std::io::copy(&mut entry, &mut out_file)?;
            return Ok(out_path);
        }
    }

    Ok(jar_path.to_path_buf())
}

pub fn list_classes(jar_path: &Path) -> Result<Vec<String>> {
    let file = std::fs::File::open(jar_path)?;
    let mut archive = zip::ZipArchive::new(file)?;
    let mut classes = Vec::new();
    for i in 0..archive.len() {
        let entry = archive.by_index(i)?;
        let name = entry.name().to_string();
        if name.ends_with(".class") && !name.contains("META-INF") {
            classes.push(name.trim_end_matches(".class").replace('/', "."));
        }
    }
    classes.sort();
    Ok(classes)
}

pub fn extract_classes(jar_path: &Path, output_dir: &Path) -> Result<u32> {
    let file = std::fs::File::open(jar_path)?;
    let mut archive = zip::ZipArchive::new(file)?;
    let mut count = 0;
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)?;
        let name = entry.name().to_string();
        if name.ends_with(".class") && !name.contains("META-INF") {
            let out_path = output_dir.join(&name);
            if let Some(parent) = out_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let mut out_file = std::fs::File::create(&out_path)?;
            std::io::copy(&mut entry, &mut out_file)?;
            count += 1;
        }
    }
    Ok(count)
}

pub fn compute_sha256(path: &Path) -> Result<String> {
    let content = std::fs::read(path)?;
    let mut hasher = Sha256::new();
    hasher.update(&content);
    Ok(format!("{:x}", hasher.finalize()))
}
