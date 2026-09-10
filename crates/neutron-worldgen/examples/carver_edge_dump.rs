// CLOSED CARVER DUMP (two-sided measurement, no fix): for seed 424242,
// classify per-cell mismatches between vanilla .mca reference chunks and
// Neutron generation that look like CARVER edges, i.e. one side is
// air / cave_air / water / lava while the other is solid:
//   missing-carve = vanilla open,   Neutron solid
//   extra-carve   = vanilla solid,  Neutron open
// plus a separate fluid-flip counter (both open, different fluid/air tag)
// and an OTHER counter so per-chunk numbers reconcile against full parity.
//
// Geometry attribution (per task brief):
//   canyon-like : the OPEN side has a vertical contiguous open run of
//                 >= RUN_MIN cells whose top reaches >= CANYON_TOP_Y
//                 (tall near-surface trough with steep walls)
//   cave-like   : otherwise (ellipsoid chain, mostly below ~y64)
// The runs are measured on whichever side is open (vanilla for
// missing-carve, Neutron for extra-carve), column-local, clipped by chunk
// borders (footnote in output).
//
// Vanilla stubs (Status != *full) are skipped per AGENTS.md.
//
// Usage:
//   cargo run --release -p neutron-worldgen --example carver_edge_dump -- \
//       [seed] [region_dir]
// default: 424242 canonical ref dir below.
// NOTE: autoexamples=false in Cargo.toml — this file is not registered there
// (Cargo.toml edits out of scope for this dump). Build standalone:
//   rustc --edition 2021 -O examples/carver_edge_dump.rs \
//     -L target/release/deps \
//     --extern neutron_world=<deps>/libneutron_world-*.rlib \
//     --extern neutron_worldgen=<deps>/libneutron_worldgen-*.rlib
// and register a [[example]] line later if it should live in cargo.

use neutron_world::nbt::ussr_nbt::owned::{List, Tag};
use neutron_world::nbt::{compound_get, read_nbt};
use neutron_world::Region;
use neutron_worldgen::surface::{is_vegetation_name, vanilla_name};
use neutron_worldgen::generator::{WORLD_BOTTOM, WORLD_TOP};
use neutron_worldgen::{ChunkGenerator, NoiseCache};
use std::collections::HashMap;
use std::path::PathBuf;

const SEED: i64 = 424242;
const TARGETS: [(i32, i32); 8] = [
    (7, 2),
    (8, 0),
    (-10, 6),
    (0, 0),
    (1, 1),
    // s68: the extra-carve cluster (scene-diff van=stone mine=air hotspots)
    (-11, -4),
    (-10, -4),
    (-14, -4),
];
const REGION_DIR: &str =
    "tools/nbt-ref/vanilla-fresh-424242/world/dimensions/minecraft/overworld/region";

/// min contiguous open cells for a trough to read "canyon"
const RUN_MIN: usize = 12;
/// top of that run must reach at least this y (canyon y range is 10..67)
const CANYON_TOP_Y: i32 = 46;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Open {
    Air,
    CaveAir,
    Water,
    Lava,
    Solid,
}

