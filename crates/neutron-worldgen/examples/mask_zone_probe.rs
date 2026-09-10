//! mask_zone_probe — CLOSED DUMP: undecorated-neighbour MASK emulation vs the
//! moss/clay mismatch pool (seed 424242, center chunk (-1,-2)).
//!
//! Questions:
//!   1. For every moss/clay-family mismatch cell in the 3×3 decoration
//!      window: which ORIGIN PASS wrote the final neutron block (per-cell
//!      pass attribution via rebuild-per-prefix decoration runs), and is the
//!      cell in its own origin (Z3), in an origin EARLIER than the writer
//!      (Z1, post-mask-restored zone) or in a LATER origin (Z2,
//!      masked-during-pass zone)?
//!   2. A/B: does NEUTRON_TMP_MASK=1 (mask the undecorated origins; shipped
//!      default = NO mask) fix or break those cells?
//!
//! Pass attribution via PREFIX runs (fresh region each, noise+surface cached):
//! decorate_region_origin_major(region, order=inner[..k]) — one call per run
//! keeps the internal sculk faces map shared across the k origins, exactly
//! like the full run. sanity asserts prefix(9) == generate_chunk.
//!
//! NOTE: window offsets from deco_schedule::window_order are buffer indices
//! 0..5 (mid=2); world chunk = (cx - 2 + a, cz - 2 + b).
//!
//! Usage: rustc --edition 2021 -O mask_zone_probe.rs --extern
//!   neutron_worldgen=target/release/libneutron_worldgen.rlib
//!   --extern neutron_world=target/release/deps/libneutron_world-<hash>.rlib
//!   -L dependency=target/release/deps

use neutron_world::nbt::ussr_nbt::owned::{List, Tag};
use neutron_world::nbt::{compound_get, read_nbt};
use neutron_world::Region;
use neutron_worldgen::carvers;
use neutron_worldgen::deco_schedule;
use neutron_worldgen::generator::{
    decorate_region_origin_major, ChunkGenerator, WORLD_BOTTOM, WORLD_TOP,
};
use neutron_worldgen::mineshaft;
use neutron_worldgen::region_buf::RegionBuf;
use neutron_worldgen::ruined_portal;
use neutron_worldgen::surface::BlockId;
use neutron_worldgen::writers;
use std::collections::HashMap;

const SEED: i64 = 424242;
const CHUNK: (i32, i32) = (-1, -2);
const REGION_DIR: &str =
    "tools/nbt-ref/vanilla-fresh-424242/world/dimensions/minecraft/overworld/region";
const FAMILY: [&str; 2] = ["moss_block", "clay"];

type NoiseCache = HashMap<(i32, i32), (Vec<u16>, Vec<i16>, Vec<u8>)>;

