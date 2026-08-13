// Copyright (c) 2026 Neutron Contributors -- MIT License
//
// Density function engine matching Minecraft 26.2 exactly.
//
// Density functions are loaded from the vanilla datapack JSONs (see
// `datapack_data.rs`) and evaluated at block coordinates. Semantics mirror
// `net.minecraft.world.level.levelgen.DensityFunctions`:
// - Arithmetic nodes (add/mul/min/max/abs/square/cube/half_negative/
//   quarter_negative/invert/squeeze/clamp)
// - Noise sampling (`noise`, `shifted_noise`, `shift_a`, `shift_b`)
// - `y_clamped_gradient`, `range_choice`, `interval_select`
// - `spline` with the exact `CubicSpline.Multipoint` float math
// - Marker wrappers (flat_cache/cache_2d/cache_once/cache_all_in_cell/
//   blend_density) which are pure caching for values and evaluate their
//   wrapped function directly; `interpolated` is handled by the chunk
//   generator's grid interpolator.
// - `find_top_surface` (used by the router's preliminarySurfaceLevel)

use std::collections::HashMap;
use std::rc::Rc;

use serde_json::Value;

use crate::noise::NormalNoise;

/// Shared density function handle.
pub type DF = Rc<DFNode>;

/// Marker wrapper kinds (pure value caching / grid interpolation).
///
/// In vanilla these are `DensityFunctions.Marker.Type` values handled by
/// `NoiseChunk.wrapNew()`.  They add caching or grid-interpolation semantics
/// around the wrapped function.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarkerKind {
    FlatCache,
    Cache2D,
    CacheOnce,
    CacheAllInCell,
    /// Handled by the chunk generator (grid interpolation); evaluates the
    /// wrapped function directly otherwise.
    Interpolated,
    /// With an empty blender (fresh generation) this is identity.
    BlendDensity,
}

/// Runtime state for NoiseChunk-style cell interpolation of every
/// `MarkerKind::Interpolated` node in the density tree.
///
/// Vanilla `NoiseChunk.wrap` replaces each Interpolated marker with a
/// `NoiseInterpolator` that samples the wrapped function on the cell grid
/// and trilinear-lerps. Noodle alone has **four** such markers; evaluating
/// them as point samples leaves tunnels solid (see run-016 / ProbeInterpDensity).
#[derive(Debug)]
pub struct CellInterpRuntime {
    /// `Rc::as_ptr` of each Interpolated marker DF (stable identity).
    pub ids: Vec<usize>,
    /// Per-marker samples on the chunk grid: index `(iy * stride_xz + iz) * stride_xz + ix`.
    pub grids: Vec<Vec<f64>>,
    pub stride_xz: usize,
    /// Current cell indices within the chunk grid.
    pub cell_ix: usize,
    pub cell_iy: usize,
    pub cell_iz: usize,
    /// In-cell factors in [0,1) — vanilla `updateForY/X/Z`.
    pub factor_x: f64,
    pub factor_y: f64,
    pub factor_z: f64,
}

impl CellInterpRuntime {
    /// Trilinear lerp for interpolator `i` at the current cell/factors.
    /// Order matches vanilla NoiseInterpolator: Y then X then Z.
    pub fn sample(&self, i: usize) -> f64 {
        let g = &self.grids[i];
        let s = self.stride_xz;
        let ix = self.cell_ix;
        let iy = self.cell_iy;
        let iz = self.cell_iz;
        let idx = |dx: usize, dy: usize, dz: usize| -> f64 {
            g[((iy + dy) * s + (iz + dz)) * s + (ix + dx)]
        };
        let n000 = idx(0, 0, 0);
        let n100 = idx(1, 0, 0);
        let n010 = idx(0, 1, 0);
        let n110 = idx(1, 1, 0);
        let n001 = idx(0, 0, 1);
        let n101 = idx(1, 0, 1);
        let n011 = idx(0, 1, 1);
        let n111 = idx(1, 1, 1);
        let v_xz00 = lerp(self.factor_y, n000, n010);
        let v_xz10 = lerp(self.factor_y, n100, n110);
        let v_xz01 = lerp(self.factor_y, n001, n011);
        let v_xz11 = lerp(self.factor_y, n101, n111);
        let v_z0 = lerp(self.factor_x, v_xz00, v_xz10);
        let v_z1 = lerp(self.factor_x, v_xz01, v_xz11);
        lerp(self.factor_z, v_z0, v_z1)
    }
}

