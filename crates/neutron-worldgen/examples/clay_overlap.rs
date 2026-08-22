// Probe: place lush_caves_clay in isolation and compare clay positions with vanilla.
// Usage: clay_overlap [seed] [cx] [cz]
use neutron_world::nbt::ussr_nbt::owned::{List, Tag};
use neutron_world::nbt::{compound_get, read_nbt};
use neutron_world::Region;
use neutron_worldgen::feature_catalog;
use neutron_worldgen::feature_dispatch::place_placed_feature;
use neutron_worldgen::feature_rng::FeatureRandom;
use neutron_worldgen::surface::BlockId;
use neutron_worldgen::ChunkGenerator;
use std::collections::HashSet;
use std::path::PathBuf;

fn load_vanilla_clay(region_dir: &str, cx: i32, cz: i32) -> Option<HashSet<(i32, i32, i32)>> {
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
    let mut map = HashSet::new();
    for sec in sections {
        let y_sec = match compound_get(sec, "Y") {
            Some(Tag::Byte(y)) => *y as i8 as i32,
            Some(Tag::Int(y)) => *y,
            _ => continue,
        };
        let Some(Tag::Compound(bs)) = compound_get(sec, "block_states") else { continue };
        let Some(Tag::List(List::Compound(palette))) = compound_get(bs, "palette") else { continue };
        let names: Vec<String> = palette
            .iter()
            .map(|pc| match compound_get(pc, "Name") {
                Some(Tag::String(s)) => s.to_string(),
                _ => "minecraft:air".into(),
            })
            .collect();
        let clay_idx: Vec<usize> = names
            .iter()
            .enumerate()
            .filter(|(_, n)| n.as_str() == "minecraft:clay")
            .map(|(i, _)| i)
            .collect();
        if clay_idx.is_empty() {
            continue;
        }
        if names.len() == 1 {
            for i in 0..4096u32 {
                let ly = (i >> 8) as i32;
                map.insert(((i & 15) as i32, y_sec * 16 + ly, ((i >> 4) & 15) as i32));
            }
            continue;
        }
        let bits = ((names.len() - 1).ilog2() + 1).max(4) as u32;
        let Some(Tag::LongArray(data)) = compound_get(bs, "data") else { continue };
        let longs: Vec<i64> = data.to_vec();
        let epl = 64 / bits;
        let mask = (1u64 << bits) - 1;
        for i in 0..4096u32 {
            let li = (i / epl) as usize;
            let bo = (i % epl) * bits;
            let idx = ((longs[li] as u64) >> bo) & mask;
            if clay_idx.contains(&(idx as usize)) {
                let ly = (i >> 8) as i32;
                map.insert(((i & 15) as i32, y_sec * 16 + ly, ((i >> 4) & 15) as i32));
            }
        }
    }
    Some(map)
}


fn load_vanilla_count(region_dir: &str, cx: i32, cz: i32, want: &str) -> i64 {
    let (rx, rz) = (cx >> 5, cz >> 5);
    let path = PathBuf::from(format!("{region_dir}/r.{rx}.{rz}.mca"));
    let Ok(region) = Region::open(&path) else { return 0 };
    let region = region.with_coords(rx, rz);
    let Ok(Some(data)) = region.get_chunk(cx & 31, cz & 31) else { return 0 };
    let Ok(nbt) = read_nbt(&data) else { return 0 };
    let Some(Tag::List(List::Compound(sections))) = compound_get(&nbt.compound, "sections") else { return 0 };
    let mut count = 0i64;
    for sec in sections {
        let y_sec = match compound_get(sec, "Y") { Some(Tag::Byte(y)) => *y as i8 as i32, Some(Tag::Int(y)) => *y, _ => continue };
        let Some(Tag::Compound(bs)) = compound_get(sec, "block_states") else { continue };
        let Some(Tag::List(List::Compound(palette))) = compound_get(bs, "palette") else { continue };
        let names: Vec<String> = palette.iter().map(|pc| match compound_get(pc, "Name") { Some(Tag::String(s)) => s.to_string(), _ => "minecraft:air".into() }).collect();
        let idxs: Vec<usize> = names.iter().enumerate().filter(|(_,n)| n.as_str() == want).map(|(i,_)| i).collect();
        if idxs.is_empty() { continue; }
        if names.len() == 1 { count += 4096; continue; }
        let bits = ((names.len()-1).ilog2()+1).max(4) as u32;
        let Some(Tag::LongArray(data)) = compound_get(bs, "data") else { continue };
        let longs: Vec<i64> = data.to_vec();
        let epl = 64/bits; let mask = (1u64<<bits)-1;
        for i in 0..4096u32 {
            let li=(i/epl) as usize; let bo=(i%epl)*bits;
            let idx=((longs[li] as u64)>>bo)&mask;
            if idxs.contains(&(idx as usize)) { count += 1; }
        }
    }
    count
}

