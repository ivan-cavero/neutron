//! Multi-chunk block buffer for features that cross chunk borders.
//!
//! Mirrors vanilla `WorldGenRegion`: a square of columns so ore blobs,
//! trees and sculk can write into neighbours during decoration.
//!
//! Copyright (c) 2026 Neutron Contributors -- MIT License

use crate::generator::{CHUNK_BLOCK_VOLUME, HEIGHTMAP_SIZE, WORLD_BOTTOM, WORLD_TOP};
use crate::surface::BlockId;

/// A square of chunks held as one dense block array for feature writes.
#[derive(Clone)]
pub struct RegionBuf {
    /// World-space min block X/Z of the region (inclusive).
    pub origin_x: i32,
    pub origin_z: i32,
    /// Side length in blocks (multiple of 16).
    pub side: i32,
    /// Index = ((y - WORLD_BOTTOM) * side + (z - origin_z)) * side + (x - origin_x)
    pub blocks: Vec<u16>,
    /// Per-chunk heightmaps, row-major over chunk grid (cz_local * n + cx_local).
    pub heightmaps: Vec<Vec<i16>>,
    /// Per-chunk quart biome grids (layout `section*64 + sy4*16 + bz4*4 + bx4`),
    /// populated by [`RegionBuf::put_chunk_biomes`]. Lets feature steps read
    /// stored biomes instead of re-sampling climate noise.
    pub biomes: Vec<Option<Vec<u8>>>,
    /// Chunks on a side (side / 16).
    pub chunks: i32,
    /// Vanilla `WorldGenRegion.random`: ONE xoroshiro stream per decorated
    /// origin pass, seeded at the origin's min block corner through the
    /// `minecraft:worldgen_region_random` positional factory (WorldGenRegion
    /// ctor → RandomState.getOrCreateRandomFactory). Set at the start of every
    /// `apply_step_origin` pass; consumed by `level.getRandom()` callers —
    /// today only MossyCarpetBlock.placeAt topper dice.
    pub(crate) region_random:
        std::cell::RefCell<Option<crate::rng::Xoroshiro128>>,
    /// Writer-attribution plane, parallel to `blocks`: id of the feature that
    /// last wrote each cell. Allocated only when NEUTRON_WRITERS=1 at
    /// RegionBuf construction (zero cost otherwise). See [`crate::writers`].
    pub writers: Option<Vec<u16>>,
    /// Id of the feature/stage currently running; stamped by drivers and by
    /// dispatch_configured. Default TERRAIN.
    pub current_writer: u16,
    /// Write buffer for decoration: accumulates writes during a step,
    /// applied atomically at step boundaries. Fixes scene-dependent
    /// feature acceptance (e.g., pale garden trees).
    write_buffer: std::collections::HashMap<(i32, i32, i32), u16>,
    /// Current decoration step (0-10). Flushed on advance.
    decoration_step: u8,
}

impl RegionBuf {
    pub fn new(center_cx: i32, center_cz: i32, radius: i32) -> Self {
        let chunks = 2 * radius + 1;
        let side = chunks * 16;
        let origin_x = (center_cx - radius) * 16;
        let origin_z = (center_cz - radius) * 16;
        let volume = (side as usize) * ((WORLD_TOP - WORLD_BOTTOM) as usize) * (side as usize);
        // Read once per buffer: attribution is a process-level opt-in.
        static WRITERS_ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        let attribution =
            *WRITERS_ON.get_or_init(|| std::env::var_os("NEUTRON_WRITERS").is_some());
        Self {
            origin_x,
            origin_z,
            side,
            blocks: vec![BlockId::Air.as_u16(); volume],
            heightmaps: vec![vec![WORLD_BOTTOM as i16; HEIGHTMAP_SIZE]; (chunks * chunks) as usize],
            biomes: vec![None; (chunks * chunks) as usize],
            chunks,
            region_random: std::cell::RefCell::new(None),
            writers: attribution.then(|| vec![crate::writers::TERRAIN; volume]),
            current_writer: crate::writers::TERRAIN,
            write_buffer: std::collections::HashMap::new(),
            decoration_step: 0,
        }
    }

    /// Install the per-pass region random (called once per origin pass).
    pub(crate) fn set_region_random(&mut self, rng: crate::rng::Xoroshiro128) {
        *self.region_random.borrow_mut() = Some(rng);
    }

    /// Run `f` with the live region random, if installed. Returns None when
    /// no pass installed one (feature paths outside decoration).
    pub(crate) fn with_region_random<R>(
        &self,
        f: impl FnOnce(&mut crate::rng::Xoroshiro128) -> R,
    ) -> Option<R> {
        let mut r = self.region_random.borrow_mut();
        r.as_mut().map(f)
    }

