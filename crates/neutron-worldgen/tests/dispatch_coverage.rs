//! Dispatch coverage: every configured_feature type reachable from the
//! overworld FeatureSorter must be handled by the generator — or explicitly
//! whitelisted as a *known* gap.
//!
//! Why this exists (D0-D4 detection, see runs/README.md "Detection rules"):
//! a Mojang version bump usually adds new configured_feature types, and an
//! unimplemented type currently falls through `dispatch_configured`'s `_ =>`
//! arm as a silent no-op. This test turns that silence into a red test at D2,
//! naming the exact feature and type to port. A whitelist entry is a *confessed
//! gap* (with reason), not a free pass: the whitelist must match the found set
//! exactly, so both new and removed types fail loudly.
//!
//! Classification scheme (verified against the generator source):
//! - `HANDLED`     — has a real dispatch arm in `feature_dispatch.rs`
//!   (`dispatch_configured`) or is placed with dedicated seeds elsewhere
//!   (`sculk_patch` → sculk module).
//! - `STEP6_BATCH` — the step-6 ore/disk/magma batch in `features.rs`
//!   (`apply_underground_ores_origin`). Gated on `step == UNDERGROUND_ORES`:
//!   a user of these types at any other step is an orphan.
//! - `KNOWN_NO_OP` — real vanilla features with NO implementation. Each entry
//!   names the type + configured id and carries the reason. `multiface_growth`
//!   is only implemented for `sculk_vein` (sculk module); every other user
//!   must be confessed here.

use std::collections::{BTreeSet, HashSet};

use serde_json::Value;

use neutron_worldgen::feature_catalog::{self, load_configured_feature, load_placed_feature, step};

/// Types with a dispatch arm in `feature_dispatch.rs` (or sculk module).
const HANDLED: &[&str] = &[
    "simple_block",
    "tree",
    "random_selector",
    "sculk_patch", // placed by the sculk module with dedicated seeds
    "vegetation_patch",
    "waterlogged_vegetation_patch",
    "spring_feature",
    "block_column",
    "simple_random_selector",
    "random_boolean_selector",
    "vines",
    "root_system",
    "sea_pickle",
    "seagrass",
    "kelp",
    "block_blob",
    "blue_ice",
    // run-058 T4 ports: every former KNOWN_NO_OP entry now has a dispatch arm
    // in feature_dispatch.rs (impl in feature_ports.rs / features.rs).
    "desert_well",
    "disk", // generic arm; step-6 disks still run via the features.rs batch
    "ore",
    "scattered_ore", // generic arm; step-6 ores still run via the batch
    "speleothem_cluster",
    "fossil",
    "freeze_top_layer",
    "spike",
    "iceberg",
    "lake",
    "large_dripstone",
    "monster_room",
    "sequence",
    "bamboo",
    "geode",
];

/// Types implemented only by the step-6 batch (`features.rs`). The test gates
/// them on `step == UNDERGROUND_ORES`; any other step is an orphan.
const STEP6_BATCH: &[&str] = &["underwater_magma"];

/// Types *not* implemented anywhere, keyed by `(type, configured id)` with the
/// reason. Every entry = a real vanilla feature the generator silently omits
/// today. Update the list AND the reason when a type is ported; the test fails
/// if the found set drifts.
const KNOWN_NO_OP: &[(&str, &str, &str)] = &[
    // Multiface growth is ONLY implemented for sculk_vein (sculk module);
    // glow_lichen is ported but MEASURED as a recall regression (-0.81pp:
    // cave-terrain coupling shifts the lush-caves features) — reverted,
    // stays a confessed gap until the terrain parity improves.
    (
        "multiface_growth",
        "glow_lichen",
        "lichen reverted: recall regression",
    ),
];

fn strip_mc(s: &str) -> &str {
    s.strip_prefix("minecraft:").unwrap_or(s)
}

