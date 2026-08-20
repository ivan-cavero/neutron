//! Diagnostic: which step each whitelisted configured feature is at.
use neutron_worldgen::feature_catalog::{self, load_configured_feature, load_placed_feature, step};
use std::collections::{BTreeSet, HashSet};

fn collect(placed_id: &str, out: &mut Vec<String>, seen: &mut HashSet<String>) {
    if let Some(cfg) = load_configured_feature(placed_id) {
        out.push(placed_id.to_string());
        // selector children
        if let Some(arr) = cfg["config"]["features"].as_array() {
            for f in arr {
                if let Some(ff) = f["feature"].as_str() {
                    collect(ff, out, seen);
                } else if let Some(ff) = f["feature"]["feature"].as_str() {
                    collect(ff, out, seen);
                }
            }
        }
        if let Some(def) = cfg["config"].get("default") {
            if let Some(ff) = def.as_str() {
                collect(ff, out, seen);
            }
        }
        if let Some(arr) = cfg["config"].get("features").and_then(|v| v.as_array()) {
            for f in arr {
                if let Some(ff) = f.as_str() {
                    collect(ff, out, seen);
                }
            }
        }
        return;
    }
    if let Some(p) = load_placed_feature(placed_id) {
        if seen.insert(placed_id.to_string()) {
            if let Some(fid) = p["feature"].as_str() {
                collect(fid, out, seen);
            } else if p["feature"].is_object() {
                out.push(placed_id.to_string());
            }
        }
    }
}

fn main() {
    let targets = [
        "desert_well",
        "ice_patch",
        "ore_infested",
        "dripstone_cluster",
        "sulfur_spike_cluster",
        "fossil_coal",
        "fossil_diamonds",
        "freeze_top_layer",
        "ice_spike",
        "iceberg_blue",
        "iceberg_packed",
        "lake_lava",
        "large_dripstone",
        "monster_room",
        "sulfur_pool",
        "bamboo_no_podzol",
        "bamboo_some_podzol",
        "amethyst_geode",
        "glow_lichen",
    ];
    let mut found: BTreeSet<(i32, String, String)> = BTreeSet::new();
    for s in step::RAW_GENERATION..=step::TOP_LAYER_MODIFICATION {
        for placed_id in feature_catalog::features_per_step_at(s) {
            let mut out = Vec::new();
            let mut seen = HashSet::new();
            collect(placed_id, &mut out, &mut seen);
            for cfg in out {
                let short = cfg.trim_start_matches("minecraft:");
                if targets.contains(&short) {
                    found.insert((s, placed_id.clone(), short.to_string()));
                }
            }
        }
    }
    for (s, placed, cfg) in found {
        println!("step={s} placed={placed} configured={cfg}");
    }
}
