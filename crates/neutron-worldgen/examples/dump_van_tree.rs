// Dump vanilla dark oak log/leaf layout around local (8,8) y 128..148
// to verify the true trunk origin and shape.
use neutron_world::nbt::ussr_nbt::owned::{List, Tag};
use neutron_world::nbt::{compound_get, read_nbt};
use neutron_world::Region;
use std::path::PathBuf;

fn main() {
    let path = std::env::args().nth(1).unwrap_or_else(|| {
        "tools/nbt-ref/vanilla1/world/dimensions/minecraft/overworld/region/r.0.-1.mca".to_string()
    });
    let region = Region::open(&PathBuf::from(path))
        .unwrap()
        .with_coords(0, -1);
    let data = region.get_chunk(6, 30).unwrap().unwrap();
    let nbt = read_nbt(&data).unwrap();
    let sections = match compound_get(&nbt.compound, "sections") {
        Some(Tag::List(List::Compound(l))) => l,
        _ => panic!("no sections"),
    };
    let mut blocks: Vec<(i32, i32, i32, String)> = Vec::new();
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
                let lz = ((i >> 4) & 15) as usize;
                let lx = (i & 15) as usize;
                let y = y_sec * 16 + ly;
                if y >= 128 && y <= 150 && names[0].contains("dark_oak") {
                    blocks.push((lx as i32, y, lz as i32, names[0].clone()));
                }
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
            let lz = ((i >> 4) & 15) as usize;
            let lx = (i & 15) as usize;
            let y = y_sec * 16 + ly;
            if y >= 128 && y <= 150 {
                if let Some(n) = names.get(idxp as usize) {
                    if n.contains("dark_oak") {
                        blocks.push((lx as i32, y, lz as i32, n.clone()));
                    }
                }
            }
        }
    }
    // Print per-y slice: which (x,z) have logs/leaves
    for y in (128..=148).rev() {
        let row: Vec<String> = blocks
            .iter()
            .filter(|(_, yy, _, _)| *yy == y)
            .map(|(x, _, z, n)| {
                let t = if n.contains("log") { "L" } else { "l" };
                format!("{}({},{})", t, x, z)
            })
            .collect();
        if !row.is_empty() {
            println!("y={y}: {}", row.join(" "));
        }
    }
    // Count all
    let logs = blocks.iter().filter(|(_, _, _, n)| n.contains("log")).count();
    let leaves = blocks.iter().filter(|(_, _, _, n)| n.contains("leaves")).count();
    println!("logs={logs} leaves={leaves}");
}
