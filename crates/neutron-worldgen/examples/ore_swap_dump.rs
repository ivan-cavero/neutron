//! ore_swap_dump — CLOSED DUMP for base-stone blob swaps (granite/diorite/
//! andesite/tuff), e.g. seed 424242 (~20k mutual-swap cells in prior ledger).
//!
//! Question: for cells where vanilla has granite/diorite/andesite/tuff and
//! Neutron outputs one of the others (or stone), which OreFeature placement
//! produced it on each side, do blob origins/shapes match draw-for-draw, and
//! what is the first divergent input?
//!
//! Method. All eight placements (`ore_{granite,diorite,andesite}_{upper,
//! lower}`, `ore_tuff`) are `minecraft:ore` features of size 64, discard 0.0,
//! target tag `base_stone_overworld` (jar datapack, byte-identical to
//! src/data/worldgen/*). So each blob unconditionally overwrites any
//! {stone,g,d,a,tuff,deepslate} cell it touches, consumes no air dice and no
//! adjacency reads; the last writer wins ordered by (decoration-origin order,
//! FeatureSorter feature index). This dump:
//!   1. loads vanilla .mca chunks and runs Neutron `generate_chunk`;
//!   2. replays the shared per-(origin,feature) RNG stream to enumerate every
//!      blob anchor/sphere (heightmap + biome gates replicated);
//!   3. predicts winners under candidate origin-order models
//!      (canonical_pregen = Neutron default, world_origin, spiral, row);
//!   4. classifies each swapped cell and clusters it (26-connectivity).
//!
//! Citations — Java: OreFeature.place/doPlace/canPlaceOre
//! tools/mc-decompiler/output/26.2/src/net/minecraft/world/level/levelgen/
//! feature/OreFeature.java:23-53/55-166/168-181; placement chain
//! net/minecraft/world/level/chunk/ChunkGenerator.java:318-401.
//! Neutron: src/features.rs:49 apply_underground_ores_origin (origin-major),
//! :1060-1082 angle/y-offset draws + ocean-floor gate, :1086-1127 spheres +
//! overlap cull, :1147-1202 write loop, :1205 target_match.
//!
//! Usage: cargo run --release -p neutron-worldgen --example ore_swap_dump

use neutron_world::nbt::ussr_nbt::owned::{List, Tag};
use neutron_world::nbt::{compound_get, read_nbt};
use neutron_world::Region;
use neutron_worldgen::biome_manager::biome_id_at_block;
use neutron_worldgen::feature_catalog;
use neutron_worldgen::feature_dispatch::biome_id_to_name;
use neutron_worldgen::feature_rng::FeatureRandom;
use neutron_worldgen::generator::{WORLD_BOTTOM, WORLD_TOP};
use neutron_worldgen::region_buf::RegionBuf;
use neutron_worldgen::ChunkGenerator;
use std::collections::{BTreeMap, HashMap};

const STEP: i32 = 6;
const SAMPLES: [(i32, i32); 5] = [(-10, 6), (7, 2), (8, 0), (0, 0), (3, -4)];
const FAMILY: [&str; 4] = ["granite", "diorite", "andesite", "tuff"];

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    Upper,
    Lower,
    Tuff,
}

/// (FeatureSorter idx, placed id, output short-name, attempt mode)
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

#[derive(Clone)]
struct Claim {
    olx: i8,
    olz: i8,
    fidx: i32,
    block: &'static str,
    ax: i32,
    ay: i32,
    az: i32,
    attempt: u16,
}

type CellKey = (i32, i32, i32);

thread_local! {
    /// Blob gates whose whole probe box fell outside the pre-carver-heightmap
    /// buffer: those "no blob" verdicts are approximate (mark in output).
    static GATE_UNRESOLVED: std::cell::RefCell<u64> = std::cell::RefCell::new(0);
}

