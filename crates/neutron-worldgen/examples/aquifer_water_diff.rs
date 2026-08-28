//! CLOSED WATER DUMP (`aquifer_water_diff`): quantify PRE-FEATURE water-cell
//! divergence between the vanilla 26.2 reference chunks and Neutron's real
//! pipeline for seed 424242, with special attention to underground aquifer
//! water at lush-cave depths (y 50..90).
//!
//! Method (both sides stripped with the SAME rules so only terrain remains):
//!   vanilla side : ref .mca block states, features stripped
//!   neutron side : `ChunkGenerator::generate_chunk_cached` (full pipeline:
//!                  doFill + surface + carvers + mineshafts + feature steps),
//!                  stripped identically
//!   strip rules  : vegetation & non-collision decorations -> air; solid
//!                  feature blocks -> carrier (moss/rooted->dirt, sculk/ore/
//!                  dripstone/geode/dripleaf/mushroom-caps -> deepslate if
//!                  y<0 else stone). run-061 lesson: a solid feature cell
//!                  becomes a solid carrier, NEVER air, so water-vs-feature
//!                  still classifies as water-vs-solid and no carve is faked.
//! Classes per cell (after strip):
//!   van_water_neu_solid / neu_water_van_solid / van_water_neu_air / other
//!   (other is sub-counted so numbers reconcile against full parity)
//!
//! For the 2 worst chunks (by water-vs-solid count) the example dumps the
//! column (y-8..y+8) at the lowest-y water-vs-solid cell and queries the Rust
//! aquifer (`NoiseBasedAquifer::for_chunk` == generator.rs `build_aquifer`)
//! at that cell and its vertical neighbours. Vanilla's aquifer answer for the
//! same cells comes from the Java probes (ProbeAquiferSubstance /
//! ProbeFluidAtWorld), run by hand; exact command lines are printed as
//! JAVAPROBE hints.
//!
//! Usage:
//!   cargo run --release -p neutron-worldgen --example aquifer_water_diff -- \
//!       [seed] [region_dir] [--wide]
//! default seed 424242, canonical ref region; `--wide` extends the lush
//! cluster (0,-1),(0,0),(1,1) to the full 5x5 around (0,0).

use neutron_world::nbt::ussr_nbt::owned::{List, Tag};
use neutron_world::nbt::{compound_get, read_nbt};
use neutron_world::Region;
use neutron_worldgen::aquifer::NoiseBasedAquifer;
use neutron_worldgen::generator::{WORLD_BOTTOM, WORLD_TOP};
use neutron_worldgen::surface::{is_vegetation_name, vanilla_name};
use neutron_worldgen::{ChunkGenerator, GeneratedChunk, NoiseCache};
use std::collections::HashMap;
use std::path::PathBuf;

const SEED: i64 = 424242;
const REGION_DIR: &str =
    "tools/nbt-ref/vanilla-fresh-424242/world/dimensions/minecraft/overworld/region";
const LUSH_CLUSTER: [(i32, i32); 3] = [(0, -1), (0, 0), (1, 1)];

const SUB_NAMES: [&str; 6] = [
    "van_air_neu_water",
    "fluid_flip",
    "van_air_neu_solid",
    "van_solid_neu_air",
    "solid_solid",
    "lava_mismatch",
];

fn open_kind(name: &str) -> u8 {
    // 0=air/cave_air, 1=water, 2=lava, 3=solid
    match name {
        "minecraft:air" | "minecraft:cave_air" => 0,
        "minecraft:water" => 1,
        "minecraft:lava" => 2,
        _ => 3,
    }
}

