// Copyright (c) 2026 Neutron Contributors — MIT License
//
// Anvil .mca region file reading/writing.
//
// Format reference: https://minecraft.wiki/Region_file_format
//
// File layout:
//   Bytes 0..4096    - Chunk offset table (1024 entries, 4 bytes each)
//     3 bytes sector offset (from file start, in 4096-byte sectors)
//     1 byte  sector count
//   Bytes 4096..8192 - Timestamp table (1024 entries, 4 bytes each, Unix seconds)
//   Bytes 8192..     - Chunk data sectors (4096-byte aligned)
//     Each chunk payload:
//       4 bytes: length of compressed data (big-endian, includes the compression byte)
//       1 byte:  compression type (2 = zlib)
//       N bytes: compressed data

use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use flate2::read::ZlibDecoder;
use flate2::write::ZlibEncoder;
use flate2::Compression;

use crate::error::{WorldError, WorldResult};

/// Size of one sector in bytes (4 KiB).
pub const SECTOR_SIZE: usize = 4096;

/// Maximum number of chunks per region (32 x 32).
pub const CHUNKS_PER_REGION: usize = 1024;

/// Offset table occupies the first 4 KiB.
const OFFSET_TABLE_SIZE: usize = SECTOR_SIZE;

/// Timestamp table occupies the second 4 KiB.
const TIMESTAMP_TABLE_SIZE: usize = SECTOR_SIZE;

/// Compression type for zlib (Anvil standard).
const COMPRESSION_ZLIB: u8 = 2;

/// Maximum compressed chunk size (2 MiB, generous safety net).
const MAX_CHUNK_SIZE: usize = 2 * 1024 * 1024;

/// An in-memory representation of a single chunk entry stored in a region.
#[derive(Debug, Clone)]
struct ChunkEntry {
    /// Compressed chunk payload (length + compression type + data).
    data: Vec<u8>,
    /// Unix timestamp of last modification.
    timestamp: u32,
    /// Whether this entry has been modified since loading.
    dirty: bool,
}

/// A Minecraft Anvil region file containing up to 32x32 chunks.
///
/// Chunk coordinates within the region are expected to be in the range [0, 31].
/// The region coordinates (rx, rz) identify which region file to load.
pub struct Region {
    /// Region X coordinate.
    rx: i32,
    /// Region Z coordinate.
    rz: i32,
    /// Indexed by `(cx & 31) + (cz & 31) * 32`. `None` means empty.
    chunks: Vec<Option<ChunkEntry>>,
}

impl Region {
    /// Create an empty region at the given region coordinates.
    pub fn new(rx: i32, rz: i32) -> Self {
        Self {
            rx,
            rz,
            chunks: (0..CHUNKS_PER_REGION).map(|_| None).collect(),
        }
    }

    /// Open and parse an existing `.mca` region file from disk.
    pub fn open(path: &Path) -> WorldResult<Self> {
        let bytes = fs::read(path)?;
        Self::from_bytes(&bytes)
    }

