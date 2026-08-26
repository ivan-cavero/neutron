//! Xoroshiro128++ matching 26.2 `XoroshiroRandomSource`.
//!
//! Seed mixing uses `RandomSupport.upgradeSeedTo128bit` (`mixStafford13` on
//! `seed ^ 0x6A09E667F3BCC909` and that value + `GOLDEN_RATIO_64`).
//!
//! Copyright (c) 2026 Neutron Contributors -- MIT License
// - `nextInt(bound)` uses the Lemire multiplier-based algorithm (full 32-bit
//   draw, no modulo rejection loop).
// - `forkPositional()` consumes two `nextLong` calls and returns a positional
//   factory seed pair.
// - `fromHashOf(name)` derives a fresh 128-bit seed as MD5(name) XOR state
//   (no state consumption).

use md5::{Digest, Md5};

/// Golden ratio constant used by `RandomSupport` (also the zero-state fallback).
pub const GOLDEN_RATIO_64: u64 = 0x9E37_79B9_7F4A_7C15; // -7046029254386353131
/// Silver ratio constant used as the zero-state fallback high word.
pub const SILVER_RATIO_64: u64 = 0x6A09_E667_F3BC_C909; // 7640891576956012809

/// XORoshiro128++ PRNG matching Minecraft's Java implementation exactly.
#[derive(Clone)]
pub struct Xoroshiro128 {
    seed_lo: u64,
    seed_hi: u64,
}

impl Xoroshiro128 {
    /// Create a new PRNG from the raw 128-bit state (two u64 halves).
    ///
    /// Matches `Xoroshiro128PlusPlus(long, long)`: if both words are zero the
    /// state is replaced with the golden/silver ratio constants.
    pub fn from_raw(seed_lo: u64, seed_hi: u64) -> Self {
        let (seed_lo, seed_hi) = if (seed_lo | seed_hi) == 0 {
            (GOLDEN_RATIO_64, SILVER_RATIO_64)
        } else {
            (seed_lo, seed_hi)
        };
        Self { seed_lo, seed_hi }
    }

    /// Create a new PRNG from a single 64-bit seed using `RandomSupport.upgradeSeedTo128bit`.
    pub fn new(seed: i64) -> Self {
        let seed = seed as u64;
        let low = seed ^ 0x6A09_E667_F3BC_C909;
        let high = low.wrapping_add(GOLDEN_RATIO_64);
        let lo = Self::mix_stafford13(low);
        let hi = Self::mix_stafford13(high);
        Self::from_raw(lo, hi)
    }

