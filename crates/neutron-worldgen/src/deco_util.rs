//! Shared decoration helpers — vanilla `Util.shuffle` and direction math.
//!
//! One copy of each algorithm; every feature module imports from here.
//! RNG behavior is parity-critical: do not "simplify" the shuffle loop.

use crate::feature_rng::FeatureRandom;

/// `Util.shuffle`: `for (i = size; i > 1; i--) swap(i-1, nextInt(i))`.
pub fn shuffle<T>(list: &mut [T], rng: &mut FeatureRandom) {
    let mut i = list.len();
    while i > 1 {
        let swap_to = rng.next_int(i as i32) as usize;
        list.swap(i - 1, swap_to);
        i -= 1;
    }
}

/// Opposite direction index (`Direction.getOpposite()`).
/// Order: DOWN=0, UP=1, NORTH=2, SOUTH=3, WEST=4, EAST=5.
pub fn opposite(dir: usize) -> usize {
    const OPP: [usize; 6] = [1, 0, 3, 2, 5, 4];
    OPP.get(dir).copied().unwrap_or(dir)
}

/// `nextInt(max - min + 1) + min` with vanilla's degenerate-range guard.
pub fn next_int_inclusive(rng: &mut FeatureRandom, min: i32, max: i32) -> i32 {
    let span = max - min + 1;
    if span <= 0 {
        min
    } else {
        min + rng.next_int(span)
    }
}
