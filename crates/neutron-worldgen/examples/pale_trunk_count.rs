//! run-058 T1 diagnostic: count pale oak trunk bases in a Neutron-generated
//! chunk vs the vanilla reference. Answers "MORE trees or BIGGER trees?".
//!
//! Usage: cargo run --release -p neutron-worldgen --example pale_trunk_count -- <seed> <cx> <cz> <region_dir>
//!
//! A trunk base = the lowest pale_oak_log in a column (2x2 dark-oak trunks
//! yield one base per column). Vanilla counts come from the reference .mca;
//! Neutron counts from ChunkGenerator::generate_chunk.

use neutron_world::nbt::ussr_nbt::owned::{List, Tag};
use neutron_world::nbt::{compound_get, read_nbt};
use neutron_world::Region;
use neutron_worldgen::surface::BlockId;
use neutron_worldgen::ChunkGenerator;
use std::path::PathBuf;

fn load_vanilla_blocks(region_dir: &str, cx: i32, cz: i32) -> Option<Vec<u16>> {
    let (rx, rz) = (cx >> 5, cz >> 5);
    let path = PathBuf::from(format!("{region_dir}/r.{rx}.{rz}.mca"));
    let region = Region::open(&path).ok()?.with_coords(rx, rz);
    let data = region.get_chunk(cx & 31, cz & 31).ok()??;
    let nbt = read_nbt(&data).ok()?;
    if let Some(Tag::String(s)) = compound_get(&nbt.compound, "Status") {
        let st = s.to_string();
        if !st.ends_with("full") {
            return None; // stub chunk (biomes-only etc.): not comparable
        }
    } else {
        return None;
    }    let sections = match compound_get(&nbt.compound, "sections") {
        Some(Tag::List(List::Compound(l))) => l,
        _ => return None,
    };
    let wb = neutron_worldgen::generator::WORLD_BOTTOM;
    let mut blocks = vec![BlockId::Air.as_u16(); 16 * 384 * 16];
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
                Some(Tag::String(s)) => s.to_string(),
                _ => "minecraft:air".into(),
            })
            .collect();
        if names.is_empty() {
            continue;
        }
        let bits = if names.len() <= 1 {
            0
        } else {
            ((names.len() - 1).ilog2() + 1).max(4) as u32
        };
        match compound_get(bs, "data") {
            Some(Tag::LongArray(data)) => {
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
                    let bid = BlockId::from_name(name.strip_prefix("minecraft:").unwrap_or(&name))
                        .map(|b| b.as_u16())
                        .unwrap_or(BlockId::Air.as_u16());
                    let bi = ((y_sec * 16 + ly - wb) * 256 + lz as i32 * 16 + lx as i32) as usize;
                    blocks[bi] = bid;
                }
            }
            _ => {
                let bid = names[0]
                    .strip_prefix("minecraft:")
                    .and_then(BlockId::from_name)
                    .map(|b| b.as_u16())
                    .unwrap_or(BlockId::Air.as_u16());
                for ly in 0..16 {
                    for lz in 0..16 {
                        for lx in 0..16 {
                            let bi = ((y_sec * 16 + ly - wb) * 256 + lz * 16 + lx) as usize;
                            blocks[bi] = bid;
                        }
                    }
                }
            }
        }
    }
    Some(blocks)
}

/// Count (base_x, base_y, base_z) trunk bases + total log blocks in a chunk.
fn trunk_stats(blocks: &[u16], cx: i32, cz: i32) -> (Vec<(i32, i32, i32)>, usize) {
    let wb = neutron_worldgen::generator::WORLD_BOTTOM;
    let mut bases = Vec::new();
    let mut logs = 0usize;
    for lz in 0..16i32 {
        for lx in 0..16i32 {
            // lowest log in the column = trunk base, then count total logs
            let mut base = None;
            let mut col_logs = 0usize;
            for ly in 0..384i32 {
                let bi = (ly * 256 + lz * 16 + lx) as usize;
                if blocks[bi] == BlockId::PaleOakLog.as_u16() {
                    col_logs += 1;
                    if base.is_none() {
                        base = Some((cx * 16 + lx, wb + ly, cz * 16 + lz));
                    }
                }
            }
            logs += col_logs;
            if let Some(b) = base {
                bases.push(b);
            }
        }
    }
    (bases, logs)
}

fn main() {
    let seed: i64 = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(424242);
    let cx: i32 = std::env::args()
        .nth(2)
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    let cz: i32 = std::env::args()
        .nth(3)
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    let region_dir = std::env::args().nth(4).unwrap_or_else(|| {
        "tools/nbt-ref/vanilla-fresh-424242/world/dimensions/minecraft/overworld/region".to_string()
    });

    let van = load_vanilla_blocks(&region_dir, cx, cz).expect("vanilla chunk");
    let (v_bases, v_logs) = trunk_stats(&van, cx, cz);

    let gen = ChunkGenerator::new(seed);
    let chunk = gen.generate_chunk(cx, cz);
    let wb = neutron_worldgen::generator::WORLD_BOTTOM;
    let mut n_bases = Vec::new();
    let mut n_logs = 0usize;
    for lz in 0..16u32 {
        for lx in 0..16u32 {
            let mut base = None;
            for ly in 0..384i32 {
                let b = chunk.block_at(lx, wb + ly, lz);
                if b == BlockId::PaleOakLog {
                    n_logs += 1;
                    if base.is_none() {
                        base = Some((cx * 16 + lx as i32, wb + ly, cz * 16 + lz as i32));
                    }
                }
            }
            if let Some(b) = base {
                n_bases.push(b);
            }
        }
    }

    println!("seed={seed} chunk=({cx},{cz})");
    println!("vanilla: {} bases, {v_logs} logs", v_bases.len());
    println!("neutron: {} bases, {n_logs} logs", n_bases.len());
    println!(
        "per tree: vanilla {:.1} logs/tree, neutron {:.1} logs/tree",
        v_logs as f64 / v_bases.len() as f64,
        n_logs as f64 / n_bases.len().max(1) as f64
    );

    // position match
    let mut matched = 0usize;
    for (nx, nz) in n_bases.iter().map(|(x, _, z)| (x, z)) {
        if v_bases.iter().any(|(vx, _, vz)| vx == nx && vz == nz) {
            matched += 1;
        }
    }
    println!(
        "base-position match: {matched}/{} neutron, {}/{} vanilla",
        n_bases.len(),
        v_bases
            .iter()
            .filter(|(vx, _, vz)| n_bases.iter().any(|(nx, _, nz)| nx == vx && nz == vz))
            .count(),
        v_bases.len()
    );
    for (x, y, z) in &n_bases {
        let has = v_bases.iter().any(|(vx, _, vz)| vx == x && vz == z);
        if !has {
            println!("  neutron-only base ({x},{y},{z})");
        }
    }
    for (x, y, z) in &v_bases {
        let has = n_bases.iter().any(|(nx, _, nz)| nx == x && nz == z);
        if !has {
            println!("  MISSING vanilla base ({x},{y},{z})");
        }
    }
}


