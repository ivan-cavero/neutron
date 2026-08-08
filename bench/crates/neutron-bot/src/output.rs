//! Output structures for all benchmark scenarios.

use serde::{Deserialize, Serialize};

// ── Join Storm ──────────────────────────────────────────────────────────────

/// Result of the join-storm scenario.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JoinStormResult {
    pub total_bots: usize,
    pub successful: usize,
    pub failed: usize,
    /// Per-bot join latencies in milliseconds (sorted ascending).
    pub latencies: Vec<f64>,
    /// Details of failed bots.
    pub failure_details: Vec<(usize, String)>,
}

impl JoinStormResult {
    /// Compute percentiles from the latency data.
    pub fn percentiles(&self) -> (f64, f64, f64) {
        crate::metrics::percentiles(&self.latencies)
    }

    /// Average join latency in ms.
    pub fn average_ms(&self) -> f64 {
        crate::metrics::average(&self.latencies)
    }
}

// ── Distributed Join ────────────────────────────────────────────────────────

/// Per-bot entry in the distributed join result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DistributedBotEntry {
    pub index: usize,
    pub join_latency_ms: f64,
    pub queue_time_ms: f64,
    pub success: bool,
}

/// Result of the distributed-join scenario.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DistributedResult {
    pub total_bots: usize,
    pub launched: usize,
    pub successful: usize,
    pub failed: usize,
    pub per_bot: Vec<DistributedBotEntry>,
    pub failure_details: Vec<(usize, String)>,
    /// All successful join latencies (sorted ascending).
    pub latencies: Vec<f64>,
}

impl DistributedResult {
    pub fn percentiles(&self) -> (f64, f64, f64) {
        crate::metrics::percentiles(&self.latencies)
    }
}

// ── Movement ────────────────────────────────────────────────────────────────

/// Chunk count for a single bot during movement.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkCount {
    pub bot_index: usize,
    pub chunks_received: usize,
    pub phase: String,
}

/// Result of the movement scenario.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MovementResult {
    pub total_bots: usize,
    pub successful: usize,
    pub failed: usize,
    pub join_latencies: Vec<f64>,
    pub movement_duration_ms: f64,
    pub chunks_per_bot: Vec<ChunkCount>,
    /// Total chunks received by all bots during movement.
    pub chunks_total: usize,
    /// Total ticks alive across all bots.
    pub ticks_total: usize,
    pub failure_details: Vec<(usize, String)>,
}

/// Result of the sustained load scenario.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SustainedLoadResult {
    pub total_bots: usize,
    pub successful: usize,
    pub failed: usize,
    pub join_latencies: Vec<f64>,
    pub duration_secs: f64,
    pub ticks_total: usize,
    pub failure_details: Vec<(usize, String)>,
}

/// Result of the stress test scenario.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StressTestResult {
    pub total_bots: usize,
    pub successful: usize,
    pub failed: usize,
    pub join_latencies: Vec<f64>,
    pub duration_secs: f64,
    pub total_chunks: usize,
    pub cps: f64,
    pub ticks_total: usize,
    pub failure_details: Vec<(usize, String)>,
}

// ── Spread ──────────────────────────────────────────────────────────────────

/// Chunk load info for a single bot after teleporting.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpreadChunkLoad {
    pub bot_index: usize,
    pub target_x: f64,
    pub target_y: f64,
    pub target_z: f64,
    pub chunks_loaded: usize,
}

/// Result of the spread scenario.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpreadResult {
    pub total_bots: usize,
    pub successful: usize,
    pub failed: usize,
    pub join_latencies: Vec<f64>,
    pub spread_duration_ms: f64,
    pub chunk_loads: Vec<SpreadChunkLoad>,
    /// Total chunks received by all bots during spread.
    pub chunks_total: usize,
    pub failure_details: Vec<(usize, String)>,
}

// ── Chunk Generation ────────────────────────────────────────────────────────

/// Per-bot chunk generation stats.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BotChunkStats {
    pub bot_index: usize,
    pub chunks_received: usize,
    pub distance_blocks: f64,
    pub duration_secs: f64,
}

/// Result of the chunk generation scenario.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkGenResult {
    pub total_bots: usize,
    pub successful: usize,
    pub failed: usize,
    pub join_latencies: Vec<f64>,
    pub walk_duration_secs: f64,
    pub total_chunks: usize,
    pub cps_total: f64,
    /// CPS per bot (total_cps / successful_bots).
    pub cps_per_bot: f64,
    pub per_bot: Vec<BotChunkStats>,
    pub failure_details: Vec<(usize, String)>,
}