/// Resolve a feature reference (placed or configured id, or inline object) and
/// collect every configured feature reachable from it, recursing through
/// nested placed features and selector children.
fn collect_configured(
    ref_id: &str,
    placed_of: &str,
    out: &mut Vec<(String, Value)>,
    seen_placed: &mut HashSet<String>,
    issues: &mut Vec<String>,
) {
    // Prefer configured (matches `place_placed_feature_step`); the id may also
    // be a nested placed feature (selector children) — recurse then.
    if let Some(cfg) = load_configured_feature(ref_id) {
        out.push((ref_id.to_string(), cfg.clone()));
        return;
    }
    if let Some(_placed) = load_placed_feature(ref_id) {
        if seen_placed.insert(ref_id.to_string()) {
            resolve_placed(ref_id, out, seen_placed, issues);
        }
        return;
    }
    issues.push(format!(
        "{placed_of} -> {ref_id} (unresolved feature reference)"
    ));
}

fn resolve_placed(
    placed_id: &str,
    out: &mut Vec<(String, Value)>,
    seen_placed: &mut HashSet<String>,
    issues: &mut Vec<String>,
) {
    let Some(placed) = load_placed_feature(placed_id) else {
        issues.push(format!(
            "placed_feature `{placed_id}` missing from data tree"
        ));
        return;
    };
    let feature = &placed["feature"];
    match feature.as_str() {
        Some(id) => collect_configured(id, placed_id, out, seen_placed, issues),
        None => {
            // Inline configured feature object.
            if feature.is_object() {
                collect_selector_children(placed_id, feature, out, seen_placed, issues);
                out.push((placed_id.to_string(), feature.clone()));
            } else {
                issues.push(format!("placed_feature `{placed_id}`: no feature field"));
            }
        }
    }
}

/// Resolve a feature reference that may be a string id OR an inline placed
/// object (`{"feature": ..., "placement": [...]}` — used by
/// `random_boolean_selector`/`random_selector` children, e.g. `lush_caves_clay`).
fn collect_feature_ref(
    value: &Value,
    placed_of: &str,
    out: &mut Vec<(String, Value)>,
    seen_placed: &mut HashSet<String>,
    issues: &mut Vec<String>,
) {
    if let Some(id) = value.as_str() {
        collect_configured(id, placed_of, out, seen_placed, issues);
        return;
    }
    if !value.is_object() {
        issues.push(format!(
            "{placed_of}: invalid feature ref (not a string or object)"
        ));
        return;
    }
    // Inline placed feature object: resolve its inner feature, then collect
    // the placed object itself (matches `place_feature_ref` at runtime).
    match value.get("feature") {
        Some(inner) if inner.is_string() => {
            collect_configured(inner.as_str().unwrap(), placed_of, out, seen_placed, issues)
        }
        Some(inner) if inner.is_object() => {
            // Inline configured feature directly in the placed object.
            collect_selector_children(placed_of, inner, out, seen_placed, issues);
            out.push((placed_of.to_string(), inner.clone()));
        }
        _ => issues.push(format!(
            "{placed_of}: inline placed object without a feature field"
        )),
    }
}

/// Push selector children onto `out` via their feature refs
/// (`random_selector`, `simple_random_selector`, `random_boolean_selector`).
fn collect_selector_children(
    placed_of: &str,
    cfg: &Value,
    out: &mut Vec<(String, Value)>,
    seen_placed: &mut HashSet<String>,
    issues: &mut Vec<String>,
) {
    let ty = strip_mc(cfg["type"].as_str().unwrap_or(""));
    let config = &cfg["config"];
    match ty {
        "random_selector" => {
            if let Some(features) = config["features"].as_array() {
                for f in features {
                    if let Some(feature) = f.get("feature") {
                        collect_feature_ref(feature, placed_of, out, seen_placed, issues);
                    }
                }
            }
            if let Some(def) = config.get("default") {
                collect_feature_ref(def, placed_of, out, seen_placed, issues);
            }
        }
        "simple_random_selector" => {
            if let Some(features) = config["features"].as_array() {
                for f in features {
                    if let Some(feature) = f.get("feature") {
                        collect_feature_ref(feature, placed_of, out, seen_placed, issues);
                    }
                }
            }
        }
        "random_boolean_selector" => {
            if let Some(feature) = config.get("feature_true") {
                collect_feature_ref(feature, placed_of, out, seen_placed, issues);
            }
            if let Some(feature) = config.get("feature_false") {
                collect_feature_ref(feature, placed_of, out, seen_placed, issues);
            }
        }
        _ => {}
    }
}

