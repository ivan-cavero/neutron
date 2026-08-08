//! TPS measurement - 3 métodos:
//!
//! 1. **Spark TPS** (Paper/Folia): `spark tps` → TPS real
//! 2. **Time query** (todos): `/time query gametime` → ticks en 5s
//! 3. **Bot tick count** (todos): ticks_alive / wall_clock → TPS efectivo

use eyre::Result;
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct TpsResult {
    pub tps_1m: f64,
    pub tps_5m: f64,
    pub tps_15m: f64,
    pub mspt_avg: f64,
    pub method: String,
}

/// Query TPS using the best available method.
pub fn query_tps(host: &str, port: u16, password: &str) -> Result<TpsResult> {
    let mut client = crate::rcon::RconClient::connect(host, port, password)?;

    // Method 1: Spark TPS (Paper/Folia) - most accurate
    if let Ok(r) = client.execute("spark tps") {
        eprintln!("  [tps] spark tps response: {}", r.chars().take(100).collect::<String>());
        if r.contains("TPS") || r.contains("Region TPS") {
            if let Ok(result) = parse_spark_tps(&r) {
                return Ok(result);
            }
        }
    }

    // Method 2: Time query gametime (all servers)
    let r1 = client.execute("time query gametime");
    eprintln!("  [tps] time query response: {:?}", r1);
    if let Ok(r1) = r1 {
        if r1.contains("time is") {
            if let Some(t1) = parse_game_time(&r1) {
                eprintln!("  [tps] t1 = {}", t1);
                std::thread::sleep(Duration::from_secs(5));
                if let Ok(r2) = client.execute("time query gametime") {
                    eprintln!("  [tps] time query 2 response: {:?}", r2);
                    if let Some(t2) = parse_game_time(&r2) {
                        eprintln!("  [tps] t2 = {}", t2);
                        let ticks_passed = t2 - t1;
                        let tps = (ticks_passed as f64 / 5.0).min(20.0);
                        eprintln!("  [tps] ticks_passed = {}, tps = {}", ticks_passed, tps);
                        return Ok(TpsResult {
                            tps_1m: tps,
                            tps_5m: tps,
                            tps_15m: tps,
                            mspt_avg: if tps > 0.0 { 1000.0 / tps } else { 50.0 },
                            method: "time_query".to_string(),
                        });
                    }
                }
            }
        }
    }

    // Method 3: RCON response latency (fallback)
    let start = Instant::now();
    let _ = client.execute("list");
    let latency_ms = start.elapsed().as_secs_f64() * 1000.0;
    let tps = if latency_ms < 50.0 { 20.0 } else if latency_ms < 100.0 { 15.0 } else if latency_ms < 200.0 { 10.0 } else { 5.0 };

    Ok(TpsResult {
        tps_1m: tps, tps_5m: tps, tps_15m: tps,
        mspt_avg: if tps > 0.0 { 1000.0 / tps } else { 50.0 },
        method: "rcon_latency".to_string(),
    })
}

/// Estimate TPS from bot tick count (effective TPS from client perspective).
pub fn estimate_tps_from_ticks(ticks_total: usize, duration_secs: f64) -> f64 {
    if duration_secs > 0.0 {
        (ticks_total as f64 / duration_secs).min(20.0)
    } else {
        20.0
    }
}

fn parse_game_time(response: &str) -> Option<u64> {
    for line in response.lines() {
        // Handle "The game time is 109 tick(s)"
        if let Some(pos) = line.find("time is") {
            let rest = line[pos + 8..].trim();
            // Take only digits
            let num_str: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
            if let Ok(val) = num_str.parse::<u64>() {
                return Some(val);
            }
        }
    }
    None
}

fn parse_spark_tps(response: &str) -> Result<TpsResult> {
    let clean: String = {
        let mut result = String::new();
        let chars: Vec<char> = response.chars().collect();
        let mut i = 0;
        while i < chars.len() {
            if chars[i] == '§' { i += 2; } else { result.push(chars[i]); i += 1; }
        }
        result
    };

    // Paper format: "TPS from last 1m, 5m, 15m: 20,1, 20,1, 20,1"
    for line in clean.lines() {
        if line.contains("1m, 5m, 15m:") {
            if let Some(pos) = line.find("15m:") {
                let parts: Vec<&str> = line[pos + 4..].split(", ").collect();
                if parts.len() >= 3 {
                    let p = |s: &str| -> f64 {
                        let s = s.trim().trim_start_matches('*').trim();
                        s.parse::<f64>().unwrap_or_else(|_| s.replace(',', ".").parse::<f64>().unwrap_or(20.0)).min(20.0)
                    };
                    let t1 = p(parts[parts.len() - 3]);
                    let t2 = p(parts[parts.len() - 2]);
                    let t3 = p(parts[parts.len() - 1]);
                    if t1 > 0.0 && t1 <= 20.0 {
                        return Ok(TpsResult { tps_1m: t1, tps_5m: t2, tps_15m: t3, mspt_avg: 1000.0 / t1, method: "spark".to_string() });
                    }
                }
            }
        }
    }

    // Folia format: "Median Region TPS: 20,00"
    let mut vals = Vec::new();
    for line in clean.lines() {
        if line.contains("Region TPS:") {
            if let Some(pos) = line.find("TPS:") {
                if let Ok(f) = line[pos + 4..].trim().replace(',', ".").parse::<f64>() {
                    vals.push(f.min(20.0));
                }
            }
        }
    }
    if !vals.is_empty() {
        let avg = vals.iter().sum::<f64>() / vals.len() as f64;
        return Ok(TpsResult { tps_1m: avg, tps_5m: avg, tps_15m: avg, mspt_avg: 1000.0 / avg, method: "spark".to_string() });
    }

    eyre::bail!("Could not parse spark TPS")
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
