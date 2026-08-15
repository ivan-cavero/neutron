//! Aquifer system matching Minecraft 26.2 `Aquifer.NoiseBasedAquifer`.
//!
//! `compute_substance` turns a density sample into a block: `None` is the
//! solid default, `Some` is fluid or air. Driven by four aquifer noises and
//! the fluid grid.
//!
//! Copyright (c) 2026 Neutron Contributors -- MIT License

use std::collections::HashMap;

use crate::density::{DensityEnv, DF};
use crate::rng::Xoroshiro128;
use crate::surface::BlockId;

/// `DimensionType.WAY_BELOW_MIN_Y` — the "no fluid" sentinel.
pub const WAY_BELOW_MIN_Y: i32 = -32512;

/// `Mth.getSeed(x, y, z)` (Mth.java:356-360). Note `x * 3129871` is an int
/// multiply (wraps on overflow) widened to long, while `z * 116129781L` is long.
pub fn get_seed(x: i32, y: i32, z: i32) -> i64 {
    let mut s = (x.wrapping_mul(3129871) as i64) ^ (z as i64 * 116_129_781) ^ y as i64;
    s = s
        .wrapping_mul(s)
        .wrapping_mul(42317861)
        .wrapping_add(s.wrapping_mul(11));
    s >> 16
}

/// `QuartPos.fromBlock(x) = x >> 2`; `QuartPos.toBlock(q) = q << 2`.
#[inline]
pub fn quart_from_block(x: i32) -> i32 {
    x >> 2
}
#[inline]
pub fn quart_to_block(q: i32) -> i32 {
    q << 2
}

/// `Mth.quantize(value, resolution)`.
#[inline]
pub fn quantize(value: f64, resolution: i32) -> i32 {
    (value / resolution as f64).floor() as i32 * resolution
}

/// `Mth.inverseLerp(value, fromMin, fromMax)`.
#[inline]
fn inverse_lerp(value: f64, from_min: f64, from_max: f64) -> f64 {
    (value - from_min) / (from_max - from_min)
}

/// `Mth.map(value, fromMin, fromMax, toMin, toMax)`.
#[inline]
fn map(value: f64, from_min: f64, from_max: f64, to_min: f64, to_max: f64) -> f64 {
    let t = inverse_lerp(value, from_min, from_max);
    to_min + t * (to_max - to_min)
}

/// `Mth.clampedMap`.
#[inline]
fn clamped_map(value: f64, from_min: f64, from_max: f64, to_min: f64, to_max: f64) -> f64 {
    let t = inverse_lerp(value, from_min, from_max).clamp(0.0, 1.0);
    to_min + t * (to_max - to_min)
}

/// A fluid level + type pair (`Aquifer.FluidStatus`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct FluidStatus {
    pub fluid_level: i32,
    pub fluid_type: BlockId,
}

impl FluidStatus {
    /// `at(blockY)`: fluid below the level, air above.
    pub fn at(&self, block_y: i32) -> BlockId {
        if block_y < self.fluid_level {
            self.fluid_type
        } else {
            BlockId::Air
        }
    }
}

/// The global fluid picker for the overworld
/// (`NoiseBasedChunkGenerator.createFluidPicker`).
pub struct GlobalFluidPicker {
    lava: FluidStatus,
    sea: FluidStatus,
}

impl GlobalFluidPicker {
    pub fn overworld(sea_level: i32) -> Self {
        Self {
            lava: FluidStatus {
                fluid_level: -54,
                fluid_type: BlockId::Lava,
            },
            sea: FluidStatus {
                fluid_level: sea_level,
                fluid_type: BlockId::Water,
            },
        }
    }

    pub fn compute_fluid(&self, _x: i32, y: i32, _z: i32) -> FluidStatus {
        if y < -54.min(self.sea.fluid_level) {
            self.lava
        } else {
            self.sea
        }
    }
}

