// Scan vanilla region for andesite above Y=64.
use neutron_world::nbt::ussr_nbt::owned::{List, Tag};
use neutron_world::nbt::{compound_get, read_nbt};
use neutron_world::Region;
use std::path::PathBuf;

fn main() {
    let path = PathBuf::from(
        "tools/nbt-ref/vanilla1/world/dimensions/minecraft/overworld/region/r.0.-1.mca",
    );
    let region = Region::open(&path).unwrap().with_coords(0, -1);
    let mut high = 0u32;
    let mut low = 0u32;
    let mut chunks_high = 0u32;
    let mut chunks_seen = 0u32;
    for lcz in 0..32 {
        for lcx in 0..32 {
            let Some(data) = region.get_chunk(lcx, lcz).ok().flatten() else {
                continue;
            };
            let Ok(nbt) = read_nbt(&data) else {
                continue;
            };
            chunks_seen += 1;
            let sections = match compound_get(&nbt.compound, "sections") {
                Some(Tag::List(List::Compound(l))) => l,
                _ => continue,
            };
            let mut this_high = 0u32;
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
                let andesite = palette.iter().any(|pc| match compound_get(pc, "Name") {
                    Some(Tag::String(s)) => s.to_string() == "minecraft:andesite",
                    _ => false,
                });
                if !andesite {
                    continue;
                }
                // count via palette expansion if needed — approximate: if single-state section
                if palette.len() == 1 {
                    let n = 4096;
                    if y_sec * 16 >= 64 {
                        this_high += n;
                    } else {
                        low += n;
                    }
                    continue;
                }
                let bits = ((palette.len() - 1).ilog2() + 1).max(4) as u32;
                let Some(Tag::LongArray(data)) = compound_get(bs, "data") else {
                    continue;
                };
                let names: Vec<bool> = palette
                    .iter()
                    .map(|pc| match compound_get(pc, "Name") {
                        Some(Tag::String(s)) => s.to_string() == "minecraft:andesite",
                        _ => false,
                    })
                    .collect();
                let epl = 64 / bits;
                let mask = (1u64 << bits) - 1;
                let longs: Vec<i64> = data.to_vec();
                for i in 0..4096u32 {
                    let li = (i / epl) as usize;
                    let bo = (i % epl) * bits;
                    let idx = ((longs[li] as u64) >> bo) & mask;
                    if names.get(idx as usize).copied().unwrap_or(false) {
                        let y = y_sec * 16 + (i >> 8) as i32;
                        if y >= 64 {
                            this_high += 1;
                        } else {
                            low += 1;
                        }
                    }
                }
            }
            if this_high > 0 {
                chunks_high += 1;
                high += this_high;
                println!(
                    "chunk ({},{}) high_andesite={this_high}",
                    lcx,
                    lcz - 32
                );
            }
        }
    }
    println!("chunks_seen={chunks_seen} chunks_with_andesite_y>=64={chunks_high}");
    println!("andesite y>=64: {high}   y<64: {low}");
}
