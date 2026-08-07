pub mod join_storm;
pub mod distributed;
pub mod movement;
pub mod spread;
pub mod chunk_gen;

use std::time::Duration;

/// Common configuration for all benchmark scenarios.
#[derive(Debug, Clone)]
pub struct ScenarioConfig {
    pub host: String,
    pub port: u16,
    pub bot_count: usize,
    pub duration: Duration,
    pub seed: String,
}

impl ScenarioConfig {
    pub fn new(host: &str, port: u16, bot_count: usize, duration_secs: u64) -> Self {
        Self {
            host: host.to_string(),
            port,
            bot_count,
            duration: Duration::from_secs(duration_secs),
            seed: "1234567890123456789".to_string(),
        }
    }
}
