// Dump vanilla section biomes at chunk (6,-2) around the andesite blob.
use neutron_world::nbt::ussr_nbt::owned::{List, Tag};
use neutron_world::nbt::{compound_get, read_nbt};
use neutron_world::Region;
use std::path::PathBuf;

fn main() {
    let path = PathBuf::from(std::env::args().nth(1).unwrap_or_else(|| {
        "tools/nbt-ref/vanilla1/world-r29/dimensions/minecraft/overworld/region/r.0.-1.mca"
            .to_string()
    }));
    let region = Region::open(&path).unwrap().with_coords(0, -1);
    let data = region.get_chunk(6, 30).unwrap().unwrap();
    let nbt = read_nbt(&data).unwrap();
    let sections = match compound_get(&nbt.compound, "sections") {
        Some(Tag::List(List::Compound(l))) => l,
        _ => panic!("no sections"),
    };
    for sec in sections {
        let y_sec = match compound_get(sec, "Y") {
            Some(Tag::Byte(y)) => *y as i8 as i32,
            Some(Tag::Int(y)) => *y,
            _ => continue,
        };
        if y_sec < 4 || y_sec > 8 {
            continue;
        }
        let Some(Tag::Compound(biomes)) = compound_get(sec, "biomes") else {
            println!("section Y={y_sec} no biomes");
            continue;
        };
        let palette: Vec<String> = match compound_get(biomes, "palette") {
            Some(Tag::List(List::String(s))) => s.iter().map(|x| x.to_string()).collect(),
            Some(Tag::List(List::Compound(cs))) => cs
                .iter()
                .map(|c| match compound_get(c, "Name") {
                    Some(Tag::String(n)) => n.to_string(),
                    _ => "?".into(),
                })
                .collect(),
            other => {
                println!("section Y={y_sec} palette={other:?}");
                continue;
            }
        };
        println!("section Y={y_sec} palette={palette:?}");
        if let Some(Tag::LongArray(data)) = compound_get(biomes, "data") {
            let n = palette.len();
            let bits = if n <= 1 {
                0
            } else {
                ((n - 1).ilog2() + 1) as u32
            };
            println!("  bits={bits} longs={}", data.len());
            if bits == 0 {
                continue;
            }
            let epl = 64 / bits;
            let mask = (1u64 << bits) - 1;
            let longs: Vec<i64> = data.to_vec();
            // 4x4x4 = 64 entries, index = qy*16 + qz*4 + qx
            for qy in 0..4 {
                for qz in 0..4 {
                    for qx in 0..4 {
                        let i = (qy * 16 + qz * 4 + qx) as u32;
                        let li = (i / epl) as usize;
                        let bo = (i % epl) * bits;
                        let idx = ((longs[li] as u64) >> bo) & mask;
                        let name = palette.get(idx as usize).cloned().unwrap_or("?".into());
                        if qx == 2 && qz == 1 {
                            println!("  quart({qx},{qy},{qz}) i={i} -> {name}");
                        }
                    }
                }
            }
        } else {
            println!("  single biome (no data)");
        }
    }
}
