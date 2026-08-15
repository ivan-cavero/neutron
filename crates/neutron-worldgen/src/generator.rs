//! `NoiseBasedChunkGenerator.doFill` for Minecraft 26.2.
//!
//! Evaluation model (mirrors `NoiseChunk`):
//! - every `interpolated` marker in `finalDensity` is sampled on a 4×8 grid
//!   and trilinearly lerped per block
//! - nonlinear ops (squeeze, min, range_choice, …) run *outside* interpolators
//! - `Aquifer.computeSubstance` picks stone / fluid / air from final density
//!
//! The density tree is `Arc`, so [`ChunkGenerator`] is `Send`.
//!
//! Copyright (c) 2026 Neutron Contributors -- MIT License

use crate::aquifer::{GlobalFluidPicker, NoiseBasedAquifer};
use crate::carvers;
use crate::density::{
    collect_interpolated, compute, interpolated_wrapped, CellInterpRuntime, DensityEnv,
    MarkerState, DF,
};
use crate::features;
use crate::ore_vein;
use crate::positional::PositionalRandomFactory;
use crate::region_buf::RegionBuf;
use crate::surface::BlockId;
use crate::surface_rules;
use crate::worldgen::WorldgenState;

/// Blocks in one 16x16x16 section.
pub const SECTION_BLOCKS: usize = 16 * 16 * 16;
/// Sections per column (384 / 16).
pub const SECTIONS_PER_COLUMN: usize = 24;
/// Total blocks per chunk column.
pub const CHUNK_BLOCK_VOLUME: usize = 16 * 384 * 16;
/// Total biome entries per column (4 x 24 x 4).
pub const CHUNK_BIOME_VOLUME: usize = 4 * 24 * 4;
/// Heightmap size (16 x 16).
pub const HEIGHTMAP_SIZE: usize = 16 * 16;

pub const WORLD_BOTTOM: i32 = -64;
pub const WORLD_TOP: i32 = 320;

/// Pre-feature column: blocks + heightmap + biomes.
pub type NoiseColumn = (Vec<u16>, Vec<i16>, Vec<u8>);

/// FIFO cache of [`NoiseColumn`] keyed by chunk XZ.
///
/// Neighbour decoration reuses the expensive noise+surface fill. Oldest
/// columns are dropped when `cap` is exceeded (insertion order, not
/// HashMap iteration). `cap == usize::MAX` means unbounded (tests).
pub struct NoiseCache {
    map: std::collections::HashMap<(i32, i32), NoiseColumn>,
    order: std::collections::VecDeque<(i32, i32)>,
    cap: usize,
}

impl NoiseCache {
    /// Unbounded cache (parity tests / one-shot generate).
    pub fn new() -> Self {
        Self::with_cap(usize::MAX)
    }

    /// Cache that drops the oldest column once `cap` entries are stored.
    pub fn with_cap(cap: usize) -> Self {
        Self {
            map: std::collections::HashMap::new(),
            order: std::collections::VecDeque::new(),
            cap: cap.max(1),
        }
    }

    /// Number of cached columns.
    pub fn len(&self) -> usize {
        self.map.len()
    }

    /// True when no columns are cached.
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    /// Return the cached column, inserting `f()` on miss (and evicting if full).
    pub fn get_or_insert_with(
        &mut self,
        key: (i32, i32),
        f: impl FnOnce() -> NoiseColumn,
    ) -> &NoiseColumn {
        if self.map.contains_key(&key) {
            return self.map.get(&key).expect("key present");
        }
        while self.map.len() >= self.cap {
            if let Some(old) = self.order.pop_front() {
                self.map.remove(&old);
            } else {
                break;
            }
        }
        self.order.push_back(key);
        self.map.entry(key).or_insert_with(f)
    }
}

impl Default for NoiseCache {
    fn default() -> Self {
        Self::new()
    }
}

/// A generated chunk column.
pub struct GeneratedChunk {
    /// Index = `(y - WORLD_BOTTOM) * 256 + z * 16 + x`.
    pub blocks: Vec<u16>,
    /// Biome IDs (currently placeholder-filled; see biome source work).
    pub biomes: Vec<u8>,
    /// Highest non-air block Y per column (index = z*16+x).
    pub heightmap: Vec<i16>,
}

impl GeneratedChunk {
    pub fn block_at(&self, x: u32, y: i32, z: u32) -> BlockId {
        debug_assert!(x < 16 && z < 16);
        debug_assert!(y >= WORLD_BOTTOM && y < WORLD_TOP);
        let idx = ((y - WORLD_BOTTOM) as usize) * 256 + (z as usize) * 16 + (x as usize);
        BlockId::from_u16(self.blocks[idx]).unwrap_or(BlockId::Air)
    }
}

