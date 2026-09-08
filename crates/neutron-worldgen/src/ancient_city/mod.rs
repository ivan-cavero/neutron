//! `AncientCityStructure` (vanilla 26.2) — jigsaw structure over the
//! `ancient_city` template pools.
//!
//! - placement: `RandomSpreadStructurePlacement.getPotentialStructureChunk`
//!   (spacing 24 / separation 8 / salt 20083232, LINEAR spread)
//! - assembly: [`jigsaw`] (`JigsawPlacement.addPieces`, start pool
//!   `ancient_city/city_center`, anchor `minecraft:city_anchor`, start
//!   height absolute −27, size 7, max horizontal distance 116)
//! - placement: template cells in NBT order into `RegionBuf` (rigid
//!   projection; jigsaw/data markers skipped like vanilla `keepJigsaws=
//!   false` — jigsaws replaced by their `final_state`).
//!
//! Copyright (c) 2026 Neutron Contributors -- MIT License

use crate::legacy_rng::LegacyRandom;
use crate::region_buf::RegionBuf;
use crate::surface::BlockId;
use crate::worldgen::WorldgenState;

mod jigsaw;
mod pools;
mod templates;

pub(crate) const SPACING: i32 = 24;
pub(crate) const SEPARATION: i32 = 8;
pub(crate) const SALT: i32 = 20083232;

/// `RandomSpreadStructurePlacement.getPotentialStructureChunk` (LINEAR
/// spreadType: nextInt(spacing - separation)).
pub fn potential_structure_chunk(level_seed: i64, source_x: i32, source_z: i32) -> (i32, i32) {
    let gx = source_x.div_euclid(SPACING);
    let gz = source_z.div_euclid(SPACING);
    // WorldgenRandom.setLargeFeatureWithSalt:
    // setSeed(x*341873128712 + z*132897987541 + seed + salt)
    let mixed = (gx as i64)
        .wrapping_mul(341_873_128_712)
        .wrapping_add((gz as i64).wrapping_mul(132_897_987_541))
        .wrapping_add(level_seed)
        .wrapping_add(SALT as i64);
    let mut rng = LegacyRandom::new(mixed);
    let limit = SPACING - SEPARATION;
    let sx = gx * SPACING + rng.next_int(limit);
    let sz = gz * SPACING + rng.next_int(limit);
    (sx, sz)
}

pub fn is_city_chunk(level_seed: i64, cx: i32, cz: i32) -> bool {
    potential_structure_chunk(level_seed, cx, cz) == (cx, cz)
}

/// Assemble the piece list for the city anchored at chunk `(cx, cz)`.
pub(crate) fn pieces_for(level_seed: i64, cx: i32, cz: i32) -> Option<Vec<jigsaw::Piece>> {
    let mut rng = LegacyRandom::new(0);
    rng.set_large_feature_seed(level_seed, cx, cz);
    jigsaw::Assembler::assemble(&mut rng, cx * 16, cz * 16)
}

/// Place all city pieces intersecting `region`.
pub fn apply_ancient_city_region(region: &mut RegionBuf, state: &WorldgenState) {
    let c0x = region.origin_x.div_euclid(16);
    let c0z = region.origin_z.div_euclid(16);
    let c1x = c0x + region.chunks - 1;
    let c1z = c0z + region.chunks - 1;
    // A city spans up to ±116 from its anchor — scan a generous ring.
    const SEARCH: i32 = 12;
    let mut placed = 0u64;
    for cz in (c0z - SEARCH)..=(c1z + SEARCH) {
        for cx in (c0x - SEARCH)..=(c1x + SEARCH) {
            if !is_city_chunk(state.seed, cx, cz) {
                continue;
            }
            let Some(pieces) = pieces_for(state.seed, cx, cz) else {
                continue;
            };
            for p in pieces {
                placed += place_piece(region, &p) as u64;
            }
        }
    }
    let _ = placed;
}

