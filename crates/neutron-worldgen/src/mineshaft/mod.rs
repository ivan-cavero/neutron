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
    for cz in (c0z - SEARCH_RADIUS)..=(c1z + SEARCH_RADIUS) {
        for cx in (c0x - SEARCH_RADIUS)..=(c1x + SEARCH_RADIUS) {
            if !is_mineshaft_chunk(state.seed, cx, cz) {
                continue;
            }
            let pieces = pieces::generate_start(state.seed, cx, cz);
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
            place::place_pieces(region, &pieces, state);
        }
    }
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
}