fn main() {
    let seed: i64 = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(424242);
    let dir = std::env::args().nth(2).unwrap_or_else(|| {
        format!("tools/nbt-ref/vanilla-fresh-{seed}/world/dimensions/minecraft/overworld/region")
    });
    let alt_order = std::env::args().nth(3); // "world_origin"|"spiral"|... A/B
    if let Some(o) = &alt_order {
        std::env::set_var("NEUTRON_SCULK_ORIGIN_ORDER", o);
        println!("[A/B] neutron generate_chunk runs under NEUTRON_SCULK_ORIGIN_ORDER={o}");
    }
    std::env::set_var("NEUTRON_WRITERS", "1");

    let gen = ChunkGenerator::new(seed);
    for &(idx, name, _, _) in BASE_ORES.iter() {
        assert_eq!(
            feature_catalog::global_feature_index(STEP, name),
            Some(idx),
            "{name} FeatureSorter drift"
        );
    }
    println!("=== ore_swap_dump seed={seed}");
    println!("samples: {:?}", SAMPLES);

    let mut class_counts: BTreeMap<String, u64> = BTreeMap::new();
    let mut expl_den = 0u64;
    let mut expl_hit = [0u64; ORDERS.len()];
    let mut expl_hit_neu = [0u64; ORDERS.len()];
    let mut writer_mix: BTreeMap<(String, u16), u64> = BTreeMap::new();
    let mut table_rows: Vec<String> = Vec::new();

    for (cx, cz) in SAMPLES {
        println!("\n=== chunk ({cx},{cz})");
        let Some(van) = load_vanilla(&dir, cx, cz) else {
            println!("  SKIP: vanilla chunk missing/not full status");
            continue;
        };
        let neu_full = gen.generate_chunk(cx, cz);
        let region = gen.generate_ores_region(cx, cz); // pre-carver hm for gates
        let state = &gen.state;
        let claims = simulate_claims(seed, state, &region, cx, cz);

        // Sanity coverage of vanilla stone-family cells against each order.
        for lz in 0..16i32 {
            for lx in 0..16i32 {
                for y in WORLD_BOTTOM..WORLD_TOP {
                    let vn = van.get(&(lx as u8, y, lz as u8)).map(|s| s.as_str());
                    let Some(vv) = vn else { continue };
                    if !FAMILY.contains(&vv) {
                        continue;
                    }
                    expl_den += 1;
                    let wx = cx * 16 + lx;
                    let wz = cz * 16 + lz;
                    let nb = neu_full.block_at(lx as u32, y, lz as u32).block_name();
                    let nn = nb.trim_start_matches("minecraft:");
                    for (mi, (omode, _)) in ORDERS.iter().enumerate() {
                        let ranks = order_ranks(cx, cz, *omode);
                        let pred = winner_block(claims.get(&(wx, y, wz)), &ranks);
                        if pred == Some(vv) {
                            expl_hit[mi] += 1;
                        }
                        if pred == Some(nn) {
                            expl_hit_neu[mi] += 1;
                        }
                    }
                    let widx = neu_full
                        .writers
                        .as_ref()
                        .map(|w| w[(((y - WORLD_BOTTOM) * 256 + lz * 16 + lx)) as usize])
                        .unwrap_or(u16::MAX);
                    if nn != vv {
                        *writer_mix.entry((format!("{vv}->{nn}"), widx)).or_insert(0) += 1;
                    }
                }
            }
        }

        // ---- swap rows + matrix ----
        let mut rows: Vec<CellKey> = Vec::new();
        let mut matrix: BTreeMap<(String, String), u64> = BTreeMap::new();
        for lz in 0..16i32 {
            for lx in 0..16i32 {
                for y in WORLD_BOTTOM..WORLD_TOP {
                    let vn = van.get(&(lx as u8, y, lz as u8)).map(|s| s.as_str()).unwrap_or("air");
                    let nn = neu_full
                        .block_at(lx as u32, y, lz as u32)
                        .block_name()
                        .trim_start_matches("minecraft:")
                        .to_string();
                    if !FAMILY.contains(&vn) {
                        continue;
                    }
                    if !(FAMILY.contains(&nn.as_str()) || nn == "stone") || vn == nn {
                        continue;
                    }
                    *matrix.entry((vn.into(), nn.clone())).or_insert(0) += 1;
                    let wx = cx * 16 + lx;
                    let wz = cz * 16 + lz;
                    if std::env::var_os("ORE_SWAP_DEBUG").is_some() {
                        let cs = claims.get(&(wx, y, wz));
                        let mut lines: Vec<String> =
                            cs.map(|v| v.iter().map(|c| format!("{}ol({},{})/f{}/a#{}={}", c.block, c.olx, c.olz,
                                c.fidx, c.attempt, c.block)).collect())
                              .unwrap_or_default();
                        lines.sort();
                        println!(
                            "[dbg] ({wx},{y},{wz}) van={vn} neu={nn} claims={}",
                            lines.join(", ")
                        );
                    }
                    rows.push((wx, y, wz));
                }
            }
        }
        println!("-- swap matrix (vanilla x neutron direction counts)");
        for ((v, n), c) in &matrix {
            println!("   {v:<9} -> {n:<9} {c}");
        }

        // ---- cluster + classify ----
        let cells: std::collections::HashSet<CellKey> = rows.iter().copied().collect();
        let clusters = group_clusters(&rows, &cells);
        println!("-- {} swap cells in {} clusters", rows.len(), clusters.len());
        for (ci, cl) in clusters.iter().enumerate() {
            let mut pairs: BTreeMap<(String, String), u64> = BTreeMap::new();
            for k in cl {
                let (x, y, z) = *k;
                let v = van.get(&(((x & 15) as u8), y, (z & 15) as u8)).unwrap().clone();
                let n = neu_full.block_at((x & 15) as u32, y, (z & 15) as u32).block_name();
                let n = n.trim_start_matches("minecraft:").to_string();
                *pairs.entry((v, n)).or_insert(0) += 1;
            }
            let (dompair, domcnt) =
                pairs.iter().max_by_key(|(_, c)| **c).map(|(p, c)| (p.clone(), *c)).unwrap();
            let (cls, detail) =
                classify_cluster(cl, &van, &neu_full, &claims, cx, cz);
            *class_counts.entry(cls.clone()).or_insert(0) += cl.len() as u64;
            let ranks = order_ranks(cx, cz, OrderMode::CanonPregen);
            let rep = cl[cl.len() / 2];
            let win = claims.get(&rep).and_then(|cs| best_claim(cs, &ranks));
            if ci < 40 {
                println!(
                    "  cl{ci:>2} n={:>4} dom={dompair:?}({domcnt}) [{cls}] {}",
                    cl.len(),
                    detail.map(|d| format!(" {d}")).unwrap_or_default(),
                );
                if let Some(c) = win {
                    println!(
                        "        canon-winner@rep({},{},{}): {} anchor=({},{},{}) attempt#{} origin-chunk({},{})",
                        rep.0, rep.1, rep.2, c.block, c.ax, c.ay, c.az, c.attempt,
                        cx + c.olx as i32, cz + c.olz as i32
                    );
                }
            }
            if table_rows.len() < 12 && ci < 6 {
                table_rows.push(rep_rank(&rep, &van, &neu_full, &claims, cx, cz));
            }
        }
    }

    println!("\n=== concrete rows (seed {}, producing-feature claims both sides)", seed);
    for r in &table_rows {
        println!("{r}");
    }
    println!("\n=== aggregate first-divergent-input classes (cell counts)");
    for (k, v) in &class_counts {
        println!("  {k:<58} {v}");
    }
    println!("\n=== blob-existence/winner explanation rate over ALL vanilla family cells");
    for (mi, (_, nm)) in ORDERS.iter().enumerate() {
        println!(
            "  {nm:<17} vanilla-hit {:>6.2}%   neutron-hit {:>6.2}%   (n={expl_den})",
            pct(expl_hit[mi], expl_den),
            pct(expl_hit_neu[mi], expl_den),
        );
    }
    println!("\n=== neutral writer mix of swapped cells (writer id @ write time, NEUTRON_WRITERS)");
    for ((pair, w), c) in &writer_mix {
        println!("  {pair:<20} writer={} {}", crate_writer_name(*w), c);
    }
    let g = GATE_UNRESOLVED.with(|g| *g.borrow());
    println!("\nnote: {g} blob gates had fully invisible probe boxes (approximate no-blob verdicts)");
}

