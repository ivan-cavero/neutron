//! Synth noise matching Minecraft 26.2 (`levelgen.synth`).
//!
//! - [`ImprovedNoise`]: Perlin with 5-arg `noise(x,y,z,yScale,yFudge)`,
//!   Simplex gradient table, 256-entry permutation
//! - [`PerlinNoise`]: octave composite (positional + legacy)
//! - [`NormalNoise`]: paired Perlin used by density functions
//! - [`BlendedNoise`]: `base_3d_noise` / old 3D terrain
//!
//! Positional Perlin uses MD5-seeded octaves; legacy Perlin consumes a shared
//! RNG (BlendedNoise). Verify against `tools/worldgen-probe`.
//!
//! Copyright (c) 2026 Neutron Contributors -- MIT License

use crate::rng::Xoroshiro128;

/// Simplex gradient table (`SimplexNoise.GRADIENT`).
const GRADIENT: [[f64; 3]; 16] = [
    [1.0, 1.0, 0.0],
    [-1.0, 1.0, 0.0],
    [1.0, -1.0, 0.0],
    [-1.0, -1.0, 0.0],
    [1.0, 0.0, 1.0],
    [-1.0, 0.0, 1.0],
    [1.0, 0.0, -1.0],
    [-1.0, 0.0, -1.0],
    [0.0, 1.0, 1.0],
    [0.0, -1.0, 1.0],
    [0.0, 1.0, -1.0],
    [0.0, -1.0, -1.0],
    [1.0, 1.0, 0.0],
    [0.0, -1.0, 1.0],
    [-1.0, 1.0, 0.0],
    [0.0, -1.0, -1.0],
];

/// `SimplexNoise.dot(gradient, x, y, z)`.
#[inline]
fn grad_dot(hash: i32, x: f64, y: f64, z: f64) -> f64 {
    let g = GRADIENT[(hash & 0xF) as usize];
    g[0] * x + g[1] * y + g[2] * z
}

/// `Mth.smoothstep(x)`.
#[inline]
pub fn smoothstep(x: f64) -> f64 {
    x * x * x * (x * (x * 6.0 - 15.0) + 10.0)
}

/// `Mth.lerp(alpha, a, b)`.
#[inline]
pub fn lerp(alpha: f64, a: f64, b: f64) -> f64 {
    a + alpha * (b - a)
}

/// `Mth.lerp2`.
#[inline]
pub fn lerp2(a1: f64, a2: f64, x00: f64, x10: f64, x01: f64, x11: f64) -> f64 {
    lerp(a2, lerp(a1, x00, x10), lerp(a1, x01, x11))
}

/// `Mth.lerp3`.
#[inline]
pub fn lerp3(
    a1: f64,
    a2: f64,
    a3: f64,
    x000: f64,
    x100: f64,
    x010: f64,
    x110: f64,
    x001: f64,
    x101: f64,
    x011: f64,
    x111: f64,
) -> f64 {
    lerp(
        a3,
        lerp2(a1, a2, x000, x100, x010, x110),
        lerp2(a1, a2, x001, x101, x011, x111),
    )
}

/// `Mth.clampedLerp(factor, min, max)`.
#[inline]
pub fn clamped_lerp(factor: f64, min: f64, max: f64) -> f64 {
    if factor < 0.0 {
        min
    } else if factor > 1.0 {
        max
    } else {
        lerp(factor, min, max)
    }
}

/// `PerlinNoise.wrap(x)`: wraps coordinates into [-2^25, 2^25] to avoid
/// floating-point precision loss at high octave frequencies.
#[inline]
pub fn wrap(x: f64) -> f64 {
    x - (x / 3.355_443_2E7 + 0.5).floor() * 3.355_443_2E7
}

/// Core Perlin noise (`net.minecraft.world.level.levelgen.synth.ImprovedNoise`).
pub struct ImprovedNoise {
    p: [u8; 256],
    pub xo: f64,
    pub yo: f64,
    pub zo: f64,
}

