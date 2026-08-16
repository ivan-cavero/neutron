// Copyright (c) 2026 Neutron Contributors — MIT License
//
// Compare two vanilla-hash JSON files and report differences.

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use serde::Deserialize;

/// A single chunk entry from a vanilla-hash JSON file.
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct ChunkEntry {
    pub region_x: i32,
    pub region_z: i32,
    pub chunk_x: i32,
    pub chunk_z: i32,
    pub hash: String,
    pub size_bytes: usize,
}

/// The top-level vanilla-hash JSON structure.
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct GoldenData {
    pub seed: i64,
    pub server: String,
    pub version: String,
    pub generated_at: String,
    #[serde(default)]
    pub hash_mode: Option<String>,
    pub chunks: Vec<ChunkEntry>,
    pub total_chunks: usize,
}

/// Comparison result for a single chunk.
#[derive(Debug)]
pub enum ChunkComparison {
    Match,
    Different {
        left_hash: String,
        right_hash: String,
    },
}

/// Full comparison report.
#[derive(Debug)]
pub struct ComparisonReport {
    pub matching: usize,
    pub different: usize,
    pub missing_in_right: usize,
    pub missing_in_left: usize,
    pub details: Vec<ChunkComparisonDetail>,
}

#[derive(Debug)]
pub struct ChunkComparisonDetail {
    pub region_x: i32,
    pub region_z: i32,
    pub chunk_x: i32,
    pub chunk_z: i32,
    pub result: ChunkComparison,
}

/// Load golden data from a JSON file.
pub fn load_golden_data(path: &Path) -> Result<GoldenData> {
    let content =
        fs::read_to_string(path).with_context(|| format!("Failed to read {}", path.display()))?;
    let data: GoldenData = serde_json::from_str(&content)
        .with_context(|| format!("Failed to parse {}", path.display()))?;
    Ok(data)
}

/// Compare two vanilla-hash files and return a report.
pub fn compare(left: &GoldenData, right: &GoldenData) -> ComparisonReport {
    // Build lookup maps: (region_x, region_z, chunk_x, chunk_z) -> hash
    let left_map: HashMap<(i32, i32, i32, i32), &ChunkEntry> = left
        .chunks
        .iter()
        .map(|c| ((c.region_x, c.region_z, c.chunk_x, c.chunk_z), c))
        .collect();

    let right_map: HashMap<(i32, i32, i32, i32), &ChunkEntry> = right
        .chunks
        .iter()
        .map(|c| ((c.region_x, c.region_z, c.chunk_x, c.chunk_z), c))
        .collect();

    let mut matching = 0;
    let mut different = 0;
    let mut missing_in_right = 0;
    let mut missing_in_left = 0;
    let mut details = Vec::new();

    // Check all chunks in left
    for (key, left_entry) in &left_map {
        match right_map.get(key) {
            Some(right_entry) => {
                if left_entry.hash == right_entry.hash {
                    matching += 1;
                    details.push(ChunkComparisonDetail {
                        region_x: key.0,
                        region_z: key.1,
                        chunk_x: key.2,
                        chunk_z: key.3,
                        result: ChunkComparison::Match,
                    });
                } else {
                    different += 1;
                    details.push(ChunkComparisonDetail {
                        region_x: key.0,
                        region_z: key.1,
                        chunk_x: key.2,
                        chunk_z: key.3,
                        result: ChunkComparison::Different {
                            left_hash: left_entry.hash.clone(),
                            right_hash: right_entry.hash.clone(),
                        },
                    });
                }
            }
            None => {
                missing_in_right += 1;
                details.push(ChunkComparisonDetail {
                    region_x: key.0,
                    region_z: key.1,
                    chunk_x: key.2,
                    chunk_z: key.3,
                    result: ChunkComparison::Different {
                        left_hash: left_entry.hash.clone(),
                        right_hash: "<missing>".to_string(),
                    },
                });
            }
        }
    }

    // Check for chunks in right but not in left
    for (key, right_entry) in &right_map {
        if !left_map.contains_key(key) {
            missing_in_left += 1;
            details.push(ChunkComparisonDetail {
                region_x: key.0,
                region_z: key.1,
                chunk_x: key.2,
                chunk_z: key.3,
                result: ChunkComparison::Different {
                    left_hash: "<missing>".to_string(),
                    right_hash: right_entry.hash.clone(),
                },
            });
        }
    }

    ComparisonReport {
        matching,
        different,
        missing_in_right,
        missing_in_left,
        details,
    }
}

/// Print the comparison report to stdout.
pub fn print_report(report: &ComparisonReport) {
    println!("=== Golden Data Comparison ===");
    println!("Matching chunks:      {}", report.matching);
    println!("Different chunks:     {}", report.different);
    println!("Missing in right:     {}", report.missing_in_right);
    println!("Missing in left:      {}", report.missing_in_left);

    if report.different > 0 || report.missing_in_right > 0 || report.missing_in_left > 0 {
        println!("\n--- Differences ---");
        for detail in &report.details {
            match &detail.result {
                ChunkComparison::Match => {}
                ChunkComparison::Different {
                    left_hash,
                    right_hash,
                } => {
                    println!(
                        "  region({}, {}) chunk({}, {}): left={} right={}",
                        detail.region_x,
                        detail.region_z,
                        detail.chunk_x,
                        detail.chunk_z,
                        left_hash,
                        right_hash,
                    );
                }
            }
        }
    }
}