    /// Parse a region from an in-memory byte buffer.
    pub fn from_bytes(bytes: &[u8]) -> WorldResult<Self> {
        let min_size = OFFSET_TABLE_SIZE + TIMESTAMP_TABLE_SIZE;
        if bytes.len() < min_size {
            return Err(WorldError::InvalidRegion {
                reason: format!(
                    "file too small: {} bytes (need at least {} for headers)",
                    bytes.len(),
                    min_size
                ),
            });
        }

        // --- Parse offset table (1024 x 4 bytes) ---
        let mut offsets = Vec::with_capacity(CHUNKS_PER_REGION);
        for i in 0..CHUNKS_PER_REGION {
            let base = i * 4;
            let sector_offset =
                u32::from_be_bytes([0, bytes[base], bytes[base + 1], bytes[base + 2]]);
            let sector_count = bytes[base + 3] as usize;
            offsets.push((sector_offset, sector_count));
        }

        // --- Parse timestamp table (1024 x 4 bytes) ---
        let ts_base = OFFSET_TABLE_SIZE;
        let mut timestamps = Vec::with_capacity(CHUNKS_PER_REGION);
        for i in 0..CHUNKS_PER_REGION {
            let base = ts_base + i * 4;
            timestamps.push(u32::from_be_bytes([
                bytes[base],
                bytes[base + 1],
                bytes[base + 2],
                bytes[base + 3],
            ]));
        }

        // --- Read chunk data ---
        let mut chunks: Vec<Option<ChunkEntry>> = Vec::with_capacity(CHUNKS_PER_REGION);
        for i in 0..CHUNKS_PER_REGION {
            let (sector_offset, sector_count) = offsets[i];
            if sector_offset == 0 || sector_count == 0 {
                chunks.push(None);
                continue;
            }

            let byte_offset = (sector_offset as usize) * SECTOR_SIZE;
            let byte_len = sector_count * SECTOR_SIZE;

            // Truncated region files (e.g. vanilla worlds interrupted mid-write)
            // can have a location-table entry past EOF. Skip those entries as
            // empty rather than rejecting the whole region — other chunks are
            // still usable for parity diagnostics.
            if byte_offset + byte_len > bytes.len() {
                tracing::warn!(
                    chunk = i,
                    byte_offset,
                    byte_len,
                    file_len = bytes.len(),
                    "region chunk location past EOF; treating as empty"
                );
                chunks.push(None);
                continue;
            }

            let chunk_bytes = &bytes[byte_offset..byte_offset + byte_len];
            // Strip the 4-byte length prefix; entry.data stores [compression_type][compressed_data].
            let data = if chunk_bytes.len() >= 5 {
                chunk_bytes[4..].to_vec()
            } else {
                chunk_bytes.to_vec()
            };
            chunks.push(Some(ChunkEntry {
                data,
                timestamp: timestamps[i],
                dirty: false,
            }));
        }

        tracing::debug!(
            rx = ?bytes.as_ptr(), // placeholder; we don't store rx/rz in the file
            "loaded region with {} chunks",
            chunks.iter().filter(|c| c.is_some()).count()
        );

        // Extract rx/rz from the filename if possible; default to 0.
        Ok(Self {
            rx: 0,
            rz: 0,
            chunks,
        })
    }

    /// Set the region coordinates (useful when creating from bytes).
    pub fn with_coords(mut self, rx: i32, rz: i32) -> Self {
        self.rx = rx;
        self.rz = rz;
        self
    }

    /// Region X coordinate.
    pub fn rx(&self) -> i32 {
        self.rx
    }

    /// Region Z coordinate.
    pub fn rz(&self) -> i32 {
        self.rz
    }

    /// Compute the flat index into the chunk array from local coordinates.
    ///
    /// Local coordinates must be in [0, 31].
    fn chunk_index(cx: i32, cz: i32) -> WorldResult<usize> {
        if cx < 0 || cx > 31 || cz < 0 || cz > 31 {
            return Err(WorldError::InvalidChunkCoords { cx, cz });
        }
        Ok(((cx & 31) + (cz & 31) * 32) as usize)
    }

    /// Get the decompressed chunk data at local coordinates (cx, cz) within this region.
    ///
    /// `cx` and `cz` must be in [0, 31].
    pub fn get_chunk(&self, cx: i32, cz: i32) -> WorldResult<Option<Vec<u8>>> {
        let idx = Self::chunk_index(cx, cz)?;
        match &self.chunks[idx] {
            None => Ok(None),
            Some(entry) => {
                let decompressed = Self::decompress_chunk(&entry.data)?;
                Ok(Some(decompressed))
            }
        }
    }

    /// Get the raw (still-compressed) chunk payload including the length + compression header.
    pub fn get_chunk_raw(&self, cx: i32, cz: i32) -> WorldResult<Option<&[u8]>> {
        let idx = Self::chunk_index(cx, cz)?;
        Ok(self.chunks[idx].as_ref().map(|e| e.data.as_slice()))
    }

    /// Get the timestamp for a chunk.
    pub fn get_timestamp(&self, cx: i32, cz: i32) -> WorldResult<Option<u32>> {
        let idx = Self::chunk_index(cx, cz)?;
        Ok(self.chunks[idx].as_ref().map(|e| e.timestamp))
    }

