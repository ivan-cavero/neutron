// Dump vanilla deep_dark quart coordinates for the 3x3 around a chunk.
use neutron_world::nbt::ussr_nbt::owned::{List, Tag};
use neutron_world::nbt::{compound_get, read_nbt};
use neutron_world::Region;
fn main() {
    let path = std::path::Path::new(
        "tools/nbt-ref/vanilla-fresh-12345/world/dimensions/minecraft/overworld/region/r.0.-1.mca",
    );
    let region = Region::open(path).unwrap().with_coords(0, -1);
    let mut out = String::new();
    for cz in -3i32..=-1 {
        for cx in 5i32..=7 {
            let data = region
                .get_chunk(cx.rem_euclid(32), cz.rem_euclid(32))
                .unwrap()
                .unwrap();
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
                let Some(Tag::Compound(bio)) = compound_get(sec, "biomes") else {
                    continue;
                };
                let Some(Tag::List(List::String(pal))) = compound_get(bio, "palette") else {
                    continue;
                };
                let dd: Vec<usize> = pal
                    .iter()
                    .enumerate()
                    .filter(|(_, s)| s.to_string() == "minecraft:deep_dark")
                    .map(|(i, _)| i)
                    .collect();
                if dd.is_empty() {
                    continue;
                }
                if pal.len() == 1 {
                    for qy in 0..4i32 {
                        for qz in 0..4i32 {
                            for qx in 0..4i32 {
                                out.push_str(&format!(
                                    "{} {} {}\n",
                                    cx * 4 + qx,
                                    y_sec * 4 + qy,
                                    cz * 4 + qz
                                ));
                            }
                        }
                    }
                    continue;
                }
                let bits = ((pal.len() - 1).ilog2() + 1) as u32;
                let Some(Tag::LongArray(data)) = compound_get(bio, "data") else {
                    continue;
                };
                let longs: Vec<i64> = data.to_vec();
                let epl = 64 / bits;
                let mask = (1u64 << bits) - 1;
                for i in 0..64u32 {
                    let li = (i / epl) as usize;
                    let bo = (i % epl) * bits;
                    let idx = ((longs[li] as u64) >> bo) & mask;
                    if dd.contains(&(idx as usize)) {
                        let qy = (i / 16) as i32;
                        let qz = ((i % 16) / 4) as i32;
                        let qx = (i % 4) as i32;
                        out.push_str(&format!(
                            "{} {} {}\n",
                            cx * 4 + qx,
                            y_sec * 4 + qy,
                            cz * 4 + qz
                        ));
                    }
                }
            }
        }
    }
    std::fs::write("tools/worldgen-probe/deep-dark-quarts.txt", out).unwrap();
    println!("written quarts");
}