fn main() {
    // Writer-attribution plane is allocated at RegionBuf::new (process opt-in).
    std::env::set_var("NEUTRON_WRITERS", "1");
    let (cx, cz) = CHUNK;
    let t0 = std::time::Instant::now();
    println!("=== mask_zone_probe seed={SEED} center=({cx},{cz}) family={FAMILY:?}");

    let gen = ChunkGenerator::new(SEED);
    let mut noise: NoiseCache = HashMap::new();

    // ---- reference build: order + world mapping ----
    let (probe, inner, _plans) = build_region(&gen, cx, cz, &mut noise);
    let world_of = |&(a, b): &(i32, i32)| (cx - 2 + a, cz - 2 + b);
    let inner_world: Vec<(i32, i32)> = inner.iter().map(world_of).collect();
    println!("inner order (pass -> buffer offset -> world chunk):");
    for (i, &(a, b)) in inner.iter().enumerate() {
        let (wx, wz) = inner_world[i];
        println!("  pass {i}: ({a},{b}) -> chunk ({wx},{wz})");
    }
    let center_idx = inner
        .iter()
        .position(|&(a, b)| a == 2 && b == 2)
        .expect("center not in inner");
    let _ = probe;

    // ---- vanilla reference for the 3×3 decoration window ----
    let mut van: HashMap<(i32, i32, i32), String> = HashMap::new();
    let mut van_loaded = 0;
    for &(ccx, ccz) in &inner_world {
        match load_vanilla(REGION_DIR, ccx, ccz) {
            Some(m) => {
                van_loaded += 1;
                for ((lx, y, lz), s) in m {
                    van.insert((ccx * 16 + lx as i32, y, ccz * 16 + lz as i32), s);
                }
            }
            None => println!("WARN: vanilla chunk ({ccx},{ccz}) missing/not full"),
        }
    }
    println!("vanilla chunks loaded: {van_loaded}/9");

    // ---- prefix runs: snaps[k] = state after decorating inner[..k] ----
    let n = inner.len();
    let mut snaps: Vec<Vec<Vec<u16>>> = Vec::with_capacity(n + 1);
    for k in 0..=n {
        let (mut region, inner_k, plans) = build_region(&gen, cx, cz, &mut noise);
        if k == 0 {
            // pre-decoration snapshot only
        } else {
            decorate_region_origin_major(&mut region, &gen.state, &inner_k[..k], (cx, cz), &plans);
        }
        snaps.push(snap(&region, &inner_world, cx, cz));
        println!(
            "prefix run k={k}/{n} done ({}ms)",
            t0.elapsed().as_millis()
        );
    }
    let final_default = snaps.last().unwrap().clone();

    // ---- sanity: prefix(9) == generate_chunk ----
    let gc = gen.generate_chunk(cx, cz);
    let sane = gc.blocks == final_default[center_idx];
    println!(
        "sanity prefix9-vs-generate_chunk: {}",
        if sane { "PASS" } else { "FAIL" }
    );

    // ---- default mismatch scan over the 3×3 ----
    let mut cells: Vec<Row> = Vec::new();
    for (ci, &(ccx, ccz)) in inner_world.iter().enumerate() {
        for y in WORLD_BOTTOM..WORLD_TOP {
            for lz in 0..16i32 {
                for lx in 0..16i32 {
                    let wx = ccx * 16 + lx;
                    let wz = ccz * 16 + lz;
                    let vn = van
                        .get(&(wx, y, wz))
                        .map(|s| s.as_str())
                        .unwrap_or("<no-van>");
                    let nv = bname(final_default[ci][idx(lx, y, lz)]);
                    let nv = nv.trim_start_matches("minecraft:").to_string();
                    if vn == nv {
                        continue;
                    }
                    if !(FAMILY.contains(&vn) || FAMILY.contains(&nv.as_str())) {
                        continue;
                    }
                    // last pass that changed this cell
                    let mut pass: i32 = -1;
                    for k in 0..n {
                        if snaps[k][ci][idx(lx, y, lz)] != snaps[k + 1][ci][idx(lx, y, lz)] {
                            pass = k as i32;
                        }
                    }
                    let zone = zone_tag(pass, ci as i32);
                    let border = lx.min(15 - lx).min(lz).min(15 - lz);
                    cells.push(Row {
                        wx,
                        y,
                        wz,
                        ci,
                        van: vn.into(),
                        neu: nv.clone(),
                        pass,
                        zone,
                        border,
                        pair: format!("{vn}->{nv}"),
                    });
                }
            }
        }
    }

    let center_cells: Vec<&Row> = cells.iter().filter(|r| r.ci == center_idx).collect();
    println!(
        "\n-- mismatch cells family={FAMILY:?}: center chunk ({},{} chunk) = {} (prior artifact: 1284); full 3×3 = {}",
        inner_world[center_idx].0,
        inner_world[center_idx].1,
        center_cells.len(),
        cells.len()
    );

    for (label, sel) in [("center chunk", true), ("full 3×3", false)] {
        println!("\n-- zone tally ({label})");
        for z in [
            "Z3_OWN",
            "Z1_EARLIER_CELL(writer later)",
            "Z2_LATER_CELL(writer earlier)",
            "TERRAIN(never feature-written)",
        ] {
            let count = cells
                .iter()
                .filter(|r| (r.ci == center_idx) == sel && zone_tag(r.pass, r.ci as i32) == z)
                .count();
            println!("   {z:<38} {count}");
        }
    }

    println!("\n-- center mismatches by (pass, pair) top 14");
    {
        use std::collections::BTreeMap;
        let mut m: BTreeMap<(i32, String), usize> = BTreeMap::new();
        for r in &center_cells {
            *m.entry((r.pass, r.pair.clone())).or_insert(0) += 1;
        }
        let mut v: Vec<(_, usize)> = m.into_iter().collect();
        v.sort_by_key(|(_, c)| std::cmp::Reverse(*c));
        for ((p, pr), c) in v.iter().take(14) {
            println!("   pass={p:>2}  {pr:<26} {c}");
        }
    }

    println!("\n-- center mismatches by pass (own-origin = cell's chunk is the pass origin)");
    for p in -1..n as i32 {
        let cnt = center_cells.iter().filter(|r| r.pass == p).count();
        if cnt > 0 {
            let own = center_cells
                .iter()
                .filter(|r| r.pass == p && r.ci as i32 == p)
                .count();
            println!("   pass {p:>2}: {cnt:>5}  (own-origin {own})");
        }
    }

    println!("\n-- center mismatches by border distance (0 = chunk edge)");
    for d in 0..8 {
        let cnt = center_cells.iter().filter(|r| r.border == d).count();
        println!("   border={d}: {cnt}");
    }

    // CSV of all 3×3 mismatch cells
    let csv = "/tmp/opencode/mask_zone_cells.csv";
    let mut out = String::from("wx,y,wz,chunkx,chunkz,pass,zone,border,vanilla,neutron,pair\n");
    for r in &cells {
        out.push_str(&format!(
            "{},{},{},{},{},{},{},{},{},{},{}\n",
            r.wx, r.y, r.wz, inner_world[r.ci].0, inner_world[r.ci].1, r.pass, r.zone, r.border,
            r.van, r.neu, r.pair
        ));
    }
    let _ = std::fs::write(csv, &out);
    println!("\ncsv: {csv} ({} rows)", cells.len());

    // ---- A/B: NEUTRON_TMP_MASK=1 ----
    println!("\n=== A/B: NEUTRON_TMP_MASK=1 (undecorated-origin masking ON)");
    std::env::set_var("NEUTRON_TMP_MASK", "1");
    let (mut region_b, inner_b, plans_b) = build_region(&gen, cx, cz, &mut noise);
    decorate_region_origin_major(&mut region_b, &gen.state, &inner_b, (cx, cz), &plans_b);
    std::env::remove_var("NEUTRON_TMP_MASK");
    let world_b: Vec<(i32, i32)> = inner_b.iter().map(world_of).collect();
    let final_mask: Vec<Vec<u16>> = world_b
        .iter()
        .map(|&(ccx, ccz)| region_b.take_chunk(ccx, ccz).0)
        .collect();

    let mut mask_total = 0usize;
    let mut fixed = 0usize;
    let mut still_same = 0usize;
    let mut still_other = 0usize;
    for r in &center_cells {
        let i = idx(r.wx & 15, r.y, r.wz & 15);
        let mv = bname(final_mask[center_idx][i]);
        let mv = mv.trim_start_matches("minecraft:").to_string();
        if mv == r.van {
            fixed += 1;
        } else {
            mask_total += 1;
            if mv == r.neu {
                still_same += 1;
            } else {
                still_other += 1;
            }
        }
    }
    // previously-matching family cells broken by the mask (center chunk)
    let mut broke = 0usize;
    for y in WORLD_BOTTOM..WORLD_TOP {
        for lz in 0..16i32 {
            for lx in 0..16i32 {
                let wx = cx * 16 + lx;
                let wz = cz * 16 + lz;
                let vn = van.get(&(wx, y, wz)).map(|s| s.as_str()).unwrap_or("");
                let dv = bname(final_default[center_idx][idx(lx, y, lz)]);
                let dv = dv.trim_start_matches("minecraft:");
                if vn == dv && FAMILY.contains(&vn) {
                    let mv = bname(final_mask[center_idx][idx(lx, y, lz)]);
                    if mv.trim_start_matches("minecraft:") != vn {
                        broke += 1;
                    }
                }
            }
        }
    }
    println!(
        "center: default mismatches {} -> with mask {mask_total}\n  fixed by mask {fixed} | still bad same pair {still_same} | still bad pair changed {still_other} | previously-matching family cells broken {broke}",
        center_cells.len()
    );

    // 3×3 totals under mask
    let mut tot3_mask = 0usize;
    for (ci, &(ccx, ccz)) in world_b.iter().enumerate() {
        for y in WORLD_BOTTOM..WORLD_TOP {
            for lz in 0..16i32 {
                for lx in 0..16i32 {
                    let wx = ccx * 16 + lx;
                    let wz = ccz * 16 + lz;
                    let vn = van.get(&(wx, y, wz)).map(|s| s.as_str()).unwrap_or("");
                    let mv = bname(final_mask[ci][idx(lx, y, lz)]);
                    let mv = mv.trim_start_matches("minecraft:");
                    if vn != mv
                        && !vn.is_empty()
                        && (FAMILY.contains(&vn) || FAMILY.contains(&mv))
                    {
                        tot3_mask += 1;
                    }
                }
            }
        }
    }
    println!("full 3×3 family mismatches: default {} -> mask {tot3_mask}", cells.len());

    // ---- 5-cell reconstruction from the dominant center-chunk set ----
    println!("\n=== 5-cell reconstruction (center chunk, first 5 mismatches)");
    for r in center_cells.iter().take(5) {
        let ci = r.ci;
        let i = idx(r.wx & 15, r.y, r.wz & 15);
        println!(
            "\ncell ({},{},{}) local=({},{}) pass={} zone={} border={} pair={}",
            r.wx,
            r.y,
            r.wz,
            r.wx - cx * 16,
            r.wz - cz * 16,
            r.pass,
            r.zone,
            r.border,
            r.pair
        );
        println!(
            "   pre-deco: {}",
            bname(snaps[0][ci][i]).trim_start_matches("minecraft:")
        );
        for k in 0..n {
            let cur = snaps[k + 1][ci][i];
            let prev = snaps[k][ci][i];
            if cur != prev {
                println!(
                    "   pass {k} (chunk {:?}): {} -> {}",
                    inner_world[k],
                    bname(prev).trim_start_matches("minecraft:"),
                    bname(cur).trim_start_matches("minecraft:")
                );
            }
        }
        println!("   neighbourhood y-offsets +1/0/-1 (vanilla | default | maskON):");
        for dy in [1, 0, -1] {
            for dz in -1i32..=1 {
                let mut line = String::new();
                for dx in -1i32..=1 {
                    let v = van
                        .get(&(r.wx + dx, r.y + dy, r.wz + dz))
                        .map(|s| s.as_str())
                        .unwrap_or("?");
                    let nv = world_block(&final_default, &inner_world, r.wx + dx, r.y + dy, r.wz + dz)
                        .map(bname)
                        .unwrap_or_else(|| "?".into());
                    let mv = world_block(&final_mask, &world_b, r.wx + dx, r.y + dy, r.wz + dz)
                        .map(bname)
                        .unwrap_or_else(|| "?".into());
                    line.push_str(&format!(
                        "{:>13}|{:>13}|{:>13} ",
                        v.trim_start_matches("minecraft:"),
                        nv.trim_start_matches("minecraft:"),
                        mv.trim_start_matches("minecraft:")
                    ));
                }
                if dy == 0 && dz == 0 {
                    line.push_str("  <cell");
                }
                println!("   dy={dy:>2} {line}");
            }
        }
    }

    println!("\ndone in {}ms", t0.elapsed().as_millis());
}