fn pct(a: u64, b: u64) -> f64 {
    if b == 0 { 0.0 } else { a as f64 / b as f64 * 100.0 }
}

/// Vanilla chunk block map, full-status chunks only (palette + unpacked
/// bit-field data; same decoder as examples/base_clusters.rs).
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

fn crate_writer_name(w: u16) -> &'static str {
    neutron_worldgen::writers::name(w)
}

// ---------------------------------------------------------------------------
// Winner selection over order models
// ---------------------------------------------------------------------------

/// Origin ranking (position in decoration sequence) for the 3x3 window around
/// sample chunk (cx,cz). Local coords rel. to window: -1..=1.
fn order_ranks(cx: i32, cz: i32, mode: OrderMode) -> HashMap<(i32, i32), usize> {
    let mut rank: HashMap<(i32, i32), usize> = HashMap::with_capacity(9);
    match mode {
        OrderMode::Row => {
            let mut p = 0;
            for dz in -1..=1 {
                for dx in -1..=1 {
                    rank.insert((dx, dz), p);
                    p += 1;
                }
            }
        }
        OrderMode::Spiral => {
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
            for (p, o) in SPIRAL.iter().enumerate() {
                rank.insert(*o, p);
            }
        }
        OrderMode::WorldOrigin => {
            // sculk/mod.rs "world_origin": squared distance of origin centre
            // from world origin, stable insertion order = row-major.
            let mut v: Vec<(i64, i32, i32)> = Vec::with_capacity(9);
            for dz in -1..=1 {
                for dx in -1..=1 {
                    let wx = ((cx + dx) * 16 + 8) as i64;
                    let wz = ((cz + dz) * 16 + 8) as i64;
                    v.push((wx * wx + wz * wz, dx, dz));
                }
            }
            v.sort();
            for (p, (_, dx, dz)) in v.iter().enumerate() {
                rank.insert((*dx, *dz), p);
            }
        }
        OrderMode::CanonPregen => {
            // sculk/mod.rs "canonical_pregen": forceload phase (inner square
            // (-8..=7)^2 first, strips after), then ChunkPos.rangeClosed cursor
            // key z*64+x.
            let mut v: Vec<(i32, i64, i32, i32)> = Vec::with_capacity(9);
            for dz in -1..=1 {
                for dx in -1..=1 {
                    let cxw = cx + dx;
                    let czw = cz + dz;
                    let phase = if (-8..=7).contains(&cxw) && (-8..=7).contains(&czw) {
                        0
                    } else if (-12..=-11).contains(&cxw) {
                        1
                    } else if (10..=11).contains(&cxw) {
                        2
                    } else if (-12..=-11).contains(&czw) {
                        3
                    } else {
                        4
                    };
                    v.push((phase, (czw as i64) * 64 + cxw as i64, dx, dz));
                }
            }
            v.sort();
            for (p, (_, _, dx, dz)) in v.iter().enumerate() {
                rank.insert((*dx, *dz), p);
            }
        }
    }
    rank
}