/// Mutable caching state for marker wrappers — owned by the generator.
///
/// Passed as `&mut` into the `DensityEnv` during evaluation so that
/// markers can update their caches in-place without a RefCell.
#[derive(Debug)]
pub struct MarkerState {
    /// Last XZ position used by Cache2D (packed as `blockX << 32 | blockZ`).
    /// Also doubles as the counter key for CacheOnce.
    pub last_pos_2d: Option<i64>,
    /// Last value returned by Cache2D or CacheOnce.
    pub last_value: f64,
    /// Counter incremented per-block (for CacheOnce).
    pub interpolation_counter: i64,
    /// Counter incremented per-array-fill (for CacheOnce array mode).
    pub array_interpolation_counter: i64,
    /// FlatCache: pre-computed values at QuartPos quantized positions.
    /// Key = `(quart_x, quart_z)`, value = cached density.
    pub flat_cache: std::collections::HashMap<(i32, i32), f64>,
    /// CacheAllInCell: pre-computed cell values.
    /// Key = cell hash, value = flat array `[cellY * cellWx + cellX] * cellWz + cellZ`.
    pub cell_cache: std::collections::HashMap<String, Vec<f64>>,
    /// Cell width and height for CacheAllInCell indexing.
    pub cell_width: usize,
    pub cell_height: usize,
    /// Active cell-interpolation runtime (set during chunk fill).
    pub cell_interp: Option<CellInterpRuntime>,
}

impl MarkerState {
    pub fn new(cell_width: usize, cell_height: usize) -> Self {
        Self {
            last_pos_2d: None,
            last_value: 0.0,
            interpolation_counter: 0,
            array_interpolation_counter: 0,
            flat_cache: std::collections::HashMap::new(),
            cell_cache: std::collections::HashMap::new(),
            cell_width,
            cell_height,
            cell_interp: None,
        }
    }

    /// `QuartPos.fromBlock(x) = x >> 2`.
    #[inline]
    pub fn quart_from_block(x: i32) -> i32 {
        x >> 2
    }

    /// Packed XZ position (same packing as Vanilla `ChunkPos.pack`).
    #[inline]
    pub fn pack_pos_2d(x: i32, z: i32) -> i64 {
        ((x as i64) << 32) | (z as i64 & 0xFFFFFFFF)
    }
}

/// A spline value: a constant or a nested spline.
#[derive(Debug, Clone)]
pub enum SplineValue {
    Const(f32),
    Spline(SplineDef),
}

/// A cubic spline definition (`CubicSpline.Multipoint`).
#[derive(Debug, Clone)]
pub struct SplineDef {
    pub coordinate: DF,
    pub locations: Vec<f32>,
    pub derivatives: Vec<f32>,
    pub values: Vec<SplineValue>,
}

/// Debug wrapper around BlendedNoise (which does not implement Debug).
pub struct BlendedNode(pub crate::noise::BlendedNoise);

impl std::fmt::Debug for BlendedNode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "BlendedNoise")
    }
}

/// Density function node.
#[derive(Debug)]
pub enum DFNode {
    Const(f64),
    Mul(DF, DF),
    Add(DF, DF),
    Min(DF, DF),
    Max(DF, DF),
    Abs(DF),
    Square(DF),
    Cube(DF),
    HalfNegative(DF),
    QuarterNegative(DF),
    Invert(DF),
    Squeeze(DF),
    Clamp(DF, f64, f64),
    RangeChoice(DF, f64, f64, DF, DF),
    IntervalSelect(DF, Vec<f64>, Vec<DF>),
    /// (noise key, xz_scale, y_scale)
    Noise(String, f64, f64),
    /// (shift_x, shift_y, shift_z, xz_scale, y_scale, noise key)
    ShiftedNoise(DF, DF, DF, f64, f64, String),
    /// shift_a: offsetNoise.getValue(x*0.25, y*0.25, z*0.25) * 4.0
    ShiftA(String),
    /// shift_b: offsetNoise.getValue(z*0.25, y*0.25, x*0.25) * 4.0
    ShiftB(String),
    YClampedGradient(f64, f64, f64, f64),
    Spline(SplineDef),
    Marker(MarkerKind, DF),
    BlendAlpha,
    BlendOffset,
    /// (density, upper_bound, lower_bound, cell_height)
    FindTopSurface(DF, DF, i32, i32),
    Beardifier,
    EndIslands(f64),
    /// The overworld/nether/end base 3D noise (`old_blended_noise`).
    BlendedNoise(BlendedNode),
}

