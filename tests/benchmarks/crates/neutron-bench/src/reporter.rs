//! Report generation: JSON output and Markdown formatting.

use eyre::{Result, WrapErr};
use serde_json::Value;
use std::fs;
use std::path::Path;

/// Extract a dotted-path f64 from a report (e.g. "aggregate.startup_ms").
/// Returns None for missing fields and JSON nulls.
pub(crate) fn metric_value(data: &Value, path: &str) -> Option<f64> {
    let mut cur = data;
    for part in path.split('.') {
        cur = cur.get(part)?;
    }
    cur.as_f64()
}

/// Format benchmark results as Markdown.
pub fn format_markdown(data: &Value) -> String {
    let server_type = data["server"]["type"].as_str().unwrap_or("unknown");
    let version = data["server"]["version"].as_str().unwrap_or("unknown");
    let scenario = data["scenario"].as_str().unwrap_or("unknown");
    let size = data["size"].as_str().unwrap_or("unknown");
    let n_bots = data["n_bots"].as_u64().unwrap_or(0);
    let runs = data["runs"].as_u64().unwrap_or(0);
    let seed = data["seed"].as_str().unwrap_or("unknown");
    let benchmark_id = data["benchmark_id"].as_str().unwrap_or("unknown");

    let hw = &data["hardware"];
    let os = hw["os"].as_str().unwrap_or("Unknown");
    let cpu = hw["cpu"].as_str().unwrap_or("Unknown");
    let ram_gb = hw["ram_gb"].as_f64().unwrap_or(0.0);

    let agg = &data["aggregate"];

    let mut md = format!(
        r#"# Benchmark: {}

**Server:** {} {} · **Scenario:** {} · **Size:** {} ({} bots)
**Runs:** {} (median) · **Seed:** {}
**OS:** {} · **CPU:** {} · **RAM:** {:.1} GB

---

## Aggregate Results

| Metric | Value |
|--------|-------|
| Startup (median) | {:.0} ms |
| RAM idle | {:.1} MB |
| RAM peak | {:.1} MB |
| CPU idle | {:.1}% |
| CPU peak | {:.1}% |"#,
        benchmark_id,
        server_type,
        version,
        scenario,
        size,
        n_bots,
        runs,
        seed,
        os,
        cpu,
        ram_gb,
        agg["startup_ms"].as_f64().unwrap_or(0.0),
        agg["ram"]["idle_mb"].as_f64().unwrap_or(0.0),
        agg["ram"]["peak_mb"].as_f64().unwrap_or(0.0),
        agg["cpu"]["idle_pct"].as_f64().unwrap_or(0.0),
        agg["cpu"]["peak_pct"].as_f64().unwrap_or(0.0),
    );

    // Join latency metrics
    if let Some(p50) = agg["join"]["p50"].as_f64() {
        let p95 = agg["join"]["p95"].as_f64().unwrap_or(0.0);
        let p99 = agg["join"]["p99"].as_f64().unwrap_or(0.0);
        md.push_str(&format!(
            "\n| Join p50 | {:.1} ms |\n| Join p95 | {:.1} ms |\n| Join p99 | {:.1} ms |",
            p50, p95, p99
        ));
    }

    // CPS metrics
    if let Some(cps) = agg["cps"].as_f64() {
        md.push_str(&format!("\n| CPS (total) | {:.1} |", cps));
    }

    // Total chunks
    if let Some(chunks) = agg["total_chunks"].as_f64() {
        md.push_str(&format!("\n| Total chunks | {:.0} |", chunks));
    }

    // TPS metrics
    if let Some(tps) = agg.get("tps") {
        if let Some(tps_1m) = tps["1m"].as_f64() {
            md.push_str(&format!("\n| TPS (1m) | {:.1} |", tps_1m));
        }
        if let Some(mspt) = tps["mspt_avg"].as_f64() {
            md.push_str(&format!("\n| MSPT avg | {:.1} ms |", mspt));
        }
    }

    // Disk I/O
    if let Some(disk) = data.get("disk_io") {
        let w = disk["write_mb_s"].as_f64().unwrap_or(0.0);
        let r = disk["read_mb_s"].as_f64().unwrap_or(0.0);
        if w > 0.0 || r > 0.0 {
            md.push_str(&format!("\n| Disk write | {:.0} MB/s |", w));
            md.push_str(&format!("\n| Disk read | {:.0} MB/s |", r));
        }
    }

    md.push_str("\n\n## Per-Run Detail\n\n");

    // Table headers vary by scenario
    match scenario {
        "join-storm" | "distributed" => {
            md.push_str("| Run | Startup (ms) | Join p50 (ms) | Join p95 (ms) | RAM idle (MB) | RAM peak (MB) | CPU idle (%) |\n");
            md.push_str("|-----|-------------|---------------|---------------|---------------|---------------|-------------|\n");
            if let Some(runs_detail) = data["runs_detail"].as_array() {
                for run in runs_detail {
                    let r = run["run"].as_u64().unwrap_or(0);
                    let startup = run["startup_ms"].as_f64().unwrap_or(0.0);
                    let ram_idle = run["ram_idle_mb"].as_f64().unwrap_or(0.0);
                    let ram_peak = run["ram_peak_mb"].as_f64().unwrap_or(0.0);
                    let cpu_idle = run["cpu_idle_pct"].as_f64().unwrap_or(0.0);
                    // Extract p50 from scenario latencies
                    let latencies = run["scenario"]["latencies"].as_array();
                    let (p50, p95) = if let Some(arr) = latencies {
                        let mut vals: Vec<f64> = arr.iter().filter_map(|v| v.as_f64()).collect();
                        vals.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
                        let (p50, p95, _) = neutron_bot::metrics::percentiles(&vals);
                        (p50, p95)
                    } else {
                        (0.0, 0.0)
                    };
                    md.push_str(&format!(
                        "| {} | {:.0} | {:.1} | {:.1} | {:.1} | {:.1} | {:.1} |\n",
                        r, startup, p50, p95, ram_idle, ram_peak, cpu_idle
                    ));
                }
            }
        }
        "chunk-gen" => {
            md.push_str("| Run | Startup (ms) | CPS | Total Chunks | RAM idle (MB) | RAM peak (MB) | CPU idle (%) |\n");
            md.push_str("|-----|-------------|-----|--------------|---------------|---------------|-------------|\n");
            if let Some(runs_detail) = data["runs_detail"].as_array() {
                for run in runs_detail {
                    let r = run["run"].as_u64().unwrap_or(0);
                    let startup = run["startup_ms"].as_f64().unwrap_or(0.0);
                    let ram_idle = run["ram_idle_mb"].as_f64().unwrap_or(0.0);
                    let ram_peak = run["ram_peak_mb"].as_f64().unwrap_or(0.0);
                    let cpu_idle = run["cpu_idle_pct"].as_f64().unwrap_or(0.0);
                    let cps = run["scenario"]["cps_total"].as_f64().unwrap_or(0.0);
                    let chunks = run["scenario"]["total_chunks"].as_f64().unwrap_or(0.0);
                    md.push_str(&format!(
                        "| {} | {:.0} | {:.1} | {:.0} | {:.1} | {:.1} | {:.1} |\n",
                        r, startup, cps, chunks, ram_idle, ram_peak, cpu_idle
                    ));
                }
            }
        }
        "movement" | "spread" => {
            md.push_str("| Run | Startup (ms) | Chunks Received | RAM idle (MB) | RAM peak (MB) | CPU idle (%) |\n");
            md.push_str("|-----|-------------|-----------------|---------------|---------------|-------------|\n");
            if let Some(runs_detail) = data["runs_detail"].as_array() {
                for run in runs_detail {
                    let r = run["run"].as_u64().unwrap_or(0);
                    let startup = run["startup_ms"].as_f64().unwrap_or(0.0);
                    let ram_idle = run["ram_idle_mb"].as_f64().unwrap_or(0.0);
                    let ram_peak = run["ram_peak_mb"].as_f64().unwrap_or(0.0);
                    let cpu_idle = run["cpu_idle_pct"].as_f64().unwrap_or(0.0);
                    let chunks = run["scenario"]["chunks_total"].as_f64().unwrap_or(0.0);
                    md.push_str(&format!(
                        "| {} | {:.0} | {:.0} | {:.1} | {:.1} | {:.1} |\n",
                        r, startup, chunks, ram_idle, ram_peak, cpu_idle
                    ));
                }
            }
        }
        _ => {
            md.push_str("| Run | Startup (ms) | RAM idle (MB) | RAM peak (MB) | CPU idle (%) |\n");
            md.push_str("|-----|-------------|---------------|---------------|-------------|\n");
            if let Some(runs_detail) = data["runs_detail"].as_array() {
                for run in runs_detail {
                    let r = run["run"].as_u64().unwrap_or(0);
                    let startup = run["startup_ms"].as_f64().unwrap_or(0.0);
                    let ram_idle = run["ram_idle_mb"].as_f64().unwrap_or(0.0);
                    let ram_peak = run["ram_peak_mb"].as_f64().unwrap_or(0.0);
                    let cpu_idle = run["cpu_idle_pct"].as_f64().unwrap_or(0.0);
                    md.push_str(&format!(
                        "| {} | {:.0} | {:.1} | {:.1} | {:.1} |\n",
                        r, startup, ram_idle, ram_peak, cpu_idle
                    ));
                }
            }
        }
    }

    md.push_str("\n---\n");
    md.push_str("*Generated by neutron-bench*\n");

    md
}

