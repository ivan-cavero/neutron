//! Java-map tripwire: every writer→Java mapping must point at a REAL file in
//! the decompiled sources. When Mojang renames/refactors a feature class, the
//! NEXT decompile makes this fail with the exact stale entry — the map can
//! never rot silently (the user-facing guarantee: attribution always names
//! code that exists).

use neutron_worldgen::writers::WRITERS;
use std::path::PathBuf;

fn decompile_roots() -> Vec<PathBuf> {
    let base = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("repo root")
        .join("tools/mc-decompiler/output");
    let mut out = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&base) {
        for e in entries.flatten() {
            let src = e.path().join("src");
            if src.is_dir() {
                out.push(src);
            }
        }
    }
    out.sort();
    out
}

#[test]
fn writer_java_paths_exist_in_decompile() {
    let roots = decompile_roots();
    if roots.is_empty() {
        eprintln!("skip: no decompile tree under tools/mc-decompiler/output/");
        return;
    }
    // Validate against EVERY available version: an entry missing from any
    // one of them is either a rename (update map) or a 26.x-specific class
    // (annotate it).
    let mut failures = Vec::new();
    for &(id, name, java) in WRITERS {
        if java.is_empty() {
            continue; // internal mechanics (terrain/mask)
        }
        for root in &roots {
            if !root.join(java).is_file() {
                failures.push(format!(
                    "writer {id} ({name}): {} missing in {}",
                    java,
                    root.display()
                ));
            }
        }
    }
    assert!(
        failures.is_empty(),
        "writer->Java map rotted (Mojang rename? update writers.rs):\n{}",
        failures.join("\n")
    );
}

#[test]
fn attribution_ids_reachable_from_dispatch_and_drivers() {
    // The ids drivers stamp must be table members with names.
    for id in [
        neutron_worldgen::writers::TERRAIN,
        neutron_worldgen::writers::MASK,
        neutron_worldgen::writers::SCULK_PATCH,
        neutron_worldgen::writers::CARVER,
        neutron_worldgen::writers::MINESHAFT,
        neutron_worldgen::writers::ORE,
        neutron_worldgen::writers::DISK,
        neutron_worldgen::writers::UNDERWATER_MAGMA,
    ] {
        assert!(
            WRITERS.iter().any(|&(i, _, _)| i == id),
            "driver id {id} absent from WRITERS"
        );
    }
}