impl DFNode {
    /// Direct child density functions (for tree traversal).
    pub fn children(&self) -> Vec<&DF> {
        match self {
            DFNode::Mul(a, b) | DFNode::Add(a, b) | DFNode::Min(a, b) | DFNode::Max(a, b) => {
                vec![a, b]
            }
            DFNode::Abs(a)
            | DFNode::Square(a)
            | DFNode::Cube(a)
            | DFNode::HalfNegative(a)
            | DFNode::QuarterNegative(a)
            | DFNode::Invert(a)
            | DFNode::Squeeze(a)
            | DFNode::Clamp(a, _, _)
            | DFNode::Marker(_, a) => vec![a],
            DFNode::RangeChoice(input, _, _, in_range, out_of_range) => {
                vec![input, in_range, out_of_range]
            }
            DFNode::IntervalSelect(input, _, functions) => {
                let mut v: Vec<&DF> = vec![input];
                v.extend(functions.iter());
                v
            }
            DFNode::ShiftedNoise(sx, sy, sz, _, _, _) => vec![sx, sy, sz],
            DFNode::Spline(spline) => {
                let mut v = vec![&spline.coordinate];
                for val in &spline.values {
                    if let SplineValue::Spline(sub) = val {
                        v.push(&sub.coordinate);
                    }
                }
                v
            }
            DFNode::FindTopSurface(density, upper, _, _) => vec![density, upper],
            _ => vec![],
        }
    }
}

/// Evaluation environment: per-seed noise instances, blend state, and marker caching.
pub struct DensityEnv<'a> {
    pub x: i32,
    pub y: i32,
    pub z: i32,
    /// Noise key -> NormalNoise instance for the current seed.
    pub noises: &'a HashMap<String, NormalNoise>,
    /// blend alpha (1.0 for fresh generation).
    pub blend_alpha: f64,
    /// blend offset (0.0 for fresh generation).
    pub blend_offset: f64,
    /// Mutable marker state — only populated during chunk generation.
    /// When `None` (e.g. during aquifer or biome calls), markers
    /// evaluate their wrapped function directly (no caching).
    pub marker_state: Option<&'a mut MarkerState>,
}

impl<'a> DensityEnv<'a> {
    pub fn new(x: i32, y: i32, z: i32, noises: &'a HashMap<String, NormalNoise>) -> Self {
        Self {
            x,
            y,
            z,
            noises,
            blend_alpha: 1.0,
            blend_offset: 0.0,
            marker_state: None,
        }
    }

    /// Create a new DensityEnv with marker caching enabled.
    pub fn with_markers(
        x: i32,
        y: i32,
        z: i32,
        noises: &'a HashMap<String, NormalNoise>,
        state: &'a mut MarkerState,
    ) -> Self {
        Self {
            x,
            y,
            z,
            noises,
            blend_alpha: 1.0,
            blend_offset: 0.0,
            marker_state: Some(state),
        }
    }

    // sub() removed: use env.y = Y; compute(...); env.y = old_y inline
}

/// `Mth.floor(double)`.
#[inline]
fn floor(v: f64) -> i32 {
    v.floor() as i32
}

/// `Mth.clamp`.
#[inline]
fn clamp(v: f64, lo: f64, hi: f64) -> f64 {
    if v < lo {
        lo
    } else if v > hi {
        hi
    } else {
        v
    }
}

/// `Mth.lerp` (double).
#[inline]
pub fn lerp(alpha: f64, a: f64, b: f64) -> f64 {
    a + alpha * (b - a)
}

/// `Mth.lerp` (float) -- used by the spline math.
#[inline]
fn lerp_f(alpha: f32, a: f32, b: f32) -> f32 {
    a + alpha * (b - a)
}

/// `Mth.smoothstep` (double).
#[inline]
pub fn smoothstep(x: f64) -> f64 {
    x * x * x * (x * (x * 6.0 - 15.0) + 10.0)
}

/// Collect every `Marker(Interpolated, _)` node in the density tree (pre-order).
///
/// Used by the chunk generator to build per-marker sample grids matching
/// vanilla `NoiseChunk` interpolators (including all four noodle markers).
pub fn collect_interpolated(df: &DF, out: &mut Vec<DF>) {
    if let DFNode::Marker(MarkerKind::Interpolated, _) = &**df {
        out.push(df.clone());
    }
    for child in df.children() {
        collect_interpolated(child, out);
    }
}

/// Wrapped function of an Interpolated marker (panics if not Interpolated).
pub fn interpolated_wrapped(df: &DF) -> DF {
    match &**df {
        DFNode::Marker(MarkerKind::Interpolated, w) => w.clone(),
        _ => panic!("expected Interpolated marker"),
    }
}

