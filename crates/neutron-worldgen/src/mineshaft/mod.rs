//! `MineshaftStructure` + `MineshaftPieces` for Minecraft 26.2.
//!
//! Placement uses `legacyProbabilityReducerWithDouble` (`legacy_type_3`).
//! `GenerationContext.makeRandom` is LegacyRandom + `setLargeFeatureSeed`.
//!
//! Pieces: room, corridor, crossing, stairs. Datapack set
//! `worldgen/structure_set/mineshafts.json` (spacing 1, frequency 0.004).
//!
//! - [`pieces`] — piece types + structure tree generation
//! - [`place`]  — carving/placement into the region (`postProcess`)
//!
//! Copyright (c) 2026 Neutron Contributors -- MIT License

use crate::legacy_rng::LegacyRandom;
use crate::region_buf::RegionBuf;
use crate::worldgen::WorldgenState;

mod pieces;
mod place;

/// Diagnostics: piece-tree generation for one start (parity examples).
pub use pieces::generate_start;

pub(super) const MAGIC_START_Y: i32 = 50;
const MAX_DEPTH: i32 = 8;
const MAX_DIST: i32 = 80;
pub(super) const SEA_LEVEL: i32 = 63;
pub(super) const WORLD_MIN_Y: i32 = -64;
const SEARCH_RADIUS: i32 = 8;
const FREQUENCY: f64 = 0.004;

/// `legacy_type_3` = `legacyProbabilityReducerWithDouble`.
pub fn is_mineshaft_chunk(level_seed: i64, cx: i32, cz: i32) -> bool {
    let mut rng = LegacyRandom::new(0);
    rng.set_large_feature_seed(level_seed, cx, cz);
    rng.next_f64() < FREQUENCY
}

/// Generate mineshaft pieces that intersect `region` (starts in ±SEARCH_RADIUS).
pub fn apply_mineshafts_region(region: &mut RegionBuf, state: &WorldgenState) {
    let c0x = region.origin_x.div_euclid(16);
    let c0z = region.origin_z.div_euclid(16);
    let c1x = c0x + region.chunks - 1;
    let c1z = c0z + region.chunks - 1;
    let starts = collect_starts(state, c0x, c0z, c1x, c1z);
    if starts.is_empty() {
        return;
    }
    // Per-origin pass — vanilla runs structure `postProcess` inside
    // `applyBiomeDecoration` per origin: `placeInChunk` only processes pieces
    // whose BB intersects the origin's writable area (the 3x3 chunks around
    // the origin), with the decoration RNG reseeded
    // `setFeatureSeed(setDecorationSeed(seed, ox, oz), 1, 3)` (mineshaft =
    // index 1 within step 3). The stream continues across pieces and starts
    // of the same origin; the LAST origin (in decoration order) to re-run a
    // piece wins its cells. Earlier single-pass model (one LegacyRandom
    // seeded `setLargeFeatureSeed(seed, 0, 0)`) produced cave_air fields the
    // ref never had (chunk (-1,-8) on 424242: 182 cells, all
    // `vanilla=X mine=cave_air`, y -16..-8).
    let order = crate::sculk::decoration_origin_order(region.chunks, region.origin_x, region.origin_z);
    for &(cxl, czl) in order.iter() {
        let ox0 = region.origin_x + cxl * 16;
        let oz0 = region.origin_z + czl * 16;
        apply_mineshafts_origin_with(region, state, ox0, oz0, &starts);
    }
}

/// Mineshaft `postProcess` for ONE origin `(ox0, oz0)` — vanilla runs this
/// inside the decoration loop at step 3 (UNDERGROUND_STRUCTURES), before the
/// origin's later feature steps read the scene.
pub fn apply_mineshafts_origin(
    region: &mut RegionBuf,
    state: &WorldgenState,
    ox0: i32,
    oz0: i32,
) {
    let c0x = ox0.div_euclid(16) - 1;
    let c0z = oz0.div_euclid(16) - 1;
    let starts = collect_starts(state, c0x, c0z, c0x + 2, c0z + 2);
    if starts.is_empty() {
        return;
    }
    apply_mineshafts_origin_with(region, state, ox0, oz0, &starts);
}

