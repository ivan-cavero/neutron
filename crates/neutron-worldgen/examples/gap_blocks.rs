//! Print vanilla block names at density_shape mismatch samples for chunk (6,-2).
use neutron_world::nbt::ussr_nbt::owned::{List, Tag};
use neutron_world::nbt::{compound_get, read_nbt};
use neutron_world::Region;
use neutron_worldgen::surface::BlockId;
use neutron_worldgen::{generator::WORLD_BOTTOM, ChunkGenerator};
use std::collections::HashMap;
use std::path::PathBuf;

fn main() {
    let path = PathBuf::from(
        "tools/nbt-ref/vanilla1/world/dimensions/minecraft/overworld/region/r.0.-1.mca",
    );
    let region = Region::open(&path).unwrap().with_coords(0, -1);
    let data = region.get_chunk(6, 30).unwrap().unwrap(); // region z: chunk -2 → local 30 in r.0.-1
    let nbt = read_nbt(&data).unwrap();
    let sections = match compound_get(&nbt.compound, "sections") {
        Some(Tag::List(List::Compound(l))) => l,
        _ => panic!(),
    };

    let mut van_name = vec!["?".to_string(); 98304];
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
            if idx < van_name.len() {
                van_name[idx] = name;
            }
        }
    }

    let gen = ChunkGenerator::new(12345);
    let chunk = gen.generate_chunk(6, -2);

    let samples: &[(i32, i32, i32)] = &[
        (0, -47, 12),
        (0, -47, 13),
        (0, -46, 9),
        (1, -45, 8),
        (2, -44, 7),
        (4, -36, 8),
        (3, -24, 4),
        (1, -22, 1),
    ];
    println!("sample mismatches:");
    for &(x, y, z) in samples {
        let idx = ((y - WORLD_BOTTOM) as usize) * 256 + (z as usize) * 16 + (x as usize);
        let vn = &van_name[idx];
        let nb = chunk.block_at(x as u32, y, z as u32);
        println!("  ({x},{y},{z}) van={vn} neu={nb:?}");
    }

    // Histogram of vanilla block names where neu is solid and van is "density-air" class
    fn dens_air(n: &str) -> bool {
        let n = n.strip_prefix("minecraft:").unwrap_or(n);
        if n == "air" || n == "cave_air" || n == "void_air" {
            return true;
        }
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
            return true;
        }
        false
    }

    let mut hist: HashMap<String, u32> = HashMap::new();
    let mut total_extra = 0u32;
    for y in WORLD_BOTTOM..320 {
        for z in 0..16usize {
            for x in 0..16usize {
                let idx = ((y - WORLD_BOTTOM) as usize) * 256 + z * 16 + x;
                let nb = chunk.block_at(x as u32, y, z as u32);
                let neu_solid = !matches!(nb, BlockId::Air | BlockId::Water | BlockId::Lava);
                let vn = &van_name[idx];
                if neu_solid && dens_air(vn) {
                    total_extra += 1;
                    *hist.entry(vn.clone()).or_default() += 1;
                }
            }
        }
    }
    println!("\nextra_solid by vanilla block (total {total_extra}):");
    let mut v: Vec<_> = hist.into_iter().collect();
    v.sort_by(|a, b| b.1.cmp(&a.1));
    for (n, c) in v.iter().take(20) {
        println!("  {c:5} {n}");
    }
}