/// Place one assembled piece; returns the number of cells written inside
/// the region.
fn place_piece(region: &mut RegionBuf, p: &jigsaw::Piece) -> usize {
    use jigsaw::Rot;
    let Some(tpl) = templates::tpl_by_name(p.tpl_name) else {
        return 0;
    };
    let prev_writer = region.current_writer;
    region.current_writer = crate::writers::ANCIENT_CITY;
    let mut written = 0usize;
    // Vanilla iterates cells in NBT order; parity compares by block so the
    // order only matters for overwrites inside the same template (rare).

    for (_, x, y, z, pal) in tpl.cells {
        let pal = *pal;
        let (tx, tz) = p.rot.transform(*x, *z);
        let wx = p.position.0 + tx;
        let wy = p.position.1 + *y;
        let wz = p.position.2 + tz;
        let Some(b) = block_from_palette(pal) else {
            continue;
        };
        region.set(wx, wy, wz, b);
        written += 1;
    }
    let _ = (Rot::None,);
    region.current_writer = prev_writer;
    written
}

/// Palette entry → BlockId (name + `k=v;k=v` properties; properties that
/// only change state variants are ignored by BlockId::from_name).
fn block_from_palette(pal: u16) -> Option<BlockId> {
    let entry = *templates::PALETTE.get(pal as usize)?;
    let name = entry.split('|').next().unwrap_or(entry);
    BlockId::from_name(name)
}

#[cfg(test)]
mod parity_10101 {
    use super::*;

    /// `RandomSpreadStructurePlacement.getPotentialStructureChunk` golden:
    /// seed 10101's city anchor is chunk (-14,9) (measured from the ref NBT).
    #[test]
    fn potential_chunk_10101() {
        assert_eq!(potential_structure_chunk(10101, -14, 9), (-14, 9));
        assert!(is_city_chunk(10101, -14, 9));
        // grid-cell stability: same 24-cell → same anchor (cell of (-14,9)
        // is x [-24,-1] × z [0,23])
        assert_eq!(
            potential_structure_chunk(10101, -14, 9),
            potential_structure_chunk(10101, -1, 23)
        );
    }

    /// Center piece math: start pool pick, city_anchor adjust and the
    /// groundLevelDelta=1 move are pinned by the ref's center BB
    /// (-228,-52,124)-(-211,-22,164) (chunk (-14,9), seed 10101).
    #[test]
    fn center_piece_10101() {
        let pieces = pieces_for(10101, -14, 9).expect("assembly");
        assert_eq!(pieces[0].bb.min_x, -228);
        assert_eq!(pieces[0].bb.min_y, -52);
        assert_eq!(pieces[0].bb.min_z, 124);
        assert_eq!(pieces[0].bb.max_x, -211);
        assert_eq!(pieces[0].bb.max_y, -22);
        assert_eq!(pieces[0].bb.max_z, 164);
    }

    /// Seed 10101 chunk (-14,9): the ref world's ancient_city start has 89
    /// pieces (measured from the chunk NBT). Two-sided assembly check —
    /// currently 38/89 exact; the drain order diverges at index 21
    /// (see STATE.md ancient-city entry).
    #[test]
    #[ignore = "diagnostic: assembly parity vs ref piece BBs (seed 10101 chunk (-14,9))"]
    fn city10101_assembly_matches_ref() {
        assert!(is_city_chunk(10101, -14, 9), "chunk must be a city chunk");
        let pieces = pieces_for(10101, -14, 9).expect("assembly failed");
        let mut min = (i32::MAX, i32::MAX, i32::MAX);
        let mut max = (i32::MIN, i32::MIN, i32::MIN);
        for p in &pieces {
            min.0 = min.0.min(p.bb.min_x);
            min.1 = min.1.min(p.bb.min_y);
            min.2 = min.2.min(p.bb.min_z);
            max.0 = max.0.max(p.bb.max_x);
            max.1 = max.1.max(p.bb.max_y);
            max.2 = max.2.max(p.bb.max_z);
        }
        let mut out = format!("CITY-ASSEMBLY pieces={}\n", pieces.len());
        for p in &pieces {
            out.push_str(&format!(
                "{}, {}, {}, {}, {}, {} | {} {} {:?}\n",
                p.bb.min_x,
                p.bb.min_y,
                p.bb.min_z,
                p.bb.max_x,
                p.bb.max_y,
                p.bb.max_z,
                p.tpl_name,
                p.rot == super::jigsaw::Rot::None,
                p.position
            ));
        }
        panic!("CITY-DUMP {out}");
    }
}
