// Copyright (c) 2026 Neutron Contributors -- MIT License
//
// WorldgenRandom-compatible RNG for feature placement.
//
// Wraps Xoroshiro128 but uses Legacy-style `nextInt(bound)` over
// `next(31)` bits, matching `WorldgenRandom` when the underlying source is
// Xoroshiro (26.2 overworld default).

use crate::rng::Xoroshiro128;

/// Feature decoration RNG.
pub struct FeatureRandom {
    rng: Xoroshiro128,
}

impl FeatureRandom {
    pub fn new(seed: i64) -> Self {
        Self {
            rng: Xoroshiro128::new(seed),
        }
    }

    pub fn set_seed(&mut self, seed: i64) {
        self.rng = Xoroshiro128::new(seed);
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

    pub fn next_long(&mut self) -> i64 {
        self.rng.next_long()
    }

    pub fn next_bits(&mut self, bits: u32) -> i32 {
        (self.rng.next_u64() >> (64 - bits)) as u32 as i32
    }

    pub fn next_f32(&mut self) -> f32 {
        (self.next_bits(24) as f32) * (1.0 / (1u32 << 24) as f32)
    }

    pub fn next_f64(&mut self) -> f64 {
        ((self.rng.next_u64() >> 11) as f64) * (1.0 / ((1u64 << 53) as f64))
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
}
