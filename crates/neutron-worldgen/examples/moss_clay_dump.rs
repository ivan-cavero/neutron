//! moss_clay_dump — CLOSED DUMP for moss_block / clay mismatches, ONE chunk.
//!
//! Question (seed 424242, chunk (-1,-2)): for cells where vanilla has
//! minecraft:moss_block or minecraft:clay vs neutron output (and the inverse),
//! what are the (vanilla, neutron) name pairs, and do the flips look like a
//! DIFFERENT-FEATURE-won-the-cell case or the same feature shifted ±1 cell?
//!
//! Decoration order = ticket_sim default (NEUTRON_SCULK_ORIGIN_ORDER NOT set).
//!
//! Vanilla loader copied from examples/ore_swap_dump.rs (full-status chunks
//! only, palette + unpacked bit-field sections).
//!
//! Usage: cargo run --release -p neutron-worldgen --example moss_clay_dump

use neutron_world::nbt::ussr_nbt::owned::{List, Tag};
use neutron_world::nbt::{compound_get, read_nbt};
use neutron_world::Region;
use neutron_worldgen::generator::{WORLD_BOTTOM, WORLD_TOP};
use neutron_worldgen::ChunkGenerator;
use std::collections::{BTreeMap, HashMap};

const SEED: i64 = 424242;
const CHUNK: (i32, i32) = (-1, -2);
const REGION_DIR: &str =
    "tools/nbt-ref/vanilla-fresh-424242/world/dimensions/minecraft/overworld/region";
const FAMILY: [&str; 2] = ["moss_block", "clay"];
const MAX_ROWS: usize = 30;
const TOP_PAIRS: usize = 8;

fn main() {
    let (cx, cz) = CHUNK;
    println!("=== moss_clay_dump seed={SEED} chunk=({cx},{cz}) families={FAMILY:?}");

    let Some(van) = load_vanilla(REGION_DIR, cx, cz) else {
        println!("SKIP: vanilla chunk missing/not full status");
        return;
    };
    let gen = ChunkGenerator::new(SEED);
    let neu = gen.generate_chunk(cx, cz);

    let mut matrix: BTreeMap<(String, String), u64> = BTreeMap::new();
    let mut rows: Vec<(i32, i32, i32, String, String)> = Vec::new();

    for lz in 0..16i32 {
        for lx in 0..16i32 {
            for y in WORLD_BOTTOM..WORLD_TOP {
                let vn = van
                    .get(&(lx as u8, y, lz as u8))
                    .map(|s| s.as_str())
                    .unwrap_or("air");
                let nn = neu
                    .block_at(lx as u32, y, lz as u32)
                    .block_name()
                    .trim_start_matches("minecraft:");
                if vn == nn {
                    continue;
                }
                if !(FAMILY.contains(&vn) || FAMILY.contains(&nn)) {
                    continue;
                }
                *matrix.entry((vn.into(), nn.to_string())).or_insert(0) += 1;
                if rows.len() < MAX_ROWS {
                    rows.push((cx * 16 + lx, y, cz * 16 + lz, vn.into(), nn.to_string()));
                }
            }
        }
    }

    let total: u64 = matrix.values().sum();
    println!("\n-- mismatch cells involving moss_block/clay (either side): {total}");

    println!("\n-- top {TOP_PAIRS} (vanilla, neutron) pairs");
    let mut pairs: Vec<(&(String, String), &u64)> = matrix.iter().collect();
    pairs.sort_by_key(|(_, c)| std::cmp::Reverse(**c));
    for ((v, n), c) in pairs.iter().take(TOP_PAIRS) {
        println!("   {v:<12} -> {n:<12} {c}");
    }

    println!("\n-- up to {MAX_ROWS} sampled mismatch rows (x,y,z | vanilla | neutron)");
    for (x, y, z, v, n) in &rows {
        println!("   ({x:>6},{y:>4},{z:>6}) | {v:<12} | {n}");
    }
}

/// Vanilla chunk block map, full-status chunks only (same decoder as
/// examples/ore_swap_dump.rs / examples/base_clusters.rs).
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
