// Copyright (c) 2026 Neutron Contributors -- MIT License
//
// Vanilla 26.2 `BiomeManager.getBiome(BlockPos)`: 4-block voronoi fuzz.
// `obfuscateSeed(worldSeed)` is Guava SHA-256 of the little-endian long.

use crate::biome_source::{climate_at_block, find_biome};
use crate::density::DensityEnv;
use crate::worldgen::WorldgenState;

/// `QuartPos.fromBlock`.
#[inline]
pub fn quart_from_block(block: i32) -> i32 {
    block >> 2
}

/// `QuartPos.toBlock`.
#[inline]
pub fn quart_to_block(quart: i32) -> i32 {
    quart << 2
}

/// `BiomeManager.obfuscateSeed` = `Hashing.sha256().hashLong(seed).asLong()`.
///
/// This is the `biomeZoomSeed` stored on `BiomeManager` / `ServerLevel`.
pub fn obfuscate_seed(world_seed: i64) -> i64 {
    let digest = sha256::hash_long(world_seed);
    i64::from_le_bytes(digest[..8].try_into().expect("sha256 digest is 32 bytes"))
}

/// `LinearCongruentialGenerator.next(rval, c)`.
#[inline]
pub fn lcg_next(rval: i64, c: i64) -> i64 {
    let mixed = rval
        .wrapping_mul(6_364_136_223_846_793_005)
        .wrapping_add(1_442_695_040_888_963_407);
    rval.wrapping_mul(mixed).wrapping_add(c)
}

/// `Math.floorMod(x, 1024)` on a Java `long`.
#[inline]
fn floor_mod_1024(x: i64) -> i64 {
    let r = x % 1024;
    if r < 0 {
        r + 1024
    } else {
        r
    }
}

/// `BiomeManager.getFiddle`.
#[inline]
pub fn get_fiddle(rval: i64) -> f64 {
    let uniform = floor_mod_1024(rval >> 24) as f64 / 1024.0;
    (uniform - 0.5) * 0.9
}

/// `BiomeManager.getFiddledDistance`.
pub fn get_fiddled_distance(
    seed: i64,
    quart_x: i32,
    quart_y: i32,
    quart_z: i32,
    distance_x: f64,
    distance_y: f64,
    distance_z: f64,
) -> f64 {
    let mut rval = seed;
    rval = lcg_next(rval, quart_x as i64);
    rval = lcg_next(rval, quart_y as i64);
    rval = lcg_next(rval, quart_z as i64);
    rval = lcg_next(rval, quart_x as i64);
    rval = lcg_next(rval, quart_y as i64);
    rval = lcg_next(rval, quart_z as i64);
    let fiddle_x = get_fiddle(rval);
    rval = lcg_next(rval, seed);
    let fiddle_y = get_fiddle(rval);
    rval = lcg_next(rval, seed);
    let fiddle_z = get_fiddle(rval);
    let dx = distance_x + fiddle_x;
    let dy = distance_y + fiddle_y;
    let dz = distance_z + fiddle_z;
    dz * dz + dy * dy + dx * dx
}

/// Voronoi-selected noise-biome quart for a block position.
///
/// Matches `BiomeManager.getBiome`: offset by -2, inspect the 8 corners of the
/// 4-block cell, pick the minimum `getFiddledDistance`.
pub fn voronoi_quart(biome_zoom_seed: i64, x: i32, y: i32, z: i32) -> (i32, i32, i32) {
    let abs_x = x - 2;
    let abs_y = y - 2;
    let abs_z = z - 2;
    let parent_x = abs_x >> 2;
    let parent_y = abs_y >> 2;
    let parent_z = abs_z >> 2;
    let fract_x = (abs_x & 3) as f64 / 4.0;
    let fract_y = (abs_y & 3) as f64 / 4.0;
    let fract_z = (abs_z & 3) as f64 / 4.0;

    let mut min_i = 0i32;
    let mut min_dist = f64::INFINITY;
    for i in 0..8 {
        let x_even = (i & 4) == 0;
        let y_even = (i & 2) == 0;
        let z_even = (i & 1) == 0;
        let corner_x = if x_even { parent_x } else { parent_x + 1 };
        let corner_y = if y_even { parent_y } else { parent_y + 1 };
        let corner_z = if z_even { parent_z } else { parent_z + 1 };
        let distance_x = if x_even { fract_x } else { fract_x - 1.0 };
        let distance_y = if y_even { fract_y } else { fract_y - 1.0 };
        let distance_z = if z_even { fract_z } else { fract_z - 1.0 };
        let next = get_fiddled_distance(
            biome_zoom_seed,
            corner_x,
            corner_y,
            corner_z,
            distance_x,
            distance_y,
            distance_z,
        );
        if min_dist > next {
            min_i = i;
            min_dist = next;
        }
    }

    let biome_x = if (min_i & 4) == 0 {
        parent_x
    } else {
        parent_x + 1
    };
    let biome_y = if (min_i & 2) == 0 {
        parent_y
    } else {
        parent_y + 1
    };
    let biome_z = if (min_i & 1) == 0 {
        parent_z
    } else {
        parent_z + 1
    };
    (biome_x, biome_y, biome_z)
}

/// `MultiNoiseBiomeSource.getNoiseBiome(quart)`: climate at `quart << 2`.
pub fn noise_biome_at_quart(state: &WorldgenState, quart_x: i32, quart_y: i32, quart_z: i32) -> u8 {
    let x = quart_to_block(quart_x);
    let y = quart_to_block(quart_y);
    let z = quart_to_block(quart_z);
    find_biome(&climate_at(state, x, y, z))
}