fn collect_starts(
    state: &WorldgenState,
    c0x: i32,
    c0z: i32,
    c1x: i32,
    c1z: i32,
) -> Vec<(i32, i32, Vec<pieces::Piece>)> {
    let mut starts: Vec<(i32, i32, Vec<pieces::Piece>)> = Vec::new();
    for cz in (c0z - SEARCH_RADIUS)..=(c1z + SEARCH_RADIUS) {
        for cx in (c0x - SEARCH_RADIUS)..=(c1x + SEARCH_RADIUS) {
            if !is_mineshaft_chunk(state.seed, cx, cz) {
                continue;
            }
            let pieces = (*pieces::generate_start_cached(state.seed, cx, cz)).clone();
            if pieces.is_empty() {
                continue;
            }
            // MineshaftStructure.findGenerationPoint rejects deep_dark at the
            // generation stub. Individual pieces also repeat the blocking check
            // during postProcess below, matching MineshaftPiece.isInInvalidLocation.
            let stub_x = (cx << 4) + 8;
            let stub_z = cz << 4;
            let stub_y = pieces[0].bb.min_y;
            if crate::biome_source::biome_id::DEEP_DARK
                == crate::biome_source::biome_id_at_block(state, stub_x, stub_y, stub_z)
            {
                continue;
            }
            starts.push((cx, cz, pieces));
        }
    }
    starts
}

fn apply_mineshafts_origin_with(
    region: &mut RegionBuf,
    state: &WorldgenState,
    ox0: i32,
    oz0: i32,
    starts: &[(i32, i32, Vec<pieces::Piece>)],
) {
    // Vanilla placeInChunk processes only pieces whose BB intersects the chunk
    // being decorated (the origin's CENTER chunk) — pieces outside it never run
    // for this origin, so their postProcess draws are not consumed.
    let mut selected: Vec<&pieces::Piece> = Vec::new();
    for (_, _, ps) in starts {
        for p in ps {
            if p.bb.max_x >= ox0 && p.bb.min_x <= ox0 + 15 && p.bb.max_z >= oz0 && p.bb.min_z <= oz0 + 15 {
                selected.push(p);
            }
        }
    }
    if selected.is_empty() {
        return;
    }
    let mut rng = crate::feature_rng::FeatureRandom::new(state.seed);
    let dec = rng.set_decoration_seed(state.seed, ox0, oz0);
    rng.set_feature_seed(dec, 1, crate::feature_catalog::step::UNDERGROUND_STRUCTURES);
    let owned: Vec<pieces::Piece> = selected.into_iter().cloned().collect();
    let clip = pieces::Bb {
        min_x: ox0,
        min_y: crate::generator::WORLD_BOTTOM,
        min_z: oz0,
        max_x: ox0 + 15,
        max_y: crate::generator::WORLD_TOP,
        max_z: oz0 + 15,
    };
    place::place_pieces(region, &owned, state, &mut rng, &clip);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::surface::BlockId;

    #[test]
    fn start_4_minus_1_first_eight_bbs_match_vanilla_nbt() {
        let pieces = pieces::generate_start(12345, 4, -1);
        let van = [
            (66, -44, -14, 75, -39, -5),
            (73, -48, -23, 75, -41, -15),
            (73, -48, -43, 75, -46, -24),
            (76, -47, -44, 80, -45, -40),
            (77, -47, -59, 79, -45, -45),
            (77, -51, -68, 79, -44, -60),
            (76, -51, -73, 80, -49, -69),
            (77, -51, -83, 79, -49, -74),
        ];
        assert_eq!(pieces.len(), 121);
        for (i, &(x0, y0, z0, x1, y1, z1)) in van.iter().enumerate() {
            let b = pieces[i].bb;
            assert_eq!(
                (b.min_x, b.min_y, b.min_z, b.max_x, b.max_y, b.max_z),
                (x0, y0, z0, x1, y1, z1),
                "piece {i}"
            );
        }
    }

    #[test]
    fn seed_12345_has_start_at_4_minus_1() {
        // Vanilla chunk (6,-2) structures.References.mineshaft = ChunkPos(4,-1).
        assert!(
            is_mineshaft_chunk(12345, 4, -1),
            "legacy_type_3 must accept (4,-1) for seed 12345"
        );
    }

    #[test]
    fn dump_start_4_minus_1() {
        let pieces = pieces::generate_start(12345, 4, -1);
        assert_eq!(pieces.len(), 121);
        assert_eq!(pieces[0].bb.min_y, -44);
    }

    #[test]
    fn apply_carves_west_neighbor() {
        let g = crate::generator::ChunkGenerator::new(12345);
        let region = g.generate_ores_region(6, -2);
        let mut air = 0u32;
        for y in -64..16 {
            for z in -32..-16 {
                for x in 80..96 {
                    if region.get(x, y, z) == BlockId::Air {
                        air += 1;
                    }
                }
            }
        }
        eprintln!("(5,-2) y<16 air={air}");
        assert!(air > 0, "mineshaft must carve air into (5,-2)");
    }

    /// Diagnostic: accepted mineshaft start chunks near the origin for a seed
    /// (run `-- --nocapture`). Used to pick parity measurement windows.
    #[test]
    fn dump_start_chunks_424242() {
        let mut hits = Vec::new();
        for cz in -96..96 {
            for cx in -96..96 {
                if is_mineshaft_chunk(424242, cx, cz) {
                    hits.push((cx, cz));
                }
            }
        }
        eprintln!("424242 mineshaft starts: {hits:?}");
        assert!(!hits.is_empty());
    }
}

