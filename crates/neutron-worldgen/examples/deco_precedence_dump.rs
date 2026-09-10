//! deco_precedence_dump — VANILLA-only decoration-precedence miner.
//!
//! Question: in what EFFECTIVE ORDER did vanilla decorate origin chunks during
//! the canonical pregen of the ref world (seed default 424242)?
//!
//! Evidence class: step-6 base-stone ores (granite/diorite/andesite/tuff,
//! FeatureSorter idx 2..8) are `minecraft:ore` size 64 blobs that
//! UNCONDITIONALLY overwrite any `base_stone_overworld`-tag cell they touch
//! (discard 0.0), draw-for-draw identical Neutron<->vanilla (proven by
//! ore_swap_dump.rs shared-RNG replay). When blobs from two DIFFERENT origin
//! passes touch one cell, vanilla's final block names whichever pass ran
//! LAST => per-cell precedence constraints "loser pass finished before
//! winner pass". Mining every contested cell yields the pairwise graph this
//! dump writes out.
//!
//! Method.
//!   1. RNG replay enumerates every blob anchor/sphere for the 9 candidate
//!      origins around each full-status consumer chunk (heightmap gate reads
//!      ONE global pre-carver WG heightfield built once via
//!      ChunkGenerator::generate_noise_and_surface; carvers never mutate
//!      region heightmaps, so values equal ore_swap_dump's RegionBuf gates).
//!   2. Vanilla sections decoded per consumer chunk. A cell is USABLE iff the
//!      final block equals exactly one surviving claimant's mineral while >=1
//!      other origin claimed a different mineral (same-mineral contests give
//!      no signal).
//!   3. PHANTOM filter: any blob claiming DECO_MIN_VOTES (4) cells inside the
//!      loaded chunk must show its own mineral on >=DECO_MIN_RATIO (0.5) of
//!      them, else terrain microdiff fabricated the placement and all its
//!      edges are dropped (drops support, never invents it). Blobs with fewer
//!      in-chunk cells are UNKNOWN -> excluded (their bulk validates wherever
//!      most of their volume lies).
//!   4. Models fit per pair: consistent iff rank_model(winner) >
//!      rank_model(loser) for {world_origin dist^2 asc, spiral, row,
//!      canonical_pregen strips} — same constructions as ore_swap_dump.
//!
//! Known biases, counted not assumed away: "ambiguous" cells (>1 origin
//! shares the winner mineral), "orphan" cells (final==family mineral but the
//! matching claimant is missing/phantom-filtered), "masked" cells (final
//! block overwritten by a later feature family).
//!
//! Citations — Java: OreFeature.place/doPlace/canPlaceOre
//! tools/mc-decompiler/output/26.2/src/net/minecraft/world/level/levelgen/
//! feature/OreFeature.java:23-53/55-166/168-181; ChunkGenerator
//! .applyBiomeDecoration net/minecraft/world/level/chunk/ChunkGenerator.java:
//! 318-401. Neutron: src/features.rs:49 apply_underground_ores_region,
//! :1060-1082 angle/y draws + ocean-floor gate, :1086-1127 spheres + overlap
//! cull, :1147-1202 write loop, :1205 target_match.
//!
//! Usage:
//!   cargo run --release -p neutron-worldgen --example deco_precedence_dump \
//!        [seed] [region_dir] [out_csv]
//! Defaults: seed 424242, canonical ref dir, /tmp/opencode/deco_pairs_<seed>.csv

use neutron_world::nbt::ussr_nbt::owned::{List, Tag};
use neutron_world::nbt::{compound_get, read_nbt};
use neutron_world::Region;
use neutron_worldgen::biome_manager::biome_id_at_block;
use neutron_worldgen::feature_catalog;
use neutron_worldgen::feature_dispatch::biome_id_to_name;
use neutron_worldgen::feature_rng::FeatureRandom;
use neutron_worldgen::generator::{WORLD_BOTTOM, WORLD_TOP};
use neutron_worldgen::worldgen::WorldgenState;
use neutron_worldgen::ChunkGenerator;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::io::Write;

const STEP: i32 = 6;

/// (FeatureSorter idx, placed id, output short-name, band mode)
const BASE_ORES: [(i32, &str, &'static str, Mode); 7] = [
    (2, "ore_granite_upper", "granite", Mode::Upper),
    (3, "ore_granite_lower", "granite", Mode::Lower),
    (4, "ore_diorite_upper", "diorite", Mode::Upper),
    (5, "ore_diorite_lower", "diorite", Mode::Lower),
    (6, "ore_andesite_upper", "andesite", Mode::Upper),
    (7, "ore_andesite_lower", "andesite", Mode::Lower),
    (8, "ore_tuff", "tuff", Mode::Tuff),
];

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    Upper,
    Lower,
    Tuff,
}

const FAMILIES: [&str; 4] = ["granite", "diorite", "andesite", "tuff"];

fn fam_short(feat: &str) -> &'static str {
    match feat {
        "ore_granite_upper" => "g_upper",
        "ore_granite_lower" => "g_lower",
        "ore_diorite_upper" => "d_upper",
        "ore_diorite_lower" => "d_lower",
        "ore_andesite_upper" => "a_upper",
        "ore_andesite_lower" => "a_lower",
        "ore_tuff" => "tuff",
        _ => "?",
    }
}

type CellKey = (i32, i32, i32);
type OriginKey = (i32, i32);

thread_local! {
    /// Probe boxes fully outside the heightfield buffer (expected 0).
    static GATE_UNRESOLVED: std::cell::RefCell<u64> = std::cell::RefCell::new(0);
    /// Probe pixels missing inside a passing box (expected 0 given +2 margin).
    static PROBE_MISSING_COLS: std::cell::RefCell<u64> = std::cell::RefCell::new(0);
}

// ---------------------------------------------------------------------------
// Global heightfield (pre-carver OCEAN_FLOOR_WG source for ore gates)
// ---------------------------------------------------------------------------

struct HField {
    x0: i32,
    z0: i32,
    nx: usize,
    nz: usize,
    maps: Vec<Vec<i16>>, // [(cz-z0)*nx + cx-x0][lz*16+lx] = solid_y
}