fn best_claim<'a>(cs: &'a [Claim], rank: &HashMap<(i32, i32), usize>) -> Option<&'a Claim> {
    cs.iter()
        .max_by_key(|c| (rank.get(&(c.olx as i32, c.olz as i32)).copied().unwrap_or(999), c.fidx))
}

fn winner_block(cs: Option<&Vec<Claim>>, rank: &HashMap<(i32, i32), usize>) -> Option<&'static str> {
    best_claim(cs?, rank).map(|c| c.block)
}

// ---------------------------------------------------------------------------
// Classification
// ---------------------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
fn classify_cluster(
    cl: &[CellKey],
    van: &HashMap<(u8, i32, u8), String>,
    neu: &neutron_worldgen::generator::GeneratedChunk,
    claims: &HashMap<CellKey, Vec<Claim>>,
    cx: i32,
    cz: i32,
) -> (String, Option<String>) {
    // Neutron ran canonical_pregen; for every swap cell vanilla resolved the
    // overlap the OPPOSITE way. A candidate model explains the cluster iff it
    // predicts vanilla's block on every flipped cell (claims are identical
    // draw-for-draw, targets unconditional => pure sequence question).
    let canon = order_ranks(cx, cz, OrderMode::CanonPregen);
    let mut flip_cells = 0u64;
    let mut anomalies = 0u64;
    let mut any_claimed = false;
    let mut examples: Vec<String> = Vec::new();
    for k in cl {
        let Some(cs) = claims.get(k) else { continue };
        any_claimed = true;
        let vn = van.get(&((k.0 & 15) as u8, k.1, (k.2 & 15) as u8)).map(|s| s.as_str()).unwrap_or("air");
        let nn = neu
            .block_at((k.0 & 15) as u32, k.1, (k.2 & 15) as u32)
            .block_name()
            .trim_start_matches("minecraft:")
            .to_string();
        let nc = best_claim(cs, &canon);
        match nc {
            Some(c) if c.block == nn.as_str() => {}
            _ => anomalies += 1, // even canonical can't explain Neutron itself
        }
        if nc.map(|c| c.block) == Some(vn) {
            continue; // vanilla agrees with canonical on this cell: no flip
        }
        flip_cells += 1;
        if examples.len() < 3 {
            let cw = nc.map(|c| c.block.to_string()).unwrap_or("<none>".into());
            let vw = cs
                .iter()
                .filter(|c| c.block == vn)
                .map(|c| format!("f{}ol({},{})try#{}", c.fidx, c.olx, c.olz, c.attempt))
                .take(2)
                .collect::<Vec<_>>()
                .join("|");
            examples.push(format!(
                "({},{},{}) canon={} vanilla-needs[{vw}]",
                k.0, k.1, k.2, cw
            ));
        }
    }
    let core = if !any_claimed {
        "upstream-non-step6".into()
    } else if flip_cells == 0 {
        format!("unresolved({anomalies}-anom)")
    } else {
        let mut fits: Vec<&str> = Vec::new();
        for (omode, nm) in ORDERS.iter() {
            let ranks = order_ranks(cx, cz, *omode);
            let ok_all = cl.iter().all(|k| {
                let Some(cs) = claims.get(k) else { return true };
                let vn = van
                    .get(&((k.0 & 15) as u8, k.1, (k.2 & 15) as u8))
                    .map(|s| s.as_str())
                    .unwrap_or("air");
                winner_block(Some(cs), &ranks) == Some(vn)
            });
            if ok_all {
                fits.push(*nm);
            }
        }
        if fits.is_empty() {
            format!("order-flip(non-parametric,{anomalies}anom)")
        } else {
            format!("order-flip(vanilla-like={})", fits.join("+"))
        }
    };
    let extra = (!examples.is_empty()).then(|| format!("e.g. {}", examples.join(" ; ")));
    (core, extra)
}