/// The 26.2 overworld chunk generator.
pub struct ChunkGenerator {
    pub state: WorldgenState,
}

impl ChunkGenerator {
    pub fn new(seed: i64) -> Self {
        Self {
            state: WorldgenState::overworld(seed),
        }
    }

    /// Generate a chunk at chunk coordinates.
    ///
    /// Builds a 3×3 neighborhood of noise+surface, then runs underground ore
    /// features for every origin in that region (so ore blobs that cross chunk
    /// borders match vanilla `WorldGenRegion` decoration).
    pub fn generate_chunk(&self, cx: i32, cz: i32) -> GeneratedChunk {
        let mut unused = NoiseCache::new();
        self.generate_chunk_cached(cx, cz, &mut unused)
    }

    /// Same as [`generate_chunk`] but reuses pre-feature noise+surface columns.
    ///
    /// Each decorated chunk still needs a clean 3×3 of noise (features must
    /// not see a neighbour that already had ores/trees). Caching that first
    /// stage is what makes the live server able to stream view-distance.
    pub fn generate_chunk_cached(
        &self,
        cx: i32,
        cz: i32,
        noise_cache: &mut NoiseCache,
    ) -> GeneratedChunk {
        const FEATURE_RADIUS: i32 = 1;
        let mut region = RegionBuf::new(cx, cz, FEATURE_RADIUS);
        let mut center_biomes = vec![0u8; CHUNK_BIOME_VOLUME];

        for dz in -FEATURE_RADIUS..=FEATURE_RADIUS {
            for dx in -FEATURE_RADIUS..=FEATURE_RADIUS {
                let ncx = cx + dx;
                let ncz = cz + dz;
                let (blocks, heightmap, biomes) = noise_cache
                    .get_or_insert_with((ncx, ncz), || self.generate_noise_and_surface(ncx, ncz));
                region.put_chunk(ncx, ncz, blocks, heightmap);
                if dx == 0 && dz == 0 {
                    center_biomes = biomes.clone();
                }
            }
        }

        // Classic carvers (caves + canyon).
        carvers::apply_carvers_region(&mut region, self.state.seed);
        // FEATURES: structure pieces (mineshafts) before biome decoration.
        crate::mineshaft::apply_mineshafts_region(&mut region, &self.state);
        // Step 6 — ores (dedicated OreFeature port).
        features::apply_underground_ores_region(&mut region, self.state.seed);
        // Step 7 — sculk_vein + sculk_patch (CFR MultifaceSpreader + ChargeCursor).
        crate::sculk::apply_sculk_region(&mut region, &self.state);
        // Step 9 — vegetal decoration via datapack feature dispatcher (trees, grass, litter).
        crate::feature_dispatch::apply_step_region(
            &mut region,
            &self.state,
            crate::feature_catalog::step::VEGETAL_DECORATION,
            "plains", // primary fallback; per-origin biome resolved inside
        );

        let (blocks, heightmap) = region.take_chunk(cx, cz);
        GeneratedChunk {
            blocks,
            biomes: center_biomes,
            heightmap,
        }
    }

    /// 3×3 noise+surface+carvers+ores (no sculk / no vegetation).
    pub fn generate_ores_region(&self, cx: i32, cz: i32) -> crate::region_buf::RegionBuf {
        const FEATURE_RADIUS: i32 = 1;
        let mut region = crate::region_buf::RegionBuf::new(cx, cz, FEATURE_RADIUS);
        for dz in -FEATURE_RADIUS..=FEATURE_RADIUS {
            for dx in -FEATURE_RADIUS..=FEATURE_RADIUS {
                let (blocks, heightmap, _) = self.generate_noise_and_surface(cx + dx, cz + dz);
                region.put_chunk(cx + dx, cz + dz, &blocks, &heightmap);
            }
        }
        crate::carvers::apply_carvers_region(&mut region, self.state.seed);
        crate::mineshaft::apply_mineshafts_region(&mut region, &self.state);
        crate::features::apply_underground_ores_region(&mut region, self.state.seed);
        region
    }