impl HField {
    #[inline]
    fn solid_y(&self, x: i32, z: i32) -> Option<i16> {
        let ix = ((x >> 4) - self.x0) as usize;
        let iz = ((z >> 4) - self.z0) as usize;
        if ix >= self.nx || iz >= self.nz {
            return None;
        }
        Some(self.maps[iz * self.nx + ix][(((z & 15) << 4) | (x & 15)) as usize])
    }
}

fn build_hfield(gen: &ChunkGenerator, x0: i32, z0: i32, nx: usize, nz: usize) -> HField {
    println!("[hfield] building {}x{}={} chunks at [{},{}]", nx, nz, nx * nz, x0, z0);
    let _ = std::io::stdout().flush();
    let t0 = std::time::Instant::now();
    let mut maps: Vec<Vec<i16>> = Vec::with_capacity(nx * nz);
    let total = (nx * nz) as u64;
    let mut done = 0u64;
    for iz in 0..nz {
        for ix in 0..nx {
            let (_, hm, _) = gen.generate_noise_and_surface(x0 + ix as i32, z0 + iz as i32);
            maps.push(hm);
            done += 1;
            if done % 128 == 0 || done == total {
                let el = t0.elapsed().as_secs_f64();
                println!(
                    "[hfield] {}/{} elapsed {:.0}s eta {:.0}s",
                    done,
                    total,
                    el,
                    el / done.max(1) as f64 * (total - done) as f64
                );
                let _ = std::io::stdout().flush();
            }
        }
    }
    HField { x0, z0, nx, nz, maps }
}

// ---------------------------------------------------------------------------
// Vanilla section decode (identical decoder to ore_swap_dump.rs)
// ---------------------------------------------------------------------------

fn load_vanilla(region_dir: &str, cx: i32, cz: i32) -> Option<HashMap<(u8, i32, u8), String>> {
    let (rx, rz) = (cx >> 5, cz >> 5);
    let path = std::path::PathBuf::from(format!("{region_dir}/r.{rx}.{rz}.mca"));
    let region = Region::open(&path).ok()?.with_coords(rx, rz);
    let data = region.get_chunk(cx & 31, cz & 31).ok()??;
    let nbt = read_nbt(&data).ok()?;
    if let Some(Tag::String(s)) = compound_get(&nbt.compound, "Status") {
        if !s.to_string().ends_with("full") {
            return None;
        }
    } else {
        return None;
    }
    let sections = match compound_get(&nbt.compound, "sections") {
        Some(Tag::List(List::Compound(l))) => l,
        _ => return None,
    };
    let mut map = HashMap::new();
    for sec in sections {
        let y_sec = match compound_get(sec, "Y") {
            Some(Tag::Byte(y)) => *y as i8 as i32,
            Some(Tag::Int(y)) => *y,
            _ => continue,
        };
        let Some(Tag::Compound(bs)) = compound_get(sec, "block_states") else { continue };
        let Some(Tag::List(List::Compound(palette))) = compound_get(bs, "palette") else {
            continue;
        };
        let names: Vec<String> = palette
            .iter()
            .map(|pc| match compound_get(pc, "Name") {
                Some(Tag::String(s)) => s.to_string().trim_start_matches("minecraft:").to_string(),
                _ => "air".into(),
            })
            .collect();
        if names.len() == 1 {
            for i in 0..4096u32 {
                map.insert(
                    ((i & 15) as u8, y_sec * 16 + (i >> 8) as i32, ((i >> 4) & 15) as u8),
                    names[0].clone(),
                );
            }
            continue;
        }
        let bits = ((names.len() - 1).ilog2() + 1).max(4) as u32;
        let Some(Tag::LongArray(d)) = compound_get(bs, "data") else { continue };
        let longs: Vec<i64> = d.to_vec();
        let epl = 64 / bits;
        let mask = (1u64 << bits) - 1;
        for i in 0..4096u32 {
            let li = (i / epl) as usize;
            let bo = (i % epl) * bits;
            let vidx = ((longs[li] as u64) >> bo) & mask;
            map.insert(
                ((i & 15) as u8, y_sec * 16 + (i >> 8) as i32, ((i >> 4) & 15) as u8),
                names.get(vidx as usize).cloned().unwrap_or_default(),
            );
        }
    }
    Some(map)
}

// ---------------------------------------------------------------------------
// Claim replay (core copied from ore_swap_dump.rs; height source swapped)
// ---------------------------------------------------------------------------

/// One surviving claim per (origin-pass, feature) on a cell. Replacement rules
/// mirror in-pass write order: later fidx clobbers same-origin claims, later
/// attempt clobbers the same-feature previous attempt.
#[derive(Clone)]
struct Claim {
    olx: i8,
    olz: i8,
    fidx: i32,
    feat: &'static str,
    block: &'static str,
}

struct BlobTrace {
    org: OriginKey,
    fidx: i32,
    feat: &'static str,
    block: &'static str,
    cells: Vec<CellKey>, // inside the scanned chunk only
}

#[allow(clippy::too_many_arguments)]
fn simulate_consumer(
    state: &WorldgenState,
    hf: &HField,
    seed: i64,
    ccx: i32,
    ccz: i32,
) -> (HashMap<CellKey, Vec<Claim>>, Vec<BlobTrace>) {
    let x_lo = ccx * 16;
    let z_lo = ccz * 16;
    let mut out: HashMap<CellKey, Vec<Claim>> = HashMap::new();
    let mut traces: Vec<BlobTrace> = Vec::new();
    for dzl in -1..=1i32 {
        for dxl in -1..=1i32 {
            let ox0 = (ccx + dxl) * 16;
            let oz0 = (ccz + dzl) * 16;
            let mut rng = FeatureRandom::new(seed);
            let dec = rng.set_decoration_seed(seed, ox0, oz0);
            for &(fidx, fname, bname, mode) in BASE_ORES.iter() {
                rng.set_feature_seed(dec, fidx, STEP);
                let attempts: u16 = match mode {
                    Mode::Upper => {
                        if rng.next_f32() < 1.0f32 / 6.0 { 1 } else { 0 } // rarity 6
                    }
                    _ => 2,
                };
                for k in 1..=attempts {
                    let lx = rng.next_int(16);
                    let lz = rng.next_int(16);
                    let x = ox0 + lx;
                    let z = oz0 + lz;
                    let y = match mode {
                        Mode::Upper => 64 + rng.next_int(65),
                        Mode::Lower => rng.next_int(61),
                        Mode::Tuff => -64 + rng.next_int(65),
                    };
                    let bid = biome_id_at_block(state, x, y, z);
                    let bn = biome_id_to_name(bid);
                    let listed = feature_catalog::features_at_step(bn, STEP)
                        .iter()
                        .any(|f| f.trim_start_matches("minecraft:") == fname);
                    if !listed {
                        continue;
                    }
                    place_blob_claims(
                        &mut rng, hf, x, y, z, ox0, oz0, fidx, fname, bname, dxl as i8,
                        dzl as i8, k, x_lo, z_lo, &mut out, &mut traces,
                    );
                }
            }
        }
    }
    (out, traces)
}

