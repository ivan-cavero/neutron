//! TPS measurement - 2 methods for comparison:
//!
//! 1. **Spark TPS** (Paper/Folia): via RCON `spark tps`
//! 2. **Time query** (todos): via RCON `/time query gametime`
//!
//! Both metrics are collected when available for cross-validation.

use eyre::Result;
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct TpsResult {
    pub tps_spark: Option<f64>,
    pub tps_time_query: Option<f64>,
    pub tps_effective: f64,
    pub mspt_avg: f64,
}

/// Query TPS using both methods.
pub fn query_tps(host: &str, port: u16, password: &str) -> Result<TpsResult> {
    let mut client = crate::rcon::RconClient::connect(host, port, password)?;

    // Method 1: Spark TPS (Paper/Folia)
    let tps_spark = match client.execute("spark tps") {
        Ok(r) if r.contains("TPS") || r.contains("Region TPS") => {
            parse_spark_tps(&r)
        }
        _ => None,
    };

    // Method 2: Time query gametime (all servers)
    let tps_time_query = match client.execute("time query gametime") {
        Ok(r1) if r1.contains("time is") => {
            if let Some(t1) = parse_game_time(&r1) {
                std::thread::sleep(Duration::from_secs(5));
                if let Ok(r2) = client.execute("time query gametime") {
                    if let Some(t2) = parse_game_time(&r2) {
                        let ticks = t2 - t1;
                        Some((ticks as f64 / 5.0).min(20.0))
                    } else { None }
                } else { None }
            } else { None }
        }
        _ => None,
    };

    // Effective TPS: prefer spark, fallback to time query
    let tps_effective = tps_spark.or(tps_time_query).unwrap_or(20.0);
    let mspt_avg = if tps_effective > 0.0 { 1000.0 / tps_effective } else { 50.0 };

    Ok(TpsResult { tps_spark, tps_time_query, tps_effective, mspt_avg })
}

fn parse_game_time(response: &str) -> Option<u64> {
    for line in response.lines() {
        if let Some(pos) = line.find("time is") {
            let rest = line[pos + 8..].trim();
            let num: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
            if let Ok(val) = num.parse::<u64>() { return Some(val); }
        }
    }
    None
}

fn parse_spark_tps(response: &str) -> Option<f64> {
    let clean: String = {
        let mut r = String::new();
        let chars: Vec<char> = response.chars().collect();
        let mut i = 0;
        while i < chars.len() {
            if chars[i] == '§' { i += 2; } else { r.push(chars[i]); i += 1; }
        }
        r
    };

    // Paper format
    for line in clean.lines() {
        if line.contains("1m, 5m, 15m:") {
            if let Some(pos) = line.find("15m:") {
                let parts: Vec<&str> = line[pos + 4..].split(", ").collect();
                if parts.len() >= 3 {
                    let p = |s: &str| -> f64 {
                        s.trim().trim_start_matches('*').trim()
                            .parse::<f64>().unwrap_or_else(|_| s.replace(',', ".").parse().unwrap_or(20.0))
                            .min(20.0)
                    };
                    let t1 = p(parts[parts.len() - 3]);
                    if t1 > 0.0 && t1 <= 20.0 { return Some(t1); }
                }
            }
        }
    }

    // Folia format
    for line in clean.lines() {
        if line.contains("Region TPS:") {
            if let Some(pos) = line.find("TPS:") {
                if let Ok(f) = line[pos + 4..].trim().replace(',', ".").parse::<f64>() {
                    return Some(f.min(20.0));
                }
            }
        }
    }

    None
}

pub fn query_tps_stable(host: &str, port: u16, password: &str, samples: u32, interval_ms: u64) -> Result<TpsResult> {
    let mut last = None;
    for i in 0..samples {
        match query_tps(host, port, password) {
            Ok(r) => { last = Some(r); }
            Err(e) => { eprintln!("  [tps] sample {} failed: {}", i, e); }
        }
        if i + 1 < samples { std::thread::sleep(Duration::from_millis(interval_ms)); }
    }
    last.ok_or_else(|| eyre::eyre!("No TPS samples collected"))
}