impl Open {
    fn from_name(n: &str) -> Self {
        match n {
            "minecraft:air" => Open::Air,
            "minecraft:cave_air" => Open::CaveAir,
            "minecraft:water" => Open::Water,
            "minecraft:lava" => Open::Lava,
            _ => Open::Solid,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Class {
    MissingCarve,
    ExtraCarve,
    FluidFlip,
    Same,
    SolidOther,
}
impl Class {
    fn label(self) -> &'static str {
        match self {
            Class::MissingCarve => "missing-carve",
            Class::ExtraCarve => "extra-carve",
            Class::FluidFlip => "fluid-flip",
            Class::Same => "same",
            Class::SolidOther => "solid-other",
        }
    }
}

/// canyon-like | cave-like attribution
enum Kind {
    Canyon,
    Cave,
}
impl Kind {
    fn label(self) -> &'static str {
        match self {
            Kind::Canyon => "canyon-like",
            Kind::Cave => "cave-like",
        }
    }
}

fn band_idx(y: i32) -> usize {
    // [-64..-34],[-33..0],[1..32],[33..64],[65+]
    if y <= -34 {
        0
    } else if y <= 0 {
        1
    } else if y <= 32 {
        2
    } else if y <= 64 {
        3
    } else {
        4
    }
}

const BAND_LABELS: [&str; 5] = ["[-64..-34]", "[-33..0]", "[1..32]", "[33..64]", "[65+]"];

#[derive(Default)]
struct Bucket {
    canyon: u64,
    cave: u64,
    unattr: u64, // open-run metrics unavailable (should be ~0)
    /// subset of `feat` rows by kind, for a clean carve-edge split
    fcanyon: u64,
    fcave: u64,
}
impl Bucket {
    fn total(&self) -> u64 {
        self.canyon + self.cave + self.unattr
    }
    fn feat(&self) -> u64 {
        self.fcanyon + self.fcave
    }
}

/// Heuristic flag for feature-driven flips masquerading as carve edges.
fn featureish(vn: &str, nn: &str) -> bool {
    fn sus(n: &str) -> bool {
        is_vegetation_name(n)
            || n.contains("vines")
            || n.contains("hanging_roots")
            || n.contains("sculk")
            || n.contains("rail")
            || n.contains("azalea")
            || n.contains("moss")
    }
    sus(vn) || sus(nn)
}

struct ChunkReport {
    cx: i32,
    cz: i32,
    /// [class][band]
    miss: [Bucket; 5],
    ext: [Bucket; 5],
    flip: u64,
    other: u64,
    samples: Vec<String>,
}

fn load_vanilla_chunk(
    regions: &mut HashMap<(i32, i32), Region>,
    region_dir: &str,
    cx: i32,
    cz: i32,
) -> Option<HashMap<(u8, i32, u8), String>> {
    let (rx, rz) = (cx >> 5, cz >> 5);
    let key = (rx, rz);
    if !regions.contains_key(&key) {
        let path = PathBuf::from(format!("{region_dir}/r.{rx}.{rz}.mca"));
        let region = Region::open(&path).ok()?.with_coords(rx, rz);
        regions.insert(key, region);
    }
    let region = regions.get(&key)?;
    let data = region.get_chunk(cx & 31, cz & 31).ok()??;
    let nbt = read_nbt(&data).ok()?;
    let Some(Tag::String(status)) = compound_get(&nbt.compound, "Status") else {
        return None;
    };
    if !status.to_string().ends_with("full") {
        eprintln!(
            "  skip chunk ({cx},{cz}): Status={} (stub, not a measurement target)",
            status
        );
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
            _ => continue,
        };
        let Some(Tag::Compound(bs)) = compound_get(sec, "block_states") else {
            continue;
        };
        let Some(Tag::List(List::Compound(palette))) = compound_get(bs, "palette") else {
            continue;
        };
        let names: Vec<String> = palette
            .iter()
            .map(|pc| match compound_get(pc, "Name") {
                Some(Tag::String(s)) => s.to_string(),
                _ => "minecraft:air".into(),
            })
            .collect();
        let nstates = names.len();
        if nstates == 1 {
            for i in 0..4096u32 {
                let ly = (i >> 8) as i32;
                let lz = ((i >> 4) & 15) as u8;
                let lx = (i & 15) as u8;
                map.insert((lx, y_sec * 16 + ly, lz), names[0].clone());
            }
            continue;
        }
        let bits = ((nstates - 1).ilog2() + 1).max(4) as u32;
        let Some(Tag::LongArray(data)) = compound_get(bs, "data") else {
            continue;
        };
        let longs: Vec<i64> = data.to_vec();
        let epl = 64 / bits;
        let mask = (1u64 << bits) - 1;
        for i in 0..4096u32 {
            let li = (i / epl) as usize;
            let bo = (i % epl) * bits;
            let idxp = ((longs[li] as u64) >> bo) & mask;
            let ly = (i >> 8) as i32;
            let lz = ((i >> 4) & 15) as u8;
            let lx = (i & 15) as u8;
            let name = names.get(idxp as usize).cloned().unwrap_or_default();
            map.insert((lx, y_sec * 16 + ly, lz), name);
        }
    }
    Some(map)
}

