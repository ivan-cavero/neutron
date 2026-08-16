use std::path::{Path, PathBuf};

pub fn version_dir(base: &Path, version: &str) -> PathBuf {
    base.join("versions").join(version)
}

pub fn src_dir(base: &Path, version: &str) -> PathBuf {
    version_dir(base, version).join("src")
}

pub fn classes_dir(base: &Path, version: &str) -> PathBuf {
    version_dir(base, version).join("classes")
}

pub fn create_version_dirs(base: &Path, version: &str) -> anyhow::Result<()> {
    for dir in &[
        version_dir(base, version),
        src_dir(base, version),
        classes_dir(base, version),
    ] {
        std::fs::create_dir_all(dir)?;
    }
    Ok(())
}

pub fn count_java_lines(dir: &Path) -> anyhow::Result<u32> {
    let mut total = 0;
    if !dir.exists() {
        return Ok(0);
    }
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            total += count_java_lines(&path)?;
        } else if path.extension().map_or(false, |ext| ext == "java") {
            total += std::fs::read_to_string(&path)?.lines().count() as u32;
        }
    }
    Ok(total)
}