fn rep_rank(
    k: &CellKey,
    van: &HashMap<(u8, i32, u8), String>,
    neu: &neutron_worldgen::generator::GeneratedChunk,
    claims: &HashMap<CellKey, Vec<Claim>>,
    cx: i32,
    cz: i32,
) -> String {
    let (x, y, z) = *k;
    let vn = van.get(&((x & 15) as u8, y, (z & 15) as u8)).map(|s| s.as_str()).unwrap_or("?").to_string();
    let nn = neu.block_at((x & 15) as u32, y, (z & 15) as u32).block_name();
    let nn = nn.trim_start_matches("minecraft:").to_string();
    let describe = |mode: OrderMode| -> String {
        let ranks = order_ranks(cx, cz, mode);
        match claims.get(k).and_then(|cs| best_claim(cs, &ranks)) {
            Some(c) => format!(
                "{} @blob({}, {}, {}) origin-chunk({}, {})",
                c.block, c.ax, c.ay, c.az, cx + c.olx as i32, cz + c.olz as i32
            ),
            None => "<no step-6 blob>".into(),
        }
    };
    format!(
        "({},{},{}) van={vn} neu={nn} | van-producer[canon]={van_desc} | neu-producer[canon]={neu_desc}",
        x, y, z,
        van_desc = describe(OrderMode::CanonPregen),
        neu_desc = describe(OrderMode::WorldOrigin),
    )
}