impl ImprovedNoise {
    /// Create a new ImprovedNoise from a PRNG.
    ///
    /// Matches `ImprovedNoise(RandomSource)`:
    /// - `xo/yo/zo = random.nextDouble() * 256.0`
    /// - `p[i] = i`, then Fisher-Yates with `random.nextInt(256 - i)`.
    pub fn new(random: &mut Xoroshiro128) -> Self {
        let xo = random.next_f64() * 256.0;
        let yo = random.next_f64() * 256.0;
        let zo = random.next_f64() * 256.0;
        let mut p = [0u8; 256];
        for (i, slot) in p.iter_mut().enumerate() {
            *slot = i as u8;
        }
        for i in 0..256usize {
            let offset = random.next_int((256 - i) as i32) as usize;
            p.swap(i, i + offset);
        }
        Self { p, xo, yo, zo }
    }

    /// 3-arg noise.
    pub fn noise(&self, x: f64, y: f64, z: f64) -> f64 {
        self.noise5(x, y, z, 0.0, 0.0)
    }

    /// 5-arg noise with Y-scale fudging (`noise(x, y, z, yScale, yFudge)`).
    pub fn noise5(&self, x_in: f64, y_in: f64, z_in: f64, y_scale: f64, y_fudge: f64) -> f64 {
        let x = x_in + self.xo;
        let y = y_in + self.yo;
        let z = z_in + self.zo;
        let xf = x.floor() as i32;
        let yf = y.floor() as i32;
        let zf = z.floor() as i32;
        let xr = x - xf as f64;
        let yr = y - yf as f64;
        let zr = z - zf as f64;
        let yr_fudge = if y_scale != 0.0 {
            let fudge_limit = if y_fudge >= 0.0 && y_fudge < yr {
                y_fudge
            } else {
                yr
            };
            (fudge_limit / y_scale + 1.0e-7f32 as f64).floor() * y_scale
        } else {
            0.0
        };
        self.sample_and_lerp(xf, yf, zf, xr, yr - yr_fudge, zr, yr)
    }

    /// `p(x) = p[x & 0xFF]` (unsigned byte).
    #[inline]
    fn p(&self, x: i32) -> usize {
        self.p[(x & 0xFF) as usize] as usize
    }

    fn sample_and_lerp(
        &self,
        x: i32,
        y: i32,
        z: i32,
        xr: f64,
        yr: f64,
        zr: f64,
        yr_original: f64,
    ) -> f64 {
        let x0 = self.p(x);
        let x1 = self.p(x + 1);
        let xy00 = self.p((x0 as i32 + y) as i32);
        let xy01 = self.p((x0 as i32 + y + 1) as i32);
        let xy10 = self.p((x1 as i32 + y) as i32);
        let xy11 = self.p((x1 as i32 + y + 1) as i32);
        let d000 = grad_dot(self.p((xy00 as i32 + z) as i32) as i32, xr, yr, zr);
        let d100 = grad_dot(self.p((xy10 as i32 + z) as i32) as i32, xr - 1.0, yr, zr);
        let d010 = grad_dot(self.p((xy01 as i32 + z) as i32) as i32, xr, yr - 1.0, zr);
        let d110 = grad_dot(
            self.p((xy11 as i32 + z) as i32) as i32,
            xr - 1.0,
            yr - 1.0,
            zr,
        );
        let d001 = grad_dot(
            self.p((xy00 as i32 + z + 1) as i32) as i32,
            xr,
            yr,
            zr - 1.0,
        );
        let d101 = grad_dot(
            self.p((xy10 as i32 + z + 1) as i32) as i32,
            xr - 1.0,
            yr,
            zr - 1.0,
        );
        let d011 = grad_dot(
            self.p((xy01 as i32 + z + 1) as i32) as i32,
            xr,
            yr - 1.0,
            zr - 1.0,
        );
        let d111 = grad_dot(
            self.p((xy11 as i32 + z + 1) as i32) as i32,
            xr - 1.0,
            yr - 1.0,
            zr - 1.0,
        );
        let x_alpha = smoothstep(xr);
        let y_alpha = smoothstep(yr_original);
        let z_alpha = smoothstep(zr);
        lerp3(
            x_alpha, y_alpha, z_alpha, d000, d100, d010, d110, d001, d101, d011, d111,
        )
    }
}