/// Contiguous open run through (lx,y,lz) on `grid`, column-local.
/// Returns (run_len, run_top_world_y); zero-length if cell itself not open.
fn run_stats(grid: &[Open], lx: usize, y: i32, lz: usize) -> (usize, i32) {
    let col = lz * 16 + lx;
    let yy = |wy: i32| -> usize { ((wy - WORLD_BOTTOM) * 256 + col as i32) as usize };
    if grid[yy(y)] == Open::Solid {
        return (0, y);
    }
    let mut top = y;
    while top + 1 < WORLD_TOP && grid[yy(top + 1)] != Open::Solid {
        top += 1;
    }
    let mut bot = y;
    while bot - 1 > WORLD_BOTTOM && grid[yy(bot - 1)] != Open::Solid {
        bot -= 1;
    }
    ((top - bot + 1) as usize, top)
}

fn attribute(grid_open_side: &[Open], lx: usize, y: i32, lz: usize) -> Option<(Kind, usize, i32)> {
    let (len, top) = run_stats(grid_open_side, lx, y, lz);
    if len == 0 {
        return None;
    }
    let k = if len >= RUN_MIN && top >= CANYON_TOP_Y {
        Kind::Canyon
    } else {
        Kind::Cave
    };
    Some((k, len, top))
}