/// Evaluate a density function at the environment coordinates.
///
/// The environment is taken by mutable reference because marker wrappers
/// (Cache2D, CacheOnce, etc.) need to update their caches in-place.
pub fn compute(df: &DF, env: &mut DensityEnv) -> f64 {
    match &**df {
        DFNode::Const(v) => *v,
        DFNode::Mul(a, b) => compute(a, env) * compute(b, env),
        DFNode::Add(a, b) => compute(a, env) + compute(b, env),
        DFNode::Min(a, b) => compute(a, env).min(compute(b, env)),
        DFNode::Max(a, b) => compute(a, env).max(compute(b, env)),
        DFNode::Abs(a) => compute(a, env).abs(),
        DFNode::Square(a) => {
            let v = compute(a, env);
            v * v
        }
        DFNode::Cube(a) => {
            let v = compute(a, env);
            v * v * v
        }
        DFNode::HalfNegative(a) => {
            let v = compute(a, env);
            if v > 0.0 {
                v
            } else {
                v * 0.5
            }
        }
        DFNode::QuarterNegative(a) => {
            let v = compute(a, env);
            if v > 0.0 {
                v
            } else {
                v * 0.25
            }
        }
        DFNode::Invert(a) => 1.0 / compute(a, env),
        DFNode::Squeeze(a) => {
            let v = clamp(compute(a, env), -1.0, 1.0);
            v / 2.0 - v * v * v / 24.0
        }
        DFNode::Clamp(a, lo, hi) => clamp(compute(a, env), *lo, *hi),
        DFNode::RangeChoice(input, min, max, in_range, out_of_range) => {
            let v = compute(input, env);
            if v >= *min && v < *max {
                compute(in_range, env)
            } else {
                compute(out_of_range, env)
            }
        }
        DFNode::IntervalSelect(input, thresholds, functions) => {
            let v = compute(input, env);
            for (i, t) in thresholds.iter().enumerate() {
                if v < *t {
                    return compute(&functions[i], env);
                }
            }
            compute(functions.last().unwrap(), env)
        }
        DFNode::Noise(key, xz_scale, y_scale) => {
            let noise = &env.noises[key];
            noise.get_value(
                env.x as f64 * xz_scale,
                env.y as f64 * y_scale,
                env.z as f64 * xz_scale,
            )
        }
        DFNode::ShiftedNoise(sx, sy, sz, xz_scale, y_scale, key) => {
            let noise = &env.noises[key];
            let x = env.x as f64 * xz_scale + compute(sx, env);
            let y = env.y as f64 * y_scale + compute(sy, env);
            let z = env.z as f64 * xz_scale + compute(sz, env);
            noise.get_value(x, y, z)
        }
        DFNode::ShiftA(key) => {
            let noise = &env.noises[key];
            noise.get_value(env.x as f64 * 0.25, 0.0, env.z as f64 * 0.25) * 4.0
        }
        DFNode::ShiftB(key) => {
            let noise = &env.noises[key];
            noise.get_value(env.z as f64 * 0.25, env.x as f64 * 0.25, 0.0) * 4.0
        }
        DFNode::YClampedGradient(from_y, to_y, from_v, to_v) => {
            let d = clamp((env.y as f64 - from_y) / (to_y - from_y), 0.0, 1.0);
            lerp(d, *from_v, *to_v)
        }
        DFNode::Spline(spline) => spline_sample(spline, env) as f64,
        DFNode::Marker(kind, wrapped) => {
            // Interpolated: use pre-sampled cell grid when filling a chunk.
            if *kind == MarkerKind::Interpolated {
                if let Some(state) = env.marker_state.as_ref() {
                    if let Some(rt) = state.cell_interp.as_ref() {
                        let id = Rc::as_ptr(df) as usize;
                        if let Some(i) = rt.ids.iter().position(|&x| x == id) {
                            return rt.sample(i);
                        }
                    }
                }
                // Fallback: point-sample wrapped (biome / SinglePoint paths).
                return compute(wrapped, env);
            }

            // If we have marker state in the env, use it. Otherwise evaluate directly.
            // Strategy: check cache with immutable borrow, call compute with mutable borrow,
            // then store in cache with mutable borrow. This avoids borrow conflicts.
            let cache_hit_value: Option<f64> = env
                .marker_state
                .as_ref()
                .map(|state| match kind {
                    MarkerKind::Cache2D => {
                        let pos2d = MarkerState::pack_pos_2d(env.x, env.z);
                        state
                            .last_pos_2d
                            .filter(|p| *p == pos2d)
                            .map(|_| state.last_value)
                    }
                    MarkerKind::CacheOnce => {
                        let counter = state.interpolation_counter;
                        state
                            .last_pos_2d
                            .filter(|p| *p == counter)
                            .map(|_| state.last_value)
                    }
                    MarkerKind::FlatCache => {
                        let qx = MarkerState::quart_from_block(env.x);
                        let qz = MarkerState::quart_from_block(env.z);
                        state.flat_cache.get(&(qx, qz)).copied()
                    }
                    MarkerKind::CacheAllInCell => {
                        let x = env.x % (state.cell_width as i32);
                        let y = env.y % (state.cell_height as i32);
                        let z = env.z % (state.cell_width as i32);
                        let x = if x < 0 {
                            x + state.cell_width as i32
                        } else {
                            x
                        };
                        let y = if y < 0 {
                            y + state.cell_height as i32
                        } else {
                            y
                        };
                        let z = if z < 0 {
                            z + state.cell_width as i32
                        } else {
                            z
                        };
                        let key = format!("{},{},{}", x, y, z);
                        state.cell_cache.get(&key).map(|cell_data| {
                            let idx =
                                ((state.cell_height as i32 - 1 - y) * state.cell_width as i32 + x)
                                    * state.cell_width as i32
                                    + z;
                            cell_data[idx as usize]
                        })
                    }
                    MarkerKind::Interpolated | MarkerKind::BlendDensity => None,
                })
                .flatten();

            if let Some(v) = cache_hit_value {
                return v;
            }

            // Cache miss — compute value
            let v = compute(wrapped, env);

            // Store in cache
            if let Some(state) = &mut env.marker_state {
                match kind {
                    MarkerKind::Cache2D | MarkerKind::CacheOnce => {
                        let pos2d = if *kind == MarkerKind::Cache2D {
                            MarkerState::pack_pos_2d(env.x, env.z)
                        } else {
                            state.interpolation_counter
                        };
                        state.last_pos_2d = Some(pos2d);
                        state.last_value = v;
                    }
                    MarkerKind::FlatCache => {
                        let qx = MarkerState::quart_from_block(env.x);
                        let qz = MarkerState::quart_from_block(env.z);
                        state.flat_cache.insert((qx, qz), v);
                    }
                    MarkerKind::CacheAllInCell => {
                        let x = env.x % state.cell_width as i32;
                        let y = env.y % state.cell_height as i32;
                        let z = env.z % state.cell_width as i32;
                        let x = if x < 0 {
                            x + state.cell_width as i32
                        } else {
                            x
                        };
                        let y = if y < 0 {
                            y + state.cell_height as i32
                        } else {
                            y
                        };
                        let z = if z < 0 {
                            z + state.cell_width as i32
                        } else {
                            z
                        };
                        let key = format!("{},{},{}", x, y, z);
                        state.cell_cache.insert(key, vec![v]);
                    }
                    _ => {}
                }
            }
            v
        }
        DFNode::BlendAlpha => env.blend_alpha,
        DFNode::BlendOffset => env.blend_offset,
        DFNode::FindTopSurface(density, upper_bound, lower_bound, cell_height) => {
            let top_y = floor(compute(upper_bound, env) / *cell_height as f64) * *cell_height;
            if top_y <= *lower_bound {
                return *lower_bound as f64;
            }
            let mut block_y = top_y;
            while block_y >= *lower_bound {
                let old_y = env.y;
                env.y = block_y;
                let result = compute(density, env);
                env.y = old_y;
                if result > 0.0 {
                    return block_y as f64;
                }
                block_y -= *cell_height;
            }
            *lower_bound as f64
        }
        DFNode::Beardifier => 0.0,
        DFNode::BlendedNoise(bn) => bn.0.compute(env.x, env.y, env.z),
        DFNode::EndIslands(offset) => {
            // Only used by the End; conservative approximation is not needed
            // for overworld parity.
            let _ = offset;
            0.0
        }
    }
}

