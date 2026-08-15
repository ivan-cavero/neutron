// Determinism boundary analysis: for two vanilla runs of the SAME chunk,
// histogram the diffs by distance to the chunk border.
// If cross-border feature interference is the nondeterminism source, diffs
// cluster within canopy radius (~5) of the border and the chunk core is exact.
// Usage: border_analysis <regionA.mca> <regionB.mca> [cxl] [czl]
use neutron_world::nbt::ussr_nbt::owned::{List, Tag};
use neutron_world::nbt::{compound_get, read_nbt};
use neutron_world::Region;
use std::collections::HashMap;
use std::path::PathBuf;

fn load(path: &str, cxl: i32, czl: i32) -> HashMap<(u8, i32, u8), String> {
    let region = Region::open(&PathBuf::from(path)).unwrap().with_coords(0, -1);
    let data = region.get_chunk(cxl, czl).unwrap().unwrap();
    let nbt = read_nbt(&data).unwrap();
    let sections = match compound_get(&nbt.compound, "sections") {
        Some(Tag::List(List::Compound(l))) => l,
        _ => panic!("no sections"),
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
    map
}

fn main() {
    let mut args = std::env::args().skip(1);
    let a_path = args.next().expect("regionA");
    let b_path = args.next().expect("regionB");
    let cxl: i32 = args.next().and_then(|s| s.parse().ok()).unwrap_or(6);
    let czl: i32 = args.next().and_then(|s| s.parse().ok()).unwrap_or(30);
    let a = load(&a_path, cxl, czl);
    let b = load(&b_path, cxl, czl);
    let mut by_dist: HashMap<i32, usize> = HashMap::new();
    let mut total = 0usize;
    for (k, va) in &a {
        if let Some(vb) = b.get(k) {
            if vb != va {
                let (x, _, z) = k;
                let d = (*x as i32).min(15 - *x as i32).min(*z as i32).min(15 - *z as i32);
                *by_dist.entry(d).or_insert(0) += 1;
                total += 1;
            }
        }
    }
    println!("total diffs: {total}");
    for d in 0..8 {
        println!("border_dist={d}: {}", by_dist.get(&d).copied().unwrap_or(0));
    }
    let core: usize = (5..8).map(|d| by_dist.get(&d).copied().unwrap_or(0)).sum();
    println!("core (dist>=5): {core}");
}