    /// Compress and store chunk data at local coordinates (cx, cz).
    ///
    /// `cx` and `cz` must be in [0, 31]. The data is expected to be uncompressed NBT chunk data.
    pub fn write_chunk(&mut self, cx: i32, cz: i32, data: &[u8]) -> WorldResult<()> {
        if data.len() > MAX_CHUNK_SIZE {
            return Err(WorldError::ChunkTooLarge {
                size: data.len(),
                max: MAX_CHUNK_SIZE,
            });
        }

        let compressed = Self::compress_chunk(data)?;
        let idx = Self::chunk_index(cx, cz)?;

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as u32;

        self.chunks[idx] = Some(ChunkEntry {
            data: compressed,
            timestamp: now,
            dirty: true,
        });

        Ok(())
    }

    /// Remove a chunk from the region.
    pub fn remove_chunk(&mut self, cx: i32, cz: i32) -> WorldResult<bool> {
        let idx = Self::chunk_index(cx, cz)?;
        Ok(self.chunks[idx].take().is_some())
    }

    /// Returns true if the region has any dirty (modified) chunks.
    pub fn is_dirty(&self) -> bool {
        self.chunks
            .iter()
            .any(|c| c.as_ref().is_some_and(|e| e.dirty))
    }

    /// Clear all dirty flags.
    pub fn clear_dirty(&mut self) {
        for entry in self.chunks.iter_mut() {
            if let Some(e) = entry {
                e.dirty = false;
            }
        }
    }

    /// Count of non-empty chunks.
    pub fn chunk_count(&self) -> usize {
        self.chunks.iter().filter(|c| c.is_some()).count()
    }

    /// Save the entire region to a file at the given path.
    ///
    /// This writes the full Anvil format: offset table, timestamp table, then sector-aligned chunk data.
    pub fn save(&self, path: &Path) -> WorldResult<()> {
        let bytes = self.to_bytes()?;
        // Write atomically: write to temp file then rename.
        let tmp_path = path.with_extension("mca.tmp");
        fs::write(&tmp_path, &bytes)?;
        fs::rename(&tmp_path, path)?;
        tracing::debug!(
            rx = self.rx,
            rz = self.rz,
            chunks = self.chunk_count(),
            "saved region to {}",
            path.display()
        );
        Ok(())
    }

    /// Serialize the region to the Anvil byte format.
    pub fn to_bytes(&self) -> WorldResult<Vec<u8>> {
        // First pass: compute sector offsets for each chunk.
        // Sectors start after the two header tables (2 * 4096 = 8192 bytes = 2 sectors).
        let mut sector_offset: Vec<u32> = vec![0; CHUNKS_PER_REGION];
        let mut sector_count: Vec<u8> = vec![0; CHUNKS_PER_REGION];
        let mut next_sector: u32 = 2; // first two sectors are headers

        for i in 0..CHUNKS_PER_REGION {
            if let Some(entry) = &self.chunks[i] {
                let data_len = entry.data.len();
                // Need: 4 bytes (length) + data_len, rounded up to sectors.
                let total_bytes = 4 + data_len;
                let sectors_needed = ((total_bytes + SECTOR_SIZE - 1) / SECTOR_SIZE) as u32;

                sector_offset[i] = next_sector;
                sector_count[i] = sectors_needed as u8;
                next_sector += sectors_needed;
            }
        }

        // Total file size.
        let total_size = next_sector as usize * SECTOR_SIZE;
        let mut buf = vec![0u8; total_size];

        // --- Write offset table (1024 x 4 bytes) ---
        for i in 0..CHUNKS_PER_REGION {
            let base = i * 4;
            let offset_bytes = sector_offset[i].to_be_bytes();
            // Only the lower 3 bytes are used (max value = 2^24 - 1 = 16,777,215 sectors = ~64 GiB).
            buf[base] = offset_bytes[1];
            buf[base + 1] = offset_bytes[2];
            buf[base + 2] = offset_bytes[3];
            buf[base + 3] = sector_count[i];
        }

        // --- Write timestamp table (1024 x 4 bytes) ---
        let ts_base = OFFSET_TABLE_SIZE;
        for i in 0..CHUNKS_PER_REGION {
            let base = ts_base + i * 4;
            let ts = self.chunks[i].as_ref().map_or(0u32, |e| e.timestamp);
            let ts_bytes = ts.to_be_bytes();
            buf[base..base + 4].copy_from_slice(&ts_bytes);
        }

        // --- Write chunk data sectors ---
        for i in 0..CHUNKS_PER_REGION {
            if let Some(entry) = &self.chunks[i] {
                let byte_offset = sector_offset[i] as usize * SECTOR_SIZE;
                let data_len = entry.data.len();

                // Write 4-byte big-endian length (compression type byte is part of the data).
                let len_bytes = (data_len as u32).to_be_bytes();
                buf[byte_offset..byte_offset + 4].copy_from_slice(&len_bytes);
                // Write chunk data (includes compression type byte + compressed payload).
                buf[byte_offset + 4..byte_offset + 4 + data_len].copy_from_slice(&entry.data);
            }
        }

        Ok(buf)
    }

