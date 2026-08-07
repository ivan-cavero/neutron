//! Report generation: JSON output and Markdown formatting.

use eyre::{Result, WrapErr};
use serde_json::Value;
use std::fs;
use std::path::Path;

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

/// Compare multiple benchmark results.
pub fn compare(files: &[String]) -> Result<()> {
    if files.len() < 2 {
        eyre::bail!("Need at least 2 files to compare");
    }

    let mut results = Vec::new();
    for file in files {
        let content = fs::read_to_string(file)
            .wrap_err_with(|| format!("reading {}", file))?;
        let data: Value = serde_json::from_str(&content)
            .wrap_err_with(|| format!("parsing {}", file))?;
        results.push(data);
    }

    println!("\n## Benchmark Comparison\n");
    println!(
        "| Server | Scenario | Size | Startup (ms) | Join p50 | Join p95 | TPS | CPS | RAM idle (MB) | RAM peak (MB) | CPU peak (%) | Disk W (MB/s) | Disk R (MB/s) |"
    );
    println!(
        "|--------|----------|------|-------------|----------|----------|-----|-----|---------------|---------------|-------------|---------------|---------------|"
    );

    for data in &results {
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
        let tps = data["aggregate"]["tps"]["1m"].as_f64();
        let disk_w = data.get("disk_io").and_then(|d| d["write_mb_s"].as_f64()).unwrap_or(0.0);
        let disk_r = data.get("disk_io").and_then(|d| d["read_mb_s"].as_f64()).unwrap_or(0.0);

        println!(
            "| {} | {} | {} | {:.0} | {} | {} | {} | {} | {:.1} | {:.1} | {:.1} | {:.0} | {:.0} |",
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

    println!();
    Ok(())
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
