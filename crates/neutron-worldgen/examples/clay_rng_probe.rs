// Probe: brute-force global index for lush_caves_clay by overlap with vanilla clay xz.
use neutron_world::nbt::ussr_nbt::owned::{List, Tag};
use neutron_world::nbt::{compound_get, read_nbt};
use neutron_world::Region;
use neutron_worldgen::feature_rng::FeatureRandom;
use std::collections::HashSet;
use std::path::PathBuf;

fn main() {
    let mut args = std::env::args().skip(1);
    let seed: i64 = args.next().and_then(|s| s.parse().ok()).unwrap_or(424242);
    let cx: i32 = args.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    let cz: i32 = args.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    let ox0 = cx * 16;
    let oz0 = cz * 16;
    // vanilla clay x,z
    let dir = "tools/nbt-ref/vanilla-fresh-424242/world/dimensions/minecraft/overworld/region";
    let (rx, rz) = (cx >> 5, cz >> 5);
    let path = PathBuf::from(format!("{dir}/r.{rx}.{rz}.mca"));
    let mut van_xz: HashSet<(i32,i32)> = HashSet::new();
    if let Ok(region) = Region::open(&path) {
        let region = region.with_coords(rx, rz);
        if let Ok(Some(data)) = region.get_chunk(cx & 31, cz & 31) {
            if let Ok(nbt) = read_nbt(&data) {
                if let Some(Tag::List(List::Compound(sections))) = compound_get(&nbt.compound, "sections") {
                    for sec in sections {
                        let y_sec = match compound_get(sec, "Y") {
                            Some(Tag::Byte(y)) => *y as i8 as i32,
                            Some(Tag::Int(y)) => *y,
                            _ => continue,
                        };
                        let Some(Tag::Compound(bs)) = compound_get(sec, "block_states") else { continue };
                        let Some(Tag::List(List::Compound(palette))) = compound_get(bs, "palette") else { continue };
                        let names: Vec<String> = palette.iter().map(|pc| match compound_get(pc, "Name") {
                            Some(Tag::String(s)) => s.to_string(), _ => "minecraft:air".into()
                        }).collect();
                        let clay_idx: Vec<usize> = names.iter().enumerate().filter(|(_,n)| n.as_str() == "minecraft:clay").map(|(i,_)| i).collect();
                        if clay_idx.is_empty() { continue; }
                        if names.len() == 1 { for i in 0..4096u32 { let lx=(i&15) as i32; let lz=((i>>4)&15) as i32; van_xz.insert((lx+ox0, lz+oz0)); } continue; }
                        let bits = ((names.len()-1).ilog2()+1).max(4) as u32;
                        if let Some(Tag::LongArray(data)) = compound_get(bs, "data") {
                            let longs: Vec<i64> = data.to_vec();
                            let epl = 64/bits; let mask = (1u64<<bits)-1;
                            for i in 0..4096u32 {
                                let li=(i/epl) as usize; let bo=(i%epl)*bits;
                                let idx=((longs[li] as u64)>>bo)&mask;
                                if clay_idx.contains(&(idx as usize)) { let lx=(i&15) as i32; let lz=((i>>4)&15) as i32; van_xz.insert((lx+ox0, lz+oz0)); }
                            }
                        }
                    }
                }
            }
        }
    }
    println!("vanilla clay xz count: {}", van_xz.len());
    let mut rng = FeatureRandom::new(seed);
    let dec = rng.set_decoration_seed(seed, ox0, oz0);
    for gi in 20..40 {
        rng.set_feature_seed(dec, gi, 9);
        let mut cand: HashSet<(i32,i32)> = HashSet::new();
        for _ in 0..62 {
            let x = ox0 + rng.next_int(16);
            let z = oz0 + rng.next_int(16);
            let _y = rng.next_int(257);
            cand.insert((x, z));
        }
        let ov = cand.intersection(&van_xz).count();
        println!("idx {gi}: overlap {ov}/{}", cand.len());
    }
}
