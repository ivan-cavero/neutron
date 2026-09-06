//! `PerlinSimplexNoise` — vanilla 26.2 `synth/PerlinSimplexNoise.java` +
//! `synth/SimplexNoise.java`.
//!
//! Used by `Biome.BIOME_INFO_NOISE` (seed 2345, octave set `[0]`) — the
//! deterministic noise behind `NoiseBasedCountPlacement` (kelp, bamboo,
//! warm_ocean_vegetation) and `Biome.getTemperature`-style ground variation.
//!
//! Only the 2D `getValue` path is implemented (BIOME_INFO_NOISE is sampled
//! 2D); the 3D octaves never fire for octave set `[0]` in worldgen.

use crate::legacy_rng::LegacyRandom;

const GRADIENT: [[i32; 3]; 16] = [
    [1, 1, 0],
    [-1, 1, 0],
    [1, -1, 0],
    [-1, -1, 0],
    [1, 0, 1],
    [-1, 0, 1],
    [1, 0, -1],
    [-1, 0, -1],
    [0, 1, 1],
    [0, -1, 1],
    [0, 1, -1],
    [0, -1, -1],
    [1, 1, 0],
    [0, -1, 1],
    [-1, 1, 0],
    [0, -1, -1],
];

const SQRT_3: f64 = 1.7320508075688772;
const F2: f64 = 0.5 * (SQRT_3 - 1.0);
const G2: f64 = (3.0 - SQRT_3) / 6.0;

/// One `SimplexNoise` octave (2D permutation + offsets).
struct SimplexNoise {
    p: [i32; 512],
    xo: f64,
    yo: f64,
    #[allow(dead_code)]
    zo: f64,
}

impl SimplexNoise {
    /// `SimplexNoise(RandomSource)`: three `nextDouble() * 256.0` offsets,
    /// then a Fisher-Yates shuffle of 0..256 using `nextInt(256 - ix)`.
    fn new(r: &mut LegacyRandom) -> Self {
        let xo = r.next_f64() * 256.0;
        let yo = r.next_f64() * 256.0;
        let zo = r.next_f64() * 256.0;
        let mut p = [0i32; 512];
        for (i, v) in p.iter_mut().enumerate().take(256) {
            *v = i as i32;
        }
        for ix in 0..256usize {
            let offset = r.next_int(256 - ix as i32) as usize;
            p.swap(ix, offset + ix);
        }
        // Upper half mirrors indices 0..256 via `p(x & 0xFF)`; precompute for
        // direct lookup (vanilla indexes p[0..512] but always masks).
        for i in 256..512 {
            p[i] = p[i - 256];
        }
        SimplexNoise { p, xo, yo, zo }
    }

    #[inline]
    fn p(&self, x: i32) -> i32 {
        self.p[(x & 0xFF) as usize]
    }

    #[inline]
    fn corner(gi: usize, x: f64, y: f64, z: f64, base: f64) -> f64 {
        let mut t0 = base - x * x - y * y - z * z;
        if t0 < 0.0 {
            return 0.0;
        }
        t0 *= t0;
        let g = GRADIENT[gi];
        t0 * t0 * (g[0] as f64 * x + g[1] as f64 * y + g[2] as f64 * z)
    }

    /// `getValue(xin, yin)` — the 2D simplex path (scale 70.0, base 0.5).
    fn get_value_2d(&self, xin: f64, yin: f64) -> f64 {
        let s = (xin + yin) * F2;
        let i = (xin + s).floor() as i32;
        let j = (yin + s).floor() as i32;
        let t = (i + j) as f64 * G2;
        let x0 = xin - (i as f64 - t);
        let y0 = yin - (j as f64 - t);
        let (i1, j1): (i32, i32) = if x0 > y0 { (1, 0) } else { (0, 1) };
        let x1 = x0 - i1 as f64 + G2;
        let y1 = y0 - j1 as f64 + G2;
        let x2 = x0 - 1.0 + 2.0 * G2;
        let y2 = y0 - 1.0 + 2.0 * G2;
        let ii = i & 0xFF;
        let jj = j & 0xFF;
        let gi0 = (self.p(ii + self.p(jj)) % 12) as usize;
        let gi1 = (self.p(ii + i1 + self.p(jj + j1)) % 12) as usize;
        let gi2 = (self.p(ii + 1 + self.p(jj + 1)) % 12) as usize;
        let n0 = Self::corner(gi0, x0, y0, 0.0, 0.5);
        let n1 = Self::corner(gi1, x1, y1, 0.0, 0.5);
        let n2 = Self::corner(gi2, x2, y2, 0.0, 0.5);
        70.0 * (n0 + n1 + n2)
    }
}

/// `PerlinSimplexNoise` over an octave set. Only the subset needed by
/// worldgen callers is implemented: octave set `[0]` (BIOME_INFO_NOISE).
pub struct PerlinSimplexNoise {
    levels: Vec<Option<SimplexNoise>>,
    highest_freq_input_factor: f64,
    highest_freq_value_factor: f64,
}

impl PerlinSimplexNoise {
    /// `new PerlinSimplexNoise(random, [0])` — the `BIOME_INFO_NOISE` shape.
    ///
    /// With octave set `{0}`: lowFreqOctaves = 0, highFreqOctaves = 0,
    /// octaves = 1, the zero octave lands at index 0, no consumeCount
    /// skips, and no high-freq reseed (highFreqOctaves == 0).
    pub fn biome_info_noise() -> Self {
        let mut r = LegacyRandom::new(2345);
        let zero = SimplexNoise::new(&mut r);
        Self {
            levels: vec![Some(zero)],
            highest_freq_input_factor: 2.0f64.powi(0),
            highest_freq_value_factor: 1.0 / (2.0f64.powi(1) - 1.0),
        }
    }

    /// `getValue(x, y, false)` — octave sum with the [0] layout.
    pub fn get_value(&self, x: f64, y: f64) -> f64 {
        let mut value = 0.0;
        let mut factor = self.highest_freq_input_factor;
        let mut value_factor = self.highest_freq_value_factor;
        for level in &self.levels {
            if let Some(n) = level {
                value += n.get_value_2d(x * factor, y * factor) * value_factor;
            }
            factor /= 2.0;
            value_factor *= 2.0;
        }
        value
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Two-sided check vs `Biome.BIOME_INFO_NOISE.getValue(x, z, false)`
    /// (ProbeBiomeInfoNoise, real 26.2 jar): 0,0 / 100,200 / -34,78 /
    /// 1234,-4321 -> 0.000000000 / -0.396296145 / 0.500425360 / -0.194304504.
    #[test]
    fn biome_info_noise_matches_vanilla() {
        let n = PerlinSimplexNoise::biome_info_noise();
        let cases: [(f64, f64, f64); 4] = [
            (0.0, 0.0, 0.000000000),
            (100.0, 200.0, -0.396296145),
            (-34.0, 78.0, 0.500425360),
            (1234.0, -4321.0, -0.194304504),
        ];
        for (x, z, want) in cases {
            let got = n.get_value(x, z);
            assert!(
                (got - want).abs() < 5e-9,
                "BIOME_INFO_NOISE({x},{z}) = {got:.9}, want {want:.9}"
            );
        }
    }
}