    /// Density fill + aquifer + surface rules for one chunk (no features).
    fn generate_noise_and_surface(&self, cx: i32, cz: i32) -> (Vec<u16>, Vec<i16>, Vec<u8>) {
        let st = &self.state;
        let mut blocks = vec![BlockId::Air.as_u16(); CHUNK_BLOCK_VOLUME];

        // Create per-chunk marker state (owned by the generator).
        let mut marker_state = MarkerState::new(st.cell_width as usize, st.cell_height as usize);

        // Collect ALL Interpolated markers in final_density (A-path + noodle).
        let mut interp_markers: Vec<DF> = Vec::new();
        collect_interpolated(&st.router.final_density, &mut interp_markers);
        assert!(
            !interp_markers.is_empty(),
            "final_density must contain at least one interpolated marker"
        );

        let cell_width = st.cell_width; // 4
        let cell_height = st.cell_height; // 8
        let cell_count_xz = 16 / cell_width; // 4
        let cell_count_y = st.height / cell_height; // 48
        let cell_noise_min_y = st.min_y.div_euclid(cell_height); // -8
        let first_cell_x = (cx * 16).div_euclid(cell_width);
        let first_cell_z = (cz * 16).div_euclid(cell_width);
        let stride_xz = (cell_count_xz + 1) as usize;
        let grid_len = stride_xz * (cell_count_y as usize + 1) * stride_xz;

        // Sample each interpolator's *wrapped* function on the cell grid.
        let mut grids: Vec<Vec<f64>> = Vec::with_capacity(interp_markers.len());
        let mut ids: Vec<usize> = Vec::with_capacity(interp_markers.len());
        for marker in &interp_markers {
            ids.push(std::sync::Arc::as_ptr(marker) as usize);
            let wrapped = interpolated_wrapped(marker);
            let mut samples = vec![0f64; grid_len];
            for iy in 0..=cell_count_y {
                let grid_y = (cell_noise_min_y + iy as i32) * cell_height;
                for iz in 0..=cell_count_xz {
                    let grid_z = (first_cell_z + iz as i32) * cell_width;
                    for ix in 0..=cell_count_xz {
                        let grid_x = (first_cell_x + ix as i32) * cell_width;
                        let mut env = DensityEnv::new(grid_x, grid_y, grid_z, st.noises.noises());
                        let si = (iy as usize * stride_xz + iz as usize) * stride_xz + ix as usize;
                        samples[si] = compute(&wrapped, &mut env);
                    }
                }
            }
            grids.push(samples);
        }

        marker_state.cell_interp = Some(CellInterpRuntime {
            ids,
            grids,
            stride_xz,
            cell_ix: 0,
            cell_iy: 0,
            cell_iz: 0,
            factor_x: 0.0,
            factor_y: 0.0,
            factor_z: 0.0,
        });

        // --- Aquifer ---
        let mut aquifer = self.build_aquifer(cx, cz);

        let chunk_min_x = cx * 16;
        let chunk_min_z = cz * 16;

        // --- Fill loop (doFill) ---
        let mut interpolation_counter: i64 = 0;
        for cell_x_index in 0..cell_count_xz {
            for cell_z_index in 0..cell_count_xz {
                for cell_y_index in (0..cell_count_y).rev() {
                    for y_in_cell in (0..cell_height).rev() {
                        let pos_y =
                            (cell_noise_min_y + cell_y_index as i32) * cell_height + y_in_cell;
                        let factor_y = y_in_cell as f64 / cell_height as f64;
                        for x_in_cell in 0..cell_width {
                            let pos_x =
                                chunk_min_x + (cell_x_index as i32) * cell_width + x_in_cell;
                            let factor_x = x_in_cell as f64 / cell_width as f64;
                            for z_in_cell in 0..cell_width {
                                let pos_z =
                                    chunk_min_z + (cell_z_index as i32) * cell_width + z_in_cell;
                                let factor_z = z_in_cell as f64 / cell_width as f64;

                                marker_state.interpolation_counter = interpolation_counter;
                                interpolation_counter += 1;

                                // Update interpolator cell + factors (like NoiseChunk updateFor*).
                                if let Some(rt) = marker_state.cell_interp.as_mut() {
                                    rt.cell_ix = cell_x_index as usize;
                                    rt.cell_iy = cell_y_index as usize;
                                    rt.cell_iz = cell_z_index as usize;
                                    rt.factor_x = factor_x;
                                    rt.factor_y = factor_y;
                                    rt.factor_z = factor_z;
                                }

                                // Full final_density with all Interpolated nodes lerping.
                                let mut env = DensityEnv::with_markers(
                                    pos_x,
                                    pos_y,
                                    pos_z,
                                    st.noises.noises(),
                                    &mut marker_state,
                                );
                                let final_density = compute(&st.router.final_density, &mut env);

                                // Material rules: aquifer first (None = solid default),
                                // then ore veinifier, else stone.
                                let state =
                                    aquifer.compute_substance(pos_x, pos_y, pos_z, final_density);
                                let block = match state {
                                    Some(b) => b,
                                    None => ore_vein::try_place_vein(
                                        pos_x,
                                        pos_y,
                                        pos_z,
                                        &st.router.vein_toggle,
                                        &st.router.vein_ridged,
                                        &st.router.vein_gap,
                                        PositionalRandomFactory::new(st.ore_lo, st.ore_hi),
                                        st.noises.noises(),
                                    )
                                    .unwrap_or(BlockId::Stone),
                                };
                                if block != BlockId::Air {
                                    let xl = (pos_x - chunk_min_x) as usize;
                                    let zl = (pos_z - chunk_min_z) as usize;
                                    let idx_b =
                                        ((pos_y - WORLD_BOTTOM) as usize) * 256 + zl * 16 + xl;
                                    blocks[idx_b] = block.as_u16();
                                }
                            }
                        }
                    }
                }
            }
        }

        // --- Heightmap (highest non-air/fluid solid; recomputed again after surface) ---
        let mut heightmap = vec![WORLD_BOTTOM as i16; HEIGHTMAP_SIZE];
        for lx in 0..16usize {
            for lz in 0..16usize {
                for y in (WORLD_BOTTOM..WORLD_TOP).rev() {
                    let idx = ((y - WORLD_BOTTOM) as usize) * 256 + lz * 16 + lx;
                    let b = BlockId::from_u16(blocks[idx]).unwrap_or(BlockId::Air);
                    if !matches!(b, BlockId::Air | BlockId::Water | BlockId::Lava) {
                        heightmap[lz * 16 + lx] = y as i16;
                        break;
                    }
                }
            }
        }

        // --- Biomes (4x4 horizontal per 16-block section; one Y sample at section mid) ---
        // Vanilla `fillBiomesFromNoise` stores 4×4×4 quarts per section via
        // `getNoiseBiome(quart)` (no voronoi). We keep one Y quart per section
        // at the section midpoint quart. Sample climate at `QuartPos.toBlock`.
        let mut biomes = vec![0u8; CHUNK_BIOME_VOLUME];
        let st = &self.state;
        for section in 0..24 {
            for bz4 in 0..4 {
                for bx4 in 0..4 {
                    let quart_x = cx * 4 + bx4;
                    let quart_z = cz * 4 + bz4;
                    let quart_y = (WORLD_BOTTOM + section * 16 + 8) >> 2;
                    let idx = section * 16 + bz4 * 4 + bx4;
                    biomes[idx as usize] =
                        crate::biome_manager::noise_biome_at_quart(st, quart_x, quart_y, quart_z);
                }
            }
        }

        // --- Surface rules from datapack surface_rule JSON ---
        surface_rules::apply_surface_rules(&mut blocks, &heightmap, cx, cz, st);

        // Heightmap after surface (features recompute later on extract)
        for lx in 0..16usize {
            for lz in 0..16usize {
                heightmap[lz * 16 + lx] = WORLD_BOTTOM as i16;
                for y in (WORLD_BOTTOM..WORLD_TOP).rev() {
                    let idx = ((y - WORLD_BOTTOM) as usize) * 256 + lz * 16 + lx;
                    let b = BlockId::from_u16(blocks[idx]).unwrap_or(BlockId::Air);
                    if !matches!(b, BlockId::Air | BlockId::Water | BlockId::Lava) {
                        heightmap[lz * 16 + lx] = y as i16;
                        break;
                    }
                }
            }
        }

        (blocks, heightmap, biomes)
    }