#[allow(clippy::too_many_arguments)]
fn sample_line(
    seed: i64,
    cx: i32,
    cz: i32,
    class: Class,
    k: Option<(Kind, usize, i32)>,
    wx: i32,
    y: i32,
    wz: i32,
    vn: &str,
    nn: &str,
) -> String {
    let kl = k
        .map(|(k, len, top)| format!("{} run={len} run_top_y={top}", k.label()))
        .unwrap_or_else(|| "n/a".into());
    format!(
        "SAMPLE seed={seed} chunk=({cx},{cz}) class={} kind({kl}) pos=({wx},{y},{wz}) band={} vanilla={vn} neutron={nn}",
        class.label(),
        BAND_LABELS[band_idx(y)],
    )
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let seed: i64 = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(SEED);
    let region_dir = args.get(2).cloned().unwrap_or_else(|| REGION_DIR.to_string());
    let span = (WORLD_TOP - WORLD_BOTTOM) as usize;

    let gen = ChunkGenerator::new(seed);
    let mut regions: HashMap<(i32, i32), Region> = HashMap::new();

    println!("carver_edge_dump seed={seed} targets={TARGETS:?}");
    println!("open-set = {{air, cave_air, water, lava}}; missing=van-open/neu-solid; extra=neu-open/van-solid");
    println!(
        "attribution: canyon-like iff vertical open run>= {RUN_MIN} with run_top_y>= {CANYON_TOP_Y}; else cave-like"
    );

    // Generate all target chunks in parallel (own noise cache each), then
    // compare serially in deterministic order.
    let generated: Vec<(i32, i32, neutron_worldgen::GeneratedChunk)> = std::thread::scope(|s| {
        let gen = &gen;
        let mut handles = Vec::with_capacity(TARGETS.len());
        for &(ccx, ccz) in &TARGETS {
            handles.push(s.spawn(move || {
                let mut cache = NoiseCache::new();
                let chunk = gen.generate_chunk_cached(ccx, ccz, &mut cache);
                (ccx, ccz, chunk)
            }));
        }
        handles.into_iter().map(|h| h.join().unwrap()).collect()
    });

    let mut reports: Vec<ChunkReport> = Vec::new();
    for (ccx, ccz, chunk) in generated {
        let Some(van) = load_vanilla_chunk(&mut regions, &region_dir, ccx, ccz) else {
            println!("chunk ({ccx},{ccz}): NO comparable vanilla chunk (missing/stub)");
            continue;
        };

        // Flatten both sides to Open grids aligned at WORLD_BOTTOM.
        let mut vgrid = vec![Open::Air; span * 256];
        let mut ngrid = vec![Open::Air; span * 256];
        for y in WORLD_BOTTOM..WORLD_TOP {
            for z in 0..16u32 {
                for x in 0..16u32 {
                    let i = ((y - WORLD_BOTTOM) * 256 + (z as i32 * 16 + x as i32)) as usize;
                    vgrid[i] = Open::from_name(
                        van.get(&(x as u8, y, z as u8))
                            .map(String::as_str)
                            .unwrap_or("minecraft:air"),
                    );
                    ngrid[i] = Open::from_name(vanilla_name(chunk.block_at(x, y, z)));
                }
            }
        }

        let mut rep = ChunkReport {
            cx: ccx,
            cz: ccz,
            miss: Default::default(),
            ext: Default::default(),
            flip: 0,
            other: 0,
            samples: Vec::new(),
        };
        // print at most these many raw examples per class per chunk
        let (cap_miss, cap_ext) = (3, 3);
        let mut seen_miss = 0u32;
        let mut seen_ext = 0u32;

        for y in WORLD_BOTTOM..WORLD_TOP {
            for z in 0..16u32 {
                for x in 0..16u32 {
                    let i = ((y - WORLD_BOTTOM) * 256 + (z as i32 * 16 + x as i32)) as usize;
                    let (vk, nk) = (vgrid[i], ngrid[i]);
                    let cls = if vk == nk {
                        Class::Same
                    } else if vk != Open::Solid && nk == Open::Solid {
                        Class::MissingCarve
                    } else if vk == Open::Solid && nk != Open::Solid {
                        Class::ExtraCarve
                    } else if vk != Open::Solid && nk != Open::Solid {
                        Class::FluidFlip
                    } else {
                        Class::SolidOther
                    };
                    let b = band_idx(y);
                    match cls {
                        Class::Same => {}
                        Class::FluidFlip => rep.flip += 1,
                        Class::SolidOther => rep.other += 1,
                        Class::MissingCarve => {
                            let vn = van
                                .get(&(x as u8, y, z as u8))
                                .map(String::as_str)
                                .unwrap_or("minecraft:air");
                            let nn = vanilla_name(chunk.block_at(x, y, z));
                            match attribute(&vgrid, x as usize, y, z as usize) {
                                Some((Kind::Canyon, _, _)) => rep.miss[b].canyon += 1,
                                Some((Kind::Cave, _, _)) => rep.miss[b].cave += 1,
                                None => rep.miss[b].unattr += 1,
                            }
                            if featureish(vn, nn) {
                                match attribute(&vgrid, x as usize, y, z as usize) {
                                    Some((Kind::Canyon, _, _)) => rep.miss[b].fcanyon += 1,
                                    _ => rep.miss[b].fcave += 1,
                                }
                            }
                            if seen_miss < cap_miss {
                                seen_miss += 1;
                                let wx = ccx * 16 + x as i32;
                                let wz = ccz * 16 + z as i32;
                                rep.samples.push(sample_line(
                                    seed,
                                    ccx,
                                    ccz,
                                    cls,
                                    attribute(&vgrid, x as usize, y, z as usize),
                                    wx,
                                    y,
                                    wz,
                                    vn,
                                    nn,
                                ));
                            }
                        }
                        Class::ExtraCarve => {
                            let vn = van
                                .get(&(x as u8, y, z as u8))
                                .map(String::as_str)
                                .unwrap_or("minecraft:air");
                            let nn = vanilla_name(chunk.block_at(x, y, z));
                            match attribute(&ngrid, x as usize, y, z as usize) {
                                Some((Kind::Canyon, _, _)) => rep.ext[b].canyon += 1,
                                Some((Kind::Cave, _, _)) => rep.ext[b].cave += 1,
                                None => rep.ext[b].unattr += 1,
                            }
                            if featureish(vn, nn) {
                                match attribute(&ngrid, x as usize, y, z as usize) {
                                    Some((Kind::Canyon, _, _)) => rep.ext[b].fcanyon += 1,
                                    _ => rep.ext[b].fcave += 1,
                                }
                            }
                            if seen_ext < cap_ext {
                                seen_ext += 1;
                                let wx = ccx * 16 + x as i32;
                                let wz = ccz * 16 + z as i32;
                                rep.samples.push(sample_line(
                                    seed,
                                    ccx,
                                    ccz,
                                    cls,
                                    attribute(&ngrid, x as usize, y, z as usize),
                                    wx,
                                    y,
                                    wz,
                                    vn,
                                    nn,
                                ));
                            }
                        }
                    }
                }
            }
        }
        reports.push(rep);
    }

    // ---- per-chunk tables ----
    let mut tot_miss: [Bucket; 5] = std::array::from_fn(|_| Bucket::default());
    let mut tot_ext: [Bucket; 5] = std::array::from_fn(|_| Bucket::default());
    let mut tot_flip = 0u64;
    let mut tot_other = 0u64;

    for r in &reports {
        println!("\n=== chunk ({},{}) ===", r.cx, r.cz);
        println!("  {:>10} {:>11} {:>12} {:>10} {:>6} {:>10} {:>11} {:>9} {:>5}", "band", "miss-total", "miss-canyon", "miss-cave", "feat", "ext-total", "ext-canyon", "ext-cave", "feat");
        for b in 0..5 {
            println!(
                "  {:>10} {:>11} {:>12} {:>10} {:>6} {:>10} {:>11} {:>9} {:>5}",
                BAND_LABELS[b],
                r.miss[b].total(),
                r.miss[b].canyon,
                r.miss[b].cave,
                r.miss[b].feat(),
                r.ext[b].total(),
                r.ext[b].canyon,
                r.ext[b].cave,
                r.ext[b].feat(),
            );
        }
        let m: u64 = r.miss.iter().map(Bucket::total).sum();
        let e: u64 = r.ext.iter().map(Bucket::total).sum();
        println!(
            "  chunk sums: missing-carve={m} extra-carve={e} fluid-flip={} solid-other={}",
            r.flip, r.other
        );
        for s in &r.samples {
            println!("  {s}");
        }
        for b in 0..5 {
            tot_miss[b].canyon += r.miss[b].canyon;
            tot_miss[b].cave += r.miss[b].cave;
            tot_miss[b].unattr += r.miss[b].unattr;
            tot_miss[b].fcanyon += r.miss[b].fcanyon; tot_miss[b].fcave += r.miss[b].fcave;
            tot_ext[b].canyon += r.ext[b].canyon;
            tot_ext[b].cave += r.ext[b].cave;
            tot_ext[b].unattr += r.ext[b].unattr;
            tot_ext[b].fcanyon += r.ext[b].fcanyon; tot_ext[b].fcave += r.ext[b].fcave;
        }
        tot_flip += r.flip;
        tot_other += r.other;
    }

    // ---- aggregate ----
    let tm: u64 = tot_miss.iter().map(Bucket::total).sum();
    let te: u64 = tot_ext.iter().map(Bucket::total).sum();
    let residual = tm + te;
    let mc: u64 = tot_miss.iter().map(|b| b.canyon).sum();
    let ec: u64 = tot_ext.iter().map(|b| b.canyon).sum();
    println!("\n=== AGGREGATE over {} chunks (seed {seed}) ===", reports.len());
    let mf: u64 = tot_miss.iter().map(|b| b.feat()).sum();
    let ef: u64 = tot_ext.iter().map(|b| b.feat()).sum();
    let clean = residual - mf - ef;
    println!("  residual (missing+extra) = {residual}");
    println!(
        "  feature-flagged subset   = {} (miss {mf} + ext {ef}); clean carve-edge residual = {clean}",
        mf + ef
    );
    println!(
        "  canyon-like: miss {mc} / ext {ec} => {:.1}% of full residual",
        100.0 * (mc + ec) as f64 / residual.max(1) as f64
    );
    let mcc: u64 = mc - tot_miss.iter().map(|b| b.fcanyon).sum::<u64>();
    let ecc: u64 = ec - tot_ext.iter().map(|b| b.fcanyon).sum::<u64>();
    println!(
        "  CLEAN (feature-flagged removed): canyon-like miss {mcc} / ext {ecc} => {:.1}% of {clean}",
        100.0 * (mcc + ecc) as f64 / clean.max(1) as f64
    );
    println!(
        "  cave-like  : miss {} / ext {} => {:.1}% of full residual",
        tm.saturating_sub(mc),
        te.saturating_sub(ec),
        100.0 * (residual - mc - ec) as f64 / residual.max(1) as f64
    );
    println!("  fluid-flip (both open, different) = {tot_flip}; solid-other mismatches = {tot_other} (NOT counted as carve)");
    println!("\n  NOTE: open-run attribution is column-local and clipped at chunk");
    println!("  borders; a canyon crossing the edge measures shorter there.");
    println!("  NOTE: 'cheese'/'spaghetti'/'noodle' are noise-router density");
    println!("  functions in 26.2 (NoiseRouterData), not WorldCarver steps - any");
    println!("  cave-like bucket therefore includes doFill-carved openings too.");
}