/// Multi-octave composite noise (`net.minecraft.world.level.levelgen.synth.PerlinNoise`).
pub struct PerlinNoise {
    /// `noiseLevels[i]` is the `ImprovedNoise` for octave `firstOctave + i`
    /// (or None for zero-amplitude octaves).
    noise_levels: Vec<Option<ImprovedNoise>>,
    first_octave: i32,
    amplitudes: Vec<f64>,
    lowest_freq_input_factor: f64,
    lowest_freq_value_factor: f64,
    max_value: f64,
}

impl PerlinNoise {
    /// `makeAmplitudes(IntSortedSet)`: returns (firstOctave, amplitudes) where
    /// `firstOctave = -lowFreqOctaves` and amplitudes are 1.0 at each octave.
    fn make_amplitudes(octaves: &[i32]) -> (i32, Vec<f64>) {
        assert!(!octaves.is_empty(), "Need some octaves!");
        let low = *octaves.iter().min().unwrap();
        let high = *octaves.iter().max().unwrap();
        let low_freq_octaves = -low;
        let first_octave = -low_freq_octaves;
        let count = (low_freq_octaves + high) + 1;
        let mut amplitudes = vec![0.0; count as usize];
        for &o in octaves {
            amplitudes[(o + low_freq_octaves) as usize] = 1.0;
        }
        (first_octave, amplitudes)
    }

    fn init_factors(first_octave: i32, amplitudes: &[f64]) -> (f64, f64, i32) {
        let zero_octave_index = -first_octave;
        let octaves = amplitudes.len();
        let lowest_freq_input_factor = 2f64.powi(-zero_octave_index);
        let lowest_freq_value_factor =
            2f64.powi((octaves - 1) as i32) / (2f64.powi(octaves as i32) - 1.0);
        (
            lowest_freq_input_factor,
            lowest_freq_value_factor,
            zero_octave_index,
        )
    }

    fn edge_value(
        noise_levels: &[Option<ImprovedNoise>],
        amplitudes: &[f64],
        value_factor0: f64,
        noise_value: f64,
    ) -> f64 {
        let mut value = 0.0;
        let mut value_factor = value_factor0;
        for (i, level) in noise_levels.iter().enumerate() {
            if level.is_some() {
                value += amplitudes[i] * noise_value * value_factor;
            }
            value_factor /= 2.0;
        }
        value
    }

    /// Positional construction (`useNewInitialization=true`): each octave gets
    /// `MD5("octave_N")` XOR the given positional factory seeds.
    pub fn create_positional(
        first_octave: i32,
        amplitudes: &[f64],
        pos_lo: u64,
        pos_hi: u64,
    ) -> Self {
        let octaves = amplitudes.len();
        let mut noise_levels: Vec<Option<ImprovedNoise>> = (0..octaves).map(|_| None).collect();
        for i in 0..octaves {
            if amplitudes[i] == 0.0 {
                continue;
            }
            let octave = first_octave + i as i32;
            let mut octave_rng =
                Xoroshiro128::from_raw(pos_lo, pos_hi).from_hash_of(&format!("octave_{octave}"));
            noise_levels[i] = Some(ImprovedNoise::new(&mut octave_rng));
        }
        let (lowest_freq_input_factor, lowest_freq_value_factor, _) =
            Self::init_factors(first_octave, amplitudes);
        let max_value = Self::edge_value(&noise_levels, amplitudes, lowest_freq_value_factor, 2.0);
        Self {
            noise_levels,
            first_octave,
            amplitudes: amplitudes.to_vec(),
            lowest_freq_input_factor,
            lowest_freq_value_factor,
            max_value,
        }
    }

