// Copyright (c) 2026 Neutron Contributors -- MIT License
//
// Load vanilla worldgen JSON from the crate's data tree (synced by
// tools/vanilla-extract/extract-worldgen.ps1). Prefer this for biomes/features
// so a datapack update is a re-run of the extract script, not a hand-edit.
//
// Paths are relative to `src/data/worldgen/` or `src/data/tags/`.

use std::path::{Path, PathBuf};

/// Resolve `crates/neutron-worldgen/src/data/...`.
fn data_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src").join("data")
}

/// Read `worldgen/<rel>` JSON (e.g. `biome/deep_dark.json`).
pub fn worldgen_json(rel: &str) -> Option<String> {
    let path = data_root().join("worldgen").join(rel);
    std::fs::read_to_string(path).ok()
}

/// Read `tags/<rel>` JSON (e.g. `block/sculk_replaceable_world_gen.json`).
pub fn tags_json(rel: &str) -> Option<String> {
    let path = data_root().join("tags").join(rel);
    std::fs::read_to_string(path).ok()
}

/// Absolute path helper for tools / diagnostics.
pub fn worldgen_path(rel: &str) -> PathBuf {
    data_root().join("worldgen").join(rel)
}

/// Fallback: also try tools/vanilla-extract server-classes (dev machine).
pub fn worldgen_json_with_fallback(rel: &str) -> Option<String> {
    if let Some(s) = worldgen_json(rel) {
        return Some(s);
    }
    let alt = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("tools")
        .join("vanilla-extract")
        .join("server-classes")
        .join("data")
        .join("minecraft")
        .join("worldgen")
        .join(rel);
    std::fs::read_to_string(alt).ok()
}
