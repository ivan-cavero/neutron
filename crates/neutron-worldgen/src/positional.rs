//! `XoroshiroPositionalRandomFactory` — seed a RNG from block / hash.
//!
//! Copyright (c) 2026 Neutron Contributors -- MIT License

use crate::rng::Xoroshiro128;

/// Vanilla `Mth.getSeed(x, y, z)`.
#[inline]
pub fn block_seed(x: i32, y: i32, z: i32) -> i64 {
    let mut i =
        (x.wrapping_mul(3129871) as i64) ^ ((z as i64).wrapping_mul(116129781)) ^ (y as i64);
    i = i
        .wrapping_mul(i)
        .wrapping_mul(42317861)
        .wrapping_add(i.wrapping_mul(11));
    i >> 16
}

/// Positional random factory: `(seedLo, seedHi)` pair from `forkPositional()`.
#[derive(Clone, Copy, Debug)]
pub struct PositionalRandomFactory {
    pub seed_lo: u64,
    pub seed_hi: u64,
}

impl PositionalRandomFactory {
    pub fn new(seed_lo: u64, seed_hi: u64) -> Self {
        Self { seed_lo, seed_hi }
    }

    /// `PositionalRandomFactory.at(x, y, z)`.
    pub fn at(&self, x: i32, y: i32, z: i32) -> Xoroshiro128 {
        let positional = block_seed(x, y, z) as u64;
        let random_seed = positional ^ self.seed_lo;
        Xoroshiro128::from_raw(random_seed, self.seed_hi)
    }

    /// `fromHashOf(name).forkPositional()` relative to this factory.
    ///
    /// Matches `RandomState.getOrCreateRandomFactory(Identifier)`.
    pub fn from_hash_of_positional(&self, name: &str) -> Self {
        let mut rng = Xoroshiro128::from_raw(self.seed_lo, self.seed_hi).from_hash_of(name);
        let (lo, hi) = rng.fork_positional();
        Self::new(lo, hi)
    }
}
