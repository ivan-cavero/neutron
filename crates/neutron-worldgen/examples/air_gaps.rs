use neutron_world::nbt::ussr_nbt::owned::{List, Tag};
use neutron_world::nbt::{compound_get, read_nbt};
use neutron_world::Region;
use neutron_worldgen::surface::BlockId;
use neutron_worldgen::{generator::WORLD_BOTTOM, ChunkGenerator};
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
    let mut van = vec!["?".to_string(); 98304];
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
                van[idx] = name;
            }
        }
    }
    let gen = ChunkGenerator::new(12345);
    let chunk = gen.generate_chunk(6, -2);
    let mut air_gaps = Vec::new();
    let mut y_hist = [0u32; 24];
    for y in WORLD_BOTTOM..320 {
        for z in 0..16usize {
            for x in 0..16usize {
                let idx = ((y - WORLD_BOTTOM) as usize) * 256 + z * 16 + x;
                let nb = chunk.block_at(x as u32, y, z as u32);
                let neu_solid = !matches!(nb, BlockId::Air | BlockId::Water | BlockId::Lava);
                let vn = van[idx].strip_prefix("minecraft:").unwrap_or(&van[idx]);
                if neu_solid && (vn == "air" || vn == "cave_air") {
                    air_gaps.push((x as i32, y, z as i32));
                    let band = ((y - WORLD_BOTTOM) / 16) as usize;
                    if band < 24 {
                        y_hist[band] += 1;
                    }
                }
            }
        }
    }
    println!("pure_air_gaps={}", air_gaps.len());
    for (i, c) in y_hist.iter().enumerate() {
        if *c > 0 {
            let y0 = WORLD_BOTTOM + (i as i32) * 16;
            println!("  Y[{}..{}]={}", y0, y0 + 16, c);
        }
    }
    for g in air_gaps.iter().take(15) {
        println!("  air_gap {:?}", g);
    }
}