#[cfg(test)]
mod parity_10101 {
    /// Seed 10101: the ref world has mineshaft starts at chunks (-14,0),
    /// (-2,8) and (12,8), and none at the negative-control chunks.
    /// The ref's (-14,0) start has 147 children (measured). My generate_start
    /// must match — the 194k air->deepslate family on 10101 (y -51..-32,
    /// z 12..95, x -224..-113) is mineshaft corridor air the port misses.
    #[test]
    #[ignore = "diagnostic: dump mineshaft piece BBs for the ref diff"]
    /// 424242 mineshaft starts within ±16 chunks.
    #[test]
    #[ignore = "diagnostic: scan 424242 mineshaft starts"]
    fn mineshaft_starts_424242() {
        let mut hits = Vec::new();
        for cz in -16..=16 {
            for cx in -16..=16 {
                if super::is_mineshaft_chunk(424242, cx, cz) {
                    hits.push((cx, cz));
                }
            }
        }
        eprintln!("MS-424242 starts: {hits:?}");
        panic!("SCAN-DONE");
    }

    #[test]
    #[ignore = "diagnostic: dump mineshaft piece BBs for the ref diff"]
    fn mineshaft_piece_dump_10101() {
        let pieces = super::pieces::generate_start(10101, -14, 0);
        for (i, p) in pieces.iter().enumerate() {
            eprintln!(
                "MS-PIECE {} {:?} {} {} {} {} {} {}",
                i,
                p.kind,
                p.bb.min_x,
                p.bb.min_y,
                p.bb.min_z,
                p.bb.max_x,
                p.bb.max_y,
                p.bb.max_z
            );
        }
        panic!("MS-DUMP-DONE");
    }

    #[test]
    fn mineshaft_starts_match_ref_10101() {
        for (cx, cz) in [(-14, 0), (-2, 8), (12, 8)] {
            assert!(
                super::is_mineshaft_chunk(10101, cx, cz),
                "expected mineshaft start at ({cx},{cz})"
            );
        }
        for (cx, cz) in [(-13, 1), (-10, 5), (-5, 3)] {
            assert!(
                !super::is_mineshaft_chunk(10101, cx, cz),
                "unexpected mineshaft start at ({cx},{cz})"
            );
        }
        assert!(!super::pieces::generate_start(10101, -14, 0).is_empty());
    }
}
