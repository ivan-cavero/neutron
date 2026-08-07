//! System metrics collection: RSS, CPU, peak tracking.

use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};
use sysinfo::System;

/// Snapshot of system metrics at a point in time.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricSnapshot {
    pub timestamp_ms: u64,
    pub rss_mb: f64,
    pub cpu_percent: f64,
}

/// Aggregated metrics from a full run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AggregatedMetrics {
    pub ram_idle_mb: f64,
    pub ram_peak_mb: f64,
    pub cpu_idle_pct: f64,
    pub cpu_peak_pct: f64,
    pub snapshots: Vec<MetricSnapshot>,
}

/// Background metrics sampler.
pub struct MetricsSampler {
    pid: sysinfo::Pid,
    system: System,
    snapshots: Vec<MetricSnapshot>,
    start_time: Instant,
    interval: Duration,
    peak_rss_mb: f64,
    peak_cpu_pct: f64,
}

impl MetricsSampler {
    /// Create a new sampler for a specific process.
    pub fn new(pid: sysinfo::Pid, interval: Duration) -> Self {
        let mut system = System::new();
        // Refresh all processes so we can find the target PID
        system.refresh_processes();
        system.refresh_memory();
        system.refresh_cpu_usage();

        Self {
            pid,
            system,
            snapshots: Vec::new(),
            start_time: Instant::now(),
            interval,
            peak_rss_mb: 0.0,
            peak_cpu_pct: 0.0,
        }
    }

    /// Create a new sampler for the current process.
    pub fn new_current(interval: Duration) -> Self {
        let pid = sysinfo::get_current_pid().expect("failed to get current PID");
        Self::new(pid, interval)
    }

    /// Sample metrics once.
    pub fn sample(&mut self) -> MetricSnapshot {
        // Refresh processes to get updated CPU/memory data
        self.system.refresh_processes();
        self.system.refresh_memory();
        self.system.refresh_cpu_usage();

        // memory() returns u64 (bytes) on all platforms in sysinfo 0.30
        let rss_mb = if let Some(process) = self.system.process(self.pid) {
            process.memory() as f64 / 1024.0 / 1024.0
        } else {
            0.0
        };

        // cpu_usage() returns f32 in sysinfo 0.30, reported per-core (max = num_cores * 100)
        // Normalize to 0-100% range
        let cpu_cores = self.system.cpus().len() as f64;
        let cpu_percent = if let Some(process) = self.system.process(self.pid) {
            let raw = process.cpu_usage() as f64;
            if cpu_cores > 0.0 { raw / cpu_cores } else { raw }
        } else {
            0.0
        };

        let snapshot = MetricSnapshot {
            timestamp_ms: self.start_time.elapsed().as_millis() as u64,
            rss_mb,
            cpu_percent,
        };

        if rss_mb > self.peak_rss_mb {
            self.peak_rss_mb = rss_mb;
        }
        if cpu_percent > self.peak_cpu_pct {
            self.peak_cpu_pct = cpu_percent;
        }

        self.snapshots.push(snapshot.clone());
        snapshot
    }

    /// Run sampling in a background task for the given duration.
    pub async fn sample_for_duration(self, duration: Duration) -> AggregatedMetrics {
        let mut sampler = self;
        let start = Instant::now();

        while start.elapsed() < duration {
            sampler.sample();
            tokio::time::sleep(sampler.interval).await;
        }

        // Final sample
        sampler.sample();

        // Compute idle metrics from first 3 samples
        let sample_count = sampler.snapshots.len().min(3);
        let idle_rss: f64 = sampler
            .snapshots
            .iter()
            .take(sample_count)
            .map(|s| s.rss_mb)
            .sum::<f64>()
            / sample_count as f64;

        let idle_cpu: f64 = sampler
            .snapshots
            .iter()
            .take(sample_count)
            .map(|s| s.cpu_percent)
            .sum::<f64>()
            / sample_count as f64;

        AggregatedMetrics {
            ram_idle_mb: idle_rss,
            ram_peak_mb: sampler.peak_rss_mb,
            cpu_idle_pct: idle_cpu,
            cpu_peak_pct: sampler.peak_cpu_pct,
            snapshots: sampler.snapshots,
        }
    }
}

/// Get hardware info for the current machine.
pub fn detect_hardware() -> HardwareInfo {
    let mut system = System::new();
    system.refresh_memory();
    system.refresh_cpu_usage();

    let os = System::long_os_version().unwrap_or_else(|| "Unknown".to_string());
    let cpu = system
        .cpus()
        .first()
        .map(|c| c.brand().to_string())
        .unwrap_or_else(|| "Unknown".to_string());
    let total_ram_gb = system.total_memory() as f64 / 1024.0 / 1024.0 / 1024.0;
    let cpu_cores = system.cpus().len() as f64;

    HardwareInfo {
        os,
        cpu,
        ram_gb: (total_ram_gb * 10.0).round() / 10.0,
        cpu_cores,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HardwareInfo {
    pub os: String,
    pub cpu: String,
    pub ram_gb: f64,
    pub cpu_cores: f64,
}