fn main() {
    let mut args = std::env::args().skip(1);
    let seed: i64 = args.next().and_then(|s| s.parse().ok()).unwrap_or(424242);
    let cx: i32 = args.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    let cz: i32 = args.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    let dir = "tools/nbt-ref/vanilla-fresh-424242/world/dimensions/minecraft/overworld/region";

    let gen = ChunkGenerator::new(seed);
    let mut region = gen.generate_ores_region(cx, cz);
    let state = &gen.state;
    let gi = feature_catalog::global_feature_index(9, "minecraft:lush_caves_clay").expect("idx");
    let ox0 = cx * 16;
    let oz0 = cz * 16;
    let mut rng = FeatureRandom::new(seed);
    let dec = rng.set_decoration_seed(seed, ox0, oz0);
    rng.set_feature_seed(dec, gi, 9);
    place_placed_feature(&mut rng, &mut region, state, ox0, oz0, "minecraft:lush_caves_clay");

    let mut iso: HashSet<(i32, i32, i32)> = HashSet::new();
    for y in -64..320 {
        for z in 0..16 {
            for x in 0..16 {
                if region.get(ox0 + x, y, oz0 + z) == BlockId::Clay {
                    iso.insert((x, y, z));
                }
            }
        }
    }
    let van = load_vanilla_clay(dir, cx, cz).unwrap_or_default();
    let overlap_full = iso.intersection(&van).count();
    let iso_xz: HashSet<(i32, i32)> = iso.iter().map(|(x, _, z)| (*x, *z)).collect();
    let van_xz: HashSet<(i32, i32)> = van.iter().map(|(x, _, z)| (*x, *z)).collect();
    let overlap_xz = iso_xz.intersection(&van_xz).count();
    println!(
        "iso clay {}/vanilla {} : full overlap {overlap_full}, xz overlap {overlap_xz}/{}",
        iso.len(),
        van.len(),
        iso_xz.len()
    );
    // full generation pale oak + clay counts
    let full = gen.generate_chunk(cx, cz);
    let mut n_leaves = 0; let mut n_log = 0; let mut n_clay = 0;
    for y in -64..320 { for z in 0..16u32 { for x in 0..16u32 {
        let b = full.block_at(x, y, z);
        if b == BlockId::PaleOakLeaves { n_leaves += 1; }
        if b == BlockId::PaleOakLog { n_log += 1; }
        if b == BlockId::Clay { n_clay += 1; }
    } } }
    println!("full generate_chunk: pale_oak_leaves={n_leaves} pale_oak_log={n_log} clay={n_clay}");
    // vanilla counts
    let van_leaves = load_vanilla_count(dir, cx, cz, "minecraft:pale_oak_leaves");
    let van_log = load_vanilla_count(dir, cx, cz, "minecraft:pale_oak_log");
    let van_clay = load_vanilla_count(dir, cx, cz, "minecraft:clay");
    println!("vanilla: pale_oak_leaves={van_leaves} pale_oak_log={van_log} clay={van_clay}");
}

