// Probe: place lush_caves_clay (global idx 29, step 9) into the ores region for
// a chunk and print surviving positions + clay count. Compare with vanilla clay.
// Usage: clay_probe [seed] [cx] [cz]
use neutron_worldgen::feature_catalog;
use neutron_worldgen::feature_dispatch::place_placed_feature;
use neutron_worldgen::feature_rng::FeatureRandom;
use neutron_worldgen::surface::BlockId;
use neutron_worldgen::ChunkGenerator;

fn main() {
    let mut args = std::env::args().skip(1);
    let seed: i64 = args.next().and_then(|s| s.parse().ok()).unwrap_or(424242);
    let cx: i32 = args.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    let cz: i32 = args.next().and_then(|s| s.parse().ok()).unwrap_or(0);

    let gen = ChunkGenerator::new(seed);
    let mut region = gen.generate_ores_region(cx, cz);
    let state = &gen.state;

    let placed_id = "minecraft:lush_caves_clay";
    let global_index = feature_catalog::global_feature_index(9, placed_id).expect("index");
    println!("lush_caves_clay global idx = {global_index}");
    let ox0 = cx * 16;
    let oz0 = cz * 16;
    let mut rng = FeatureRandom::new(seed);
    let dec = rng.set_decoration_seed(seed, ox0, oz0);
    println!("decoration seed = {dec}");
    rng.set_feature_seed(dec, global_index, 9);
    place_placed_feature(&mut rng, &mut region, state, ox0, oz0, placed_id);

    // Count clay in this chunk's 16x16 column.
    let mut clay = 0;
    for y in -64..320 {
        for z in 0..16 {
            for x in 0..16 {
                if region.get(ox0 + x, y, oz0 + z) == BlockId::Clay {
                    clay += 1;
                }
            }
        }
    }
    println!("clay placed by isolated lush_caves_clay in chunk ({cx},{cz}) = {clay}");
    use std::collections::HashSet;
    let mut iso_pos: HashSet<(i32,i32,i32)> = HashSet::new();
    for y in -64..320 {
        for z in 0..16 {
            for x in 0..16 {
                if region.get(ox0 + x, y, oz0 + z) == BlockId::Clay {
                    iso_pos.insert((x, y, z));
                }
            }
        }
    }
    println!("iso clay first pos: {:?}", iso_pos.iter().take(8).collect::<Vec<_>>());

    // Full generation clay count.
    let full = gen.generate_chunk(cx, cz);
    let mut full_clay = 0;
    for y in -64..320 {
        for z in 0..16u32 {
            for x in 0..16u32 {
                if full.block_at(x, y, z) == BlockId::Clay {
                    full_clay += 1;
                }
            }
        }
    }
    println!("clay in full generate_chunk ({cx},{cz}) = {full_clay}");

    // Vanilla clay count in this chunk.
    let dir = "tools/nbt-ref/vanilla-fresh-424242/world/dimensions/minecraft/overworld/region";
    use neutron_world::nbt::ussr_nbt::owned::{List, Tag};
    use neutron_world::nbt::{compound_get, read_nbt};
    use neutron_world::Region;
    use std::path::PathBuf;
    let (rx, rz) = (cx >> 5, cz >> 5);
    let path = PathBuf::from(format!("{dir}/r.{rx}.{rz}.mca"));
    if let Ok(region) = Region::open(&path) {
        let region = region.with_coords(rx, rz);
        if let Ok(Some(data)) = region.get_chunk(cx & 31, cz & 31) {
            if let Ok(nbt) = read_nbt(&data) {
                let mut van_clay = 0;
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
                        if names.len() == 1 {
                            for i in 0..4096u32 { van_clay += 1; }
                            continue;
                        }
                        let bits = ((names.len()-1).ilog2()+1).max(4) as u32;
                        if let Some(Tag::LongArray(data)) = compound_get(bs, "data") {
                            let longs: Vec<i64> = data.to_vec();
                            let epl = 64/bits; let mask = (1u64<<bits)-1;
                            for i in 0..4096u32 {
                                let li=(i/epl) as usize; let bo=(i%epl)*bits;
                                let idx=((longs[li] as u64)>>bo)&mask;
                                if clay_idx.contains(&(idx as usize)) { van_clay += 1; }
                            }
                        }
                    }
                println!("clay in vanilla chunk ({cx},{cz}) = {van_clay}");
                    // collect vanilla clay positions (local)
                    let mut van_pos: HashSet<(i32,i32,i32)> = HashSet::new();
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
                    if names.len() == 1 { for i in 0..4096u32 { let ly=(i>>8) as i32; let lz=((i>>4)&15) as u8; let lx=(i&15) as u8; van_pos.insert((lx as i32, y_sec*16+ly, lz as i32)); } continue; }
                    let bits = ((names.len()-1).ilog2()+1).max(4) as u32;
                    if let Some(Tag::LongArray(data)) = compound_get(bs, "data") {
                        let longs: Vec<i64> = data.to_vec();
                        let epl = 64/bits; let mask = (1u64<<bits)-1;
                        for i in 0..4096u32 {
                            let li=(i/epl) as usize; let bo=(i%epl)*bits;
                            let idx=((longs[li] as u64)>>bo)&mask;
                            if clay_idx.contains(&(idx as usize)) { let ly=(i>>8) as i32; let lz=((i>>4)&15) as u8; let lx=(i&15) as u8; van_pos.insert((lx as i32, y_sec*16+ly, lz as i32)); }
                        }
                    }
                }
                    let overlap = iso_pos.intersection(&van_pos).count();
                    println!("overlap iso vs vanilla clay = {overlap} / {} iso, {} van", iso_pos.len(), van_pos.len());
                }
            }
        }
    } else {
        println!("no vanilla region");
    }
}