/// `CubicSpline.sample` -- float math.
fn spline_sample(spline: &SplineDef, env: &mut DensityEnv) -> f32 {
    let input = compute(&spline.coordinate, env) as f32;
    let last = spline.locations.len() - 1;
    let start = find_interval_start(&spline.locations, input);
    if start < 0 {
        return linear_extend(
            input,
            &spline.locations,
            sample_spline_value(&spline.values[0], env),
            spline.derivatives[0],
        );
    }
    if start as usize == last {
        return linear_extend(
            input,
            &spline.locations,
            sample_spline_value(&spline.values[last], env),
            spline.derivatives[last],
        );
    }
    let x1 = spline.locations[start as usize];
    let x2 = spline.locations[start as usize + 1];
    let t = (input - x1) / (x2 - x1);
    let y1 = sample_spline_value(&spline.values[start as usize], env);
    let y2 = sample_spline_value(&spline.values[start as usize + 1], env);
    let d1 = spline.derivatives[start as usize];
    let d2 = spline.derivatives[start as usize + 1];
    let a = d1 * (x2 - x1) - (y2 - y1);
    let b = -d2 * (x2 - x1) + (y2 - y1);
    lerp_f(t, y1, y2) + t * (1.0 - t) * lerp_f(t, a, b)
}