/// Expand a `*`/`?` glob against the filesystem (regex crate, already a dep).
fn expand_glob(pattern: &str) -> Vec<String> {
    if !pattern.contains(['*', '?']) {
        return vec![pattern.to_string()];
    }
    let (dir, file_pat) = match pattern.rfind(['/', '\\']) {
        Some(i) => (&pattern[..i], &pattern[i + 1..]),
        None => (".", pattern),
    };
    let re = glob_regex(file_pat);
    let mut out: Vec<String> = fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            if re.is_match(&name) {
                Some(format!("{}/{}", dir, name))
            } else {
                None
            }
        })
        .collect();
    out.sort();
    out
}

fn glob_regex(pat: &str) -> regex::Regex {
    let mut re = String::from("^");
    for c in pat.chars() {
        match c {
            '*' => re.push_str("[^/\\\\]*"),
            '?' => re.push_str("[^/\\\\]"),
            c => re.push_str(&regex::escape(&c.to_string())),
        }
    }
    re.push('$');
    regex::Regex::new(&re).expect("glob compiles to a valid regex")
}

/// Resolve compare file args: expand globs; relative paths that don't exist in
/// cwd are retried anchored at the benchmarks workspace root, so
/// `neutron-bench compare results/history/<glob>` works from any cwd.
fn resolve_files(args: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    for f in args {
        let has_wildcard = f.contains(['*', '?']);
        let mut expanded = expand_glob(f);
        let anchored = crate::ws_root().join(f);
        let anchored_expanded = expand_glob(&anchored.to_string_lossy());
        let use_anchored = (!has_wildcard && !Path::new(f).exists())
            || (expanded.is_empty() && !anchored_expanded.is_empty());
        if use_anchored {
            expanded = anchored_expanded;
        }
        out.extend(expanded);
    }
    out
}