/// The overworld noise-based aquifer.
pub struct NoiseBasedAquifer<'a> {
    env_noises: &'a std::collections::HashMap<String, crate::noise::NormalNoise>,
    barrier_noise: DF,
    fluid_level_floodedness_noise: DF,
    fluid_level_spread_noise: DF,
    lava_noise: DF,
    erosion: DF,
    depth: DF,
    preliminary_surface_level: DF,
    /// (aquifer positional seed lo, hi) for `at(gx, gy, gz)`.
    pos_lo: u64,
    pos_hi: u64,
    min_grid_x: i32,
    min_grid_y: i32,
    min_grid_z: i32,
    grid_size_x: i32,
    grid_size_y: i32,
    grid_size_z: i32,
    skip_sampling_above_y: i32,
    global_fluid_picker: GlobalFluidPicker,
    location_cache: Vec<Option<(i32, i32, i32)>>,
    status_cache: Vec<Option<FluidStatus>>,
    surface_cache: HashMap<(i32, i32), i32>,
    pub should_schedule_fluid_update: bool,
}

impl<'a> NoiseBasedAquifer<'a> {
    const SURFACE_SAMPLING_OFFSETS: [(i32, i32); 13] = [
        (0, 0),
        (-2, -1),
        (-1, -1),
        (0, -1),
        (1, -1),
        (-3, 0),
        (-2, 0),
        (-1, 0),
        (1, 0),
        (-2, 1),
        (-1, 1),
        (0, 1),
        (1, 1),
    ];

    /// `Aquifer.create(noiseChunk, pos, router, aquiferRandom, minBlockY, yBlockSize, fluidRule)`.
    ///
    /// The vanilla constructor derives `skipSamplingAboveY` from
    /// `noiseChunk.maxPreliminarySurfaceLevel(fromGridX(minGridX, 0),
    /// fromGridZ(minGridZ, 0), fromGridX(maxGridX, 9), fromGridZ(maxGridZ, 9))`
    /// (Aquifer.java:124-126); we compute the same max here.
    pub fn create(
        env_noises: &'a std::collections::HashMap<String, crate::noise::NormalNoise>,
        barrier_noise: DF,
        fluid_level_floodedness_noise: DF,
        fluid_level_spread_noise: DF,
        lava_noise: DF,
        erosion: DF,
        depth: DF,
        preliminary_surface_level: DF,
        pos_lo: u64,
        pos_hi: u64,
        chunk_min_x: i32,
        chunk_min_z: i32,
        min_block_y: i32,
        y_block_size: i32,
        global_fluid_picker: GlobalFluidPicker,
    ) -> Self {
        let min_grid_x = grid_x(chunk_min_x - 5);
        let max_grid_x = grid_x(chunk_min_x + 15 - 5) + 1;
        let min_grid_y = grid_y(min_block_y + 1) - 1;
        let max_grid_y = grid_y(min_block_y + y_block_size + 1) + 1;
        let min_grid_z = grid_z(chunk_min_z - 5);
        let max_grid_z = grid_z(chunk_min_z + 15 - 5) + 1;
        let grid_size_x = max_grid_x - min_grid_x + 1;
        let grid_size_y = max_grid_y - min_grid_y + 1;
        let grid_size_z = max_grid_z - min_grid_z + 1;

        // `NoiseChunk.maxPreliminarySurfaceLevel(minBlockX, minBlockZ, maxBlockX, maxBlockZ)`
        // (NoiseChunk.java:198-207): max over the grid rectangle sampled every 4 blocks.
        let mut max_preliminary_surface_level = i32::MIN;
        for bz in (from_grid_z(min_grid_z, 0)..=from_grid_z(max_grid_z, 9)).step_by(4) {
            for bx in (from_grid_x(min_grid_x, 0)..=from_grid_x(max_grid_x, 9)).step_by(4) {
                let v = preliminary_surface_level_at(
                    env_noises,
                    &preliminary_surface_level,
                    bx,
                    bz,
                );
                max_preliminary_surface_level = max_preliminary_surface_level.max(v);
            }
        }

        let max_adjusted_surface = max_preliminary_surface_level + 8;
        let skip_sampling_above_grid_y = grid_y(max_adjusted_surface + 12) + 1;
        let skip_sampling_above_y = from_grid_y(skip_sampling_above_grid_y, 11) - 1;

        let total = (grid_size_x * grid_size_y * grid_size_z) as usize;
        Self {
            env_noises,
            barrier_noise,
            fluid_level_floodedness_noise,
            fluid_level_spread_noise,
            lava_noise,
            erosion,
            depth,
            preliminary_surface_level,
            pos_lo,
            pos_hi,
            min_grid_x,
            min_grid_y,
            min_grid_z,
            grid_size_x,
            grid_size_y,
            grid_size_z,
            skip_sampling_above_y,
            global_fluid_picker,
            location_cache: vec![None; total],
            status_cache: vec![None; total],
            surface_cache: HashMap::new(),
            should_schedule_fluid_update: false,
        }
    }

