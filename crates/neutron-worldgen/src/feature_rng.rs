//! `WorldgenRandom` wrapping Xoroshiro128 (26.2 overworld default).
//!
//! `next(bits)` = `xoroshiro.nextLong() >>> (64-bits)`.
//! `nextInt` / `nextLong` / `nextFloat` / `nextDouble` follow `BitRandomSource`
//! (legacy-style): `nextLong` and `nextDouble` each consume two `next(bits)`.
//!
//! Copyright (c) 2026 Neutron Contributors -- MIT License

use crate::rng::Xoroshiro128;

/// Feature decoration RNG.
pub struct FeatureRandom {
    rng: Xoroshiro128,
    /// `next(bits)` calls since last `reset_draw_count` (debug / probes).
    draw_count: u32,
    /// Marsaglia polar spare for `nextGaussian`.
    have_next_gaussian: bool,
    next_next_gaussian: f64,
}

impl FeatureRandom {
    pub fn new(seed: i64) -> Self {
        Self {
            rng: Xoroshiro128::new(seed),
            draw_count: 0,
            have_next_gaussian: false,
            next_next_gaussian: 0.0,
        }
    }

    pub fn set_seed(&mut self, seed: i64) {
        self.rng = Xoroshiro128::new(seed);
    }

    /// Reset the `next(bits)` counter used by sculk attempt dumps.
    pub fn reset_draw_count(&mut self) {
        self.draw_count = 0;
    }

    /// `WorldgenRandom.next(bits)` invocations since the last reset.
    pub fn draw_count(&self) -> u32 {
        self.draw_count
    }

    /// `WorldgenRandom.setDecorationSeed(levelSeed, blockX, blockZ)`.
    ///
    /// `blockX/Z` are typically `chunkX * 16` / `chunkZ * 16`.
    pub fn set_decoration_seed(&mut self, level_seed: i64, block_x: i32, block_z: i32) -> i64 {
        self.set_seed(level_seed);
        let a = self.next_long() | 1;
        let b = self.next_long() | 1;
        let decoration = (block_x as i64)
            .wrapping_mul(a)
            .wrapping_add((block_z as i64).wrapping_mul(b))
            ^ level_seed;
        self.set_seed(decoration);
        decoration
    }

    /// `WorldgenRandom.setFeatureSeed(decorationSeed, index, step)`.
    ///
    /// Vanilla 26.2 (CFR): `seed + (long)index + (long)(10000 * step)`.
    /// The `10000 * step` multiply is 32-bit, then widened — same as Java.
    pub fn set_feature_seed(&mut self, decoration_seed: i64, feature_index: i32, step: i32) {
        let seed = decoration_seed
            .wrapping_add(feature_index as i64)
            .wrapping_add((10_000i32.wrapping_mul(step)) as i64);
        self.set_seed(seed);
    }

    /// `BitRandomSource.nextLong()` via `WorldgenRandom.next(32)` twice.
    ///
    /// Each `next(bits)` on a Xoroshiro wrapper is `(int)(xoroshiro.nextLong() >>> (64-bits))`.
    /// This is **not** a single xoroshiro `nextLong()`.
    pub fn next_long(&mut self) -> i64 {
        let hi = self.next_bits(32) as i64;
        let lo = self.next_bits(32) as i64;
        (hi << 32).wrapping_add(lo)
    }

    /// `WorldgenRandom.next(bits)` wrapping Xoroshiro: `xoroshiro.nextLong() >>> (64-bits)`.
    pub fn next_bits(&mut self, bits: u32) -> i32 {
        self.draw_count = self.draw_count.wrapping_add(1);
        let v = (self.rng.next_u64() >> (64 - bits)) as u32 as i32;
        if rng_trace_enabled() {
            eprintln!("RNG next({bits})={v} bits={}", self.draw_count);
        }
        v
    }

    /// `BitRandomSource.nextFloat()` = `next(24) * 2^-24`.
    pub fn next_f32(&mut self) -> f32 {
        (self.next_bits(24) as f32) * (1.0 / (1u32 << 24) as f32)
    }

    /// `BitRandomSource.nextDouble()` = `(next(26) << 27) + next(27)` × `2^-53`.
    pub fn next_f64(&mut self) -> f64 {
        let a = self.next_bits(26) as i64;
        let b = self.next_bits(27) as i64;
        ((a << 27).wrapping_add(b) as f64) * (1.0 / ((1u64 << 53) as f64))
    }

    /// Legacy-style `nextInt(bound)` using `next(31)`.
    pub fn next_int(&mut self, bound: i32) -> i32 {
        assert!(bound > 0);
        // Power-of-two fast path (LegacyRandomSource)
        if (bound & -bound) == bound {
            return (((bound as i64) * (self.next_bits(31) as i64)) >> 31) as i32;
        }
        loop {
            let bits = self.next_bits(31);
            let val = bits % bound;
            if bits - val + (bound - 1) >= 0 {
                return val;
            }
        }
    }

    /// Unbounded `nextInt()` — low 32 bits of nextLong (via next(32)).
    pub fn next_int32(&mut self) -> i32 {
        self.next_bits(32)
    }

    /// `RandomSource.nextBoolean()` = `next(1) != 0`.
    pub fn next_boolean(&mut self) -> bool {
        self.next_bits(1) != 0
    }