/// Compare multiple benchmark results (history globs included): summary table,
/// then per-metric deltas vs the first file with a winner per metric.
pub fn compare(files: &[String]) -> Result<()> {
    let files = resolve_files(files);
    if files.len() < 2 {
        eyre::bail!("Need at least 2 files to compare (found {})", files.len());
    }

    let mut results = Vec::new();
    for file in &files {
        let content = fs::read_to_string(file)
            .wrap_err_with(|| format!("reading {}", file))?;
        let data: Value = serde_json::from_str(&content)
            .wrap_err_with(|| format!("parsing {}", file))?;
        results.push(data);
    }

    let labels: Vec<String> = files
        .iter()
        .map(|f| {
            Path::new(f)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(f)
                .to_string()
        })
        .collect();

    println!("\n## Benchmark Comparison\n");
    println!(
        "| File | Server | Scenario | Size | Startup (ms) | Join p50 | Join p95 | TPS | CPS | RAM idle (MB) | RAM peak (MB) | CPU peak (%) | Disk W (MB/s) | Disk R (MB/s) |"
    );
    println!(
        "|------|--------|----------|------|-------------|----------|----------|-----|-----|---------------|---------------|-------------|---------------|---------------|"
    );

    for (i, data) in results.iter().enumerate() {
        let server = data["server"]["type"].as_str().unwrap_or("?");
        let scenario = data["scenario"].as_str().unwrap_or("?");
        let size = data["size"].as_str().unwrap_or("?");
        let startup = data["aggregate"]["startup_ms"].as_f64().unwrap_or(0.0);
        let p50 = data["aggregate"]["join"]["p50"].as_f64();
        let p95 = data["aggregate"]["join"]["p95"].as_f64();
        let cps = data["aggregate"]["cps"].as_f64();
        let ram_idle = data["aggregate"]["ram"]["idle_mb"].as_f64().unwrap_or(0.0);
        let ram_peak = data["aggregate"]["ram"]["peak_mb"].as_f64().unwrap_or(0.0);
        let cpu_peak = data["aggregate"]["cpu"]["peak_pct"].as_f64().unwrap_or(0.0);
        let tps = metric_value(data, "aggregate.tps.effective")
            .or_else(|| metric_value(data, "aggregate.tps.1m"));
        let disk_w = data.get("disk_io").and_then(|d| d["write_mb_s"].as_f64()).unwrap_or(0.0);
        let disk_r = data.get("disk_io").and_then(|d| d["read_mb_s"].as_f64()).unwrap_or(0.0);

        println!(
            "| {} | {} | {} | {} | {:.0} | {} | {} | {} | {} | {:.1} | {:.1} | {:.1} | {:.0} | {:.0} |",
            labels[i],
            server,
            scenario,
            size,
            startup,
            p50.map(|v| format!("{:.1}", v)).unwrap_or_else(|| "N/A".to_string()),
            p95.map(|v| format!("{:.1}", v)).unwrap_or_else(|| "N/A".to_string()),
            tps.map(|v| format!("{:.1}", v)).unwrap_or_else(|| "N/A".to_string()),
            cps.map(|v| format!("{:.1}", v)).unwrap_or_else(|| "N/A".to_string()),
            ram_idle,
            ram_peak,
            cpu_peak,
            disk_w,
            disk_r,
        );
    }

    println_deltas(&labels, &results);
    Ok(())
}