    #[inline]
    fn get_index(&self, grid_x: i32, grid_y: i32, grid_z: i32) -> usize {
        let x = grid_x - self.min_grid_x;
        let y = grid_y - self.min_grid_y;
        let z = grid_z - self.min_grid_z;
        ((y * self.grid_size_z + z) * self.grid_size_x + x) as usize
    }

    /// `positionalRandomFactory.at(gx, gy, gz)`.
    fn at(&self, gx: i32, gy: i32, gz: i32) -> Xoroshiro128 {
        let positional_seed = get_seed(gx, gy, gz) as u64;
        Xoroshiro128::from_raw(positional_seed ^ self.pos_lo, self.pos_hi)
    }

    /// `computeSubstance(context, density)`.
    pub fn compute_substance(&mut self, x: i32, y: i32, z: i32, density: f64) -> Option<BlockId> {
        if density > 0.0 {
            self.should_schedule_fluid_update = false;
            return None;
        }
        let global_fluid = self.global_fluid_picker.compute_fluid(x, y, z);
        if y > self.skip_sampling_above_y {
            self.should_schedule_fluid_update = false;
            return Some(global_fluid.at(y));
        }
        if global_fluid.at(y) == BlockId::Lava {
            self.should_schedule_fluid_update = false;
            return Some(BlockId::Lava);
        }
        let x_anchor = grid_x(x - 5);
        let y_anchor = grid_y(y + 1);
        let z_anchor = grid_z(z - 5);

        let mut dist = [i32::MAX; 4];
        let mut closest = [0usize; 4];
        for x1 in 0..=1 {
            for y1 in -1..=1 {
                for z1 in 0..=1 {
                    let gx = x_anchor + x1;
                    let gy = y_anchor + y1;
                    let gz = z_anchor + z1;
                    let index = self.get_index(gx, gy, gz);
                    let (lx, ly, lz) = self.get_location(index, gx, gy, gz);
                    let dx = lx - x;
                    let dy = ly - y;
                    let dz = lz - z;
                    let new_dist = dx * dx + dy * dy + dz * dz;
                    if dist[0] >= new_dist {
                        closest[3] = closest[2];
                        closest[2] = closest[1];
                        closest[1] = closest[0];
                        closest[0] = index;
                        dist[3] = dist[2];
                        dist[2] = dist[1];
                        dist[1] = dist[0];
                        dist[0] = new_dist;
                    } else if dist[1] >= new_dist {
                        closest[3] = closest[2];
                        closest[2] = closest[1];
                        closest[1] = index;
                        dist[3] = dist[2];
                        dist[2] = dist[1];
                        dist[1] = new_dist;
                    } else if dist[2] >= new_dist {
                        closest[3] = closest[2];
                        closest[2] = index;
                        dist[3] = dist[2];
                        dist[2] = new_dist;
                    } else if dist[3] < new_dist {
                        // skip
                    } else {
                        closest[3] = index;
                        dist[3] = new_dist;
                    }
                }
            }
        }

        let status1 = self.get_aquifer_status(closest[0]);
        let similarity12 = similarity(dist[0], dist[1]);
        let fluid_state = status1.at(y);
        if similarity12 <= 0.0 {
            let status2 = self.get_aquifer_status(closest[1]);
            self.should_schedule_fluid_update =
                similarity12 >= FLOWING_UPDATE_SIMULARITY && status1 != status2;
            return Some(fluid_state);
        }
        if fluid_state == BlockId::Water
            && self
                .global_fluid_picker
                .compute_fluid(x, y - 1, z)
                .at(y - 1)
                == BlockId::Lava
        {
            self.should_schedule_fluid_update = true;
            return Some(fluid_state);
        }
        let mut barrier_noise = f64::NAN;
        let status2 = self.get_aquifer_status(closest[1]);
        let barrier12 =
            similarity12 * self.calculate_pressure(&mut barrier_noise, status1, status2, x, y, z);
        if density + barrier12 > 0.0 {
            self.should_schedule_fluid_update = false;
            return None;
        }
        let status3 = self.get_aquifer_status(closest[2]);
        let similarity13 = similarity(dist[0], dist[2]);
        if similarity13 > 0.0 {
            let barrier13 = similarity12
                * similarity13
                * self.calculate_pressure(&mut barrier_noise, status1, status3, x, y, z);
            if density + barrier13 > 0.0 {
                self.should_schedule_fluid_update = false;
                return None;
            }
        }
        let similarity23 = similarity(dist[1], dist[2]);
        if similarity23 > 0.0 {
            let barrier23 = similarity12
                * similarity23
                * self.calculate_pressure(&mut barrier_noise, status2, status3, x, y, z);
            if density + barrier23 > 0.0 {
                self.should_schedule_fluid_update = false;
                return None;
            }
        }
        let may_flow12 = status1 != status2;
        let may_flow23 = similarity23 >= FLOWING_UPDATE_SIMULARITY && status2 != status3;
        let may_flow13 = similarity13 >= FLOWING_UPDATE_SIMULARITY && status1 != status3;
        self.should_schedule_fluid_update = if may_flow12 || may_flow23 || may_flow13 {
            true
        } else {
            similarity13 >= FLOWING_UPDATE_SIMULARITY
                && similarity(dist[0], dist[3]) >= FLOWING_UPDATE_SIMULARITY
                && status1 != self.get_aquifer_status(closest[3])
        };
        Some(fluid_state)
    }