    #[inline]
    pub fn index(&self, x: i32, y: i32, z: i32) -> Option<usize> {
        if y < WORLD_BOTTOM || y >= WORLD_TOP {
            return None;
        }
        let lx = x - self.origin_x;
        let lz = z - self.origin_z;
        if lx < 0 || lz < 0 || lx >= self.side || lz >= self.side {
            return None;
        }
        Some(
            ((y - WORLD_BOTTOM) as usize) * (self.side as usize) * (self.side as usize)
                + (lz as usize) * (self.side as usize)
                + (lx as usize),
        )
    }

        pub fn get(&self, x: i32, y: i32, z: i32) -> BlockId {
            match self.index(x, y, z) {
                Some(i) => BlockId::from_u16(self.blocks[i]).unwrap_or(BlockId::Air),
                None => BlockId::Air,
            }
        }

        /// Read block with write-buffer overlay. Used by predicates
        /// (would_survive, matching_block_tag) to see pending writes
        /// from the same decoration step.
        pub fn get_buffered(&self, x: i32, y: i32, z: i32) -> BlockId {
            if let Some(&block_u16) = self.write_buffer.get(&(x, y, z)) {
                return BlockId::from_u16(block_u16).unwrap_or(BlockId::Air);
            }
            self.get(x, y, z)
        }

        /// Buffer a write during decoration (not applied immediately).
        /// Use for features that should not affect later features' acceptance
        /// decisions until the step boundary.
        pub fn buffer_write(&mut self, x: i32, y: i32, z: i32, b: BlockId) {
            let idx = self.index(x, y, z);
            self.write_buffer.insert((x, y, z), b.as_u16());
            if let (Some(w), Some(i)) = (&mut self.writers, idx) {
                w[i] = self.current_writer;
            }
        }

        /// Flush write buffer to main storage (at step boundary).
        pub fn flush_buffer(&mut self) {
            let entries: Vec<_> = self.write_buffer.drain().collect();
            for ((x, y, z), block_u16) in entries {
                if let Some(i) = self.index(x, y, z) {
                    self.blocks[i] = block_u16;
                }
            }
        }

        /// Advance decoration step (triggers flush when step increases).
        pub fn advance_step(&mut self, new_step: u8) {
            if new_step > self.decoration_step {
                self.flush_buffer();
                self.decoration_step = new_step;
            }
        }

        /// Get current decoration step.
        pub fn current_step(&self) -> u8 {
            self.decoration_step
        }

    pub fn set(&mut self, x: i32, y: i32, z: i32, b: BlockId) {
        if let Some(i) = self.index(x, y, z) {
            self.blocks[i] = b.as_u16();
            if let Some(w) = &mut self.writers {
                w[i] = self.current_writer;
            }
        }
        if crate::sculk::SET_TRACE.load(std::sync::atomic::Ordering::Relaxed) {
            eprintln!("W {x},{y},{z} {}", b.block_name());
        }
        // NEUTRON_SET_LOG=1: stream every feature write (writer-id tagged) for
        // two-sided diffing against the Java probes' PROBE_WRITE_LOG.
        static SET_LOG_ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        if *SET_LOG_ENABLED.get_or_init(|| std::env::var_os("NEUTRON_SET_LOG").is_some()) {
            eprintln!(
                "NSET {}|{}|{}|{}|{}|{}|{}",
                x, y, z, b.block_name(), self.current_writer, self.origin_x, self.origin_z
            );
        }
    }

    /// Extract the writer plane for one chunk column (same layout as blocks).
    /// Returns None when attribution is disabled.
    pub fn take_chunk_writers(&self, cx: i32, cz: i32) -> Option<Vec<u16>> {
        let plane = self.writers.as_ref()?;
        let base_x = cx * 16;
        let base_z = cz * 16;
        let mut out = vec![crate::writers::TERRAIN; CHUNK_BLOCK_VOLUME];
        for y in WORLD_BOTTOM..WORLD_TOP {
            for z in 0..16i32 {
                for x in 0..16i32 {
                    if let Some(i) = self.index(base_x + x, y, base_z + z) {
                        let dst =
                            ((y - WORLD_BOTTOM) as usize) * 256 + (z as usize) * 16 + x as usize;
                        out[dst] = plane[i];
                    }
                }
            }
        }
        Some(out)
    }