/// Collect every (step, placed_id, configured_id, type) reachable from the
/// overworld FeatureSorter, plus resolution issues.
fn sweep() -> (Vec<(i32, String, String, String)>, Vec<String>) {
    let mut all = Vec::new();
    let mut issues = Vec::new();
    for step_idx in step::RAW_GENERATION..=step::TOP_LAYER_MODIFICATION {
        for placed_id in feature_catalog::features_per_step_at(step_idx) {
            let mut cfgs = Vec::new();
            let mut seen_placed = HashSet::new();
            resolve_placed(placed_id, &mut cfgs, &mut seen_placed, &mut issues);
            for (cfg_id, cfg) in cfgs {
                let ty = strip_mc(cfg["type"].as_str().unwrap_or("")).to_string();
                all.push((step_idx, placed_id.clone(), cfg_id, ty));
            }
        }
    }
    all.sort_by(|a, b| (&a.2, &a.3).cmp(&(&b.2, &b.3)));
    all.dedup();
    (all, issues)
}

#[test]
fn overworld_sorter_dispatch_coverage() {
    let (all, issues) = sweep();

    // Integrity: every reference must resolve (nothing dangles).
    assert!(
        issues.is_empty(),
        "data integrity failures (26.2 data tree):\n  {}",
        issues.join("\n  ")
    );

    let handled: BTreeSet<&str> = HANDLED.iter().copied().collect();
    let batch: BTreeSet<&str> = STEP6_BATCH.iter().copied().collect();
    let no_op: BTreeSet<(&str, &str)> = KNOWN_NO_OP.iter().map(|(t, c, _)| (*t, *c)).collect();
    let mut found_no_op: BTreeSet<(String, String)> = BTreeSet::new();
    let mut orphans: Vec<(i32, &str, &str, &str)> = Vec::new();

    for (step_idx, placed_id, cfg_id, ty) in &all {
        let config_id = strip_mc(cfg_id.as_str());
        // 1. Confessed gaps match exactly (type, configured id).
        if no_op.contains(&(ty.as_str(), config_id)) {
            found_no_op.insert((ty.clone(), config_id.to_string()));
            continue;
        }
        // 2. sculk_vein plants via the sculk module with dedicated seeds.
        if ty == "multiface_growth" && config_id == "sculk_vein" {
            continue;
        }
        // 3. Step-6 batch (features.rs) — only at UNDERGROUND_ORES.
        if batch.contains(ty.as_str()) && *step_idx == step::UNDERGROUND_ORES {
            continue;
        }
        // 4. Dispatch arms.
        if handled.contains(ty.as_str()) {
            continue;
        }
        orphans.push((*step_idx, placed_id, config_id, ty));
    }

    assert!(
        orphans.is_empty(),
        "configured feature types with NO dispatch and NO whitelist entry \
         (26.2 overworld sorter). Port them or add to KNOWN_NO_OP with a reason:\n  {}",
        orphans
            .iter()
            .map(|(s, p, c, t)| format!("step={s} placed={p} configured={c} type={t}"))
            .collect::<Vec<_>>()
            .join("\n  ")
    );

    // Whitelist drift: addition AND removal must fail (both are changes).
    let expected: BTreeSet<(String, String)> = no_op
        .iter()
        .map(|(t, c)| (t.to_string(), c.to_string()))
        .collect();
    assert_eq!(
        found_no_op,
        expected,
        "KNOWN_NO_OP whitelist drift: {} (update the list and the reason)",
        if found_no_op.is_empty() && !expected.is_empty() {
            "features vanished — confirm removal and update KNOWN_NO_OP"
        } else {
            "new no-op feature appeared — port it or confess the gap"
        }
    );

    println!(
        "dispatch coverage: {} reachable configured features; {} dispatched types, \
         {} step-6 batch types, {} confessed no-ops ({} whitelist entries)",
        all.len(),
        handled.len(),
        batch.len(),
        found_no_op.len(),
        no_op.len(),
    );
    println!("confessed gaps: {}", found_no_op.len());
}