    /// `RandomSource.nextGaussian()` — Marsaglia polar, cached spare. Each
    /// call consumes two `nextDouble` on a miss, none on a hit.
    pub fn next_gaussian(&mut self) -> f64 {
        if self.have_next_gaussian {
            self.have_next_gaussian = false;
            return self.next_next_gaussian;
        }
        let (x, y, s) = loop {
            let x = 2.0 * self.next_f64() - 1.0;
            let y = 2.0 * self.next_f64() - 1.0;
            let s = x * x + y * y;
            if s < 1.0 && s != 0.0 {
                break (x, y, s);
            }
        };
        let multiplier = (-2.0 * s.ln() / s).sqrt();
        self.next_next_gaussian = y * multiplier;
        self.have_next_gaussian = true;
        x * multiplier
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decoration_seed_deterministic() {
        let mut a = FeatureRandom::new(12345);
        let mut b = FeatureRandom::new(12345);
        let sa = a.set_decoration_seed(12345, 96, -32);
        let sb = b.set_decoration_seed(12345, 96, -32);
        assert_eq!(sa, sb);
        a.set_feature_seed(sa, 0, 6);
        b.set_feature_seed(sb, 0, 6);
        for _ in 0..100 {
            assert_eq!(a.next_int(16), b.next_int(16));
        }
    }

    #[test]
    fn feature_seed_formula_matches_vanilla() {
        // WorldgenRandom.setFeatureSeed(dec, index, step) = dec + index + 10000*step
        // NOT dec + step + 10000*index (the previous swapped formula).
        let dec = 0x1111_2222_3333_4444u64 as i64;
        let mut rng = FeatureRandom::new(0);
        rng.set_feature_seed(dec, 52, 9);
        let expected = dec.wrapping_add(52).wrapping_add(90_000);
        let mut check = FeatureRandom::new(0);
        check.set_seed(expected);
        for _ in 0..16 {
            assert_eq!(rng.next_int(16), check.next_int(16));
        }
        // Distinct from the swapped formula.
        let mut swapped = FeatureRandom::new(0);
        swapped.set_seed(dec.wrapping_add(9).wrapping_add(10_000 * 52));
        assert_ne!(rng.next_int(16), swapped.next_int(16));
    }

    /// Ground truth: `tools/worldgen-probe/src/ProbeWorldgenRandom.java` vs 26.2 jar.
    /// WorldgenRandom wraps Xoroshiro; nextLong/nextDouble use BitRandomSource
    /// (two `next(bits)`), not a raw xoroshiro nextLong.
    #[test]
    fn worldgen_random_matches_vanilla_xoroshiro_wrapper() {
        let seed = 12345i64;
        let mut rng = FeatureRandom::new(seed);
        rng.set_seed(seed);
        assert_eq!(rng.next_long(), -8118485272768813798);
        assert_eq!(rng.next_long(), 4143755031235356457);

        let mut rng = FeatureRandom::new(seed);
        let dec = rng.set_decoration_seed(seed, 96, -32);
        assert_eq!(dec, -8084287573569489607);

        rng.set_feature_seed(dec, 0, 6);
        assert_eq!(rng.next_int(16), 12);
        assert_eq!(rng.next_int(16), 7);
        assert_eq!(rng.next_int(161), 56);
        assert_eq!(rng.next_f32().to_bits(), 0.14968139f32.to_bits());
        assert_eq!(rng.next_f64().to_bits(), 0.16489983713749623f64.to_bits());
        assert_eq!(rng.next_long(), -2797163788994301519);

        let mut rng = FeatureRandom::new(seed);
        let dec = rng.set_decoration_seed(seed, 96, -32);
        rng.set_feature_seed(dec, 52, 9);
        let ints: Vec<i32> = (0..8).map(|_| rng.next_int(16)).collect();
        assert_eq!(ints, vec![0, 11, 11, 1, 10, 15, 3, 12]);

        let seed = 0x1111_2222_3333_4444u64 as i64;
        let mut rng = FeatureRandom::new(seed);
        rng.set_seed(seed);
        let ints: Vec<i32> = (0..8).map(|_| rng.next_int(16)).collect();
        assert_eq!(ints, vec![14, 4, 12, 2, 3, 7, 8, 1]);
        assert_eq!(rng.next_f32().to_bits(), 0.3558504f32.to_bits());
        assert_eq!(rng.next_f64().to_bits(), 0.9180557537010783f64.to_bits());
        assert_eq!(rng.next_long(), -5824706931741106560);

        // RandomBooleanSelectorFeature uses nextBoolean = next(1)!=0, not nextInt(2).
        let mut a = FeatureRandom::new(424242);
        let mut b = FeatureRandom::new(424242);
        let nbool: Vec<bool> = (0..16).map(|_| a.next_boolean()).collect();
        let nint2: Vec<bool> = (0..16).map(|_| b.next_int(2) == 0).collect();
        assert_ne!(nbool, nint2);
    }
}

/// Whether `NEUTRON_RNG_TRACE` is set — resolved once, checked on every draw.
fn rng_trace_enabled() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var_os("NEUTRON_RNG_TRACE").is_some())
}
