use neutron_world::nbt::ussr_nbt::owned::{List, Tag};
use neutron_world::nbt::{compound_get, read_nbt};
use neutron_world::Region;
use neutron_worldgen::surface::BlockId;
use neutron_worldgen::{generator::WORLD_BOTTOM, ChunkGenerator};
use std::path::PathBuf;

fn solid_class(name: &str) -> u8 {
    let n = name.strip_prefix("minecraft:").unwrap_or(name);
    if n == "air" || n == "cave_air" || n == "void_air" {
        return 0;
    }
    if n == "water" || n == "lava" {
        return 1;
    }
    if n.contains("leaves")
        || n.contains("log")
        || n == "leaf_litter"
        || n.contains("sculk")
        || n == "short_grass"
        || n == "vine"
        || n == "glow_lichen"
    {
        return 2;
    } // veg
    3 // solid terrain
}

fn neu_name(b: BlockId) -> &'static str {
    match b {
        BlockId::Air => "minecraft:air",
        BlockId::Water => "minecraft:water",
        BlockId::Lava => "minecraft:lava",
        _ => "minecraft:stone",
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
        Some(Tag::List(List::Compound(list))) => list,
        _ => panic!(),
    };
    let mut van = vec![0u8; 98304];
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
        let get = |i: u32| -> String {
            if nstates == 1 {
                return names[0].clone();
            }
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
        for i in 0..4096u32 {
            let ly = (i >> 8) as i32;
            let lz = ((i >> 4) & 15) as usize;
            let lx = (i & 15) as usize;
            let y = y_sec * 16 + ly;
            let idx = ((y - WORLD_BOTTOM) as usize) * 256 + lz * 16 + lx;
            if idx < van.len() {
                van[idx] = solid_class(&get(i));
            }
        }
    }
    let gen = ChunkGenerator::new(12345);
    let chunk = gen.generate_chunk(6, -2);
    let mut match_sa = 0u32;
    let mut tot_sa = 0u32;
    let mut match_solid = 0u32;
    let mut tot_solid = 0u32;
    for y in WORLD_BOTTOM..320 {
        for z in 0..16usize {
            for x in 0..16usize {
                let idx = ((y - WORLD_BOTTOM) as usize) * 256 + z * 16 + x;
                let vc = van[idx];
                let nc = solid_class(neu_name(chunk.block_at(x as u32, y, z as u32)));
                // solid vs air (ignore veg class for both)
                let vs = if vc == 2 { 0 } else { vc }; // veg as air for shape? better: veg as solid
                let vs = if vc == 2 { 3 } else { vc };
                let ns = if nc == 2 { 3 } else { nc };
                if vs == 0 || vs == 1 || vs == 3 {
                    tot_sa += 1;
                    // map: air=0 fluid=1 solid=3
                    let vbin = if vs == 0 {
                        0
                    } else if vs == 1 {
                        1
                    } else {
                        2
                    };
                    let nbin = if ns == 0 {
                        0
                    } else if ns == 1 {
                        1
                    } else {
                        2
                    };
                    if vbin == nbin {
                        match_sa += 1;
                    }
                }
                if vs == 3 && ns == 3 {
                    match_solid += 1;
                }
                if vs == 3 || ns == 3 {
                    tot_solid += 1;
                }
            }
        }
    }
    println!(
        "air/fluid/solid shape match: {match_sa}/{tot_sa} ({:.2}%)",
        100.0 * match_sa as f64 / tot_sa as f64
    );
}
