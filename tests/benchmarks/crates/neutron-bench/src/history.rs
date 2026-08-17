//! Versioned report history: durable, timestamped reports under
//! `results/history/` plus `history list` for at-a-glance trend review.

use eyre::{Result, WrapErr};
use serde_json::Value;
use std::fs;
use std::path::PathBuf;

/// History directory (`results/history/` under the benchmarks workspace root).
pub fn dir() -> PathBuf {
    crate::ws_root().join("results/history")
}

/// Versioned report filename:
/// `<server>-<version>-<scenario>-<size>-<runs>-<timestamp>.json`.
pub fn report_filename(
    server: &str,
    version: &str,
    scenario: &str,
    size: &str,
    runs: u32,
    timestamp: &str,
) -> String {
    let version = version.replace(['/', '\\', ':'], "_");
    format!("{}-{}-{}-{}-{}-{}.json", server, version, scenario, size, runs, timestamp)
}

/// Write a report into the history dir, returning the written path.
pub fn write_report(filename: &str, output: &Value) -> Result<PathBuf> {
    let history_dir = dir();
    fs::create_dir_all(&history_dir)
        .wrap_err_with(|| format!("creating history dir: {}", history_dir.display()))?;
    let path = history_dir.join(filename);
    fs::write(&path, serde_json::to_string_pretty(output)?)
        .wrap_err_with(|| format!("writing {}", path.display()))?;
    Ok(path)
}

/// Human-readable timestamp from the report's `%Y%m%d-%H%M%S` field.
fn display_timestamp(ts: &str) -> String {
    if ts.len() == 15 && ts.as_bytes()[8] == b'-' {
        format!(
            "{}-{}-{} {}:{}:{}",
            &ts[0..4], &ts[4..6], &ts[6..8], &ts[9..11], &ts[11..13], &ts[13..15]
        )
    } else {
        ts.to_string()
    }
}

/// List past runs sorted by time (newest first) with their key metrics.
pub fn list() -> Result<()> {
    let history_dir = dir();
    if !history_dir.exists() {
        println!(
            "No history yet ({}). Run `neutron-bench run` first.",
            history_dir.display()
        );
        return Ok(());
    }

    let mut rows: Vec<(String, String, Value)> = Vec::new(); // (sort_key, display_ts, report)
    for entry in fs::read_dir(&history_dir)? {
        let entry = entry.wrap_err("reading history dir")?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let content = fs::read_to_string(&path)
            .wrap_err_with(|| format!("reading {}", path.display()))?;
        match serde_json::from_str::<Value>(&content) {
            Ok(v) => {
                let ts = match v["timestamp"].as_str() {
                    Some(t) => t.to_string(),
                    None => entry.file_name().to_string_lossy().into_owned(),
                };
                rows.push((ts.clone(), display_timestamp(&ts), v));
            }
            Err(e) => println!("  (skipping {}: {})", path.display(), e),
        }
    }
    rows.sort_by(|a, b| b.0.cmp(&a.0)); // newest first

    println!(
        "History: {} report(s) in {} (newest first)\n",
        rows.len(),
        history_dir.display()
    );
    println!(
        "| Timestamp           | Server   | Version | Scenario     | Size  | Runs | Startup ms | Join p50 | TPS   | CPS   | RAM peak MB |"
    );
    println!(
        "|---------------------|----------|---------|--------------|-------|------|------------|----------|-------|-------|-------------|"
    );
    for (_, ts, v) in &rows {
        let server = v["server"]["type"].as_str().unwrap_or("?");
        let version = v["server"]["version"].as_str().unwrap_or("?");
        let scenario = v["scenario"].as_str().unwrap_or("?");
        let size = v["size"].as_str().unwrap_or("?");
        let runs = v["runs"].as_u64().unwrap_or(0);
        let startup = crate::reporter::metric_value(v, "aggregate.startup_ms").unwrap_or(0.0);
        let p50 = crate::reporter::metric_value(v, "aggregate.join.p50");
        let tps = crate::reporter::metric_value(v, "aggregate.tps.effective")
            .or_else(|| crate::reporter::metric_value(v, "aggregate.tps.1m"));
        let cps = crate::reporter::metric_value(v, "aggregate.cps");
        let ram_peak = crate::reporter::metric_value(v, "aggregate.ram.peak_mb").unwrap_or(0.0);
        println!(
            "| {} | {:<8} | {:<7} | {:<12} | {:<5} | {:<4} | {:<10.0} | {:<8} | {:<5} | {:<5} | {:<11.1} |",
            ts,
            server,
            version,
            scenario,
            size,
            runs,
            startup,
            p50.map(|x| format!("{:.1}", x)).unwrap_or_else(|| "N/A".to_string()),
            tps.map(|x| format!("{:.1}", x)).unwrap_or_else(|| "N/A".to_string()),
            cps.map(|x| format!("{:.1}", x)).unwrap_or_else(|| "N/A".to_string()),
            ram_peak,
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_filename_matches_bar_pattern() {
        assert_eq!(
            report_filename("vanilla", "26.2", "join-storm", "small", 1, "20260817-185504"),
            "vanilla-26.2-join-storm-small-1-20260817-185504.json"
        );
        // Version strings must not break the path.
        assert_eq!(
            report_filename("paper", "1.21/2", "spread", "medium", 3, "20260817-185504"),
            "paper-1.21_2-spread-medium-3-20260817-185504.json"
        );
    }

    #[test]
    fn display_timestamp_reformats() {
        assert_eq!(
            display_timestamp("20260817-185504"),
            "2026-08-17 18:55:04"
        );
        assert_eq!(display_timestamp("garbage"), "garbage");
    }
}