struct Row {
    wx: i32,
    y: i32,
    wz: i32,
    ci: usize,
    van: String,
    neu: String,
    pass: i32,
    zone: &'static str,
    border: i32,
    pair: String,
}

fn zone_tag(pass: i32, cell_pass: i32) -> &'static str {
    if pass < 0 {
        "TERRAIN(never feature-written)"
    } else if pass == cell_pass {
        "Z3_OWN"
    } else if pass > cell_pass {
        "Z1_EARLIER_CELL(writer later)"
    } else {
        "Z2_LATER_CELL(writer earlier)"
    }
}

fn bname(v: u16) -> String {
    BlockId::from_u16(v)
        .unwrap_or(BlockId::Air)
        .block_name()
        .to_string()
}

fn idx(lx: i32, y: i32, lz: i32) -> usize {
    ((y - WORLD_BOTTOM) as usize) * 256 + (lz as usize) * 16 + (lx as usize)
}

/// Replicates `generate_chunk_cached` up to (and including) the frozen
/// ruined-portal plans. Noise+surface columns are cached across calls.
fn build_region(
    gen: &ChunkGenerator,
    cx: i32,
    cz: i32,
    noise: &mut NoiseCache,
) -> (RegionBuf, Vec<(i32, i32)>, ruined_portal::Plans) {
    let mut region = RegionBuf::new(cx, cz, 2);
    for dz in -2..=2 {
        for dx in -2..=2 {
            let key = (cx + dx, cz + dz);
            let entry = noise
                .entry(key)
                .or_insert_with(|| gen.generate_noise_and_surface(key.0, key.1));
            let (blocks, heightmap, biomes) = entry.clone();
            region.put_chunk(key.0, key.1, &blocks, &heightmap);
            region.put_chunk_biomes(key.0, key.1, &biomes);
        }
    }
    region.current_writer = writers::CARVER;
    carvers::apply_carvers_region(&mut region, &gen.state);
    region.current_writer = writers::MINESHAFT;
    mineshaft::apply_mineshafts_region(&mut region, &gen.state);
    region.current_writer = writers::TERRAIN;
    // ticket_sim order (same as sculk::decoration_origin_order default arm)
    let order = deco_schedule::window_order(region.chunks, region.origin_x, region.origin_z);
    let mid = region.chunks / 2;
    let inner: Vec<(i32, i32)> = order
        .into_iter()
        .filter(|&(a, b)| (a - mid).abs() <= 1 && (b - mid).abs() <= 1)
        .collect();
    let rp_owners: Vec<(i32, i32)> = inner
        .iter()
        .map(|&(a, b)| ((region.origin_x >> 4) + a, (region.origin_z >> 4) + b))
        .collect();
    let plans = ruined_portal::prepare_region_plans(&gen.state, &region, &rp_owners);
    (region, inner, plans)
}

/// Snapshot the inner chunks (block ids, per-chunk arrays), world coords.
fn snap(region: &RegionBuf, inner_world: &[(i32, i32)], _cx: i32, _cz: i32) -> Vec<Vec<u16>> {
    inner_world
        .iter()
        .map(|&(ccx, ccz)| region.take_chunk(ccx, ccz).0)
        .collect()
}

/// Look a world coord up in a set of per-chunk block arrays.
fn world_block(
    chunks: &[Vec<u16>],
    inner_world: &[(i32, i32)],
    wx: i32,
    y: i32,
    wz: i32,
) -> Option<u16> {
    let (ci, _) = inner_world
        .iter()
        .enumerate()
        .find(|(_, &(ccx, ccz))| ccx == wx >> 4 && ccz == wz >> 4)?;
    Some(chunks[ci][idx(wx & 15, y, wz & 15)])
}

/// Vanilla chunk block map, full-status chunks only (decoder copied from
/// examples/moss_clay_dump.rs).
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
        let Some(Tag::Compound(bs)) = compound_get(sec, "block_states") else {
            continue;
        };
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
        let Some(Tag::LongArray(d)) = compound_get(bs, "data") else {
            continue;
        };
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