/// Per-metric deltas vs the baseline (first) file, with a winner per metric.
/// Lower-is-better for latencies/RAM/CPU/startup, higher-is-better for TPS/CPS/disk.
fn println_deltas(labels: &[String], results: &[Value]) {
    if results.len() < 2 {
        return;
    }
    let metrics: [(&str, &str, bool); 10] = [
        ("Startup (ms)", "aggregate.startup_ms", true),
        ("Join p50 (ms)", "aggregate.join.p50", true),
        ("Join p95 (ms)", "aggregate.join.p95", true),
        ("TPS", "aggregate.tps.effective", false),
        ("CPS", "aggregate.cps", false),
        ("RAM idle (MB)", "aggregate.ram.idle_mb", true),
        ("RAM peak (MB)", "aggregate.ram.peak_mb", true),
        ("CPU peak (%)", "aggregate.cpu.peak_pct", true),
        ("Disk W (MB/s)", "disk_io.write_mb_s", false),
        ("Disk R (MB/s)", "disk_io.read_mb_s", false),
    ];
    println!("\n## Per-metric deltas (vs baseline {})", labels[0]);
    for (label, path, lower_better) in metrics {
        let vals: Vec<Option<f64>> = results.iter().map(|d| metric_value(d, path)).collect();
        if vals.iter().all(|v| v.is_none()) {
            continue;
        }
        let baseline = vals[0];
        let best = vals
            .iter()
            .flatten()
            .copied()
            .reduce(|a, b| if lower_better { a.min(b) } else { a.max(b) });
        println!("{}:", label);
        for (i, (l, v)) in labels.iter().zip(&vals).enumerate() {
            let val = v.map(|x| format!("{:.1}", x)).unwrap_or_else(|| "N/A".to_string());
            if i == 0 {
                println!("  {:<30} {:<10} (baseline)", l, val);
            } else {
                let delta = match (baseline, v) {
                    (Some(b), Some(x)) if b != 0.0 => {
                        format!("Δ {:+.1} ({:+.1}%)", x - b, (x - b) / b * 100.0)
                    }
                    (Some(b), Some(x)) => format!("Δ {:+.1}", x - b),
                    _ => "Δ N/A".to_string(),
                };
                let winner = if *v == best { "  <= winner" } else { "" };
                println!("  {:<30} {:<10} {:<20}{}", l, val, delta, winner);
            }
        }
    }
    println!();
}