// ---------------------------------------------------------------------------
// Clustering (26-connectivity union-find inside one chunk)
// ---------------------------------------------------------------------------

fn group_clusters(rows: &[CellKey], cells: &std::collections::HashSet<CellKey>) -> Vec<Vec<CellKey>> {
    let idx: HashMap<&CellKey, usize> = rows.iter().enumerate().map(|(i, k)| (k, i)).collect();
    let mut parent: Vec<usize> = (0..rows.len()).collect();
    fn find(p: &mut Vec<usize>, a: usize) -> usize {
        if p[a] != a {
            let r = find(p, p[a]);
            p[a] = r;
        }
        p[a]
    }
    for (i, &(x, y, z)) in rows.iter().enumerate() {
        for dy in -1..=1i32 {
            for dz in -1..=1i32 {
                for dx in -1..=1i32 {
                    if dx == 0 && dy == 0 && dz == 0 {
                        continue;
                    }
                    if let Some(j) = idx.get(&(x + dx, y + dy, z + dz)) {
                        if cells.contains(&(x + dx, y + dy, z + dz)) {
                            let a = find(&mut parent, i);
                            let b = find(&mut parent, *j);
                            if a != b {
                                parent[a] = b;
                            }
                        }
                    }
                }
            }
        }
    }
    let mut groups: HashMap<usize, Vec<CellKey>> = HashMap::new();
    for (i, k) in rows.iter().enumerate() {
        groups.entry(find(&mut parent, i)).or_default().push(*k);
    }
    let mut out: Vec<Vec<CellKey>> = groups.into_values().collect();
    out.sort_by_key(|g| std::cmp::Reverse(g.len()));
    out
}

// ---------------------------------------------------------------------------
// Shared RNG/blob replay (mirrors features.rs exactly; heightmaps/biomes only)
// ---------------------------------------------------------------------------

fn simulate_claims(
    seed: i64,
    state: &neutron_worldgen::worldgen::WorldgenState,
    region: &RegionBuf,
    cx: i32,
    cz: i32,
) -> HashMap<CellKey, Vec<Claim>> {
    let mut out: HashMap<CellKey, Vec<Claim>> = HashMap::new();
    let x_lo = cx * 16;
    let z_lo = cz * 16;
    for dzl in -1..=1i32 {
        for dxl in -1..=1i32 {
            let ox0 = (cx + dxl) * 16;
            let oz0 = (cz + dzl) * 16;
            let mut rng = FeatureRandom::new(seed);
            let dec = rng.set_decoration_seed(seed, ox0, oz0);
            for &(fidx, fname, bname, mode) in BASE_ORES.iter() {
                rng.set_feature_seed(dec, fidx, STEP);
                let attempts: u16 = match mode {
                    Mode::Upper => {
                        // rarity_filter chance=6 before in_square
                        if rng.next_f32() < 1.0f32 / 6.0 { 1 } else { 0 }
                    }
                    _ => 2, // count=2, no draw
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
                    // biome gate mirror of features.rs::biome_gate_ok (Any):
                    let bid = biome_id_at_block(state, x, y, z);
                    let bn = biome_id_to_name(bid);
                    let listed = feature_catalog::features_at_step(bn, STEP)
                        .iter()
                        .any(|f| f.trim_start_matches("minecraft:") == fname);
                    if !listed {
                        continue;
                    }
                    place_blob_claims(
                        &mut rng, region, x, y, z, fidx, bname, dxl, dzl, k, x_lo, z_lo,
                        &mut out,
                    );
                }
            }
        }
    }
    out
}