    /// Legacy construction (`createLegacyForBlendedNoise`): consumes the shared
    /// RNG directly, one `ImprovedNoise` per non-zero-amplitude octave.
    pub fn create_legacy(random: &mut Xoroshiro128, octave_set: &[i32]) -> Self {
        let (first_octave, amplitudes) = Self::make_amplitudes(octave_set);
        let octaves = amplitudes.len();
        let (lowest_freq_input_factor, lowest_freq_value_factor, zero_octave_index) =
            Self::init_factors(first_octave, &amplitudes);

        let mut noise_levels: Vec<Option<ImprovedNoise>> = (0..octaves).map(|_| None).collect();
        let zero_octave = ImprovedNoise::new(random);
        let zero_octave_index = zero_octave_index as usize;
        if zero_octave_index < octaves && amplitudes[zero_octave_index] != 0.0 {
            noise_levels[zero_octave_index] = Some(zero_octave);
        }
        for i in (0..zero_octave_index).rev() {
            if i < octaves {
                if amplitudes[i] != 0.0 {
                    noise_levels[i] = Some(ImprovedNoise::new(random));
                } else {
                    random.consume(262);
                }
            } else {
                random.consume(262);
            }
        }
        let max_value = Self::edge_value(&noise_levels, &amplitudes, lowest_freq_value_factor, 2.0);
        Self {
            noise_levels,
            first_octave,
            amplitudes,
            lowest_freq_input_factor,
            lowest_freq_value_factor,
            max_value,
        }
    }

    /// `getValue(x, y, z)`.
    pub fn get_value(&self, x: f64, y: f64, z: f64) -> f64 {
        self.get_value5(x, y, z, 0.0, 0.0)
    }

    /// `getValue(x, y, z, yScale, yFudge)`.
    pub fn get_value5(&self, x: f64, y: f64, z: f64, y_scale: f64, y_fudge: f64) -> f64 {
        let mut value = 0.0;
        let mut factor = self.lowest_freq_input_factor;
        let mut value_factor = self.lowest_freq_value_factor;
        for (i, level) in self.noise_levels.iter().enumerate() {
            if let Some(noise) = level {
                let nv = noise.noise5(
                    wrap(x * factor),
                    wrap(y * factor),
                    wrap(z * factor),
                    y_scale * factor,
                    y_fudge * factor,
                );
                value += self.amplitudes[i] * nv * value_factor;
            }
            factor *= 2.0;
            value_factor /= 2.0;
        }
        value
    }

    /// `getOctaveNoise(i)`: octave access used by BlendedNoise.
    pub fn get_octave_noise(&self, i: usize) -> Option<&ImprovedNoise> {
        let idx = self.noise_levels.len() - 1 - i;
        self.noise_levels[idx].as_ref()
    }

    /// `maxBrokenValue(yScale)`.
    pub fn max_broken_value(&self, y_scale: f64) -> f64 {
        Self::edge_value(
            &self.noise_levels,
            &self.amplitudes,
            self.lowest_freq_value_factor,
            y_scale + 2.0,
        )
    }

    pub fn max_value(&self) -> f64 {
        self.max_value
    }

    /// Debug: dump octave states (octave number, xo/yo/zo per level).
    pub fn dump_octaves(&self) {
        for (i, level) in self.noise_levels.iter().enumerate() {
            match level {
                Some(n) => println!(
                    "  [{}] octave={} xo={:.17e} yo={:.17e} zo={:.17e}",
                    i,
                    self.first_octave + i as i32,
                    n.xo,
                    n.yo,
                    n.zo
                ),
                None => println!("  [{}] octave={} null", i, self.first_octave + i as i32),
            }
        }
    }

    /// Sample noise at a position, optionally using the amplitude offset.
    ///
    /// When `use_offset` is true, this is equivalent to `get_value(x, y, z)`.
    pub fn sample(&self, x: f64, y: f64, z: f64, use_offset: bool) -> f64 {
        if use_offset {
            self.get_value(x, y, z)
        } else {
            // Without offset, skip the amplitude weighting.
            let mut value = 0.0;
            let mut factor = self.lowest_freq_input_factor;
            for (i, level) in self.noise_levels.iter().enumerate() {
                if let Some(noise) = level {
                    let nv = noise.noise5(
                        wrap(x * factor),
                        wrap(y * factor),
                        wrap(z * factor),
                        0.0,
                        0.0,
                    );
                    value += self.amplitudes[i] * nv;
                }
                factor *= 2.0;
            }
            value
        }
    }
}

/// Octave Perlin noise — wrapper around `PerlinNoise`.
///
/// Provides the `new_with_first_octave` and `sample` methods used by the
/// biome source and chunk generator.
pub struct OctavePerlinNoise {
    inner: PerlinNoise,
}

