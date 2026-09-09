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
    let state = WorldgenState::overworld(level_seed);
    pieces_with_center(&state, cx, cz).map(|(p, _)| p)
}

/// Assembly cache — the jigsaw expansion is expensive (~1-2 ms per start)
/// and every chunk in a ±7-chunk radius repeats it during doFill. Values
/// are Arc-shared so cache hits never clone the piece list.
type AssemblyCache = std::collections::HashMap<
    (i64, i32, i32),
    Option<(std::sync::Arc<Vec<jigsaw::Piece>>, (i32, i32, i32))>,
>;

fn assembly_cache() -> &'static std::sync::Mutex<AssemblyCache> {
    static CACHE: std::sync::OnceLock<std::sync::Mutex<AssemblyCache>> =
        std::sync::OnceLock::new();
    CACHE.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()))
}

/// Gated, cached assembly (the deep_dark biome gate included).
pub fn pieces_gated(
    state: &WorldgenState,
    cx: i32,
    cz: i32,
) -> Option<(std::sync::Arc<Vec<jigsaw::Piece>>, (i32, i32, i32))> {
    let key = (state.seed, cx, cz);
    if let Ok(cache) = assembly_cache().lock() {
        if let Some(hit) = cache.get(&key) {
            return hit.clone();
        }
    }
    let result = pieces_with_center(state, cx, cz)
        .map(|(p, c)| (std::sync::Arc::new(p), c));
    if let Ok(mut cache) = assembly_cache().lock() {
        cache.insert(key, result.clone());
    }
    result
}

/// Assemble + biome gate. Returns the pieces and the stub position
/// (`isValidBiome` samples the biome here — deep_dark required).
fn pieces_with_center(
    state: &WorldgenState,
    cx: i32,
    cz: i32,
) -> Option<(Vec<jigsaw::Piece>, (i32, i32, i32))> {
    let mut rng = LegacyRandom::new(0);
    rng.set_large_feature_seed(state.seed, cx, cz);
    let (mut pieces, center) = jigsaw::Assembler::assemble(&mut rng, cx * 16, cz * 16)?;
    // isValidBiome → BiomeManager.getBiome(stub): the 8-corner fiddled
    // voronoi vote (obfuscated zoom seed), NOT the raw quart climate. The
    // 8 corner climate samples SHARE one flat_cache/cache_2d marker state
    // (vanilla's NoiseChunk caches are per-instance; the sampler's caches
    // persist across the 8 samples and pollute corners 2..8 with corner-1
    // shift values) — replicate that exactly.
    // isValidBiome → BiomeManager.getBiome(stub): the 8-corner fiddled
    // voronoi vote with CLEAN caches (structure stage precedes NoiseChunk
    // creation, so the sampler caches are cold). Verified: 10101 stub →
    // deep_dark (city placed); 424242 stub → deep_dark per this lookup but
    // dark_forest per the vanilla probe — the remaining gap is the climate
    // evaluation at the WINNER quart (a ±1-quart neighbor of the stub),
    // probed next.
    let biome = crate::biome::manager::biome_id_at_block(state, center.0, center.1, center.2);
    if std::env::var_os("NEUTRON_CITY_DRAWS").is_some() {
        eprintln!("NEU-GATE424 seed={} center={:?} biome={}", state.seed, center, biome);
    }
    if biome != crate::biome::source::biome_id::DEEP_DARK {
        return None;
    }
    if std::env::var_os("NEUTRON_CITY_DRAWS").is_some() {
        let st3 = crate::worldgen::WorldgenState::overworld(424242);
        let cl = crate::biome::manager::climate_at(&st3, -224, -40, 140);
        eprintln!(
            "NEU-CLIMATE-WINNER (-224,-40,140) t={} h={} c={} e={} d={} w={}",
            cl.temperature, cl.humidity, cl.continentalness, cl.erosion, cl.depth, cl.weirdness
        );
        let found = crate::biome::source::find_biome(&cl);
        eprintln!("NEU-FIND-BIOME winner={found}");
        for (px, py, pz) in [(-208i32, -37, 148), (-208, -37, 149)] {
            let cl2 = crate::biome::manager::climate_at(&st3, px, py, pz);
            let f2 = crate::biome::source::find_biome(&cl2);
            eprintln!(
                "NEU-GATE-CLIMATE ({px},{py},{pz}) t={} c={} e={} d={} w={} biome={f2}",
                cl2.temperature, cl2.continentalness, cl2.erosion, cl2.depth, cl2.weirdness
            );
        }
        let st3 = crate::worldgen::WorldgenState::overworld(424242);
        let sx = st3
            .noises
            .get("offset")
            .get_value(-224.0 * 0.25, 0.0, 140.0 * 0.25)
            * 4.0;
        let sz = st3
            .noises
            .get("offset")
            .get_value(140.0 * 0.25, -224.0 * 0.25, 0.0)
            * 4.0;
        eprintln!("NEU-SHIFT-WINNER sx={sx} sz={sz}");
        eprintln!(
            "NEU-SHIFTED-C {}",
            st3.noises
                .get("continentalness")
                .get_value(-224.0 * 0.25 + sx, 0.0, 140.0 * 0.25 + sz)
        );
    }
    // CAVE-BIOME GATE FINDING (s35): at seed 424242 stub (-219,-37,144) the
    // vanilla climate target is t=1217 h=4117 c=1600 e=-1866 d=11053 w=-4634
    // (dark_forest → no city), while this lookup returns t=-2611 h=-9 c=2608
    // e=-3724 d=11778 w=4538 (deep_dark → city placed). The divergence is in
    // the shifted-noise climate evaluation (temperature/offset noise), not
    // the gate logic. Until the climate lookup matches vanilla at negative-y
    // cave positions, the gate cannot be trusted.
    let _ = state;
    if std::env::var_os("NEUTRON_CITY_DRAWS").is_some() {
        // Vanilla ProbeNoiseVal 424242: offset(-54.75,0,36) = -0.19571926,
        // temperature(-54.75,-9.25,36) = 0.11632854.
        let st2 = crate::worldgen::WorldgenState::overworld(424242);
        eprintln!(
            "NEU-NOISE offset={} temperature={}",
            st2.noises.get("offset").get_value(-54.75, 0.0, 36.0),
            st2.noises.get("temperature").get_value(-54.75, -9.25, 36.0)
        );
    }
    if std::env::var_os("NEUTRON_CITY_DRAWS").is_some() {
        eprintln!(
            "NEU-GATE biome={} center={:?}",
            biome, center
        );
    }
    Some((pieces, center))
}

