use neutron_world::nbt::ussr_nbt::owned::{List, Tag};
use neutron_world::nbt::{compound_get, read_nbt};
use neutron_world::Region;
use neutron_worldgen::surface::BlockId;
use neutron_worldgen::{generator::WORLD_BOTTOM, ChunkGenerator};
use std::path::PathBuf;

/// Density-phase occupancy: air/fluid/solid, treating feature-fill as air.
fn dens_class_name(n: &str) -> u8 {
    let n = n.strip_prefix("minecraft:").unwrap_or(n);
    if n == "air" || n == "cave_air" || n == "void_air" {
        return 0;
    }
    if n == "water" || n == "lava" {
        return 1;
    }
    // Features placed into caves / on surface after density:
    if n.contains("sculk")
        || n.contains("leaves")
        || n.contains("log")
        || n.contains("wood")
        || n == "leaf_litter"
        || n == "vine"
        || n == "glow_lichen"
        || n == "short_grass"
        || n == "fern"
        || n.contains("mushroom")
        || n == "moss_carpet"
        || n.contains("azalea")
        || n == "spore_blossom"
        || n == "hanging_roots"
        || n.contains("dripleaf")
        || n == "cave_vines"
        || n == "cave_vines_plant"
        || n == "big_dripleaf"
        || n == "small_dripleaf"
        || n == "rooted_dirt"
        || n == "moss_block"
    {
        return 0; // treat as density-air (feature fill)
    }
    2 // solid from density/surface/ores
}

fn is_pure_air(n: &str) -> bool {
    let n = n.strip_prefix("minecraft:").unwrap_or(n);
    n == "air" || n == "cave_air" || n == "void_air"
}

fn dens_class_neu(b: BlockId) -> u8 {
    match b {
        BlockId::Air
        | BlockId::Sculk
        | BlockId::SculkVein
        | BlockId::SculkCatalyst
        | BlockId::SculkSensor
        | BlockId::SculkShrieker
        | BlockId::MossBlock
        | BlockId::OakLeaves
        | BlockId::DarkOakLeaves
        | BlockId::ShortGrass
        | BlockId::LeafLitter
        | BlockId::OakLog
        | BlockId::DarkOakLog => 0, // feature-fill / density-air class
        BlockId::Water | BlockId::Lava => 1,
        _ => 2,
    }
}

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
    let mut van = vec![0u8; 98304];
    let mut van_name = vec![String::new(); 98304];
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
        for i in 0..4096u32 {
            let name = if nstates == 1 {
                names[0].clone()
            } else {
                let bits = ((nstates - 1).ilog2() + 1).max(4) as u32;
                let Tag::LongArray(data) = compound_get(bs, "data").unwrap() else {
                    panic!()
                };
                let longs: Vec<i64> = data.to_vec();
                let epl = 64 / bits;
                let mask = (1u64 << bits) - 1;
                let li = (i / epl) as usize;
                let bo = (i % epl) * bits;
                let idx = ((longs[li] as u64) >> bo) & mask;
                names[idx as usize].clone()
            };
            let ly = (i >> 8) as i32;
            let lz = ((i >> 4) & 15) as usize;
            let lx = (i & 15) as usize;
            let y = y_sec * 16 + ly;
            let idx = ((y - WORLD_BOTTOM) as usize) * 256 + lz * 16 + lx;
            if idx < van.len() {
                van[idx] = dens_class_name(&name);
                van_name[idx] = name;
            }
        }
    }
    let gen = ChunkGenerator::new(12345);
    let chunk = gen.generate_chunk(6, -2);
    let mut match_c = 0u32;
    let mut total = 0u32;
    let mut neu_extra_solid = 0u32;
    let mut neu_missing_solid = 0u32;
    let mut fluid_m = 0u32;
    let mut pure_air_extra = 0u32;
    let mut feature_fill_extra = 0u32;
    let mut samples = Vec::new();
    for y in WORLD_BOTTOM..320 {
        for z in 0..16usize {
            for x in 0..16usize {
                total += 1;
                let idx = ((y - WORLD_BOTTOM) as usize) * 256 + z * 16 + x;
                let vc = van[idx];
                let nc = dens_class_neu(chunk.block_at(x as u32, y, z as u32));
                if vc == nc {
                    match_c += 1;
                } else {
                    if nc == 2 && vc == 0 {
                        neu_extra_solid += 1;
                        if is_pure_air(&van_name[idx]) {
                            pure_air_extra += 1;
                        } else {
                            feature_fill_extra += 1;
                        }
                        if samples.len() < 20 {
                            samples.push((x as i32, y, z as i32, "extra_solid"));
                        }
                    }
                    if nc == 0 && vc == 2 {
                        neu_missing_solid += 1;
                        if samples.len() < 40 {
                            samples.push((x as i32, y, z as i32, "missing_solid"));
                        }
                    }
                    if (vc == 1) != (nc == 1) {
                        fluid_m += 1;
                    }
                }
            }
        }
    }
    println!(
        "DENSITY-PHASE shape (feature-fill as air): {match_c}/{total} ({:.4}%)",
        100.0 * match_c as f64 / total as f64
    );
    println!(
        "neu_extra_solid={neu_extra_solid} (pure_air={pure_air_extra} feature_fill={feature_fill_extra}) neu_missing_solid={neu_missing_solid} fluid_m={fluid_m}"
    );
    println!("mismatches total={}", total - match_c);
    println!("note: R10 noodle-interp; residual feature_fill=sculk — see runs/run-016.md");
    for s in samples.iter().take(25) {
        println!("  sample {:?}", s);
    }
}
