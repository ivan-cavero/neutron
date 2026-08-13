use neutron_world::nbt::ussr_nbt::owned::{List, Tag};
use neutron_world::nbt::{compound_get, read_nbt};
use neutron_world::Region;
use neutron_worldgen::{
    biome_source::{biome_id, climate_at_block, find_biome},
    density::DensityEnv,
    generator::WORLD_BOTTOM,
    surface::BlockId,
    ChunkGenerator,
};
use std::path::PathBuf;
fn is_solid(b: BlockId) -> bool {
    !matches!(
        b,
        BlockId::Air
            | BlockId::Water
            | BlockId::Lava
            | BlockId::Sculk
            | BlockId::SculkVein
            | BlockId::SculkCatalyst
            | BlockId::ShortGrass
            | BlockId::OakLeaves
            | BlockId::Snow
            | BlockId::PowderSnow
    )
}
fn main() {
    let g = ChunkGenerator::new(12345);
    let st = &g.state;
    // disable path: generate then count wall cells - but sculk is on
    let ch = g.generate_chunk(6, -2);
    let mut wall = 0u32;
    let mut sculk_wall = 0u32;
    let dirs = [
        (0i32, -1, 0),
        (0, 1, 0),
        (0, 0, -1),
        (0, 0, 1),
        (-1, 0, 0),
        (1, 0, 0),
    ];
    for y in WORLD_BOTTOM..320 {
        for z in 0..16i32 {
            for x in 0..16i32 {
                let wx = 96 + x;
                let wz = -32 + z;
                let mut env = DensityEnv::new(wx, y, wz, st.noises.noises());
                let climate = climate_at_block(
                    &mut env,
                    &st.router.temperature,
                    &st.router.vegetation,
                    &st.router.continents,
                    &st.router.erosion,
                    &st.router.depth,
                    &st.router.ridges,
                );
                if find_biome(&climate) != biome_id::DEEP_DARK {
                    continue;
                }
                let b = ch.block_at(x as u32, y, z as u32);
                // count solids (or sculk) with air neighbor
                let solidish = is_solid(b) || matches!(b, BlockId::Sculk | BlockId::SculkCatalyst);
                if !solidish {
                    continue;
                }
                let mut has_open = false;
                for (dx, dy, dz) in dirs {
                    let nx = x + dx;
                    let nz = z + dz;
                    let ny = y + dy;
                    if nx < 0 || nx >= 16 || nz < 0 || nz >= 16 || ny < WORLD_BOTTOM || ny >= 320 {
                        continue;
                    }
                    let nb = ch.block_at(nx as u32, ny, nz as u32);
                    if matches!(nb, BlockId::Air | BlockId::Water | BlockId::SculkVein) {
                        has_open = true;
                        break;
                    }
                }
                if has_open {
                    wall += 1;
                    if matches!(b, BlockId::Sculk | BlockId::SculkCatalyst) {
                        sculk_wall += 1;
                    }
                }
            }
        }
    }
    // vanilla sculk count
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
    let mut van_sculk = 0u32;
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
            if name.contains("sculk") && !name.contains("vein") {
                // sculk + catalyst + sensor etc
                if name.ends_with(":sculk") || name.contains("catalyst") {
                    van_sculk += 1;
                }
            }
        }
        let _ = y_sec;
    }
    // recount van sculk properly with y
    let mut van_sculk2 = 0u32;
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
            if idx < van_name.len() {
                van_name[idx] = name;
            }
        }
    }
    for n in &van_name {
        let s = n.strip_prefix("minecraft:").unwrap_or(n);
        if s == "sculk" || s == "sculk_catalyst" {
            van_sculk2 += 1;
        }
    }
    // overlap: van sculk where neu is solid wall
    let mut match_pos = 0u32;
    let mut van_only = 0u32;
    for y in WORLD_BOTTOM..320 {
        for z in 0..16usize {
            for x in 0..16usize {
                let idx = ((y - WORLD_BOTTOM) as usize) * 256 + z * 16 + x;
                let vn = &van_name[idx];
                let s = vn.strip_prefix("minecraft:").unwrap_or(vn);
                if s != "sculk" && s != "sculk_catalyst" {
                    continue;
                }
                let nb = ch.block_at(x as u32, y, z as u32);
                if matches!(nb, BlockId::Sculk | BlockId::SculkCatalyst) {
                    match_pos += 1;
                } else {
                    van_only += 1;
                }
            }
        }
    }
    println!("neu deep_dark cave-wall solids+sculk={wall} of which sculk={sculk_wall}");
    println!("van sculk+catalyst={van_sculk2}");
    println!("overlap match={match_pos} van_missing_in_neu={van_only}");
}
