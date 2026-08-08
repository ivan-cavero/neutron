//! Main benchmark harness: orchestrates server lifecycle, bot scenarios, and metrics.

use chrono::Local;
use eyre::{Result, WrapErr};
use neutron_bot::scenarios::{self, ScenarioConfig};
use std::fs;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use crate::hardware;
use crate::types::{Scenario, ServerType, Size};
use crate::metrics::MetricsSampler;
use crate::reporter;
use crate::server;
use crate::tps;
use sysinfo::System;

/// Run a single benchmark scenario.
pub async fn run_scenario(
    server_type: ServerType,
    size: Size,
    scenario: Scenario,
    host: &str,
    port: u16,
    runs: u32,
    seed: &str,
    warmup_secs: u64,
    duration: u64,
    results_dir: &str,
    log_dir: &str,
) -> Result<()> {
    let bot_count = size.bot_count();
    let size_label = size.label();
    let scenario_label = scenario.label();
    let server_label = server_type.label();

    let timestamp = Local::now().format("%Y%m%d-%H%M%S").to_string();
    let benchmark_id = format!(
        "{}-{}-{}-{}",
        server_label, size_label, scenario_label, timestamp
    );

    let results_path = PathBuf::from(results_dir).join(format!("{}.json", benchmark_id));
    let log_path = PathBuf::from(log_dir);

    fs::create_dir_all(results_dir)
        .wrap_err_with(|| format!("creating results dir: {}", results_dir))?;
    fs::create_dir_all(&log_path)
        .wrap_err_with(|| format!("creating log dir: {}", log_path.display()))?;

    let hardware = hardware::detect_hardware();

    // Measure disk I/O once at the start
    println!("  Measuring disk I/O...");
    let disk_io = crate::diskio::benchmark(&log_path).unwrap_or_else(|e| {
        println!("  Disk I/O measurement failed: {}", e);
        crate::diskio::DiskIoResult {
            write_mb_s: 0.0,
            read_mb_s: 0.0,
            write_iops: 0.0,
            read_iops: 0.0,
        }
    });
    println!(
        "  Disk I/O: write={:.0} MB/s, read={:.0} MB/s, write_iops={:.0}, read_iops={:.0}",
        disk_io.write_mb_s, disk_io.read_mb_s, disk_io.write_iops, disk_io.read_iops
    );

    let mut run_details = Vec::new();

    for run_idx in 0..runs {
        let run_id = format!("{}-run{}", benchmark_id, run_idx);
        let server_dir = PathBuf::from(format!("bench/test-{}", server_label));

        println!(
            "  Run {}/{}: starting {} server...",
            run_idx + 1,
            runs,
            server_label
        );

        // Start server
        let mut proc = server::start(
            server_type,
            &server_dir,
            &run_id,
            bot_count,
            seed,
            &log_path,
        )
        .wrap_err_with(|| format!("starting {} server", server_label))?;

        // Wait for server ready
        let startup_ms = proc
            .wait_ready(Duration::from_secs(120))
            .wrap_err("waiting for server ready")?;
        let startup_ms_val = startup_ms.as_secs_f64() * 1000.0;
        println!("    Server ready in {:.0}ms", startup_ms_val);

        // Warmup phase with metrics sampling (measure the SERVER process, not harness)
        println!("    Warmup: {}s...", warmup_secs);
        let server_pid = proc.pid();
        let sampler = MetricsSampler::new(server_pid, Duration::from_secs(1));
        let warmup_handle = tokio::task::spawn_blocking(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async move {
                sampler.sample_for_duration(Duration::from_secs(warmup_secs)).await
            })
        });

        tokio::time::sleep(Duration::from_secs(warmup_secs)).await;

        // Measure TPS via RCON (Paper/Folia only)
        let tps_result = match server_type {
            ServerType::Paper | ServerType::Folia => {
                println!("    Measuring TPS via RCON...");
                match tps::query_tps_stable("127.0.0.1", 25575, "neutronbench", 3, 1000) {
                    Ok(r) => {
                        println!(
                            "    TPS: 1m={:.1}, 5m={:.1}, 15m={:.1}, MSPT={:.1}ms",
                            r.tps_1m, r.tps_5m, r.tps_15m, r.mspt_avg
                        );
                        Some(r)
                    }
                    Err(e) => {
                        println!("    TPS measurement failed: {}", e);
                        None
                    }
                }
            }
            _ => None,
        };

        // Run scenario (synchronous - threading is internal to neutron-bot)
        let scenario_config = ScenarioConfig {
            host: host.to_string(),
            port,
            bot_count,
            duration: Duration::from_secs(duration),
            seed: seed.to_string(),
        };

        println!(
            "    Running scenario: {} ({} bots, {}s)...",
            scenario_label,
            bot_count,
            duration
        );

        let scenario_start = Instant::now();

        let scenario_result = match scenario {
            Scenario::JoinStorm => {
                let result = scenarios::join_storm::run(&scenario_config);
                println!(
                    "    Join storm: {}/{} bots connected, p50={:.0}ms, p95={:.0}ms",
                    result.successful,
                    result.total_bots,
                    result.percentiles().0,
                    result.percentiles().1,
                );
                serde_json::to_value(&result)?
            }
            Scenario::Distributed => {
                let result = scenarios::distributed::run(&scenario_config);
                println!(
                    "    Distributed: {}/{} bots launched",
                    result.successful, result.total_bots
                );
                serde_json::to_value(&result)?
            }
            Scenario::Movement => {
                let result = scenarios::movement::run(&scenario_config);
                println!(
                    "    Movement: {}/{} bots active, {} chunks received",
                    result.successful, result.total_bots, result.chunks_total
                );
                serde_json::to_value(&result)?
            }
            Scenario::Spread => {
                let result = scenarios::spread::run(&scenario_config);
                println!(
                    "    Spread: {}/{} bots spread, {} chunks received",
                    result.successful, result.total_bots, result.chunks_total
                );
                serde_json::to_value(&result)?
            }
            Scenario::ChunkGen => {
                let result = scenarios::chunk_gen::run(&scenario_config);
                println!(
                    "    Chunk gen: {}/{} bots walking, CPS={:.1}, total_chunks={}",
                    result.successful, result.total_bots, result.cps_total, result.total_chunks
                );
                serde_json::to_value(&result)?
            }
            Scenario::SustainedLoad => {
                let result = scenarios::sustained_load::run(&scenario_config);
                println!(
                    "    Sustained load: {}/{} bots idle for {}s, {} ticks",
                    result.successful, result.total_bots, result.duration_secs as u64, result.ticks_total
                );
                serde_json::to_value(&result)?
            }
            Scenario::StressTest => {
                let result = scenarios::stress_test::run(&scenario_config);
                println!(
                    "    Stress test: {}/{} bots moving, CPS={:.1}, {} chunks, {} ticks",
                    result.successful, result.total_bots, result.cps, result.total_chunks, result.ticks_total
                );
                serde_json::to_value(&result)?
            }
        };

        let scenario_duration_ms = scenario_start.elapsed().as_secs_f64() * 1000.0;

        // Collect final metrics
        let metrics = warmup_handle.await?;

        // Measure disk I/O during scenario (read disk stats before/after)
        let disk_io_during = crate::diskio::benchmark(&log_path).unwrap_or(crate::diskio::DiskIoResult {
            write_mb_s: 0.0, read_mb_s: 0.0, write_iops: 0.0, read_iops: 0.0,
        });

        // Get CPU core count (estimate for thread count)
        let thread_count = {
            let mut sys = sysinfo::System::new();
            sys.refresh_all();
            sys.cpus().len()
        };

        // Stop server
        proc.stop()?;

        let run_detail = serde_json::json!({
            "run": run_idx + 1,
            "startup_ms": startup_ms_val,
            "scenario_duration_ms": scenario_duration_ms,
            "ram_idle_mb": metrics.ram_idle_mb,
            "ram_peak_mb": metrics.ram_peak_mb,
            "cpu_idle_pct": metrics.cpu_idle_pct,
            "cpu_peak_pct": metrics.cpu_peak_pct,
            "disk_io_during": disk_io_during,
            "thread_count": thread_count,
            "tps": tps_result.as_ref().map(|t| serde_json::json!({
                "1m": t.tps_1m,
                "5m": t.tps_5m,
                "15m": t.tps_15m,
                "mspt_avg": t.mspt_avg,
            })),
            "scenario": scenario_result,
        });

        run_details.push(run_detail);
        println!("    Run {} complete.", run_idx + 1);
    }

    // Build aggregate result
    let aggregate = build_aggregate(&run_details, scenario);

    let output = serde_json::json!({
        "benchmark_id": benchmark_id,
        "server": {
            "type": server_label,
            "version": "26.2",
        },
        "scenario": scenario_label,
        "size": size_label,
        "n_bots": bot_count,
        "runs": runs,
        "seed": seed,
        "config": {
            "view_distance": 10,
            "simulation_distance": 10,
            "online_mode": false,
            "jvm_args": "-Xms2G -Xmx2G -XX:+AlwaysPreTouch",
        },
        "aggregate": aggregate,
        "runs_detail": run_details,
        "hardware": hardware,
        "disk_io": disk_io,
    });

    // Write JSON
    fs::write(&results_path, serde_json::to_string_pretty(&output)?)
        .wrap_err_with(|| format!("writing {}", results_path.display()))?;

    // Write markdown
    let md_path = results_path.with_extension("md");
    let md_content = reporter::format_markdown(&output);
    fs::write(&md_path, md_content)
        .wrap_err_with(|| format!("writing {}", md_path.display()))?;

    println!(
        "  Results: {} + {}",
        results_path.display(),
        md_path.display()
    );

    Ok(())
}