impl OctavePerlinNoise {
    /// Create a new octave Perlin noise from an RNG, first octave, and amplitudes.
    pub fn new_with_first_octave(
        random: &mut Xoroshiro128,
        first_octave: i32,
        amplitudes: &[f64],
    ) -> Self {
        let mut octaves = Vec::new();
        for (i, &amp) in amplitudes.iter().enumerate() {
            if amp != 0.0 {
                octaves.push(first_octave + i as i32);
            }
        }
        let inner = PerlinNoise::create_legacy(random, &octaves);
        Self { inner }
    }

    /// Sample noise at a position, optionally using the amplitude offset.
    pub fn sample(&self, x: f64, y: f64, z: f64, use_offset: bool) -> f64 {
        if use_offset {
            self.inner.get_value(x, y, z)
        } else {
            let mut value = 0.0;
            let mut factor = self.inner.lowest_freq_input_factor;
            for (i, level) in self.inner.noise_levels.iter().enumerate() {
                if let Some(noise) = level {
                    let nv = noise.noise5(
                        wrap(x * factor),
                        wrap(y * factor),
                        wrap(z * factor),
                        0.0,
                        0.0,
                    );
                    value += self.inner.amplitudes[i] * nv;
                }
                factor *= 2.0;
            }
            value
        }
    }
}

/// Create an octave Perlin noise from an RNG and parameters.
pub fn create_octave_noise(
    random: &mut Xoroshiro128,
    first_octave: i32,
    amplitudes: &[f64],
) -> OctavePerlinNoise {
    OctavePerlinNoise::new_with_first_octave(random, first_octave, amplitudes)
}

/// Normalized noise (`net.minecraft.world.level.levelgen.synth.NormalNoise`).
///
/// Two PerlinNoise instances, the second sampled with INPUT_FACTOR-scaled
/// coordinates, combined via a deviation-derived value factor.
pub struct NormalNoise {
    first: PerlinNoise,
    second: PerlinNoise,
    value_factor: f64,
    max_value: f64,
}

impl NormalNoise {
    const INPUT_FACTOR: f64 = 1.018_126_888_217_522_7;

    /// `NormalNoise.create(random, params)` using the positional factory seeds
    /// derived from `noise_key` (via `PositionalRandomFactory.fromHashOf`).
    pub fn create(noise_lo: u64, noise_hi: u64, first_octave: i32, amplitudes: &[f64]) -> Self {
        // `random.forkPositional()` twice: once per PerlinNoise.
        let mut noise_rng = Xoroshiro128::from_raw(noise_lo, noise_hi);
        let (l1, h1) = noise_rng.fork_positional();
        let first = PerlinNoise::create_positional(first_octave, amplitudes, l1, h1);
        let (l2, h2) = noise_rng.fork_positional();
        let second = PerlinNoise::create_positional(first_octave, amplitudes, l2, h2);

        let mut min_octave = i32::MAX;
        let mut max_octave = i32::MIN;
        for (i, &a) in amplitudes.iter().enumerate() {
            if a != 0.0 {
                min_octave = min_octave.min(i as i32);
                max_octave = max_octave.max(i as i32);
            }
        }
        let value_factor =
            (1.0 / 6.0) / (0.1 * (1.0 + 1.0 / ((max_octave - min_octave + 1) as f64)));
        let max_value = (first.max_value() + second.max_value()) * value_factor;
        Self {
            first,
            second,
            value_factor,
            max_value,
        }
    }

    pub fn get_value(&self, x: f64, y: f64, z: f64) -> f64 {
        let x2 = x * Self::INPUT_FACTOR;
        let y2 = y * Self::INPUT_FACTOR;
        let z2 = z * Self::INPUT_FACTOR;
        (self.first.get_value(x, y, z) + self.second.get_value(x2, y2, z2)) * self.value_factor
    }

    pub fn max_value(&self) -> f64 {
        self.max_value
    }
}

/// The overworld base 3D terrain noise
/// (`net.minecraft.world.level.levelgen.synth.BlendedNoise`).
pub struct BlendedNoise {
    min_limit_noise: PerlinNoise,
    max_limit_noise: PerlinNoise,
    main_noise: PerlinNoise,
    xz_multiplier: f64,
    y_multiplier: f64,
    xz_factor: f64,
    y_factor: f64,
    smear_scale_multiplier: f64,
    max_value: f64,
}