/// Generate a markdown report from a JSON file.
pub fn generate_markdown(file: &str) -> Result<()> {
    let content = fs::read_to_string(file)
        .wrap_err_with(|| format!("reading {}", file))?;
    let data: Value = serde_json::from_str(&content)
        .wrap_err_with(|| format!("parsing {}", file))?;

    let md = format_markdown(&data);

    let md_path = Path::new(file).with_extension("md");
    fs::write(&md_path, &md)
        .wrap_err_with(|| format!("writing {}", md_path.display()))?;

    println!("Report written to {}", md_path.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn expand_glob_matches_files_in_dir() {
        let dir = std::env::temp_dir().join(format!("nb-glob-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        for name in ["a.json", "b.json", "c.md"] {
            fs::File::create(dir.join(name)).unwrap().write_all(b"{}").unwrap();
        }
        let pat = format!("{}/history-*.json", dir.display());
        fs::create_dir_all(dir.join("history")).unwrap();
        fs::File::create(dir.join("history/v1.json")).unwrap().write_all(b"{}").unwrap();
        fs::File::create(dir.join("history/v2.json")).unwrap().write_all(b"{}").unwrap();

        let flat = expand_glob(&format!("{}/*.json", dir.display()));
        assert_eq!(flat.len(), 2);
        assert!(flat.iter().any(|p| p.ends_with("a.json")));
        assert!(flat.iter().any(|p| p.ends_with("b.json")));

        let nested = expand_glob(&format!("{}/history/*.json", dir.display()));
        assert_eq!(nested.len(), 2);
        assert!(nested[0].ends_with("v1.json"));
        assert!(nested[1].ends_with("v2.json"));

        // No wildcard: pattern passes through untouched.
        assert_eq!(expand_glob("plain.json"), vec!["plain.json"]);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn metric_value_reads_dotted_paths() {
        let data: Value = serde_json::json!({
            "aggregate": { "startup_ms": 1234.5, "join": { "p50": null } },
            "disk_io": { "write_mb_s": 42.0 },
        });
        assert_eq!(metric_value(&data, "aggregate.startup_ms"), Some(1234.5));
        assert_eq!(metric_value(&data, "aggregate.join.p50"), None); // null
        assert_eq!(metric_value(&data, "disk_io.write_mb_s"), Some(42.0));
        assert_eq!(metric_value(&data, "aggregate.missing"), None);
    }

    #[test]
    fn resolve_files_anchors_relative_paths_to_ws_root() {
        // cwd during tests is the crate dir, which has no results/; the path is
        // retried anchored at the benchmarks workspace root.
        let resolved = resolve_files(&["results/history/v1.json".to_string()]);
        assert_eq!(resolved.len(), 1);
        assert_eq!(
            resolved[0],
            crate::ws_root().join("results/history/v1.json").to_string_lossy()
        );

        // Absolute paths pass through untouched (no anchoring possible).
        let abs = crate::ws_root().join("results/history/v1.json");
        let resolved = resolve_files(&[abs.to_string_lossy().to_string()]);
        assert_eq!(resolved, vec![abs.to_string_lossy().to_string()]);
    }
}