/// features.rs:1046 place_ore_blob_inner; records claims inside sample chunk.
#[allow(clippy::too_many_arguments)]
fn place_blob_claims(
    rng: &mut FeatureRandom,
    region: &RegionBuf,
    ox: i32,
    oy: i32,
    oz: i32,
    fidx: i32,
    bname: &'static str,
    dxl: i32,
    dzl: i32,
    attempt: u16,
    x_lo: i32,
    z_lo: i32,
    out: &mut HashMap<CellKey, Vec<Claim>>,
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

    // ocean_floor_wg_allows_ore probe (pre-carver WG heights).
    let mut pass = false;
    let mut missing = 0usize;
    let mut cols = 0usize;
    for px in sbx..=sbx + size_xz {
        for pz in sbz..=sbz + size_xz {
            cols += 1;
            match region_hm(region, px, pz) {
                Some(h) => {
                    if sby <= h {
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
        return; // no sphere stream consumed
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
                    let e = out.entry((x, y, z)).or_default();
                    if let Some(prev) = e.last_mut().filter(|p| {
                        p.olx == dxl as i8 && p.olz == dzl as i8
                    }) {
                        if prev.fidx <= fidx {
                            *prev = Claim {
                                olx: dxl as i8,
                                olz: dzl as i8,
                                fidx,
                                block: bname,
                                ax: ox,
                                ay: oy,
                                az: oz,
                                attempt,
                            };
                            continue;
                        }
                    }
                    e.push(Claim {
                        olx: dxl as i8,
                        olz: dzl as i8,
                        fidx,
                        block: bname,
                        ax: ox,
                        ay: oy,
                        az: oz,
                        attempt,
                    });
                }
            }
        }
    }
}

/// Pre-carver OCEAN_FLOOR_WG first-available (solid Y + 1), features.rs:689.
fn region_hm(region: &RegionBuf, x: i32, z: i32) -> Option<i32> {
    let lx = x - region.origin_x;
    let lz = z - region.origin_z;
    if lx < 0 || lz < 0 || lx >= region.side || lz >= region.side {
        return None;
    }
    let hi = (lz / 16 * region.chunks + lx / 16) as usize;
    let hm = region.heightmaps.get(hi)?;
    let solid_y = hm[((lz % 16) * 16 + (lx % 16)) as usize] as i32;
    if solid_y <= WORLD_BOTTOM {
        return None;
    }
    Some(solid_y + 1)
}

// --- math helpers duplicating internal crate fns -----------------------------

const PI: f32 = 3.1415927;
const SIN_SCALE: f64 = 10430.378350470453; // carvers.rs:55 (Java Mth table)

fn sin_tbl(v: f64) -> f32 {
    let idx = ((v * SIN_SCALE) as i64 as u64 & 0xFFFF) as usize;
    static TABLE: std::sync::OnceLock<Vec<f32>> = std::sync::OnceLock::new();
    let t = TABLE.get_or_init(|| (0..65536usize).map(|i| (i as f64 / SIN_SCALE).sin() as f32).collect());
    t[idx]
}

#[inline]
fn lerp(t: f64, a: f64, b: f64) -> f64 {
    a + t * (b - a)
}

#[inline]
fn floor(v: f64) -> i32 {
    v.floor() as i32
}