/// Feature strip shared by BOTH sides (run-061: solid feature cells get a
/// solid carrier, never air). Returns (stripped name, rule tag).
fn strip_name(name: &str, y: i32) -> (String, Option<&'static str>) {
    let s = name.strip_prefix("minecraft:").unwrap_or(name);
    let carrier = if y < 0 { "minecraft:deepslate" } else { "minecraft:stone" };
    let rule: (&str, &'static str) = if s == "moss_block" || s == "pale_moss_block" {
        ("minecraft:dirt", "moss->dirt")
    } else if s == "rooted_dirt" {
        ("minecraft:dirt", "rooted->dirt")
    } else if matches!(s, "sculk" | "sculk_catalyst" | "sculk_sensor" | "sculk_shrieker") {
        (carrier, "sculk->carrier")
    } else if matches!(
        s,
        "coal_ore" | "deepslate_coal_ore"
            | "iron_ore" | "deepslate_iron_ore"
            | "copper_ore" | "deepslate_copper_ore"
            | "gold_ore" | "deepslate_gold_ore"
            | "redstone_ore" | "deepslate_redstone_ore"
            | "diamond_ore" | "deepslate_diamond_ore"
            | "lapis_ore" | "deepslate_lapis_ore"
            | "emerald_ore" | "deepslate_emerald_ore"
            | "raw_iron_block" | "raw_copper_block" | "ancient_debris"
    ) {
        (carrier, "ore->carrier")
    } else if matches!(s, "dripstone_block" | "pointed_dripstone") {
        (carrier, "dripstone->carrier")
    } else if matches!(
        s,
        "amethyst_block" | "budding_amethyst" | "small_amethyst_bud"
            | "medium_amethyst_bud" | "large_amethyst_bud" | "amethyst_cluster"
            | "smooth_basalt"
    ) {
        (carrier, "geode->carrier")
    } else if matches!(s, "big_dripleaf" | "small_dripleaf" | "big_dripleaf_stem") {
        (carrier, "dripleaf->carrier")
    } else if matches!(s, "red_mushroom_block" | "brown_mushroom_block" | "mushroom_stem") {
        (carrier, "mushroom_cap->carrier")
    } else if is_vegetation_name(name) {
        // sculk_vein / glow_lichen / cave_vines / hanging_roots / carpets /
        // logs / leaves / plants / azalea bushes: non-collision decorations
        // or blocks placed in air -> air (moss/sculk/rooted handled above).
        ("minecraft:air", "veg->air")
    } else {
        return (name.to_string(), None);
    };
    (rule.0.to_string(), Some(rule.1))
}

fn band_lo(y: i32) -> i32 {
    y.div_euclid(16) * 16
}

fn band_label(lo: i32) -> String {
    if lo >= 176 {
        "[176+]".to_string()
    } else {
        format!("[{lo}..{}]", lo + 15)
    }
}