    /// `getLocation` with per-grid-cell random offsets, cached.
    fn get_location(&mut self, index: usize, gx: i32, gy: i32, gz: i32) -> (i32, i32, i32) {
        if let Some(loc) = self.location_cache[index] {
            return loc;
        }
        let mut random = self.at(gx, gy, gz);
        let loc = (
            from_grid_x(gx, random.next_int(10)),
            from_grid_y(gy, random.next_int(9)),
            from_grid_z(gz, random.next_int(10)),
        );
        self.location_cache[index] = Some(loc);
        loc
    }

    fn get_aquifer_status(&mut self, index: usize) -> FluidStatus {
        if let Some(status) = self.status_cache[index] {
            return status;
        }
        let (x, y, z) = self.location_cache[index].expect("location must be computed first");
        let status = self.compute_fluid(x, y, z);
        self.status_cache[index] = Some(status);
        status
    }

    /// `computeFluid(x, y, z)`.
    fn compute_fluid(&mut self, x: i32, y: i32, z: i32) -> FluidStatus {
        let global_fluid = self.global_fluid_picker.compute_fluid(x, y, z);
        let top_of_aquifer_cell = y + 12;
        let bottom_of_aquifer_cell = y - 12;
        let mut lowest_preliminary_surface = i32::MAX;
        let mut surface_at_center_is_under_global_fluid_level = false;
        for (ox, oz) in Self::SURFACE_SAMPLING_OFFSETS {
            let sample_x = x + ox * 16;
            let sample_z = z + oz * 16;
            let preliminary_surface_level = self.preliminary_surface_level(sample_x, sample_z);
            let adjusted_surface_level = preliminary_surface_level + 8;
            let start = ox == 0 && oz == 0;
            if start && bottom_of_aquifer_cell > adjusted_surface_level {
                return global_fluid;
            }
            let pokes_above = top_of_aquifer_cell > adjusted_surface_level;
            if (pokes_above || start)
                && !self
                    .global_fluid_picker
                    .compute_fluid(sample_x, adjusted_surface_level, sample_z)
                    .at(adjusted_surface_level)
                    .is_air()
            {
                if start {
                    surface_at_center_is_under_global_fluid_level = true;
                }
                if pokes_above {
                    return self.global_fluid_picker.compute_fluid(
                        sample_x,
                        adjusted_surface_level,
                        sample_z,
                    );
                }
            }
            lowest_preliminary_surface = lowest_preliminary_surface.min(preliminary_surface_level);
        }
        let fluid_surface_level = self.compute_surface_level(
            x,
            y,
            z,
            global_fluid,
            lowest_preliminary_surface,
            surface_at_center_is_under_global_fluid_level,
        );
        FluidStatus {
            fluid_level: fluid_surface_level,
            fluid_type: self.compute_fluid_type(x, y, z, global_fluid, fluid_surface_level),
        }
    }

