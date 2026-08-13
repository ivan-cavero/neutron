// Copyright (c) 2026 Neutron Contributors -- MIT License
//
// `LegacyRandomSource` — Java `java.util.Random` LCG used by carvers via
// `WorldgenRandom(new LegacyRandomSource(...))`.

/// 48-bit LCG matching `LegacyRandomSource` / `java.util.Random`.
pub struct LegacyRandom {
    seed: u64,
}

impl LegacyRandom {
    pub fn new(seed: i64) -> Self {
        let mut r = Self { seed: 0 };
        r.set_seed(seed);
        r
    }

    pub fn set_seed(&mut self, seed: i64) {
        self.seed = (seed as u64 ^ 0x5DEE_CE66_D) & ((1u64 << 48) - 1);
    }

    fn next(&mut self, bits: u32) -> i32 {
        self.seed = self.seed.wrapping_mul(0x5DEE_CE66_D).wrapping_add(0xB) & ((1u64 << 48) - 1);
        (self.seed >> (48 - bits)) as i32
    }

    pub fn next_int(&mut self, bound: i32) -> i32 {
        assert!(bound > 0);
        if (bound & -bound) == bound {
            return ((bound as i64 * self.next(31) as i64) >> 31) as i32;
        }
        loop {
            let bits = self.next(31);
            let val = bits % bound;
            if bits - val + (bound - 1) >= 0 {
                return val;
            }
        }
    }

    pub fn next_int32(&mut self) -> i32 {
        self.next(32)
    }

    pub fn next_long(&mut self) -> i64 {
        let hi = self.next(32) as i64;
        let lo = self.next(32) as i64;
        (hi << 32).wrapping_add(lo)
    }

    pub fn next_f32(&mut self) -> f32 {
        self.next(24) as f32 * (1.0 / (1u32 << 24) as f32)
    }

    pub fn next_f64(&mut self) -> f64 {
        let a = self.next(26) as i64;
        let b = self.next(27) as i64;
        ((a << 27).wrapping_add(b) as f64) * (1.0 / ((1u64 << 53) as f64))
    }

    /// `WorldgenRandom.setLargeFeatureSeed(seed, chunkX, chunkZ)`.
    pub fn set_large_feature_seed(&mut self, seed: i64, chunk_x: i32, chunk_z: i32) {
        self.set_seed(seed);
        let a = self.next_long();
        let b = self.next_long();
        // setSeed((chunkX * a) ^ (chunkZ * b) ^ seed)
        let mixed = (chunk_x as i64).wrapping_mul(a) ^ (chunk_z as i64).wrapping_mul(b) ^ seed;
        self.set_seed(mixed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_determinism() {
        let mut a = LegacyRandom::new(12345);
        let mut b = LegacyRandom::new(12345);
        for _ in 0..100 {
            assert_eq!(a.next_f32().to_bits(), b.next_f32().to_bits());
            assert_eq!(a.next_int(16), b.next_int(16));
        }
    }
}
