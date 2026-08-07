//! Bot-side metrics collection.

use std::time::Instant;

/// Snapshot of bot metrics at a point in time.
#[derive(Debug, Clone)]
pub struct BotMetrics {
    pub timestamp: Instant,
    pub chunks_received: usize,
    pub position: Option<(f64, f64, f64)>,
    pub ticks_alive: u64,
}

impl BotMetrics {
    pub fn new() -> Self {
        Self {
            timestamp: Instant::now(),
            chunks_received: 0,
            position: None,
            ticks_alive: 0,
        }
    }
}

/// Compute p50/p95/p99 percentiles from a sorted slice of f64 values.
pub fn percentiles(sorted: &[f64]) -> (f64, f64, f64) {
    if sorted.is_empty() {
        return (0.0, 0.0, 0.0);
    }

    let p50 = percentile(sorted, 0.50);
    let p95 = percentile(sorted, 0.95);
    let p99 = percentile(sorted, 0.99);

    (p50, p95, p99)
}

/// Compute a single percentile using linear interpolation.
fn percentile(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    if sorted.len() == 1 {
        return sorted[0];
    }

    let index = p * (sorted.len() - 1) as f64;
    let lower = index.floor() as usize;
    let upper = index.ceil() as usize;

    if lower == upper {
        sorted[lower]
    } else {
        let weight = index - lower as f64;
        sorted[lower] * (1.0 - weight) + sorted[upper] * weight
    }
}

/// Compute average of a slice of f64 values.
pub fn average(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    values.iter().sum::<f64>() / values.len() as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_percentiles() {
        let data = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0];
        let (p50, p95, p99) = percentiles(&data);
        assert!((p50 - 5.5).abs() < 0.01);
        assert!((p95 - 9.55).abs() < 0.01);
        assert!((p99 - 9.91).abs() < 0.01);
    }

    #[test]
    fn test_average() {
        let data = vec![10.0, 20.0, 30.0];
        assert!((average(&data) - 20.0).abs() < 0.01);
    }

    #[test]
    fn test_empty() {
        let data: Vec<f64> = vec![];
        let (p50, p95, p99) = percentiles(&data);
        assert_eq!(p50, 0.0);
        assert_eq!(p95, 0.0);
        assert_eq!(p99, 0.0);
    }
}