    /// `RandomSupport.mixStafford13(long)`.
    pub fn mix_stafford13(mut z: u64) -> u64 {
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Raw u64 output of the xoroshiro128++ algorithm (`nextLong`).
    pub fn next_u64(&mut self) -> u64 {
        let result = rotl(self.seed_lo.wrapping_add(self.seed_hi), 17).wrapping_add(self.seed_lo);
        self.seed_hi ^= self.seed_lo;
        self.seed_lo = rotl(self.seed_lo, 49) ^ self.seed_hi ^ (self.seed_hi << 21);
        self.seed_hi = rotl(self.seed_hi, 28);
        result
    }

    /// Raw `nextLong()` as signed.
    pub fn next_long(&mut self) -> i64 {
        self.next_u64() as i64
    }

    /// Java `XoroshiroRandomSource.nextBoolean()` — overridden in 26.2 as
    /// `(nextLong() & 1L) != 0` (LOW bit; the RandomSource interface default
    /// would use the top bit via nextBits(1), but this class overrides it).
    pub fn next_boolean(&mut self) -> bool {
        (self.next_u64() & 1) != 0
    }

    /// Java `RandomSource.nextInt()` -- the low 32 bits of `nextLong()`.
    pub fn next_int32(&mut self) -> i32 {
        self.next_u64() as u32 as i32
    }

    /// Java `RandomSource.nextInt(int bound)` (Lemire's algorithm).
    pub fn next_int(&mut self, bound: i32) -> i32 {
        assert!(bound > 0, "bound must be positive");
        let bound = bound as u32;
        let mut random_bits = self.next_u64() as u32 as u64;
        let mut multiplied = random_bits.wrapping_mul(bound as u64);
        let mut fractional = multiplied & 0xFFFF_FFFF;
        if fractional < bound as u64 {
            // Integer.remainderUnsigned(~bound + 1, bound)
            let unbiased_buckets_start = (bound.wrapping_neg()) % bound;
            while fractional < unbiased_buckets_start as u64 {
                random_bits = self.next_u64() as u32 as u64;
                multiplied = random_bits.wrapping_mul(bound as u64);
                fractional = multiplied & 0xFFFF_FFFF;
            }
        }
        (multiplied >> 32) as i32
    }

    /// Java `RandomSource.nextDouble()`: `(nextLong() >>> 11) * 2^-53`.
    pub fn next_f64(&mut self) -> f64 {
        ((self.next_u64() >> 11) as f64) * (1.0 / ((1u64 << 53) as f64))
    }

    /// Java `RandomSource.nextFloat()`: `(nextLong() >>> 40) * 2^-24`.
    pub fn next_f32(&mut self) -> f32 {
        ((self.next_u64() >> 40) as f32) * (1.0 / ((1u32 << 24) as f32))
    }

    /// Java `RandomSource.consumeCount(int rounds)`: discards `rounds` nextLong values.
    pub fn consume(&mut self, rounds: u32) {
        for _ in 0..rounds {
            self.next_u64();
        }
    }

    /// Java `RandomSource.forkPositional()`: consumes two nextLongs and returns
    /// the positional factory seed pair `(seedLo, seedHi)`.
    pub fn fork_positional(&mut self) -> (u64, u64) {
        (self.next_u64(), self.next_u64())
    }

    /// Java `PositionalRandomFactory.fromHashOf(String name)` — the 26.2
    /// overworld uses `legacy_random_source = false` -> Xoroshiro factory.
    /// Empirically (ProbeNoiseSeed) the noise seed matches an MD5-based hash
    /// XORed with the factory pair (the JBR/JDK `fromHashOf` seeding). Keep the
    /// MD5 convention that reproduces the vanilla noise values exactly.
    ///
    /// Does NOT consume state. `name` is the raw identifier string
    /// (e.g. `"minecraft:temperature"`, `"octave_-10"`).
    pub fn from_hash_of(&self, name: &str) -> Xoroshiro128 {
        let mut hasher = Md5::new();
        hasher.update(name.as_bytes());
        let digest = hasher.finalize();
        let hash_lo = u64::from_be_bytes(digest[0..8].try_into().unwrap());
        let hash_hi = u64::from_be_bytes(digest[8..16].try_into().unwrap());
        Xoroshiro128::from_raw(self.seed_lo ^ hash_lo, self.seed_hi ^ hash_hi)
    }

    /// Return the internal seed state as two u64 halves (for testing/debugging).
    pub fn seed(&self) -> (u64, u64) {
        (self.seed_lo, self.seed_hi)
    }
}

/// Bitwise left rotation.
const fn rotl(x: u64, k: u32) -> u64 {
    (x << k) | (x >> (64 - k))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn determinism_same_seed_same_sequence() {
        let mut a = Xoroshiro128::new(42);
        let mut b = Xoroshiro128::new(42);
        for _ in 0..1000 {
            assert_eq!(a.next_u64(), b.next_u64());
            assert_eq!(a.next_f64(), b.next_f64());
        }
    }

    #[test]
    fn different_seeds_different_sequences() {
        let mut a = Xoroshiro128::new(1);
        let mut b = Xoroshiro128::new(2);
        let mut any_diff = false;
        for _ in 0..100 {
            if a.next_u64() != b.next_u64() {
                any_diff = true;
                break;
            }
        }
        assert!(
            any_diff,
            "different seeds should produce different sequences"
        );
    }

    #[test]
    fn next_int_bounds() {
        let mut rng = Xoroshiro128::new(12345);
        for _ in 0..10000 {
            let v = rng.next_int(256);
            assert!(v >= 0 && v < 256, "nextInt(256) returned {v}");
        }
        let mut rng = Xoroshiro128::new(999);
        for _ in 0..10000 {
            let v = rng.next_int(5);
            assert!(v >= 0 && v < 5, "nextInt(5) returned {v}");
        }
    }

    #[test]
    fn next_f64_range() {
        let mut rng = Xoroshiro128::new(99999);
        for _ in 0..10000 {
            let v = rng.next_f64();
            assert!(v >= 0.0 && v < 1.0, "nextF64() returned {v}");
        }
    }

    #[test]
    fn from_hash_of_deterministic() {
        let a = Xoroshiro128::new(42);
        let b = Xoroshiro128::new(42);
        let (s1, s2) = (
            a.from_hash_of("minecraft:temperature"),
            b.from_hash_of("minecraft:temperature"),
        );
        assert_eq!(s1.seed(), s2.seed());
    }

    #[test]
    fn zero_state_fallback() {
        let rng = Xoroshiro128::from_raw(0, 0);
        assert_eq!(rng.seed(), (GOLDEN_RATIO_64, SILVER_RATIO_64));
    }
}