    fn build_aquifer(&self, cx: i32, cz: i32) -> NoiseBasedAquifer<'_> {
        let st = &self.state;
        // maxPreliminarySurfaceLevel is derived inside `create` (same rectangle
        // and 4-block sampling as the vanilla constructor, Aquifer.java:124).
        NoiseBasedAquifer::create(
            st.noises.noises(),
            st.router.barrier.clone(),
            st.router.fluid_level_floodedness.clone(),
            st.router.fluid_level_spread.clone(),
            st.router.lava.clone(),
            st.router.erosion.clone(),
            st.router.depth.clone(),
            st.router.preliminary_surface_level.clone(),
            st.aquifer_lo,
            st.aquifer_hi,
            cx * 16,
            cz * 16,
            st.min_y,
            st.height,
            GlobalFluidPicker::overworld(st.sea_level),
        )
    }
}

/// `Mth.lerp`.
#[inline]
pub fn lerp(alpha: f64, a: f64, b: f64) -> f64 {
    a + alpha * (b - a)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chunk_generator_is_send() {
        fn assert_send<T: Send>() {}
        assert_send::<ChunkGenerator>();
        assert_send::<NoiseCache>();
    }

    #[test]
    fn noise_cache_drops_oldest() {
        let mut cache = NoiseCache::with_cap(2);
        cache.get_or_insert_with((0, 0), || (vec![1], vec![0], vec![0]));
        cache.get_or_insert_with((1, 0), || (vec![2], vec![0], vec![0]));
        cache.get_or_insert_with((2, 0), || (vec![3], vec![0], vec![0]));
        assert_eq!(cache.len(), 2);
        assert!(cache.map.contains_key(&(1, 0)));
        assert!(cache.map.contains_key(&(2, 0)));
        assert!(!cache.map.contains_key(&(0, 0)));
    }
}