    /// Store a chunk's quart biome grid (see `generate_noise_and_surface`).
    pub fn put_chunk_biomes(&mut self, cx: i32, cz: i32, biomes: &[u8]) {
        let cxl = (cx * 16 - self.origin_x) / 16;
        let czl = (cz * 16 - self.origin_z) / 16;
        if cxl < 0 || czl < 0 || cxl >= self.chunks || czl >= self.chunks {
            return;
        }
        let hi = (czl * self.chunks + cxl) as usize;
        if hi < self.biomes.len() && biomes.len() == 4 * 4 * 4 * 24 {
            self.biomes[hi] = Some(biomes.to_vec());
        }
    }

    /// Stored noise-biome id at quart coords, from the grid written by
    /// [`RegionBuf::put_chunk_biomes`] (`None` when out of buffer / unstored).
    pub fn stored_noise_biome(&self, quart_x: i32, quart_y: i32, quart_z: i32) -> Option<u8> {
        // Chunk holding this quart.
        let cx = quart_x.div_euclid(4);
        let cz = quart_z.div_euclid(4);
        let bx4 = quart_x.rem_euclid(4);
        let bz4 = quart_z.rem_euclid(4);
        let cxl = cx - self.origin_x / 16;
        let czl = cz - self.origin_z / 16;
        if cxl < 0 || czl < 0 || cxl >= self.chunks || czl >= self.chunks {
            return None;
        }
        let hi = (czl * self.chunks + cxl) as usize;
        let section = ((quart_y * 4 - WORLD_BOTTOM) >> 4) as usize; // (y_q*4 - bottom)/16
        if section >= 24 {
            return None;
        }
        let within = (quart_y * 4 - (WORLD_BOTTOM + (section as i32) * 16)) as usize;
        let sy4 = within >> 2;
        let idx = section * 64 + sy4 * 16 + bz4 as usize * 4 + bx4 as usize;
        self.biomes.get(hi)?.as_ref()?.get(idx).copied()
    }

    /// Copy a generated 16×H×16 chunk column into the region.
    pub fn put_chunk(&mut self, cx: i32, cz: i32, blocks: &[u16], heightmap: &[i16]) {
        let lx0 = cx * 16 - self.origin_x;
        let lz0 = cz * 16 - self.origin_z;
        if lx0 < 0 || lz0 < 0 || lx0 + 16 > self.side || lz0 + 16 > self.side {
            return;
        }
        for y in WORLD_BOTTOM..WORLD_TOP {
            for z in 0..16i32 {
                for x in 0..16i32 {
                    let src = ((y - WORLD_BOTTOM) as usize) * 256 + (z as usize) * 16 + x as usize;
                    let dx = lx0 + x;
                    let dz = lz0 + z;
                    let dst =
                        ((y - WORLD_BOTTOM) as usize) * (self.side as usize) * (self.side as usize)
                            + (dz as usize) * (self.side as usize)
                            + (dx as usize);
                    self.blocks[dst] = blocks[src];
                }
            }
        }
        let cxl = (cx * 16 - self.origin_x) / 16;
        let czl = (cz * 16 - self.origin_z) / 16;
        let hi = (czl * self.chunks + cxl) as usize;
        if hi < self.heightmaps.len() {
            self.heightmaps[hi].copy_from_slice(heightmap);
        }
    }

    /// Extract one chunk column from the region.
    pub fn take_chunk(&self, cx: i32, cz: i32) -> (Vec<u16>, Vec<i16>) {
        let mut blocks = vec![BlockId::Air.as_u16(); CHUNK_BLOCK_VOLUME];
        let mut heightmap = vec![WORLD_BOTTOM as i16; HEIGHTMAP_SIZE];
        let base_x = cx * 16;
        let base_z = cz * 16;
        for y in WORLD_BOTTOM..WORLD_TOP {
            for z in 0..16i32 {
                for x in 0..16i32 {
                    let b = self.get(base_x + x, y, base_z + z);
                    let dst = ((y - WORLD_BOTTOM) as usize) * 256 + (z as usize) * 16 + x as usize;
                    blocks[dst] = b.as_u16();
                }
            }
        }
        for z in 0..16i32 {
            for x in 0..16i32 {
                for y in (WORLD_BOTTOM..WORLD_TOP).rev() {
                    let b = self.get(base_x + x, y, base_z + z);
                    if !matches!(b, BlockId::Air | BlockId::CaveAir | BlockId::Water | BlockId::Lava) {
                        heightmap[(z as usize) * 16 + x as usize] = y as i16;
                        break;
                    }
                }
            }
        }
        (blocks, heightmap)
    }
}
