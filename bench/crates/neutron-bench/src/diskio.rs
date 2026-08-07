//! Disk I/O metrics measurement.
//!
//! Measures sequential read/write speeds and IOPS.

use eyre::Result;
use std::fs::{self, File};
use std::io::{Read, Write, BufWriter, BufReader};
use std::path::Path;
use std::time::Instant;

/// Disk I/O benchmark result.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DiskIoResult {
    /// Sequential write speed in MB/s.
    pub write_mb_s: f64,
    /// Sequential read speed in MB/s.
    pub read_mb_s: f64,
    /// Write IOPS (small 4K blocks).
    pub write_iops: f64,
    /// Read IOPS (small 4K blocks).
    pub read_iops: f64,
}

/// Run a disk I/O benchmark in the given directory.
///
/// Writes a 64MB file, reads it back, then tests IOPS with 4K blocks.
pub fn benchmark(dir: &Path) -> Result<DiskIoResult> {
    let test_file = dir.join("bench_diskio_test.dat");
    let size_mb = 64;
    let block_size = 4096; // 4K for IOPS test
    let large_block = 1024 * 1024; // 1M for sequential test

    // Sequential write
    let data = vec![0xABu8; large_block];
    let start = Instant::now();
    {
        let mut file = BufWriter::new(File::create(&test_file)?);
        let mut written = 0usize;
        while written < size_mb * 1024 * 1024 {
            file.write_all(&data)?;
            written += large_block;
        }
        file.flush()?;
    }
    let write_duration = start.elapsed();
    let write_mb_s = (size_mb as f64) / write_duration.as_secs_f64();

    // Sequential read
    let start = Instant::now();
    {
        let mut file = BufReader::new(File::open(&test_file)?);
        let mut buf = vec![0u8; large_block];
        loop {
            match file.read(&mut buf) {
                Ok(0) => break,
                Ok(_) => {}
                Err(_) => break,
            }
        }
    }
    let read_duration = start.elapsed();
    let read_mb_s = (size_mb as f64) / read_duration.as_secs_f64();

    // Write IOPS (4K random-ish)
    let small_data = vec![0xCDu8; block_size];
    let iops_count = 1000;
    let start = Instant::now();
    {
        let mut file = BufWriter::new(File::create(&test_file)?);
        for _ in 0..iops_count {
            file.write_all(&small_data)?;
        }
        file.flush()?;
    }
    let write_iops_duration = start.elapsed();
    let write_iops = iops_count as f64 / write_iops_duration.as_secs_f64();

    // Read IOPS (4K)
    let start = Instant::now();
    {
        let mut file = BufReader::new(File::open(&test_file)?);
        let mut buf = vec![0u8; block_size];
        for _ in 0..iops_count {
            let _ = file.read(&mut buf);
        }
    }
    let read_iops_duration = start.elapsed();
    let read_iops = iops_count as f64 / read_iops_duration.as_secs_f64();

    // Cleanup
    let _ = fs::remove_file(&test_file);

    Ok(DiskIoResult {
        write_mb_s,
        read_mb_s,
        write_iops,
        read_iops,
    })
}