    /// `preliminarySurfaceLevel(sampleX, sampleZ)` with quart quantization + cache.
    pub fn preliminary_surface_level(&mut self, sample_x: i32, sample_z: i32) -> i32 {
        let qx = quart_to_block(quart_from_block(sample_x));
        let qz = quart_to_block(quart_from_block(sample_z));
        if let Some(&v) = self.surface_cache.get(&(qx, qz)) {
            return v;
        }
        let v = preliminary_surface_level_at(
            self.env_noises,
            &self.preliminary_surface_level,
            qx,
            qz,
        );
        self.surface_cache.insert((qx, qz), v);
        v
    }

    /// `computeSurfaceLevel`.
    fn compute_surface_level(
        &mut self,
        x: i32,
        y: i32,
        z: i32,
        global_fluid: FluidStatus,
        lowest_preliminary_surface: i32,
        surface_at_center_is_under_global_fluid_level: bool,
    ) -> i32 {
        let (partially_floodedness, fully_floodedness) = {
            let mut env = DensityEnv::new(x, y, z, self.env_noises);
            let erosion_v = crate::density::compute(&self.erosion, &mut env);
            let depth_v = crate::density::compute(&self.depth, &mut env);
            if erosion_v < -0.225 && depth_v > 0.9 {
                (-1.0, -1.0)
            } else {
                let distance_below_surface = lowest_preliminary_surface + 8 - y;
                let floodedness_factor = if surface_at_center_is_under_global_fluid_level {
                    clamped_map(distance_below_surface as f64, 0.0, 64.0, 1.0, 0.0)
                } else {
                    0.0
                };
                let floodedness_noise_value =
                    crate::density::compute(&self.fluid_level_floodedness_noise, &mut env)
                        .clamp(-1.0, 1.0);
                let fully_flooded_threshold = map(floodedness_factor, 1.0, 0.0, -0.3, 0.8);
                let partially_flooded_threshold = map(floodedness_factor, 1.0, 0.0, -0.8, 0.4);
                (
                    floodedness_noise_value - partially_flooded_threshold,
                    floodedness_noise_value - fully_flooded_threshold,
                )
            }
        };
        if fully_floodedness > 0.0 {
            global_fluid.fluid_level
        } else if partially_floodedness > 0.0 {
            self.compute_randomized_fluid_surface_level(x, y, z, lowest_preliminary_surface)
        } else {
            WAY_BELOW_MIN_Y
        }
    }

    /// `computeRandomizedFluidSurfaceLevel`.
    fn compute_randomized_fluid_surface_level(
        &mut self,
        x: i32,
        y: i32,
        z: i32,
        lowest_preliminary_surface: i32,
    ) -> i32 {
        let fluid_level_cell_x = x.div_euclid(16);
        let fluid_level_cell_y = y.div_euclid(40);
        let fluid_level_cell_z = z.div_euclid(16);
        let fluid_cell_middle_y = fluid_level_cell_y * 40 + 20;
        let mut env = DensityEnv::new(
            fluid_level_cell_x,
            fluid_level_cell_y,
            fluid_level_cell_z,
            self.env_noises,
        );
        let fluid_level_spread =
            crate::density::compute(&self.fluid_level_spread_noise, &mut env) * 10.0;
        let fluid_level_spread_quantized = quantize(fluid_level_spread, 3);
        let target = fluid_cell_middle_y + fluid_level_spread_quantized;
        lowest_preliminary_surface.min(target)
    }

