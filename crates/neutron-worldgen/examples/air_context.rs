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
    // For pure air gaps, check neighbors in vanilla for sculk (feature cave) vs pure stone surroundings
    let gen = ChunkGenerator::new(12345);
    let chunk = gen.generate_chunk(6, -2);
    let mut near_sculk = 0u32;
    let mut near_air_only = 0u32;
    for y in WORLD_BOTTOM..320 {
        for z in 0..16usize {
            for x in 0..16usize {
                let idx = ((y - WORLD_BOTTOM) as usize) * 256 + z * 16 + x;
                let nb = chunk.block_at(x as u32, y, z as u32);
                let neu_solid = !matches!(nb, BlockId::Air | BlockId::Water | BlockId::Lava);
                let vn = van[idx].strip_prefix("minecraft:").unwrap_or(&van[idx]);
                if !(neu_solid && (vn == "air" || vn == "cave_air")) {
                    continue;
                }
                // check 6-neighborhood for sculk
                let mut has_sculk = false;
                for (dx, dy, dz) in [
                    (1, 0, 0),
                    (-1, 0, 0),
                    (0, 1, 0),
                    (0, -1, 0),
                    (0, 0, 1),
                    (0, 0, -1),
                ] {
                    let nx = x as i32 + dx;
                    let ny = y + dy;
                    let nz = z as i32 + dz;
                    if nx < 0 || nx > 15 || nz < 0 || nz > 15 || ny < WORLD_BOTTOM || ny >= 320 {
                        continue;
                    }
                    let nidx =
                        ((ny - WORLD_BOTTOM) as usize) * 256 + (nz as usize) * 16 + (nx as usize);
                    if van[nidx].contains("sculk") {
                        has_sculk = true;
                    }
                }
                if has_sculk {
                    near_sculk += 1;
                } else {
                    near_air_only += 1;
                }
            }
        }
    }
    println!("pure_air near_sculk={near_sculk} isolated_or_air_cluster={near_air_only}");
}
