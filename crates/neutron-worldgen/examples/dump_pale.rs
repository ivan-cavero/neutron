//! run-058 T1: dump all vanilla pale_oak blocks in a chunk (diagnostic).
use neutron_world::nbt::ussr_nbt::owned::{List, Tag};
use neutron_world::nbt::{compound_get, read_nbt};
use neutron_world::Region;
use neutron_worldgen::surface::BlockId;
use std::path::PathBuf;

fn main() {
    let cx: i32 = std::env::args().nth(1).and_then(|s| s.parse().ok()).unwrap_or(0);
    let cz: i32 = std::env::args().nth(2).and_then(|s| s.parse().ok()).unwrap_or(0);
    let region_dir = std::env::args().nth(3).unwrap_or_else(|| {
        "tools/nbt-ref/vanilla-fresh-424242/world/dimensions/minecraft/overworld/region".to_string()
    });
    let (rx, rz) = (cx >> 5, cz >> 5);
    let path = PathBuf::from(format!("{region_dir}/r.{rx}.{rz}.mca"));
    let region = Region::open(&path).expect("open").with_coords(rx, rz);
    let data = region.get_chunk(cx & 31, cz & 31).expect("get").expect("present");
    let nbt = read_nbt(&data).expect("nbt");
    let sections = match compound_get(&nbt.compound, "sections") {
        Some(Tag::List(List::Compound(l))) => l,
        _ => panic!("no sections"),
    };
    let mut logs = Vec::new();
    let mut leaves = Vec::new();
    for sec in sections {
        let y_sec = match compound_get(sec, "Y") {
            Some(Tag::Byte(y)) => *y as i8 as i32,
            Some(Tag::Int(y)) => *y,
            _ => continue,
        };
        let Some(Tag::Compound(bs)) = compound_get(sec, "block_states") else { continue; };
        let Some(Tag::List(List::Compound(palette))) = compound_get(bs, "palette") else { continue; };
        let names: Vec<String> = palette.iter().map(|pc| match compound_get(pc, "Name") {
            Some(Tag::String(s)) => s.to_string(), _ => "minecraft:air".into() }).collect();
        let nstates = names.len();
        if nstates == 1 {
            if names[0].contains("pale_oak") {
                for i in 0..4096u32 {
                    let ly = (i >> 8) as i32;
                    let lz = ((i >> 4) & 15) as u8;
                    let lx = (i & 15) as u8;
                    let p = (lx, y_sec * 16 + ly, lz);
                    if names[0].contains("log") { logs.push(p); } else { leaves.push(p); }
                }
            }
            continue;
        }
        let bits = ((nstates - 1).ilog2() + 1).max(4) as u32;
        let Some(Tag::LongArray(data)) = compound_get(bs, "data") else { continue; };
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
            if name.contains("pale_oak") {
                let p = (lx, y_sec * 16 + ly, lz);
                if name.contains("log") { logs.push(p); } else { leaves.push(p); }
            }
        }
    }
    println!("chunk ({cx},{cz}) pale_oak_logs: {} pale_oak_leaves: {}", logs.len(), leaves.len());
    // group logs by (x,z) column
    let mut cols: std::collections::BTreeMap<(u8, u8), Vec<i32>> = Default::default();
    for (x, y, z) in &logs {
        cols.entry((*x, *z)).or_default().push(*y);
    }
    for ((x, z), mut ys) in cols {
        ys.sort();
        println!("log column ({x},{z}) y={ys:?}");
    }
}