    /// Decompress a chunk payload: `[compression_type_byte][compressed_data]`.
    fn decompress_chunk(raw: &[u8]) -> WorldResult<Vec<u8>> {
        if raw.is_empty() {
            return Err(WorldError::InvalidRegion {
                reason: "chunk payload too short for header".to_string(),
            });
        }

        let compression_type = raw[0];
        let compressed_data = &raw[1..];

        match compression_type {
            COMPRESSION_ZLIB => {
                let mut decoder = ZlibDecoder::new(compressed_data);
                let mut decompressed = Vec::new();
                decoder.read_to_end(&mut decompressed)?;
                Ok(decompressed)
            }
            other => Err(WorldError::InvalidCompression(other)),
        }
    }

    /// Compress chunk data with zlib. Returns `[compression_type_byte][compressed_data]`.
    ///
    /// The 4-byte length prefix is added by `to_bytes` during serialization.
    fn compress_chunk(data: &[u8]) -> WorldResult<Vec<u8>> {
        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(data)?;
        let compressed = encoder.finish()?;

        // Build payload: compression type byte + compressed data.
        let mut payload = Vec::with_capacity(1 + compressed.len());
        payload.push(COMPRESSION_ZLIB);
        payload.extend_from_slice(&compressed);

        Ok(payload)
    }
}

/// Compute the region file path from region coordinates.
///
/// Returns `directory/r.X.Z.mca` (the vanilla convention).
pub fn region_path(directory: &Path, rx: i32, rz: i32) -> PathBuf {
    directory.join(format!("r.{}.{}.mca", rx, rz))
}