fn sample_spline_value(v: &SplineValue, env: &mut DensityEnv) -> f32 {
    match v {
        SplineValue::Const(c) => *c,
        SplineValue::Spline(s) => spline_sample(s, env),
    }
}

/// `Multipoint.linearExtend`.
fn linear_extend(input: f32, locations: &[f32], value: f32, derivative: f32) -> f32 {
    if derivative == 0.0 {
        value
    } else {
        value + derivative * (input - locations[0])
    }
}

/// `Mth.binarySearch(0, len, i -> input < locations[i]) - 1`.
fn find_interval_start(locations: &[f32], input: f32) -> i32 {
    let mut lo = 0i32;
    let mut hi = locations.len() as i32;
    while lo < hi {
        let mid = (lo + hi) / 2;
        if input < locations[mid as usize] {
            hi = mid;
        } else {
            lo = mid + 1;
        }
    }
    lo - 1
}

// ---------------------------------------------------------------------------
// JSON loading
// ---------------------------------------------------------------------------

/// A registry of named density functions built from the datapack JSONs.
pub struct DensityRegistry {
    /// name (e.g. "overworld/offset") -> function.
    functions: HashMap<String, DF>,
    /// noise key -> (firstOctave, amplitudes).
    noise_params: HashMap<String, (i32, Vec<f64>)>,
    /// Terrain random seed pair for the base_3d_noise (RandomState re-seeds
    /// the BlendedNoise with `fromHashOf("terrain")`).
    terrain_random: Option<(u64, u64)>,
}

impl DensityRegistry {
    /// Build the registry from the embedded datapack data.
    pub fn build() -> Self {
        let mut reg = Self {
            functions: HashMap::new(),
            noise_params: HashMap::new(),
            terrain_random: None,
        };
        // Load all noise params.
        for path in crate::datapack_data::all_paths() {
            if let Some(key) = path
                .strip_prefix("noise/")
                .and_then(|k| k.strip_suffix(".json"))
            {
                if let Some(json) = crate::datapack_data::datapack_json(path) {
                    if let Some(params) = parse_noise_json(json) {
                        reg.noise_params.insert(key.to_string(), params);
                    }
                }
            }
        }
        // Parse density functions lazily on first access.
        reg
    }

    /// Get or parse a density function by datapack key (e.g. "overworld/offset").
    pub fn function(&mut self, key: &str) -> DF {
        if let Some(f) = self.functions.get(key) {
            return f.clone();
        }
        let path = format!("density_function/{key}.json");
        let json = crate::datapack_data::datapack_json(&path)
            .unwrap_or_else(|| panic!("missing density function JSON: {path}"));
        let value: Value = serde_json::from_str(json).expect("invalid density function JSON");
        let f = self.parse(&value);
        self.functions.insert(key.to_string(), f.clone());
        f
    }