/// features.rs:1046 place_ore_blob_inner.
#[allow(clippy::too_many_arguments)]
fn place_blob_claims(
    rng: &mut FeatureRandom,
    hf: &HField,
    ox: i32,
    oy: i32,
    oz: i32,
    oox: i32,
    ooz: i32,
    fidx: i32,
    fname: &'static str,
    bname: &'static str,
    dxl: i8,
    dzl: i8,
    _attempt: u16,
    x_lo: i32,
    z_lo: i32,
    out: &mut HashMap<CellKey, Vec<Claim>>,
    traces: &mut Vec<BlobTrace>,
) {
    let size = 64i32;
    let angle = rng.next_f32() * PI;
    let f = size as f32 / 8.0;
    let start_x = ox as f64 + (angle as f64).sin() * f as f64;
    let end_x = ox as f64 - (angle as f64).sin() * f as f64;
    let start_z = oz as f64 + (angle as f64).cos() * f as f64;
    let end_z = oz as f64 - (angle as f64).cos() * f as f64;
    let start_y = (oy + rng.next_int(3) - 2) as f64;
    let end_y = (oy + rng.next_int(3) - 2) as f64;

    let cellw = ((size as f32 / 16.0 * 2.0 + 1.0) / 2.0).ceil() as i32;
    let f_ceil = f.ceil() as i32;
    let sbx = ox - f_ceil - cellw;
    let sby = oy - 2 - cellw;
    let sbz = oz - f_ceil - cellw;
    let size_xz = 2 * (f_ceil + cellw);
    let size_y = 2 * (2 + cellw);

    // ocean_floor_wg_allows_ore probe (pre-carver WG heights)
    let mut pass = false;
    let mut missing = 0usize;
    let mut cols = 0usize;
    for px in sbx..=sbx + size_xz {
        for pz in sbz..=sbz + size_xz {
            cols += 1;
            match hf.solid_y(px, pz) {
                Some(h) => {
                    if sby <= h as i32 {
                        pass = true;
                    }
                }
                None => missing += 1,
            }
        }
    }
    if !pass {
        if missing == cols {
            GATE_UNRESOLVED.with(|g| *g.borrow_mut() += 1);
        }
        return;
    }
    if missing > 0 {
        PROBE_MISSING_COLS.with(|g| *g.borrow_mut() += missing as u64);
    }

    let mut spheres = vec![0f64; (size as usize) * 4];
    for i in 0..size {
        let t = i as f32 / size as f32;
        let td = t as f64;
        let sx = lerp(td, start_x, end_x);
        let sy = lerp(td, start_y, end_y);
        let sz = lerp(td, start_z, end_z);
        let blip = rng.next_f64() * size as f64 / 16.0;
        let radius = (((sin_tbl((PI * t) as f64) + 1.0f32) as f64) * blip + 1.0) / 2.0;
        let base = (i as usize) * 4;
        spheres[base] = sx;
        spheres[base + 1] = sy;
        spheres[base + 2] = sz;
        spheres[base + 3] = radius;
    }
    for i in 0..size - 1 {
        let bi = (i as usize) * 4;
        if spheres[bi + 3] <= 0.0 {
            continue;
        }
        for j in (i + 1)..size {
            let bj = (j as usize) * 4;
            if spheres[bj + 3] <= 0.0 {
                continue;
            }
            let dx = spheres[bi] - spheres[bj];
            let dy = spheres[bi + 1] - spheres[bj + 1];
            let dz = spheres[bi + 2] - spheres[bj + 2];
            let dr = spheres[bi + 3] - spheres[bj + 3];
            if dr * dr > dx * dx + dy * dy + dz * dz {
                if dr > 0.0 {
                    spheres[bj + 3] = -1.0;
                } else {
                    spheres[bi + 3] = -1.0;
                }
            }
        }
    }

    let ti = match traces.iter().position(|t| t.org == (oox, ooz) && t.fidx == fidx) {
        Some(i) => i,
        None => {
            traces.push(BlobTrace {
                // chunk coords (oox/ooz arrive as block-corner coords)
                org: (oox >> 4, ooz >> 4),
                fidx,
                feat: fname,
                block: bname,
                cells: Vec::new(),
            });
            traces.len() - 1
        }
    };

    let mut bitset = vec![false; (size_xz * size_y * size_xz).max(1) as usize];
    for i in 0..size {
        let bi = (i as usize) * 4;
        let r = spheres[bi + 3];
        if r < 0.0 {
            continue;
        }
        let scx = spheres[bi];
        let scy = spheres[bi + 1];
        let scz = spheres[bi + 2];
        let mn_x = floor(scx - r).max(sbx);
        let mn_y = floor(scy - r).max(sby);
        let mn_z = floor(scz - r).max(sbz);
        let mx_x = floor(scx + r).max(mn_x);
        let mx_y = floor(scy + r).max(mn_y);
        let mx_z = floor(scz + r).max(mn_z);
        for x in mn_x..=mx_x {
            let dx = ((x as f64 + 0.5) - scx) / r;
            if dx * dx >= 1.0 {
                continue;
            }
            for y in mn_y..=mx_y {
                if y < WORLD_BOTTOM || y >= WORLD_TOP {
                    continue;
                }
                let dy = ((y as f64 + 0.5) - scy) / r;
                if dx * dx + dy * dy >= 1.0 {
                    continue;
                }
                for z in mn_z..=mx_z {
                    let dz = ((z as f64 + 0.5) - scz) / r;
                    if dx * dx + dy * dy + dz * dz >= 1.0 {
                        continue;
                    }
                    if x < x_lo || x >= x_lo + 16 || z < z_lo || z >= z_lo + 16 {
                        continue;
                    }
                    let bit =
                        ((x - sbx) + (y - sby) * size_xz + (z - sbz) * size_xz * size_y) as usize;
                    if bit >= bitset.len() || bitset[bit] {
                        continue;
                    }
                    bitset[bit] = true;
                    traces[ti].cells.push((x, y, z));
                    let e = out.entry((x, y, z)).or_default();
                    if let Some(prev) = e.last_mut().filter(|p| p.olx == dxl && p.olz == dzl) {
                        if prev.fidx <= fidx {
                            *prev = Claim {
                                olx: dxl,
                                olz: dzl,
                                fidx,
                                feat: fname,
                                block: bname,
                            };
                            continue;
                        }
                    }
                    e.push(Claim {
                        olx: dxl,
                        olz: dzl,
                        fidx,
                        feat: fname,
                        block: bname,
                    });
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Order models (constructions mirror ore_swap_dump::order_ranks; larger =
// later decoration pass)
// ---------------------------------------------------------------------------

fn pregen_phase(cxw: i32, czw: i32) -> u8 {
    if (-8..=7).contains(&cxw) && (-8..=7).contains(&czw) {
        0
    } else if (-12..=-11).contains(&cxw) {
        1
    } else if (10..=11).contains(&cxw) {
        2
    } else if (-12..=-11).contains(&czw) {
        3
    } else {
        4
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum OrderMode {
    CanonPregen,
    WorldOrigin,
    Spiral,
    Row,
}
const ORDERS: [(OrderMode, &str); 4] = [
    (OrderMode::CanonPregen, "canonical_pregen"),
    (OrderMode::WorldOrigin, "world_origin"),
    (OrderMode::Spiral, "spiral"),
    (OrderMode::Row, "row"),
];

fn model_rank(mode: OrderMode, cx: i32, cz: i32) -> (i64, i64, i64) {
    match mode {
        OrderMode::WorldOrigin => {
            let wx = (cx * 16 + 8) as i64;
            let wz = (cz * 16 + 8) as i64;
            (wx * wx + wz * wz, cz as i64, cx as i64)
        }
        OrderMode::Row => (cz as i64, cx as i64, 0),
        OrderMode::CanonPregen => (
            pregen_phase(cx, cz) as i64,
            (cz as i64) * 64 + cx as i64,
            cx as i64,
        ),
        OrderMode::Spiral => unreachable!("spiral compares via window offsets"),
    }
}

const SPIRAL: [(i32, i32); 9] = [
    (0, 0),
    (1, 0),
    (1, 1),
    (0, 1),
    (-1, 1),
    (-1, 0),
    (-1, -1),
    (0, -1),
    (1, -1),
];

fn consistent(mode: OrderMode, win: OriginKey, lose: OriginKey, ccx: i32, ccz: i32) -> bool {
    match mode {
        OrderMode::Spiral => {
            let w = SPIRAL
                .iter()
                .position(|&(dx, dz)| win.0 - ccx == dx && win.1 - ccz == dz);
            let l = SPIRAL
                .iter()
                .position(|&(dx, dz)| lose.0 - ccx == dx && lose.1 - ccz == dz);
            match (w, l) {
                (Some(w), Some(l)) => w > l,
                _ => false,
            }
        }
        _ => model_rank(mode, win.0, win.1) > model_rank(mode, lose.0, lose.1),
    }
}

// ---------------------------------------------------------------------------
// Main scan
// ---------------------------------------------------------------------------

fn main() {
    let t_start = std::time::Instant::now();
    let seed: i64 = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(424242);
    let dir = std::env::args().nth(2).unwrap_or_else(|| {
        format!("tools/nbt-ref/vanilla-fresh-{seed}/world/dimensions/minecraft/overworld/region")
    });
    let out_csv =
        std::env::args().nth(3).unwrap_or_else(|| format!("/tmp/opencode/deco_pairs_{seed}.csv"));

    let envn = |k: &str| std::env::var(k).ok();
    let min_votes: usize =
        envn("DECO_MIN_VOTES").and_then(|v| v.parse().ok()).unwrap_or(4);
    let min_hits_abs: u64 =
        envn("DECO_MIN_HITS").and_then(|v| v.parse().ok()).unwrap_or(3);
    let xmin = envn("DECO_XMIN").and_then(|v| v.parse().ok());
    let xmax = envn("DECO_XMAX").and_then(|v| v.parse().ok());
    let zmin = envn("DECO_ZMIN").and_then(|v| v.parse().ok());
    let zmax = envn("DECO_ZMAX").and_then(|v| v.parse().ok());
    let stride: usize = envn("DECO_STRIDE").and_then(|v| v.parse().ok()).unwrap_or(1);
    let limit: usize = envn("DECO_LIMIT").and_then(|v| v.parse().ok()).unwrap_or(usize::MAX);

    println!("=== deco_precedence_dump seed={seed} dir={dir}");
    println!(
        "knobs min_votes={min_votes} min_hits_abs={min_hits_abs} stride={stride} limit={limit} clamp x{xmin:?}..{xmax:?} z{zmin:?}..{zmax:?}"
    );

    // ---- Phase A: full-status consumer set ---------------------------------
    let gen = ChunkGenerator::new(seed);
    let mut full: Vec<(i32, i32)> = Vec::new();
    for rx in -1..=0i32 {
        for rz in -1..=0i32 {
            let path = std::path::PathBuf::from(format!("{dir}/r.{rx}.{rz}.mca"));
            let Ok(region) = Region::open(&path) else { continue };
            let region = region.with_coords(rx, rz);
            for lx in 0..32i32 {
                for lz in 0..32i32 {
                    let Ok(Some(data)) = region.get_chunk(lx, lz) else { continue };
                    let Ok(nbt) = read_nbt(&data) else { continue };
                    if let Some(Tag::String(s)) = compound_get(&nbt.compound, "Status") {
                        if s.to_string().ends_with("full") {
                            let (cx, cz) = (rx * 32 + lx, rz * 32 + lz);
                            let inwin = xmin.map_or(true, |a| cx >= a)
                                && xmax.map_or(true, |a| cx <= a)
                                && zmin.map_or(true, |a| cz >= a)
                                && zmax.map_or(true, |a| cz <= a);
                            if inwin {
                                full.push((cx, cz));
                            }
                        }
                    }
                }
            }
        }
    }
    full.sort();
    let full_sel: Vec<(i32, i32)> = full.into_iter().step_by(stride).take(limit).collect();
    let n_chunks = full_sel.len();
    assert!(n_chunks > 0, "no full-status chunks selected");
    let hcx0 = full_sel.iter().map(|c| c.0).min().unwrap() - 2;
    let hcx1 = full_sel.iter().map(|c| c.0).max().unwrap() + 2;
    let hcz0 = full_sel.iter().map(|c| c.1).min().unwrap() - 2;
    let hcz1 = full_sel.iter().map(|c| c.1).max().unwrap() + 2;
    println!("[chunks] {n_chunks} consumers; heights [{hcx0}..{hcx1}]x[{hcz0}..{hcz1}]");

    // ---- Phase B: heightfield ----------------------------------------------
    let hf = build_hfield(&gen, hcx0, hcz0, (hcx1 - hcx0 + 1) as usize, (hcz1 - hcz0 + 1) as usize);

    // ---- Phase C: mine --------------------------------------------------------
    let mut w = std::io::BufWriter::new(std::fs::File::create(&out_csv).expect("csv create"));
    writeln!(w, "# deco precedence pairs seed={seed} dir={dir} knobs votes={min_votes} min_hits_abs={min_hits_abs}").unwrap();
    writeln!(w, "ccx,ccz,win_ox,win_oz,win_feat,lose_ox,lose_oz,lose_feat,delta_dx,delta_dz").unwrap();

    let mut recs: Vec<Rec> = Vec::new();
    let mut n_contested = 0u64; // >=2 distinct-origin claimants (post-filter)
    let mut n_single_writer = 0u64; // exactly 1 distinct origin claimant (post-filter)
    let mut n_masked = 0u64; // contested but final block is not a family mineral
    let mut n_ambig = 0u64; // contested, winner mineral matches >1 origin
    let mut n_orphan_hard = 0u64; // final==family, no post-filter claimant matched
    let mut n_orphan_soft = 0u64; // match existed pre-filter but died in phantom filter
    let mut n_used_cells = 0u64;
    let mut blobs_real = 0u64;
    let mut blobs_phantom = 0u64;
    let mut blobs_unknown = 0u64;
    let mut ratio_hist_decile: BTreeMap<u64, u64> = BTreeMap::new();

    let dbg_printed = std::sync::atomic::AtomicU64::new(0);
    for (ci, &(ccx, ccz)) in full_sel.iter().enumerate() {
        let Some(van) = load_vanilla(&dir, ccx, ccz) else {
            eprintln!("[skip] ({ccx},{ccz}) vanilla missing/not-full");
            continue;
        };
        let (claims, traces) = simulate_consumer(&gen.state, &hf, seed, ccx, ccz);

        // ---- classify every traced blob against its own predicted cells ----
        // VALIDITY = ABSOLUTE hit count (hits >= min_hits): final-state
        // visibility is depressed precisely on contested cells (that is the
        // signal we mine!), so a RATIO test would flag the true winner of an
        // overlap as phantom. Microdiff-fabricated placements cannot score:
        // their spheres land elsewhere => near-zero exact-mineral stamps.
        let mut verdict: HashMap<(OriginKey, i32), bool> = HashMap::new();
        for t in &traces {
            if t.cells.is_empty() {
                continue;
            }
            let hits = t
                .cells
                .iter()
                .filter(|&&c| {
                    van.get(&(((c.0 & 15) as u8), c.1, (c.2 & 15) as u8))
                        .map(|s| s.as_str())
                        == Some(t.block)
                })
                .count();
            let n = t.cells.len();
            if n < min_votes {
                blobs_unknown += 1;
                verdict.insert((t.org, t.fidx), false);
                continue;
            }
            let ok = hits as u64 >= min_hits_abs;
            *ratio_hist_decile.entry(hits.min(10) as u64).or_insert(0) += 1;
            if std::env::var_os("DECO_DEBUG").is_some()
                && !ok
                && dbg_printed.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
                    < envn("DECO_DEBUG_TRACES").and_then(|v| v.parse::<u64>().ok()).unwrap_or(3)
            {
                println!(
                    "[trace-dead] org=({},{}) feat={} cells={} hits={}",
                    t.org.0, t.org.1, t.feat, t.cells.len(), hits
                );
                for c in t.cells.iter().take(12) {
                    println!(
                        "    ({},{},{}) van={}",
                        c.0,
                        c.1,
                        c.2,
                        van.get(&(((c.0 & 15) as u8), c.1, (c.2 & 15) as u8))
                            .unwrap_or(&"<none?>".to_string())
                    );
                }
            }
            if ok {
                blobs_real += 1;
            } else {
                blobs_phantom += 1;
            }
            verdict.insert((t.org, t.fidx), ok);
        }
        let live = |c: &Claim| {
            verdict.get(&((ccx + c.olx as i32, ccz + c.olz as i32), c.fidx)).copied() == Some(true)
        };

        if std::env::var_os("DECO_DEBUG").is_some() {
            let mut multi_raw = 0u64;
            let mut multi_live = 0u64;
            let mut samples: Vec<String> = Vec::new();
            for (&(x, y, z), cs) in claims.iter() {
                let o: HashSet<(i8, i8)> = cs.iter().map(|c| (c.olx, c.olz)).collect();
                if o.len() > 1 {
                    multi_raw += 1;
                    let cl: Vec<bool> = cs.iter().map(&live).collect();
                    if cl.iter().filter(|b| **b).count() >= 2 {
                        multi_live += 1;
                    } else if samples.len() < 4 {
                        samples.push(format!(
                            "({},{},{}) claims={} live={:?} feats={}",
                            x,
                            y,
                            z,
                            cs.len(),
                            cl,
                            cs.iter().map(|c| format!("{}@{},{}", c.feat, ccx + c.olx as i32, ccz + c.olz as i32)).collect::<Vec<_>>().join("|")
                        ));
                    }
                }
            }
            println!(
                "[dbg] ({},{}) raw_multi={} live_multi={} dead_examples={}",
                ccx,
                ccz,
                multi_raw,
                multi_live,
                samples.join(" ;; ")
            );
        }

        for (&(x, y, z), cs_all) in claims.iter() {
            let cs: Vec<&Claim> = cs_all.iter().filter(|c| live(c)).collect();
            let origins: HashSet<OriginKey> =
                cs.iter().map(|c| (ccx + c.olx as i32, ccz + c.olz as i32)).collect();
            if origins.len() <= 1 {
                if origins.len() == 1 {
                    n_single_writer += 1;
                }
                continue;
            }
            n_contested += 1;
            let vn = van
                .get(&(((x & 15) as u8), y, (z & 15) as u8))
                .map(|s| s.as_str())
                .unwrap_or("air");
            if !FAMILIES.contains(&vn) {
                n_masked += 1; // carver/later family decided this cell instead
                continue;
            }
            let winners: Vec<&Claim> = cs.iter().copied().filter(|c| c.block == vn).collect();
            let raw_winner_existed = cs_all.iter().any(|c| c.block == vn);
            if winners.is_empty() {
                if raw_winner_existed {
                    n_orphan_soft += 1;
                } else {
                    n_orphan_hard += 1;
                }
                continue;
            }
            let worigins: HashSet<OriginKey> = winners
                .iter()
                .map(|c| (ccx + c.olx as i32, ccz + c.olz as i32))
                .collect();
            if worigins.len() > 1 {
                n_ambig += 1;
                continue;
            }
            let wc = winners[0];
            let worg = (ccx + wc.olx as i32, ccz + wc.olz as i32);
            let phase = pregen_phase(ccx, ccz);
            n_used_cells += 1;
            let mut emitted: HashSet<OriginKey> = HashSet::new();
            for lc in &cs {
                let lord = (ccx + lc.olx as i32, ccz + lc.olz as i32);
                if lord == worg || !emitted.insert(lord) {
                    continue;
                }
                writeln!(
                    w,
                    "{},{},{},{},{},{},{},{},{},{}",
                    ccx,
                    ccz,
                    worg.0,
                    worg.1,
                    wc.feat,
                    lord.0,
                    lord.1,
                    lc.feat,
                    worg.0 - lord.0,
                    worg.1 - lord.1
                )
                .unwrap();
                recs.push(Rec {
                    win: worg,
                    lose: lord,
                    wfam: wc.feat,
                    lfam: lc.feat,
                    phase,
                    delta: (worg.0 - lord.0, worg.1 - lord.1),
                    ccx,
                    ccz,
                });
            }
        }

        if (ci + 1) % 50 == 0 || ci + 1 == n_chunks {
            println!(
                "[scan] {}/{} chunks recs={} contested={n_contested} masked={n_masked} elapsed {:.0}s",
                ci + 1,
                n_chunks,
                recs.len(),
                t_start.elapsed().as_secs_f64()
            );
            let _ = std::io::stdout().flush();
        }
    }
    w.flush().unwrap();

    report(
        &recs,
        &out_csv,
        &ratio_hist_decile,
        blobs_real,
        blobs_phantom,
        blobs_unknown,
        Stats {
            contested: n_contested,
            masked: n_masked,
            ambig: n_ambig,
            orphan_hard: n_orphan_hard,
            orphan_soft: n_orphan_soft,
            single: n_single_writer,
            used: n_used_cells,
        },
    );

    GATE_UNRESOLVED.with(|g| println!("note: gate-unresolved boxes = {} (expect 0)", g.borrow()));
    PROBE_MISSING_COLS.with(|g| println!("note: probe pixels off-heightfield = {} (expect 0)", g.borrow()));
    println!("total elapsed {:.0}s", t_start.elapsed().as_secs_f64());
}

struct Stats {
    contested: u64,
    masked: u64,
    ambig: u64,
    orphan_hard: u64,
    orphan_soft: u64,
    single: u64,
    used: u64,
}

// math helpers duplicating internal crate fns (carvers.rs:55 table trick)
const PI: f32 = 3.1415927;
const SIN_SCALE: f64 = 10430.378350470453;

fn sin_tbl(v: f64) -> f32 {
    let idx = ((v * SIN_SCALE) as i64 as u64 & 0xFFFF) as usize;
    static TABLE: std::sync::OnceLock<Vec<f32>> = std::sync::OnceLock::new();
    TABLE
        .get_or_init(|| (0..65536usize).map(|i| (i as f64 / SIN_SCALE).sin() as f32).collect())[idx]
}

#[inline]
fn lerp(t: f64, a: f64, b: f64) -> f64 {
    a + t * (b - a)
}

#[inline]
fn floor(v: f64) -> i32 {
    v.floor() as i32
}

// ---------------------------------------------------------------------------
// Reports (the four deliverables)
// ---------------------------------------------------------------------------

struct Rec {
    win: OriginKey,
    lose: OriginKey,
    wfam: &'static str,
    lfam: &'static str,
    phase: u8,
    delta: (i32, i32),
    ccx: i32,
    ccz: i32,
}

fn report(
    recs: &[Rec],
    csv_path: &str,
    ratio_hist: &BTreeMap<u64, u64>,
    br: u64,
    bp: u64,
    bu: u64,
    st: Stats,
) {
    let n = recs.len() as f64;
    println!("\n==================== REPORT ====================");
    println!("CSV artifact: {csv_path}");
    println!(
        "cells: contested={contested} usable_cells={used} single-writer={single} masked={masked} ambiguous={ambig} orphan_hard={oh} orphan_soft(phantom-culled)={os}",
        contested = st.contested,
        used = st.used,
        single = st.single,
        masked = st.masked,
        ambig = st.ambig,
        oh = st.orphan_hard,
        os = st.orphan_soft,
    );
    println!("blobs: real={br} phantom(dropped)={bp} unknown-lowvote(excluded)={bu}");
    println!("      blob self-hit-ratio deciles (0.0..1.0 -> count): {ratio_hist:?}");

    // ---------------- 1. pair totals + family contributions -----------------
    println!("\n[1] PRECEDENCE PAIRS");
    let uniq_edges: HashSet<(OriginKey, OriginKey)> =
        recs.iter().map(|r| (r.win, r.lose)).collect();
    println!(
        "      total pair rows {} over {} distinct cells; distinct directed origin-pairs: {}",
        recs.len(),
        st.used,
        uniq_edges.len()
    );
    let mut wins_by_feat: BTreeMap<&str, u64> = BTreeMap::new();
    for r in recs {
        *wins_by_feat.entry(r.wfam).or_insert(0) += 1;
    }
    println!("      winner-feature contribution:");
    for (f, c) in &wins_by_feat {
        println!("        {:<20} {:>8} ({:>5.1}%)", f, c, *c as f64 / n * 100.0);
    }
    let mut fam_mat: BTreeMap<(&'static str, &'static str), u64> = BTreeMap::new();
    for r in recs {
        *fam_mat.entry((fam_short(r.wfam), fam_short(r.lfam))).or_insert(0) += 1;
    }
    let mut fm: Vec<_> = fam_mat.into_iter().collect();
    fm.sort_by_key(|(_, c)| std::cmp::Reverse(*c));
    println!("      top family-vs-family constraints:");
    for ((wf, lf), c) in fm.iter().take(12) {
        println!("        {wf:<8} beats {lf:<8} {c:>8}");
    }

    // ---------------- 2. model fits ------------------------------------------
    println!("\n[2] FIT vs candidate decoration-order models");
    for band_name in ["all", "interior[-8..7]^2 only", "strips/rings only"] {
        let sub: Vec<&Rec> = recs
            .iter()
            .filter(|r| match band_name {
                "all" => true,
                b if b.starts_with("interior") => r.phase == 0,
                _ => r.phase != 0,
            })
            .collect();
        if sub.is_empty() || band_name == "all" && false {
            continue;
        }
        let ns = sub.len() as f64;
        print!("      {band_name:<26} n={ns:>7.0}: ");
        for (mode, nm) in ORDERS.iter() {
            let k = sub.iter().filter(|r| consistent(*mode, r.win, r.lose, r.ccx, r.ccz)).count();
            print!("{nm}={:.2}% ", k as f64 / ns * 100.0);
        }
        let any = sub
            .iter()
            .filter(|r| ORDERS.iter().any(|(m, _)| consistent(*m, r.win, r.lose, r.ccx, r.ccz)))
            .count();
        println!("any_of_four={:.2}%", any as f64 / ns * 100.0);
    }

    // strip-phase matrix of the two endpoints' canonical phases
    let mut pm: BTreeMap<(u8, u8), u64> = BTreeMap::new();
    for r in recs {
        *pm.entry((pregen_phase(r.lose.0, r.lose.1), pregen_phase(r.win.0, r.win.1)))
            .or_insert(0) += 1;
    }
    println!("      canonical-pregen phase(loser)->phase(winner) support:");
    for ((pl, pw), c) in pm {
        if c >= 50 {
            println!("        phase{pl}->phase{pw}  {c}");
        }
    }

    // ---------------- 3. anomalies -------------------------------------------
    println!("\n[3] ANOMALIES vs every simple spatial model");
    struct EdgeAgg {
        n: u64,
        fams: BTreeMap<String, u64>,
        centers: Vec<(i32, i32)>,
    }
    let mut anom: HashMap<(OriginKey, OriginKey), EdgeAgg> = HashMap::new();
    let mut anomaly_rows = 0u64;
    for r in recs {
        let any_ok =
            ORDERS.iter().any(|(m, _)| consistent(*m, r.win, r.lose, r.ccx, r.ccz));
        if any_ok {
            continue;
        }
        anomaly_rows += 1;
        let e = anom
            .entry((r.lose, r.win))
            .or_insert_with(|| EdgeAgg { n: 0, fams: BTreeMap::new(), centers: Vec::new() });
        e.n += 1;
        *e.fams.entry(format!("{}>{}", fam_short(r.lfam), fam_short(r.wfam))).or_insert(0) += 1;
        if !e.centers.contains(&(r.ccx, r.ccz)) && e.centers.len() < 4 {
            e.centers.push((r.ccx, r.ccz));
        }
    }
    println!(
        "      anomaly rows {anomaly_rows}/{} pairs; {} distinct ordered origin-pairs never predicted",
        recs.len(),
        anom.len()
    );
    let mut av: Vec<_> = anom.into_iter().collect();
    av.sort_by_key(|(_, e)| std::cmp::Reverse(e.n));
    for ((lose, win), e) in av.iter().take(20) {
        let top = e.fams.iter().rev().take(3).map(|(k, v)| format!("{k}:{v}")).collect::<Vec<_>>().join(" ");
        println!(
            "        L({:>4},{:>4}) -> W({:>4},{:>4}) d=({:+},{:+}) sup={:<6} [{top}] windows={:?}",
            lose.0,
            lose.1,
            win.0,
            win.1,
            win.0 - lose.0,
            win.1 - lose.1,
            e.n,
            e.centers
        );
    }

    // corner-vs-center replication (per window: origin at center vs diagonal)
    println!("      window center-vs-diagonal replication:");
    let mut diag: BTreeMap<(i32, i32), (u64, u64)> = BTreeMap::new();
    let mut card: BTreeMap<(i32, i32), (u64, u64)> = BTreeMap::new();
    for r in recs {
        let w_off = (r.win.0 - r.ccx, r.win.1 - r.ccz);
        let l_off = (r.lose.0 - r.ccx, r.lose.1 - r.ccz);
        let diag_rel = |o: (i32, i32)| o.0.abs() == 1 && o.1.abs() == 1;
        let card_rel = |o: (i32, i32)| o.0.abs() + o.1.abs() == 1;
        if w_off == (0, 0) && diag_rel(l_off) {
            diag.entry(l_off).or_insert((0, 0)).1 += 1; // center beat corner
        } else if l_off == (0, 0) && diag_rel(w_off) {
            diag.entry(w_off).or_insert((0, 0)).0 += 1; // corner ran before its center
        } else if w_off == (0, 0) && card_rel(l_off) {
            card.entry(l_off).or_insert((0, 0)).1 += 1;
        } else if l_off == (0, 0) && card_rel(w_off) {
            card.entry(w_off).or_insert((0, 0)).0 += 1;
        }
    }
    for ((dx, dz), (corner_first, center_first)) in &diag {
        let tot = corner_first + center_first;
        if tot > 0 {
            println!(
                "        diag({:+},{:+}): corner-beats-center {}/{} ({:.1}%)",
                dx,
                dz,
                corner_first,
                tot,
                (*corner_first) as f64 / tot as f64 * 100.0
            );
        }
    }
    for ((dx, dz), (nb_first, center_first)) in &card {
        let tot = nb_first + center_first;
        if tot > 0 {
            println!(
                "        card({:+},{:+}): neighbour-beats-center {}/{} ({:.1}%)",
                dx,
                dz,
                nb_first,
                tot,
                (*nb_first) as f64 / tot as f64 * 100.0
            );
        }
    }

    // offset-delta table
    println!("      ordered deltas winner-loser (top, with reverse):");
    let mut od: BTreeMap<(i32, i32), u64> = BTreeMap::new();
    for r in recs {
        *od.entry(r.delta).or_insert(0) += 1;
    }
    let mut odv: Vec<_> = od.iter().map(|(k, v)| (*k, *v)).collect();
    odv.sort_by_key(|(_, c)| std::cmp::Reverse(*c));
    for ((dx, dz), c) in odv.iter().take(16) {
        let rev = od.get(&(-dx, -dz)).copied().unwrap_or(0);
        println!("        delta=({dx:+},{dz:+}) {c:>8}   reverse({:-},{:+}) {rev:>8}", -dx, -dz);
    }

    // ---------------- 4. transitivity ----------------------------------------
    println!("\n[4] TRANSITIVITY / CONFLICTS");
    let mut dirs: HashMap<(OriginKey, OriginKey), u64> = HashMap::new();
    for r in recs {
        *dirs.entry((r.win, r.lose)).or_insert(0) += 1;
    }
    let undirected: HashSet<(OriginKey, OriginKey)> =
        dirs.keys().map(|&(a, b)| if a < b { (a, b) } else { (b, a) }).collect();
    let mut conflicted = 0u64;
    let mut conflict_cells = 0u64;
    let mut worst: Option<(OriginKey, OriginKey, u64, u64)> = None;
    for &(a, b) in &undirected {
        let ab = dirs.get(&(a, b)).copied().unwrap_or(0);
        let ba = dirs.get(&(b, a)).copied().unwrap_or(0);
        if ab > 0 && ba > 0 {
            conflicted += 1;
            conflict_cells += ab.min(ba);
            if worst.map_or(true, |(_, _, x, y)| ab.min(ba) > x.min(y)) {
                worst = Some((a, b, ab, ba));
            }
        }
    }
    println!(
        "      directed edges {} across {} unordered origin-pairs; both-direction conflicts {} pairs ({:.2}% of pairs)",
        dirs.len(),
        undirected.len(),
        conflicted,
        conflicted as f64 / undirected.len().max(1) as f64 * 100.0
    );
    println!(
        "      cell rows sitting on conflicting sides: {conflict_cells} ({:.2}% of all rows)",
        conflict_cells as f64 / n.max(1.0) * 100.0
    );
    if let Some((a, b, ab, ba)) = worst {
        println!(
            "      strongest mutual pair ({a:?})-({b:?}): {ab} vs {ba} weaker side {:.1}%",
            ba.min(ab) as f64 / ab.max(ba).max(1) as f64 * 100.0
        );
    }

    const TRI_MIN: u64 = 5;
    let nodes: Vec<OriginKey> = dirs.keys().flat_map(|&(a, _)| [a]).collect();
    let nodes: HashSet<OriginKey> = nodes.into_iter().chain(recs.iter().map(|r| r.win)).collect();
    let adj: HashMap<OriginKey, Vec<OriginKey>> = {
        let mut m: HashMap<OriginKey, Vec<OriginKey>> = HashMap::new();
        for (&(a, b), &cnt) in &dirs {
            if cnt >= TRI_MIN {
                m.entry(a).or_default().push(b);
            }
        }
        m
    };
    let mut cycles = 0u64;
    let mut examples: Vec<String> = Vec::new();
    let sorted_nodes: Vec<OriginKey> = nodes.into_iter().collect();
    for &a in &sorted_nodes {
        for &b in adj.get(&a).into_iter().flatten() {
            if !(a < b) {
                continue;
            }
            for &c in adj.get(&b).into_iter().flatten() {
                if !(b < c) {
                    continue;
                }
                if adj.get(&c).map_or(false, |l| l.binary_search(&a).is_ok()) {
                    cycles += 1;
                    if examples.len() < 6 {
                        examples.push(format!(
                            "({},{})->({},{}):{}, ({},{})->({},{}):{}, ({},{})->({},{}):{}",
                            a.0, a.1, b.0, b.1, dirs.get(&(a, b)).copied().unwrap_or(0),
                            b.0, b.1, c.0, c.1, dirs.get(&(b, c)).copied().unwrap_or(0),
                            c.0, c.1, a.0, a.1, dirs.get(&(c, a)).copied().unwrap_or(0),
                        ));
                    }
                }
            }
        }
    }
    println!("      oriented triangle cycles (edges >= {TRI_MIN} cells each): {cycles}");
    for ex in &examples {
        println!("        {ex}");
    }
    println!("\nnote: models are probe hypotheses only; scheduler-wavefront remains the live candidate (STATE.md).");
}

