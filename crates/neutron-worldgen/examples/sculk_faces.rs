use neutron_world::nbt::ussr_nbt::owned::{List, Tag};
use neutron_world::nbt::{compound_get, read_nbt};
use neutron_world::Region;
use neutron_worldgen::{generator::WORLD_BOTTOM, surface::BlockId, ChunkGenerator};
use std::path::PathBuf;
fn main() {
    let g = ChunkGenerator::new(12345);
    let ch = g.generate_chunk(6, -2);
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
    let mut van = vec![String::new(); 98304];
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
    let mut floor = 0u32;
    let mut ceil = 0u32;
    let mut wall = 0u32;
    let mut multi = 0u32;
    let mut wrong_floor = 0u32;
    let mut wrong_ceil = 0u32;
    let mut wrong_wall = 0u32;
    let mut wrong_multi = 0u32;
    let mut neu_sculk = 0u32;
    let mut match_c = 0u32;
    let mut over = 0u32;
    for y in WORLD_BOTTOM..320 {
        for z in 0..16i32 {
            for x in 0..16i32 {
                let idx = ((y - WORLD_BOTTOM) as usize) * 256 + (z as usize) * 16 + (x as usize);
                let nb = ch.block_at(x as u32, y, z as u32);
                let is_sc = matches!(nb, BlockId::Sculk | BlockId::SculkCatalyst);
                if !is_sc {
                    continue;
                }
                neu_sculk += 1;
                let s = van[idx].strip_prefix("minecraft:").unwrap_or(&van[idx]);
                let van_sc = s == "sculk" || s == "sculk_catalyst";
                // open dirs
                let up = matches!(
                    ch.block_at(x as u32, y + 1, z as u32),
                    BlockId::Air | BlockId::Water | BlockId::SculkVein
                );
                let dn = y > WORLD_BOTTOM
                    && matches!(
                        ch.block_at(x as u32, y - 1, z as u32),
                        BlockId::Air | BlockId::Water | BlockId::SculkVein
                    );
                let mut side = false;
                for (dx, dz) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
                    let nx = x + dx;
                    let nz = z + dz;
                    if nx < 0 || nx >= 16 || nz < 0 || nz >= 16 {
                        continue;
                    }
                    if matches!(
                        ch.block_at(nx as u32, y, nz as u32),
                        BlockId::Air | BlockId::Water | BlockId::SculkVein
                    ) {
                        side = true;
                    }
                }
                let nopen = (up as u32) + (dn as u32) + (side as u32);
                if van_sc {
                    match_c += 1;
                    if up && !dn && !side {
                        floor += 1;
                    } else if dn && !up && !side {
                        ceil += 1;
                    } else if side && !up && !dn {
                        wall += 1;
                    } else {
                        multi += 1;
                    }
                } else {
                    over += 1;
                    if up && !dn && !side {
                        wrong_floor += 1;
                    } else if dn && !up && !side {
                        wrong_ceil += 1;
                    } else if side && !up && !dn {
                        wrong_wall += 1;
                    } else {
                        wrong_multi += 1;
                    }
                }
                let _ = nopen;
            }
        }
    }
    println!("neu_sculk={neu_sculk} match={match_c} overpaint={over}");
    println!("match faces: floor_only={floor} ceil_only={ceil} side_only={wall} multi={multi}");
    println!("over  faces: floor_only={wrong_floor} ceil_only={wrong_ceil} side_only={wrong_wall} multi={wrong_multi}");
}
