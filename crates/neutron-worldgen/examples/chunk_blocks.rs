use neutron_world::nbt::ussr_nbt::owned::{List, Tag};
use neutron_world::nbt::{compound_get, read_nbt};
use neutron_world::Region;
use std::collections::HashMap;
use std::path::PathBuf;

fn main() {
    let path = PathBuf::from(
        "tools/nbt-ref/vanilla1/world/dimensions/minecraft/overworld/region/r.0.-1.mca",
    );
    let region = Region::open(&path).unwrap().with_coords(0, -1);
    let data = region.get_chunk(6, 30).unwrap().unwrap();
    let nbt = read_nbt(&data).unwrap();
    let sections = match compound_get(&nbt.compound, "sections") {
        Some(Tag::List(List::Compound(l))) => l,
        _ => panic!(),
    };
    let mut hist: HashMap<String, u32> = HashMap::new();
    let mut deep_hist: HashMap<String, u32> = HashMap::new();
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
        // approximate counts: if single state, 4096 of that name
        if names.len() == 1 {
            *hist.entry(names[0].clone()).or_default() += 4096;
            if y_sec <= 0 {
                *deep_hist.entry(names[0].clone()).or_default() += 4096;
            }
        } else {
            let bits = ((names.len() - 1).ilog2() + 1).max(4) as u32;
            let Tag::LongArray(data) = compound_get(bs, "data").unwrap() else {
                panic!()
            };
            let longs: Vec<i64> = data.to_vec();
            let epl = 64 / bits;
            let mask = (1u64 << bits) - 1;
            for i in 0..4096u32 {
                let li = (i / epl) as usize;
                let bo = (i % epl) * bits;
                let idx = ((longs[li] as u64) >> bo) & mask;
                let n = names[idx as usize].clone();
                *hist.entry(n.clone()).or_default() += 1;
                if y_sec <= 0 {
                    *deep_hist.entry(n).or_default() += 1;
                }
            }
        }
    }
    println!("=== blocks containing city/sculk/brick keywords ===");
    let mut v: Vec<_> = hist.into_iter().collect();
    v.sort_by(|a, b| b.1.cmp(&a.1));
    for (n, c) in &v {
        if n.contains("sculk")
            || n.contains("deepslate_brick")
            || n.contains("deepslate_tile")
            || n.contains("wool")
            || n.contains("candle")
            || n.contains("soul")
            || n.contains("chest")
            || n.contains("ancient")
            || n.contains("reinforced")
            || n.contains("mineshaft")
            || n.contains("plank")
            || n.contains("cobweb")
            || n.contains("rail")
            || n.contains("fence")
        {
            println!("  {c:5} {n}");
        }
    }
    println!("=== top deep (Y section <=0) blocks ===");
    let mut d: Vec<_> = deep_hist.into_iter().collect();
    d.sort_by(|a, b| b.1.cmp(&a.1));
    for (n, c) in d.iter().take(25) {
        println!("  {c:5} {n}");
    }
}
