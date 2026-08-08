//! TPS measurement via RCON.
//!
//! Sends `spark tps` or `tps` command via RCON and parses the response.
//! Works with Paper/Folia which bundle spark.

use eyre::Result;
use std::time::Duration;

/// TPS measurement result.
#[derive(Debug, Clone)]
pub struct TpsResult {
    /// 1-minute average TPS.
    pub tps_1m: f64,
    /// 5-minute average TPS.
    pub tps_5m: f64,
    /// 15-minute average TPS.
    pub tps_15m: f64,
    /// Average MSPT (ms per tick).
    pub mspt_avg: f64,
}

/// Query TPS via RCON.
///
/// Sends `spark tps` command and parses the response.
/// Paper format: "TPS from last 1m, 5m, 15m: *20.0, *20.0, *20.0"
/// Or: "TPS from last 1m, 5m, 15m: 20.0, 20.0, 20.0"
pub fn query_tps_rcon(host: &str, port: u16, password: &str) -> Result<TpsResult> {
    let mut client = crate::rcon::RconClient::connect(host, port, password)?;

    // Try spark tps first, then fallback to plain tps
    let response = match client.execute("spark tps") {
        Ok(r) if !r.is_empty() && r.contains("TPS") => r,
        _ => match client.execute("tps") {
            Ok(r) if !r.is_empty() && r.contains("TPS") => r,
            _ => {
                // Try spark health for MSPT
                match client.execute("spark health") {
                    Ok(r) if !r.is_empty() => r,
                    _ => eyre::bail!("Could not get TPS via RCON"),
                }
            }
        }
    };

    parse_tps_response(&response)
}

/// Parse TPS response from spark/tps command.
///
/// Handles:
/// - Minecraft color codes (§a, §6, §r, etc.)
/// - European number format (comma as decimal: 20,1)
/// - Star prefix (*20.0)
/// - Multiple line formats
fn parse_tps_response(response: &str) -> Result<TpsResult> {
    // Strip ALL Minecraft color codes (§x where x is any character)
    // Handles: §a, §6, §r, §x§4§f§a§4§f§0 (hex colors), etc.
    let clean: String = {
        let mut result = String::new();
        let chars: Vec<char> = response.chars().collect();
        let mut i = 0;
        while i < chars.len() {
            if chars[i] == '§' {
                // Skip the § and the next character (the color code)
                i += 2;
            } else {
                result.push(chars[i]);
                i += 1;
            }
        }
        result
    };

    // Try Paper format: "TPS from last 1m, 5m, 15m: 20,1, 20,1, 20,1"
    for line in clean.lines() {
        if line.contains("1m, 5m, 15m:") {
            if let Some(pos) = line.find("15m:") {
                let values_str = &line[pos + 4..];
                let parts: Vec<&str> = values_str.split(", ").collect();

                if parts.len() >= 3 {
                    let parse_value = |s: &str| -> f64 {
                        let s = s.trim().trim_start_matches('*').trim();
                        if let Ok(f) = s.parse::<f64>() {
                            return f;
                        }
                        let s = s.replace(',', ".");
                        s.parse::<f64>().unwrap_or(20.0)
                    };

                    let tps_1m = parse_value(parts[parts.len() - 3]).min(20.0);
                    let tps_5m = parse_value(parts[parts.len() - 2]).min(20.0);
                    let tps_15m = parse_value(parts[parts.len() - 1]).min(20.0);

                    if tps_1m > 0.0 && tps_1m <= 20.0 {
                        return Ok(TpsResult {
                            tps_1m,
                            tps_5m,
                            tps_15m,
                            mspt_avg: 1000.0 / tps_1m,
                        });
                    }
                }
            }
        }
    }

    // Try Folia format: "Median Region TPS: 20,00"
    let mut tps_values = Vec::new();
    for line in clean.lines() {
        if line.contains("Region TPS:") {
            if let Some(pos) = line.find("TPS:") {
                let val_str = &line[pos + 4..].trim();
                let val_str = val_str.replace(',', ".");
                if let Ok(f) = val_str.parse::<f64>() {
                    tps_values.push(f.min(20.0));
                }
            }
        }
    }

    if !tps_values.is_empty() {
        // Use median as TPS 1m, and same for 5m/15m (Folia doesn't distinguish)
        let tps_1m = tps_values.iter().copied().sum::<f64>() / tps_values.len() as f64;
        return Ok(TpsResult {
            tps_1m,
            tps_5m: tps_1m,
            tps_15m: tps_1m,
            mspt_avg: if tps_1m > 0.0 { 1000.0 / tps_1m } else { 50.0 },
        });
    }

    eyre::bail!("Could not parse TPS from response: {}", response)
}

/// Query TPS multiple times and return the last result.
pub fn query_tps_stable(
    host: &str,
    port: u16,
    password: &str,
    samples: u32,
    interval_ms: u64,
) -> Result<TpsResult> {
    let mut last_result = None;

    for i in 0..samples {
        match query_tps_rcon(host, port, password) {
            Ok(r) => {
                last_result = Some(r);
            }
            Err(e) => {
                eprintln!("  [tps] sample {} failed: {}", i, e);
            }
        }
        if i + 1 < samples {
            std::thread::sleep(Duration::from_millis(interval_ms));
        }
    }

    last_result.ok_or_else(|| eyre::eyre!("No TPS samples collected"))
}