/// Diagnostic: count cells written per piece for one city.
#[test]
#[ignore = "diagnostic: placement write counts for seed 10101 city"]
fn city10101_place_counts() {
    let pieces = pieces_for(10101, -14, 9).expect("assembly");
    // 424242 has a potential city at (-13,9) — the gate must REJECT it
    // (winner quart climate = dark_forest per vanilla).
    assert!(pieces_for(424242, -13, 9).is_none(), "424242 gate must reject");
    let mut region = crate::region_buf::RegionBuf::new(-16, 8, 3);
    let mut total = 0usize;
    for p in &pieces {
        let w = place_piece(&mut region, p);
        total += w;
    }
    eprintln!(
        "PLACE-TOTAL total={total} pieces={} junctions={}",
        pieces.len(),
        pieces.iter().map(|p| p.junctions.len()).sum::<usize>()
    );
    eprintln!(
        "NEU-NOISE offset={} temperature={}",
        crate::worldgen::WorldgenState::overworld(424242)
            .noises
            .get("offset")
            .get_value(-54.75, 0.0, 36.0),
        crate::worldgen::WorldgenState::overworld(424242)
            .noises
            .get("temperature")
            .get_value(-54.75, -9.25, 36.0)
    );
    eprintln!(
        "NEU-NOISE2 continentalness(shifted-input -54.75,-9.25,36)={}",
        crate::worldgen::WorldgenState::overworld(424242)
            .noises
            .get("continentalness")
            .get_value(-54.75, -9.25, 36.0)
    );
    {
        let st3 = crate::worldgen::WorldgenState::overworld(424242);
        let sx = st3
            .noises
            .get("offset")
            .get_value(-219.0 * 0.25, 0.0, 144.0 * 0.25)
            * 4.0;
        let sz = st3
            .noises
            .get("offset")
            .get_value(144.0 * 0.25, -219.0 * 0.25, 0.0)
            * 4.0;
        eprintln!("NEU-SHIFT sx={sx} sz={sz}");
        eprintln!(
            "NEU-SHIFTED-VALUE continents={} temperature={}",
            st3.noises
                .get("continentalness")
                .get_value(-219.0 * 0.25 + sx, 0.0, 144.0 * 0.25 + sz),
            st3.noises
                .get("temperature")
                .get_value(-219.0 * 0.25 + sx, 0.0, 144.0 * 0.25 + sz)
        );
    }
    // read back the center chunk's column: world x -224..-209, z 144..159
    let mut nonzero = 0usize;
    let mut air = 0usize;
    for y in crate::generator::WORLD_BOTTOM..crate::generator::WORLD_TOP {
        for lz in 144..160 {
            for lx in -224..-208 {
                let b = region.get(lx, y, lz);
                if b == crate::surface::BlockId::Air {
                    air += 1;
                } else {
                    nonzero += 1;
                }
            }
        }
    }
    eprintln!("CENTER-READBACK nonzero={nonzero} air={air}");
    panic!("PLACE-DONE");
}