/// Vanilla loader: full-status chunks only, raw palette names.
fn load_vanilla_chunk(
    regions: &mut HashMap<(i32, i32), Region>,
    region_dir: &str,
    cx: i32,
    cz: i32,
) -> Option<HashMap<(u8, i32, u8), String>> {
    let (rx, rz) = (cx >> 5, cz >> 5);
    if !regions.contains_key(&(rx, rz)) {
        let path = PathBuf::from(format!("{region_dir}/r.{rx}.{rz}.mca"));
        let region = Region::open(&path).ok()?.with_coords(rx, rz);
        regions.insert((rx, rz), region);
    }
    let region = regions.get(&(rx, rz))?;
    let data = region.get_chunk(cx & 31, cz & 31).ok()??;
    let nbt = read_nbt(&data).ok()?;
    let Some(Tag::String(status)) = compound_get(&nbt.compound, "Status") else {
        return None;
    };
    if !status.to_string().ends_with("full") {
        eprintln!("  skip chunk ({cx},{cz}): Status={} (stub)", status);
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
                Some(Tag::String(s)) => s.to_string(),
                _ => "minecraft:air".into(),
            })
            .collect();
        if names.is_empty() {
            continue;
        }
        if names.len() == 1 {
            for ly in 0..16i32 {
                for lz in 0..16u8 {
                    for lx in 0..16u8 {
                        map.insert((lx, y_sec * 16 + ly, lz), names[0].clone());
                    }
                }
            }
            continue;
        }
        let bits = ((names.len() - 1).ilog2() + 1).max(4) as u32;
        let Some(Tag::LongArray(data)) = compound_get(bs, "data") else { continue };
        let longs: Vec<i64> = data.to_vec();
        let epl = 64 / bits;
        let mask = (1u64 << bits) - 1;
        for i in 0..4096u32 {
            let li = i as usize / epl as usize;
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

#[derive(Default)]
struct ChunkCounts {
    /// [van_water_neu_solid, neu_water_van_solid, van_water_neu_air, other]
    class: [u64; 4],
    /// OTHER sub-buckets (parallel to SUB_NAMES)
    sub: [u64; 6],
    /// water-vs-solid solid-side attribution: [class 0..=1][0=raw-terrain
    /// solid, 1=feature-stripped solid]
    solid_side: [[u64; 2]; 2],
    /// [water class 0..=2][band lo] counts for the three water classes
    bands: [HashMap<i32, u64>; 3],
    /// lowest-y water-vs-solid cell: (y, lx, lz, raw van, raw neu)
    lowest: Option<(i32, u8, u8, String, String)>,
    /// lowest-y water-vs-solid cell at lush-cave aquifer depth (y in 64..96)
    lowest_deep: Option<(i32, u8, u8, String, String)>,
    /// lowest-y neu_water_van_solid cell (class 1), for inverse-direction probe
    lowest_neu_water: Option<(i32, u8, u8, String, String)>,
    /// lowest-y class-0 cell whose neutron solid side is RAW terrain (no
    /// feature strip) — the aquifer/spring suspect on the vanilla side
    lowest_van_w_terrain: Option<(i32, u8, u8, String, String)>,
}

struct ChunkStats {
    cx: i32,
    cz: i32,
    counts: ChunkCounts,
    chunk: GeneratedChunk,
}

/// Column dump (y-8..y+8) at a mismatch water cell + Rust aquifer probe
/// (`NoiseBasedAquifer::for_chunk` == generator.rs `build_aquifer`).
fn dump_column_and_probe(
    gen: &ChunkGenerator,
    van: &HashMap<(u8, i32, u8), String>,
    chunk: &GeneratedChunk,
    cx: i32,
    cz: i32,
    seed: i64,
    cell: &(i32, u8, u8, String, String),
) {
    let (y, x, z) = (cell.0, cell.1 as i32, cell.2 as i32);
    let wx = cx * 16 + x;
    let wz = cz * 16 + z;
    println!(
        "\nCOLUMN seed={seed} chunk=({cx},{cz}) pos=({wx},{y},{wz}) van={} neu={}",
        cell.3, cell.4
    );
    for yy in (y - 8)..=(y + 8) {
        if yy < WORLD_BOTTOM || yy >= WORLD_TOP {
            continue;
        }
        let vn_raw = van
            .get(&(x as u8, yy, z as u8))
            .map(String::as_str)
            .unwrap_or("minecraft:air");
        let nn_raw = vanilla_name(chunk.block_at(x as u32, yy, z as u32));
        let (vns, vr) = strip_name(vn_raw, yy);
        let (nns, nr) = strip_name(nn_raw, yy);
        let mark = if vns != nns { "  <-- MISMATCH" } else { "" };
        println!(
            "  y={yy:>4} van={vns}{} neu={nns}{}{mark}",
            if vr.is_some() { "*" } else { "" },
            if nr.is_some() { "*" } else { "" },
        );
    }
    println!("  (* = feature-stripped before classification; mismatch cell raw names in COLUMN header)");

    let ws = &gen.state;
    let mut aq = NoiseBasedAquifer::for_chunk(ws, cx, cz);
    for dy in [-2i32, -1, 0, 1] {
        let yy = y + dy;
        let (lvl, ty, lowest_surf) = aq.probe_fluid(wx, yy, wz);
        let (d_lvl, d_lowest, under, factor, noise, partial, full) =
            aq.probe_fluid_debug(wx, yy, wz);
        let sub0 = aq
            .compute_substance(wx, yy, wz, 0.0)
            .map(|b| vanilla_name(b).to_string())
            .unwrap_or_else(|| "None(solid)".into());
        let subneg = aq
            .compute_substance(wx, yy, wz, -1.0)
            .map(|b| vanilla_name(b).to_string())
            .unwrap_or_else(|| "None(solid)".into());
        let rawdens = ws.eval(&ws.router.final_density, wx, yy, wz);
        println!(
            "RUST_AQUIFER ({wx},{yy},{wz}) substance(d=0)={sub0} substance(d=-1)={subneg} fluid_level={lvl}/{d_lvl} fluid_type={} lowest_prelim={lowest_surf}/{d_lowest} under_global={under} flooded_factor={factor:.4} floodedness_noise={noise:.6} partial_thr={partial:.4} full_thr={full:.4} raw_final_density(uninterp)={rawdens:.6}",
            vanilla_name(ty),
        );
    }
    println!(
        "JAVAPROBE hint: java -cp bin:<jar>:<libs> ProbeAquiferSubstance {seed} {cx} {cz} {wx} {y} {wz} {wx} {} {wz} {wx} {} {wz}",
        y - 1,
        y + 1
    );
    println!(
        "JAVAPROBE hint: java -cp bin:<jar>:<libs> ProbeFluidAtWorld {seed} {wx} {y} {wz} {wx} {} {wz} {wx} {} {wz}   (floodedness/spread/barrier/prelim_surface)",
        y - 1,
        y + 1
    );
    println!(
        "NOTE: substance(d=0)=water + real pipeline solid => doFill density>0 there (density mismatch, aquifer OK);\n\
         substance(d=0)=None(solid) while vanilla has water => barrier/fluid-status divergence (compare RUST_AQUIFER vs ProbeAquiferSubstance / ProbeFluidAtWorld)."
    );
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let seed: i64 = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(SEED);
    let region_dir = args.get(2).cloned().unwrap_or_else(|| REGION_DIR.to_string());
    let wide = args.iter().any(|a| a == "--wide");
    let targets: Vec<(i32, i32)> = if wide {
        let mut v = Vec::new();
        for cz in -2..=2 {
            for cx in -2..=2 {
                v.push((cx, cz));
            }
        }
        v
    } else {
        LUSH_CLUSTER.to_vec()
    };

    let gen = ChunkGenerator::new(seed);
    println!(
        "aquifer_water_diff seed={seed} chunks={targets:?} wide={wide}\n\
         strip: veg/non-collision->air; moss/rooted->dirt; sculk/ore/dripstone/geode/dripleaf/mushroom-caps->carrier(y<0?deepslate:stone);\n\
         both sides stripped identically before classification"
    );

    // Generate all target chunks in parallel (own noise cache each), then
    // compare serially in deterministic order.
    let generated: Vec<(i32, i32, GeneratedChunk)> = std::thread::scope(|s| {
        let gen = &gen;
        let mut handles = Vec::with_capacity(targets.len());
        for &(ccx, ccz) in &targets {
            handles.push(s.spawn(move || {
                let mut cache = NoiseCache::new();
                let chunk = gen.generate_chunk_cached(ccx, ccz, &mut cache);
                (ccx, ccz, chunk)
            }));
        }
        handles.into_iter().map(|h| h.join().unwrap()).collect()
    });

    let mut regions: HashMap<(i32, i32), Region> = HashMap::new();
    let mut stats: Vec<ChunkStats> = Vec::new();

    for (ccx, ccz, chunk) in generated {
        let Some(van) = load_vanilla_chunk(&mut regions, &region_dir, ccx, ccz) else {
            println!("chunk ({ccx},{ccz}): NO comparable vanilla chunk (missing/stub)");
            continue;
        };
        let mut counts = ChunkCounts::default();
        for y in WORLD_BOTTOM..WORLD_TOP {
            for z in 0..16u32 {
                for x in 0..16u8 {
                    let vn_raw = van
                        .get(&(x, y, z as u8))
                        .map(String::as_str)
                        .unwrap_or("minecraft:air")
                        .to_string();
                    let nn_raw = vanilla_name(chunk.block_at(x as u32, y, z as u32)).to_string();
                    let (vn, vr) = strip_name(&vn_raw, y);
                    let (nn, nr) = strip_name(&nn_raw, y);
                    if vn == nn {
                        continue;
                    }
                    let (vk, nk) = (open_kind(&vn), open_kind(&nn));
                    // class: 0 van_water_neu_solid, 1 neu_water_van_solid,
                    // 2 van_water_neu_air, 3 other (with sub-bucket)
                    let (cls, sub): (usize, Option<usize>) = match (vk, nk) {
                        (1, 3) => (0, None),
                        (3, 1) => (1, None),
                        (1, 0) => (2, None),
                        (0, 1) => (3, Some(0)),
                        (1, 2) | (2, 1) => (3, Some(1)),
                        (0, 3) => (3, Some(2)),
                        (3, 0) => (3, Some(3)),
                        (3, 3) => (3, Some(4)),
                        _ => (3, Some(5)), // lava-vs-air and other open mixes
                    };
                    counts.class[cls] += 1;
                    if let Some(si) = sub {
                        counts.sub[si] += 1;
                    }
                    if cls < 2 || (cls == 3 && matches!(sub, Some(0) | Some(2) | Some(3))) {
                        if let Ok(cellpath) = std::env::var("WATERDIFF_CELLS") {
                            use std::io::Write as _;
                            let mut f = std::fs::OpenOptions::new()
                                .create(true)
                                .append(true)
                                .open(&cellpath)
                                .expect("cell dump path");
                            let clsx = if cls < 2 { cls } else { 10 + sub.unwrap_or(9) };
                            writeln!(
                                f,
                                "{},{},{},{},{},{},{},{}",
                                ccx,
                                ccz,
                                ccx * 16 + x as i32,
                                y,
                                ccz * 16 + z as i32,
                                clsx,
                                vn_raw,
                                nn_raw
                            )
                            .ok();
                        }
                    }
                    if cls < 2 {
                        *counts.bands[cls].entry(band_lo(y)).or_insert(0) += 1;
                        // was the SOLID side a feature block (stripped) or raw
                        // terrain? (water side may itself be feature water —
                        // the column dump + probes attribute that)
                        let solid_was_feature = if cls == 0 { nr.is_some() } else { vr.is_some() };
                        counts.solid_side[cls][usize::from(solid_was_feature)] += 1;
                        let better = match counts.lowest {
                            None => true,
                            Some((ly, sx, sz, _, _)) => {
                                (y, z as i32, x as i32) < (ly, sz as i32, sx as i32)
                            }
                        };
                        if better {
                            counts.lowest =
                                Some((y, x, z as u8, vn_raw.clone(), nn_raw.clone()));
                        }
                        if (64..96).contains(&y) {
                            let better = match counts.lowest_deep {
                                None => true,
                                Some((ly, sx, sz, _, _)) => {
                                    (y, z as i32, x as i32) < (ly, sz as i32, sx as i32)
                                }
                            };
                            if better {
                                counts.lowest_deep =
                                    Some((y, x, z as u8, vn_raw.clone(), nn_raw.clone()));
                            }
                        }
                        if cls == 1 {
                            let better = match counts.lowest_neu_water {
                                None => true,
                                Some((ly, sx, sz, _, _)) => {
                                    (y, z as i32, x as i32) < (ly, sz as i32, sx as i32)
                                }
                            };
                            if better {
                                counts.lowest_neu_water =
                                    Some((y, x, z as u8, vn_raw.clone(), nn_raw.clone()));
                            }
                        }
                        if cls == 0 && nr.is_none() {
                            let better = match counts.lowest_van_w_terrain {
                                None => true,
                                Some((ly, sx, sz, _, _)) => {
                                    (y, z as i32, x as i32) < (ly, sz as i32, sx as i32)
                                }
                            };
                            if better {
                                counts.lowest_van_w_terrain =
                                    Some((y, x, z as u8, vn_raw.clone(), nn_raw.clone()));
                            }
                        }
                    } else if cls == 2 {
                        *counts.bands[2].entry(band_lo(y)).or_insert(0) += 1;
                    }
                }
            }
        }
        println!(
            "WATERDIFF ({ccx},{ccz}) van_water_neu_solid={} neu_water_van_solid={} van_water_neu_air={} other={}",
            counts.class[0], counts.class[1], counts.class[2], counts.class[3]
        );
        let mut subs = String::new();
        for (i, n) in counts.sub.iter().enumerate() {
            if *n > 0 {
                subs.push_str(&format!(" {}={}", SUB_NAMES[i], n));
            }
        }
        println!("  other-subcounts:{subs}");
        println!(
            "  solid-side (van_w_neu_s): raw-terrain={} feature-stripped={}   (neu_w_van_s): raw-terrain={} feature-stripped={}",
            counts.solid_side[0][0],
            counts.solid_side[0][1],
            counts.solid_side[1][0],
            counts.solid_side[1][1]
        );
        let mut bands: Vec<(i32, [u64; 3])> = Vec::new();
        for bi in 0..3 {
            for (&lo, &n) in &counts.bands[bi] {
                match bands.iter_mut().find(|(l, _)| *l == lo) {
                    Some((_, arr)) => arr[bi] = n,
                    None => {
                        let mut arr = [0u64; 3];
                        arr[bi] = n;
                        bands.push((lo, arr));
                    }
                }
            }
        }
        bands.sort_by_key(|(l, _)| *l);
        for (lo, arr) in &bands {
            println!(
                "  y-band {} van_w_neu_s={} neu_w_van_s={} van_w_neu_air={}",
                band_label(*lo),
                arr[0],
                arr[1],
                arr[2]
            );
        }
        stats.push(ChunkStats { cx: ccx, cz: ccz, counts, chunk });
    }

    // ---- worst chunks: (a) by water-vs-solid count, (b) by water-vs-
    // RAW-TERRAIN-solid count (the aquifer/spring suspect, since a stripped
    // solid side means the "solid" was a feature block) ----
    let mut ranked: Vec<&ChunkStats> = stats.iter().collect();
    ranked.sort_by_key(|c| {
        std::cmp::Reverse(c.counts.class[0] + c.counts.class[1])
    });
    let mut ranked_terrain: Vec<&ChunkStats> = stats.iter().collect();
    ranked_terrain.sort_by_key(|c| std::cmp::Reverse(c.counts.solid_side[0][0] + c.counts.solid_side[1][0]));
    println!(
        "\n=== column dumps: worst overall water-vs-solid chunk + worst raw-terrain chunk ==="
    );
    let mut regions2: HashMap<(i32, i32), Region> = HashMap::new();
    let mut picked: Vec<(i32, i32)> = Vec::new();
    let mut ranked_all = vec![ranked[0], ranked[1], ranked_terrain[0], ranked_terrain[1]];
    ranked_all.dedup_by(|a, b| (a.cx, a.cz) == (b.cx, b.cz));
    for st in ranked_all {
        let (cx, cz) = (st.cx, st.cz);
        if picked.contains(&(cx, cz)) {
            continue;
        }
        picked.push((cx, cz));
        println!(
            "  (chunk ({cx},{cz}): van_w_neu_s={} neu_w_van_s={} raw-terrain solids van/neu={}/{})",
            st.counts.class[0],
            st.counts.class[1],
            st.counts.solid_side[0][0],
            st.counts.solid_side[1][0]
        );
        let Some(van) = load_vanilla_chunk(&mut regions2, &region_dir, cx, cz) else {
            continue;
        };
        // dump the lowest-y water-vs-solid cell AND the lowest-y cell in the
        // lush-cave aquifer band (64..96) when that is a different cell
        let mut cells: Vec<&(i32, u8, u8, String, String)> = Vec::new();
        if let Some(c) = st.counts.lowest.as_ref() {
            cells.push(c);
        }
        if let Some(c) = st.counts.lowest_deep.as_ref() {
            if !cells.iter().any(|e| e.0 == c.0 && e.1 == c.1 && e.2 == c.2) {
                cells.push(c);
            }
        }
        if let Some(c) = st.counts.lowest_neu_water.as_ref() {
            if !cells.iter().any(|e| e.0 == c.0 && e.1 == c.1 && e.2 == c.2) {
                cells.push(c);
            }
        }
        if let Some(c) = st.counts.lowest_van_w_terrain.as_ref() {
            if !cells.iter().any(|e| e.0 == c.0 && e.1 == c.1 && e.2 == c.2) {
                cells.push(c);
            }
        }
        for cell in cells {
            dump_column_and_probe(&gen, &van, &st.chunk, cx, cz, seed, cell);
        }
    }

    let mut agg = [0u64; 4];
    for c in &stats {
        for i in 0..4 {
            agg[i] += c.counts.class[i];
        }
    }
    println!(
        "\n=== AGGREGATE over {} chunks: van_water_neu_solid={} neu_water_van_solid={} van_water_neu_air={} other={} ===",
        stats.len(),
        agg[0],
        agg[1],
        agg[2],
        agg[3]
    );
}