/// `BiomeManager.getBiome(BlockPos)` → voronoi quart → `getNoiseBiome`.
pub fn biome_id_at_block(state: &WorldgenState, x: i32, y: i32, z: i32) -> u8 {
    let zoom = obfuscate_seed(state.seed);
    let (qx, qy, qz) = voronoi_quart(zoom, x, y, z);
    noise_biome_at_quart(state, qx, qy, qz)
}

fn climate_at(state: &WorldgenState, x: i32, y: i32, z: i32) -> crate::biome_source::ClimateTarget {
    let mut env = DensityEnv::new(x, y, z, state.noises.noises());
    climate_at_block(
        &mut env,
        &state.router.temperature,
        &state.router.vegetation,
        &state.router.continents,
        &state.router.erosion,
        &state.router.depth,
        &state.router.ridges,
    )
}

/// SHA-256 of an 8-byte little-endian long (Guava `Hasher.putLong` + `asLong`).
mod sha256 {
    const H0: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];

    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];

    pub fn hash_long(seed: i64) -> [u8; 32] {
        let mut block = [0u8; 64];
        block[..8].copy_from_slice(&seed.to_le_bytes());
        block[8] = 0x80;
        // bit length = 64, stored big-endian in the last 8 bytes
        block[63] = 64;
        compress(&block)
    }

    fn compress(block: &[u8; 64]) -> [u8; 32] {
        let mut w = [0u32; 64];
        for i in 0..16 {
            let j = i * 4;
            w[i] = u32::from_be_bytes([block[j], block[j + 1], block[j + 2], block[j + 3]]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }

        let mut a = H0[0];
        let mut b = H0[1];
        let mut c = H0[2];
        let mut d = H0[3];
        let mut e = H0[4];
        let mut f = H0[5];
        let mut g = H0[6];
        let mut h = H0[7];

        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ (!e & g);
            let temp1 = h
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = s0.wrapping_add(maj);
            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }

        let hs = [
            H0[0].wrapping_add(a),
            H0[1].wrapping_add(b),
            H0[2].wrapping_add(c),
            H0[3].wrapping_add(d),
            H0[4].wrapping_add(e),
            H0[5].wrapping_add(f),
            H0[6].wrapping_add(g),
            H0[7].wrapping_add(h),
        ];
        let mut out = [0u8; 32];
        for (i, word) in hs.iter().enumerate() {
            out[i * 4..i * 4 + 4].copy_from_slice(&word.to_be_bytes());
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn obfuscate_seed_matches_guava_sha256_hash_long() {
        // Probed with Guava 33.6 `Hashing.sha256().hashLong(seed).asLong()`.
        assert_eq!(obfuscate_seed(0), 8794265229978523055);
        assert_eq!(obfuscate_seed(1), -6467378160175308932);
        assert_eq!(obfuscate_seed(42), -4111196313959201555);
        assert_eq!(obfuscate_seed(-1), 6759447113877070610);
        assert_eq!(obfuscate_seed(12345), 293737985876514017);
    }

    #[test]
    fn lcg_next_matches_java() {
        assert_eq!(lcg_next(1, 0), 7806831264735756412);
        assert_eq!(lcg_next(0, 0), 0);
        assert_eq!(lcg_next(42, 7), -2477882527166265071);
        assert_eq!(lcg_next(-1, 1), 4921441182957829599);
    }

    #[test]
    fn get_fiddle_matches_java() {
        assert_eq!(get_fiddle(0), -0.45);
        assert_eq!(get_fiddle(1), -0.45);
        assert_eq!(get_fiddle(42), -0.45);
        assert_eq!(get_fiddle(12345), -0.45);
        assert!((get_fiddle(-1) - 0.44912109375000003).abs() < 1e-15);
    }

    #[test]
    fn get_fiddled_distance_matches_java() {
        let seed = obfuscate_seed(42);
        let d0 = get_fiddled_distance(seed, 0, 0, 0, 0.0, 0.0, 0.0);
        assert!((d0 - 0.18402854919433595).abs() < 1e-15);
        let d1 = get_fiddled_distance(seed, 1, -16, 2, 0.25, 0.5, 0.75);
        assert!((d1 - 1.2579092407226562).abs() < 1e-15);
    }

    #[test]
    fn voronoi_quart_matches_java_probe() {
        let zoom = obfuscate_seed(42);
        assert_eq!(voronoi_quart(zoom, 0, 64, 0), (-1, 15, -1));
        assert_eq!(voronoi_quart(zoom, 1, 64, 1), (0, 15, 0));
        assert_eq!(voronoi_quart(zoom, 8, 64, 8), (2, 15, 2));
        assert_eq!(voronoi_quart(zoom, -5, -20, 3), (-2, -6, 0));
        assert_eq!(voronoi_quart(zoom, 100, 80, -40), (25, 19, -11));
        assert_eq!(voronoi_quart(zoom, 7, 63, 15), (1, 15, 3));
    }

    #[test]
    fn voronoi_can_pick_a_neighbor_quart() {
        let zoom = obfuscate_seed(42);
        // Block (1,64,1) lives in quart (0,16,0) without fuzz; with -2 offset
        // the parent cell is (-1,15,-1) and the winner is a neighbor.
        let raw = (1 >> 2, 64 >> 2, 1 >> 2);
        assert_eq!(raw, (0, 16, 0));
        assert_eq!(voronoi_quart(zoom, 1, 64, 1), (0, 15, 0));
    }

    #[test]
    fn quart_pos_roundtrip() {
        assert_eq!(quart_from_block(7), 1);
        assert_eq!(quart_to_block(1), 4);
        assert_eq!(quart_from_block(-5), -2);
        assert_eq!(quart_to_block(-2), -8);
    }
}