impl BlendedNoise {
    /// `createUnseeded(...)` with `new XoroshiroRandomSource(0L)`.
    pub fn create_unseeded(
        xz_scale: f64,
        y_scale: f64,
        xz_factor: f64,
        y_factor: f64,
        smear_scale_multiplier: f64,
    ) -> Self {
        Self::with_random(
            Xoroshiro128::new(0),
            xz_scale,
            y_scale,
            xz_factor,
            y_factor,
            smear_scale_multiplier,
        )
    }

    /// `withNewRandom(random)` -- the BlendedNoise constructor used by the
    /// overworld router (RandomState re-seeds via `fromHashOf("terrain")`).
    pub fn with_random(
        random: Xoroshiro128,
        xz_scale: f64,
        y_scale: f64,
        xz_factor: f64,
        y_factor: f64,
        smear_scale_multiplier: f64,
    ) -> Self {
        let mut rng = random;
        let min_limit_noise = PerlinNoise::create_legacy(&mut rng, &(-15..=0).collect::<Vec<_>>());
        let max_limit_noise = PerlinNoise::create_legacy(&mut rng, &(-15..=0).collect::<Vec<_>>());
        let main_noise = PerlinNoise::create_legacy(&mut rng, &(-7..=0).collect::<Vec<_>>());
        let xz_multiplier = 684.412 * xz_scale;
        let y_multiplier = 684.412 * y_scale;
        let max_value = min_limit_noise.max_broken_value(y_multiplier);
        Self {
            min_limit_noise,
            max_limit_noise,
            main_noise,
            xz_multiplier,
            y_multiplier,
            xz_factor,
            y_factor,
            smear_scale_multiplier,
            max_value,
        }
    }

    /// `compute(FunctionContext)`.
    pub fn compute(&self, block_x: i32, block_y: i32, block_z: i32) -> f64 {
        let limit_x = block_x as f64 * self.xz_multiplier;
        let limit_y = block_y as f64 * self.y_multiplier;
        let limit_z = block_z as f64 * self.xz_multiplier;
        let main_x = limit_x / self.xz_factor;
        let main_y = limit_y / self.y_factor;
        let main_z = limit_z / self.xz_factor;
        let limit_smear = self.y_multiplier * self.smear_scale_multiplier;
        let main_smear = limit_smear / self.y_factor;

        let mut blend_min = 0.0;
        let mut blend_max = 0.0;
        let mut main_noise_value = 0.0;
        let mut pow = 1.0;
        for i in 0..8usize {
            if let Some(noise) = self.main_noise.get_octave_noise(i) {
                main_noise_value += noise.noise5(
                    wrap(main_x * pow),
                    wrap(main_y * pow),
                    wrap(main_z * pow),
                    main_smear * pow,
                    main_y * pow,
                ) / pow;
            }
            pow /= 2.0;
        }
        let factor = (main_noise_value / 10.0 + 1.0) / 2.0;
        let is_max = factor >= 1.0;
        let is_min = factor <= 0.0;
        pow = 1.0;
        for i in 0..16usize {
            let wx = wrap(limit_x * pow);
            let wy = wrap(limit_y * pow);
            let wz = wrap(limit_z * pow);
            let y_scale_pow = limit_smear * pow;
            if !is_max {
                if let Some(min_noise) = self.min_limit_noise.get_octave_noise(i) {
                    blend_min += min_noise.noise5(wx, wy, wz, y_scale_pow, limit_y * pow) / pow;
                }
            }
            if !is_min {
                if let Some(max_noise) = self.max_limit_noise.get_octave_noise(i) {
                    blend_max += max_noise.noise5(wx, wy, wz, y_scale_pow, limit_y * pow) / pow;
                }
            }
            pow /= 2.0;
        }
        clamped_lerp(factor, blend_min / 512.0, blend_max / 512.0) / 128.0
    }

    pub fn max_value(&self) -> f64 {
        self.max_value
    }

    /// Debug: dump octave states of the three PerlinNoise instances.
    pub fn dump_octaves(&self) {
        println!("minLimit:");
        self.min_limit_noise.dump_octaves();
        println!("maxLimit:");
        self.max_limit_noise.dump_octaves();
        println!("main:");
        self.main_noise.dump_octaves();
    }
}