/// Build aggregate metrics from run details.
fn build_aggregate(runs: &[serde_json::Value], scenario: Scenario) -> serde_json::Value {
    let startups: Vec<f64> = runs
        .iter()
        .filter_map(|r| r["startup_ms"].as_f64())
        .collect();
    let ram_idle: Vec<f64> = runs
        .iter()
        .filter_map(|r| r["ram_idle_mb"].as_f64())
        .collect();
    let ram_peak: Vec<f64> = runs
        .iter()
        .filter_map(|r| r["ram_peak_mb"].as_f64())
        .collect();
    let cpu_idle: Vec<f64> = runs
        .iter()
        .filter_map(|r| r["cpu_idle_pct"].as_f64())
        .collect();
    let cpu_peak: Vec<f64> = runs
        .iter()
        .filter_map(|r| r["cpu_peak_pct"].as_f64())
        .collect();

    let median_startup = median(&startups);
    let avg_ram_idle = average(&ram_idle);
    let max_ram_peak = ram_peak.iter().cloned().fold(0.0_f64, f64::max);
    let avg_cpu_idle = average(&cpu_idle);
    let max_cpu_peak = cpu_peak.iter().cloned().fold(0.0_f64, f64::max);

    let (join_p50, join_p95, join_p99) = match scenario {
        Scenario::JoinStorm | Scenario::Distributed => {
            let all_latencies: Vec<f64> = runs
                .iter()
                .filter_map(|r| r["scenario"]["latencies"].as_array())
                .flat_map(|arr| arr.iter().filter_map(|v| v.as_f64()))
                .collect();
            let mut sorted = all_latencies;
            sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            let (p50, p95, p99) = neutron_bot::metrics::percentiles(&sorted);
            (Some(p50), Some(p95), Some(p99))
        }
        _ => (None, None, None),
    };

    let (cps, total_chunks) = match scenario {
        Scenario::ChunkGen | Scenario::StressTest => {
            let total_cps: f64 = runs
                .iter()
                .filter_map(|r| {
                    let v = &r["scenario"];
                    v["cps"].as_f64().or_else(|| v["cps_total"].as_f64())
                })
                .sum();
            let total_chunks_sum: f64 = runs
                .iter()
                .filter_map(|r| r["scenario"]["total_chunks"].as_f64())
                .sum();
            let count = runs
                .iter()
                .filter(|r| {
                    let v = &r["scenario"];
                    v["cps"].is_number() || v["cps_total"].is_number()
                })
                .count();
            if count > 0 {
                (Some(total_cps / count as f64), Some(total_chunks_sum / count as f64))
            } else {
                (None, None)
            }
        }
        Scenario::Movement | Scenario::Spread => {
            let total_chunks_sum: f64 = runs
                .iter()
                .filter_map(|r| r["scenario"]["chunks_total"].as_f64())
                .sum();
            let count = runs.len().max(1);
            (None, Some(total_chunks_sum / count as f64))
        }
        _ => (None, None),
    };

    // Thread count from first run
    let thread_count = runs.iter()
        .find_map(|r| r["thread_count"].as_u64())
        .map(|v| v as usize);

    // TPS from first run that has it
    let tps_data = runs.iter()
        .find_map(|r| r.get("tps").cloned())
        .filter(|t| !t.is_null());

    serde_json::json!({
        "startup_ms": median_startup,
        "join": {
            "p50": join_p50,
            "p95": join_p95,
            "p99": join_p99,
        },
        "cps": cps,
        "total_chunks": total_chunks,
        "tps": tps_data,
        "ram": {
            "idle_mb": avg_ram_idle,
            "peak_mb": max_ram_peak,
        },
        "cpu": {
            "idle_pct": avg_cpu_idle,
            "peak_pct": max_cpu_peak,
        },
        "thread_count": thread_count,
    })
}

fn median(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let mid = sorted.len() / 2;
    if sorted.len() % 2 == 0 {
        (sorted[mid - 1] + sorted[mid]) / 2.0
    } else {
        sorted[mid]
    }
}

fn average(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    values.iter().sum::<f64>() / values.len() as f64
}