    /// `computeFluidType`.
    fn compute_fluid_type(
        &self,
        x: i32,
        y: i32,
        z: i32,
        global_fluid: FluidStatus,
        fluid_surface_level: i32,
    ) -> BlockId {
        let mut fluid_type = global_fluid.fluid_type;
        if fluid_surface_level <= -10
            && fluid_surface_level != WAY_BELOW_MIN_Y
            && global_fluid.fluid_type != BlockId::Lava
        {
            let cell_x = x.div_euclid(64);
            let cell_y = y.div_euclid(40);
            let cell_z = z.div_euclid(64);
            let mut env = DensityEnv::new(cell_x, cell_y, cell_z, self.env_noises);
            let lava_noise_value = crate::density::compute(&self.lava_noise, &mut env);
            if lava_noise_value.abs() > 0.3 {
                fluid_type = BlockId::Lava;
            }
        }
        fluid_type
    }

    /// `calculatePressure`.
    fn calculate_pressure(
        &mut self,
        barrier_noise_value: &mut f64,
        status1: FluidStatus,
        status2: FluidStatus,
        x: i32,
        y: i32,
        z: i32,
    ) -> f64 {
        let type1 = status1.at(y);
        let type2 = status2.at(y);
        if (type1 == BlockId::Lava && type2 == BlockId::Water)
            || (type1 == BlockId::Water && type2 == BlockId::Lava)
        {
            return 2.0;
        }
        let fluid_y_diff = (status1.fluid_level - status2.fluid_level).abs();
        if fluid_y_diff == 0 {
            return 0.0;
        }
        let average_fluid_y = 0.5 * (status1.fluid_level + status2.fluid_level) as f64;
        let how_far_above_average = y as f64 + 0.5 - average_fluid_y;
        let base_value = fluid_y_diff as f64 / 2.0;
        let distance_from_barrier_edge = base_value - how_far_above_average.abs();
        let gradient = if how_far_above_average > 0.0 {
            let center = 0.0 + distance_from_barrier_edge;
            if center > 0.0 {
                center / 1.5
            } else {
                center / 2.5
            }
        } else {
            let center = 3.0 + distance_from_barrier_edge;
            if center > 0.0 {
                center / 3.0
            } else {
                center / 10.0
            }
        };
        let noise_value = if gradient < -2.0 || gradient > 2.0 {
            0.0
        } else if barrier_noise_value.is_nan() {
            let mut env = DensityEnv::new(x, y, z, self.env_noises);
            let v = crate::density::compute(&self.barrier_noise, &mut env);
            *barrier_noise_value = v;
            v
        } else {
            *barrier_noise_value
        };
        2.0 * (noise_value + gradient)
    }
}

/// `similarity(distanceSqr1, distanceSqr2)`.
fn similarity(distance_sqr1: i32, distance_sqr2: i32) -> f64 {
    1.0 - (distance_sqr2 - distance_sqr1) as f64 / 25.0
}

/// `NoiseChunk.computePreliminarySurfaceLevel` (NoiseChunk.java:217-219):
/// `floor(preliminarySurfaceLevel.compute(SinglePointContext(blockX, 0, blockZ)))`.
/// The caller passes already-quantized block coords (`QuartPos.toBlock(QuartPos.fromBlock(x))`).
fn preliminary_surface_level_at(
    env_noises: &std::collections::HashMap<String, crate::noise::NormalNoise>,
    preliminary_surface_level: &DF,
    block_x: i32,
    block_z: i32,
) -> i32 {
    let mut env = DensityEnv::new(block_x, 0, block_z, env_noises);
    crate::density::compute(preliminary_surface_level, &mut env).floor() as i32
}

/// `FLOWING_UPDATE_SIMULARITY = similarity(square(10), square(12))`.
const FLOWING_UPDATE_SIMULARITY: f64 = 1.0 - (144 - 100) as f64 / 25.0;

#[inline]
fn grid_x(block_coord: i32) -> i32 {
    block_coord >> 4
}
#[inline]
fn from_grid_x(grid_coord: i32, block_offset: i32) -> i32 {
    (grid_coord << 4) + block_offset
}
#[inline]
fn grid_y(block_coord: i32) -> i32 {
    block_coord.div_euclid(12)
}
#[inline]
fn from_grid_y(grid_coord: i32, block_offset: i32) -> i32 {
    grid_coord * 12 + block_offset
}
#[inline]
fn grid_z(block_coord: i32) -> i32 {
    block_coord >> 4
}
#[inline]
fn from_grid_z(grid_coord: i32, block_offset: i32) -> i32 {
    (grid_coord << 4) + block_offset
}