    /// Parse a density function JSON value into a node.
    pub fn parse(&mut self, value: &Value) -> DF {
        match value {
            Value::Number(n) => Rc::new(DFNode::Const(n.as_f64().unwrap())),
            Value::String(s) => {
                // A reference to a named density function (e.g. "minecraft:overworld/offset").
                let key = s.strip_prefix("minecraft:").unwrap_or(s);
                self.function(key)
            }
            Value::Object(obj) => {
                let t = obj.get("type").and_then(|v| v.as_str()).unwrap_or("");
                let t = t.strip_prefix("minecraft:").unwrap_or(t);
                match t {
                    "constant" => Rc::new(DFNode::Const(0.0)),
                    "mul" => Rc::new(DFNode::Mul(
                        self.parse(&obj["argument1"]),
                        self.parse(&obj["argument2"]),
                    )),
                    "add" => Rc::new(DFNode::Add(
                        self.parse(&obj["argument1"]),
                        self.parse(&obj["argument2"]),
                    )),
                    "min" => Rc::new(DFNode::Min(
                        self.parse(&obj["argument1"]),
                        self.parse(&obj["argument2"]),
                    )),
                    "max" => Rc::new(DFNode::Max(
                        self.parse(&obj["argument1"]),
                        self.parse(&obj["argument2"]),
                    )),
                    "abs" => Rc::new(DFNode::Abs(self.parse(&obj["argument"]))),
                    "square" => Rc::new(DFNode::Square(self.parse(&obj["argument"]))),
                    "cube" => Rc::new(DFNode::Cube(self.parse(&obj["argument"]))),
                    "half_negative" => Rc::new(DFNode::HalfNegative(self.parse(&obj["argument"]))),
                    "quarter_negative" => {
                        Rc::new(DFNode::QuarterNegative(self.parse(&obj["argument"])))
                    }
                    "invert" => Rc::new(DFNode::Invert(self.parse(&obj["argument"]))),
                    "squeeze" => Rc::new(DFNode::Squeeze(self.parse(&obj["argument"]))),
                    "clamp" => {
                        let inner = self.parse(&obj["input"]);
                        let lo = obj["min"].as_f64().unwrap();
                        let hi = obj["max"].as_f64().unwrap();
                        Rc::new(DFNode::Clamp(inner, lo, hi))
                    }
                    "range_choice" => {
                        let input = self.parse(&obj["input"]);
                        let min = obj["min_inclusive"].as_f64().unwrap();
                        let max = obj["max_exclusive"].as_f64().unwrap();
                        let in_range = self.parse(&obj["when_in_range"]);
                        let out_of_range = self.parse(&obj["when_out_of_range"]);
                        Rc::new(DFNode::RangeChoice(input, min, max, in_range, out_of_range))
                    }
                    "interval_select" => {
                        let input = self.parse(&obj["input"]);
                        let thresholds: Vec<f64> = obj["thresholds"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .map(|v| v.as_f64().unwrap())
                            .collect();
                        let functions: Vec<DF> = obj["functions"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .map(|v| self.parse(v))
                            .collect();
                        Rc::new(DFNode::IntervalSelect(input, thresholds, functions))
                    }
                    "noise" => {
                        let key = obj["noise"]
                            .as_str()
                            .unwrap()
                            .trim_start_matches("minecraft:")
                            .to_string();
                        let xz_scale = obj.get("xz_scale").and_then(|v| v.as_f64()).unwrap_or(1.0);
                        let y_scale = obj.get("y_scale").and_then(|v| v.as_f64()).unwrap_or(0.5);
                        Rc::new(DFNode::Noise(key, xz_scale, y_scale))
                    }
                    "shifted_noise" => {
                        let sx = self.parse(&obj["shift_x"]);
                        let sy = self.parse(&obj["shift_y"]);
                        let sz = self.parse(&obj["shift_z"]);
                        let xz_scale = obj["xz_scale"].as_f64().unwrap();
                        let y_scale = obj.get("y_scale").and_then(|v| v.as_f64()).unwrap_or(0.0);
                        let key = obj["noise"]
                            .as_str()
                            .unwrap()
                            .trim_start_matches("minecraft:")
                            .to_string();
                        Rc::new(DFNode::ShiftedNoise(sx, sy, sz, xz_scale, y_scale, key))
                    }
                    "shift_a" => {
                        let key = obj["argument"]
                            .as_str()
                            .unwrap()
                            .trim_start_matches("minecraft:")
                            .to_string();
                        Rc::new(DFNode::ShiftA(key))
                    }
                    "shift_b" => {
                        let key = obj["argument"]
                            .as_str()
                            .unwrap()
                            .trim_start_matches("minecraft:")
                            .to_string();
                        Rc::new(DFNode::ShiftB(key))
                    }
                    "y_clamped_gradient" => Rc::new(DFNode::YClampedGradient(
                        obj["from_y"].as_f64().unwrap(),
                        obj["to_y"].as_f64().unwrap(),
                        obj["from_value"].as_f64().unwrap(),
                        obj["to_value"].as_f64().unwrap(),
                    )),
                    "spline" => {
                        let spline = self.parse_spline(&obj["spline"]);
                        Rc::new(DFNode::Spline(spline))
                    }
                    "flat_cache" => {
                        let inner = self.parse(&obj["argument"]);
                        Rc::new(DFNode::Marker(MarkerKind::FlatCache, inner))
                    }
                    "cache_2d" => {
                        let inner = self.parse(&obj["argument"]);
                        Rc::new(DFNode::Marker(MarkerKind::Cache2D, inner))
                    }
                    "cache_once" => {
                        let inner = self.parse(&obj["argument"]);
                        Rc::new(DFNode::Marker(MarkerKind::CacheOnce, inner))
                    }
                    "cache_all_in_cell" => {
                        let inner = self.parse(&obj["argument"]);
                        Rc::new(DFNode::Marker(MarkerKind::CacheAllInCell, inner))
                    }
                    "interpolated" => {
                        let inner = self.parse(&obj["argument"]);
                        Rc::new(DFNode::Marker(MarkerKind::Interpolated, inner))
                    }
                    "blend_density" => {
                        let inner = self.parse(&obj["argument"]);
                        Rc::new(DFNode::Marker(MarkerKind::BlendDensity, inner))
                    }
                    "blend_alpha" => Rc::new(DFNode::BlendAlpha),
                    "blend_offset" => Rc::new(DFNode::BlendOffset),
                    "find_top_surface" => {
                        let density = self.parse(&obj["density"]);
                        let upper_bound = self.parse(&obj["upper_bound"]);
                        let lower_bound = obj["lower_bound"].as_i64().unwrap() as i32;
                        let cell_height = obj["cell_height"].as_i64().unwrap() as i32;
                        Rc::new(DFNode::FindTopSurface(
                            density,
                            upper_bound,
                            lower_bound,
                            cell_height,
                        ))
                    }
                    "beardifier" => Rc::new(DFNode::Beardifier),
                    "old_blended_noise" => {
                        let (tlo, thi) = self
                            .terrain_random
                            .expect("terrain random must be set before parsing");
                        Rc::new(DFNode::BlendedNoise(BlendedNode(
                            crate::noise::BlendedNoise::with_random(
                                crate::rng::Xoroshiro128::from_raw(tlo, thi),
                                obj["xz_scale"].as_f64().unwrap(),
                                obj["y_scale"].as_f64().unwrap(),
                                obj["xz_factor"].as_f64().unwrap(),
                                obj["y_factor"].as_f64().unwrap(),
                                obj["smear_scale_multiplier"].as_f64().unwrap(),
                            ),
                        )))
                    }
                    other => {
                        panic!("unhandled density function type: {other}");
                    }
                }
            }
            other => panic!("unhandled density function value: {other}"),
        }
    }

    fn parse_spline(&mut self, value: &Value) -> SplineDef {
        let coordinate = self.parse(&value["coordinate"]);
        let points = value["points"].as_array().unwrap();
        let mut locations = Vec::with_capacity(points.len());
        let mut derivatives = Vec::with_capacity(points.len());
        let mut values = Vec::with_capacity(points.len());
        for p in points {
            locations.push(p["location"].as_f64().unwrap() as f32);
            derivatives.push(p["derivative"].as_f64().unwrap() as f32);
            let v = &p["value"];
            if v.is_number() {
                values.push(SplineValue::Const(v.as_f64().unwrap() as f32));
            } else if v.get("coordinate").is_some() {
                values.push(SplineValue::Spline(self.parse_spline(v)));
            } else {
                panic!("unhandled spline value: {v}");
            }
        }
        SplineDef {
            coordinate,
            locations,
            derivatives,
            values,
        }
    }

    /// Look up a noise parameter set.
    pub fn noise_params(&self, key: &str) -> &(i32, Vec<f64>) {
        self.noise_params
            .get(key)
            .unwrap_or_else(|| panic!("missing noise params for {key}"))
    }

    /// All noise registry keys.
    pub fn noise_keys(&self) -> impl Iterator<Item = &String> {
        self.noise_params.keys()
    }

    /// Set the terrain random seed pair (derived from the world seed).
    pub fn set_terrain_random(&mut self, lo: u64, hi: u64) {
        self.terrain_random = Some((lo, hi));
    }
}

fn parse_noise_json(json: &str) -> Option<(i32, Vec<f64>)> {
    let value: Value = serde_json::from_str(json).ok()?;
    let first_octave = value.get("firstOctave")?.as_i64()? as i32;
    let amplitudes: Vec<f64> = value
        .get("amplitudes")?
        .as_array()?
        .iter()
        .map(|v| v.as_f64().unwrap())
        .collect();
    Some((first_octave, amplitudes))
}