/// Extract region coordinates from a `.mca` filename.
///
/// Returns `Some((rx, rz))` if the filename matches the pattern `r.<rx>.<rz>.mca`.
pub fn parse_region_filename(filename: &str) -> Option<(i32, i32)> {
    let name = filename.strip_suffix(".mca")?;
    let mut parts = name.split('.');
    let _prefix = parts.next()?; // "r"
    let rx: i32 = parts.next()?.parse().ok()?;
    let rz: i32 = parts.next()?.parse().ok()?;
    if parts.next().is_some() {
        return None; // too many parts
    }
    Some((rx, rz))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chunk_index_calculation() {
        // (0, 0) -> index 0
        assert_eq!(Region::chunk_index(0, 0).unwrap(), 0);
        // (1, 0) -> index 1
        assert_eq!(Region::chunk_index(1, 0).unwrap(), 1);
        // (0, 1) -> index 32
        assert_eq!(Region::chunk_index(0, 1).unwrap(), 32);
        // (31, 31) -> index 1023
        assert_eq!(Region::chunk_index(31, 31).unwrap(), 1023);
    }

    #[test]
    fn test_chunk_index_out_of_bounds() {
        assert!(Region::chunk_index(-1, 0).is_err());
        assert!(Region::chunk_index(0, -1).is_err());
        assert!(Region::chunk_index(32, 0).is_err());
        assert!(Region::chunk_index(0, 32).is_err());
    }

    #[test]
    fn test_region_path_construction() {
        let dir = Path::new("/world/region");
        let path = region_path(dir, -1, 2);
        assert_eq!(path, PathBuf::from("/world/region/r.-1.2.mca"));
    }

    #[test]
    fn test_parse_region_filename() {
        assert_eq!(parse_region_filename("r.0.0.mca"), Some((0, 0)));
        assert_eq!(parse_region_filename("r.-3.12.mca"), Some((-3, 12)));
        assert_eq!(parse_region_filename("r.0.0"), None);
        assert_eq!(parse_region_filename("foo.txt"), None);
    }

    #[test]
    fn test_compress_decompress_roundtrip() {
        let original = b"hello minecraft world data";
        let compressed = Region::compress_chunk(original).unwrap();
        let decompressed = Region::decompress_chunk(&compressed).unwrap();
        assert_eq!(original.as_slice(), decompressed.as_slice());
    }

    #[test]
    fn test_empty_region_roundtrip() {
        let region = Region::new(5, -3);
        let bytes = region.to_bytes().unwrap();
        // Minimum size: 2 header tables = 8192 bytes.
        assert_eq!(bytes.len(), 2 * SECTOR_SIZE);
    }

    #[test]
    fn test_write_and_get_chunk_roundtrip() {
        let mut region = Region::new(0, 0);
        let data = b"test chunk payload";
        region.write_chunk(5, 10, data).unwrap();

        let got = region.get_chunk(5, 10).unwrap();
        assert_eq!(got.as_deref(), Some(data.as_slice()));
    }

    #[test]
    fn test_get_empty_chunk() {
        let region = Region::new(0, 0);
        let got = region.get_chunk(0, 0).unwrap();
        assert!(got.is_none());
    }

    #[test]
    fn test_region_to_bytes_and_back() {
        let mut region = Region::new(1, 2);
        let data = b"another test chunk";
        region.write_chunk(0, 0, data).unwrap();
        region.write_chunk(31, 31, data).unwrap();

        let bytes = region.to_bytes().unwrap();
        let restored = Region::from_bytes(&bytes).unwrap();

        assert_eq!(
            restored.get_chunk(0, 0).unwrap().as_deref(),
            Some(data.as_slice())
        );
        assert_eq!(
            restored.get_chunk(31, 31).unwrap().as_deref(),
            Some(data.as_slice())
        );
        assert!(restored.get_chunk(1, 1).unwrap().is_none());
    }

    #[test]
    fn test_remove_chunk() {
        let mut region = Region::new(0, 0);
        region.write_chunk(5, 5, b"data").unwrap();
        assert!(region.get_chunk(5, 5).unwrap().is_some());

        let removed = region.remove_chunk(5, 5).unwrap();
        assert!(removed);
        assert!(region.get_chunk(5, 5).unwrap().is_none());
    }

    #[test]
    fn test_dirty_tracking() {
        let mut region = Region::new(0, 0);
        assert!(!region.is_dirty());

        region.write_chunk(1, 1, b"data").unwrap();
        assert!(region.is_dirty());

        region.clear_dirty();
        assert!(!region.is_dirty());
    }

    #[test]
    fn test_save_and_open_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = region_path(dir.path(), 3, -1);

        let mut region = Region::new(3, -1);
        region.write_chunk(4, 7, b"persistent data").unwrap();
        region.save(&path).unwrap();

        let loaded = Region::open(&path).unwrap();
        assert_eq!(
            loaded.get_chunk(4, 7).unwrap().as_deref(),
            Some(b"persistent data".as_slice())
        );
    }

    #[test]
    fn test_chunk_too_large() {
        let mut region = Region::new(0, 0);
        let huge = vec![0u8; MAX_CHUNK_SIZE + 1];
        assert!(region.write_chunk(0, 0, &huge).is_err());
    }

    #[test]
    fn test_offset_table_entry_format() {
        let region = Region::new(0, 0);
        let bytes = region.to_bytes().unwrap();

        // First chunk entry (index 0): offset and sector count should be 0.
        assert_eq!(bytes[0], 0); // offset high byte
        assert_eq!(bytes[1], 0); // offset mid byte
        assert_eq!(bytes[2], 0); // offset low byte
        assert_eq!(bytes[3], 0); // sector count
    }
}