/// Beardifier piece boxes for every city start whose pieces come within
/// 12 blocks of `cx,cz` (`Beardifier.forStructuresInChunk`: piece bb
/// intersects the chunk range expanded by 12; RIGID pieces only — all
/// city elements are rigid).
pub fn beard_boxes_for(
    state: &WorldgenState,
    cx: i32,
    cz: i32,
) -> (Vec<crate::density::BeardBox>, Vec<crate::density::BeardJunction>) {
    let mut out = Vec::new();
    let mut junctions = Vec::new();
    // A city spans up to ±116 from its anchor — scan the anchor grid.
    const SEARCH: i32 = 12;
    for cz in (cz - SEARCH)..=(cz + SEARCH) {
        for cx in (cx - SEARCH)..=(cx + SEARCH) {
            if !is_city_chunk(state.seed, cx, cz) {
                continue;
            }
            let Some((pieces, _)) = pieces_gated(state, cx, cz) else {
                continue;
            };
            for p in pieces.iter() {
                let bb = &p.bb;
                // isCloseToChunk(chunkPos, 12)
                let near = bb.max_x >= cx * 16 - 12
                    && bb.min_x <= cx * 16 + 15 + 12
                    && bb.max_z >= cz * 16 - 12
                    && bb.min_z <= cz * 16 + 15 + 12;
                if !near {
                    continue;
                }
                out.push(crate::density::BeardBox {
                    min_x: bb.min_x,
                    max_x: bb.max_x,
                    min_y: bb.min_y,
                    max_y: bb.max_y,
                    min_z: bb.min_z,
                    max_z: bb.max_z,
                    ground_level_delta: p.ground_level_delta,
                });
                for j in &p.junctions {
                    // vanilla filters junctions to the chunk ±12 window
                    if j.source_x > cx * 16 - 12
                        && j.source_z > cz * 16 - 12
                        && j.source_x < cx * 16 + 15 + 12
                        && j.source_z < cz * 16 + 15 + 12
                    {
                        junctions.push(crate::density::BeardJunction {
                            source_x: j.source_x,
                            source_ground_y: j.source_ground_y,
                            source_z: j.source_z,
                        });
                    }
                }
            }
        }
    }
    (out, junctions)
}

/// Place all city pieces intersecting `region`.
pub fn apply_ancient_city_region(region: &mut RegionBuf, state: &WorldgenState) {
    let c0x = region.origin_x.div_euclid(16);
    let c0z = region.origin_z.div_euclid(16);
    let c1x = c0x + region.chunks - 1;
    let c1z = c0z + region.chunks - 1;
    if std::env::var_os("NEUTRON_CITY_DRAWS").is_some() {
        eprintln!(
            "CITY-APPLY region=({}, {}) chunks={} seed={}",
            region.origin_x, region.origin_z, region.chunks, state.seed
        );
    }
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
        assert!(is_city_chunk(424242, -13, 9), "chunk must be a city chunk");
        let pieces = pieces_for(424242, -13, 9).expect("assembly failed");
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
        eprintln!("CITY-ASSEMBLY pieces={}", pieces.len());
        for p in &pieces {
            let rot = match p.rot {
                super::jigsaw::Rot::None => "NONE",
                super::jigsaw::Rot::Cw90 => "CLOCKWISE_90",
                super::jigsaw::Rot::Cw180 => "CLOCKWISE_180",
                super::jigsaw::Rot::Ccw90 => "COUNTERCLOCKWISE_90",
            };
            eprintln!(
                "NEU-PIECE {} {} {} {} {} {} {} {}",
                if p.is_feature { "FEATURE" } else { p.tpl_name },
                rot,
                p.bb.min_x,
                p.bb.min_y,
                p.bb.min_z,
                p.bb.max_x,
                p.bb.max_y,
                p.bb.max_z
            );
        }
        panic!("CITY-DUMP-END");
    }
}

#[cfg(test)]
mod shuffle_micro {
    use crate::legacy_rng::LegacyRandom;

    /// Java oracle (UtilShuffle, seed -321779700379): SHUFFLED [2,5,0,4,1,3],
    /// nextInt(16) after = [0,8,15,0,6,13].
    #[test]
    fn util_shuffle_matches_java() {
        let mut rng = LegacyRandom::new(-321779700379);
        let mut list: Vec<usize> = (0..6).collect();
        // Util.shuffle: for i in (2..=len).rev() { swap(i-1, nextInt(i)) }
        for i in (2..=list.len()).rev() {
            let swap_to = rng.next_int(i as i32) as usize;
            list.swap(i - 1, swap_to);
        }
        assert_eq!(list, vec![2, 5, 0, 4, 1, 3], "shuffle mismatch");
        let ints: Vec<i32> = (0..6).map(|_| rng.next_int(16)).collect();
        assert_eq!(ints, vec![0, 8, 15, 0, 6, 13], "nextInt mismatch");
    }
}

#[cfg(test)]
mod city_scan_424242 {
    use super::*;

    /// List city chunks in the 424242 ref range (chunks -11..11).
    #[test]
    #[ignore = "diagnostic: scan 424242 city starts"]
    fn city424242_starts() {
        let mut hits = Vec::new();
        for cz in -20..=20 {
            for cx in -20..=20 {
                if is_city_chunk(424242, cx, cz) {
                    hits.push((cx, cz));
                }
            }
        }
        eprintln!("CITY-424242 starts: {hits:?}");
        panic!("SCAN-DONE");
    }
}
