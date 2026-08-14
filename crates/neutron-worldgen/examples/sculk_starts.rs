// Compare vanilla sculk/catalyst cells vs Neutron patch starts.
// cargo run -p neutron-worldgen --example sculk_starts --release

use neutron_world::nbt::ussr_nbt::owned::{List, Tag};
use neutron_world::nbt::{compound_get, read_nbt};
use neutron_world::Region;
use neutron_worldgen::feature_catalog::{self, step};
use neutron_worldgen::feature_rng::FeatureRandom;
use neutron_worldgen::generator::WORLD_BOTTOM;
use neutron_worldgen::{biome_source::biome_id_at_block, surface::BlockId, ChunkGenerator};
use std::path::PathBuf;

fn load_van() -> Vec<String> {
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
    let mut van = vec!["air".to_string(); 16 * 384 * 16];
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
        let names: Vec<String> = palette
            .iter()
            .map(|pc| match compound_get(pc, "Name") {
                Some(Tag::String(s)) => s
                    .to_string()
                    .strip_prefix("minecraft:")
                    .unwrap_or(&s.to_string())
                    .to_string(),
                _ => "air".into(),
            })
            .collect();
        let nstates = names.len();
        for i in 0..4096u32 {
            let name = if nstates == 1 {
                names[0].clone()
            } else {
                let bits = ((nstates - 1).ilog2() + 1).max(4) as u32;
                let Some(Tag::LongArray(data)) = compound_get(bs, "data") else {
                    continue;
                };
                let longs: Vec<i64> = data.to_vec();
                let epl = 64 / bits;
                let mask = (1u64 << bits) - 1;
                let li = (i / epl) as usize;
                let bo = (i % epl) * bits;
                names
                    .get((((longs[li] as u64) >> bo) & mask) as usize)
                    .cloned()
                    .unwrap_or_else(|| "air".into())
            };
            let ly = (i >> 8) as i32;
            let lz = ((i >> 4) & 15) as usize;
            let lx = (i & 15) as usize;
            let y = y_sec * 16 + ly;
            let idx = ((y - WORLD_BOTTOM) as usize) * 256 + lz * 16 + lx;
            van[idx] = name;
        }
    }
    van
}

fn idx(x: usize, y: i32, z: usize) -> usize {
    ((y - WORLD_BOTTOM) as usize) * 256 + z * 16 + x
}

fn main() {
    let van = load_van();
    println!("vanilla catalysts / shriekers / sensors in (6,-2):");
    for y in WORLD_BOTTOM..320 {
        for z in 0..16usize {
            for x in 0..16usize {
                let n = van[idx(x, y, z)].as_str();
                if n.contains("catalyst") || n.contains("shrieker") || n.contains("sensor") {
                    println!(
                        "  van {n} local=({x},{y},{z}) world=({},{},{})",
                        96 + x as i32,
                        y,
                        -32 + z as i32
                    );
                }
            }
        }
    }

    let g = ChunkGenerator::new(12345);
    let patch_gi = feature_catalog::global_feature_index(
        step::UNDERGROUND_DECORATION,
        "sculk_patch_deep_dark",
    )
    .unwrap();
    println!("\nAll 9 origins × 256 attempts that are deep_dark (center-first order):");
    let mut order = vec![(1i32, 1i32)];
    for czl in 0..3 {
        for cxl in 0..3 {
            if cxl == 1 && czl == 1 {
                continue;
            }
            order.push((cxl, czl));
        }
    }
    let mut dd = 0u32;
    let mut in_band = 0u32;
    let mut van_sculk_at_attempt = 0u32;
    for (cxl, czl) in order {
        let ox = 80 + cxl * 16;
        let oz = -48 + czl * 16;
        let mut rng = FeatureRandom::new(12345);
        let dec = rng.set_decoration_seed(12345, ox, oz);
        rng.set_feature_seed(dec, patch_gi, step::UNDERGROUND_DECORATION);
        for i in 0..256 {
            let x = ox + rng.next_int(16);
            let z = oz + rng.next_int(16);
            let y = WORLD_BOTTOM + rng.next_int(256 - WORLD_BOTTOM + 1);
            if biome_id_at_block(&g.state, x, y, z)
                != neutron_worldgen::biome_source::biome_id::DEEP_DARK
            {
                continue;
            }
            dd += 1;
            let in_chunk = (96..112).contains(&x) && (-32..-16).contains(&z);
            if y >= -32 && y < -16 {
                in_band += 1;
                let mark = if in_chunk {
                    let vn = van[idx((x - 96) as usize, y, (z + 32) as usize)].as_str();
                    if vn == "sculk" || vn.contains("catalyst") {
                        van_sculk_at_attempt += 1;
                    }
                    format!(" in_chunk van={vn}")
                } else {
                    String::new()
                };
                println!("  att o=({ox},{oz}) i={i} ({x},{y},{z}){mark}");
            }
        }
    }
    println!("deep_dark attempts={dd} in Y=-32..-17={in_band} van_sculk_at_in_chunk_attempt={van_sculk_at_attempt}");

    // Was vanilla catalyst (103,-26,-31) among the raw 256 of any origin?
    println!("\nscan raw attempts for (103,-26,-31) and nearby:");
    for (cxl, czl) in [(1i32, 1), (1, 0), (0, 1), (2, 1), (1, 2)] {
        let ox = 80 + cxl * 16;
        let oz = -48 + czl * 16;
        let mut rng = FeatureRandom::new(12345);
        let dec = rng.set_decoration_seed(12345, ox, oz);
        rng.set_feature_seed(dec, patch_gi, step::UNDERGROUND_DECORATION);
        for i in 0..256 {
            let x = ox + rng.next_int(16);
            let z = oz + rng.next_int(16);
            let y = WORLD_BOTTOM + rng.next_int(256 - WORLD_BOTTOM + 1);
            if (x - 103).abs() <= 1 && (y + 26).abs() <= 1 && (z + 31).abs() <= 1 {
                let bid = biome_id_at_block(&g.state, x, y, z);
                println!("  o=({ox},{oz}) i={i} ({x},{y},{z}) biome={bid}");
            }
        }
    }
    let st = &g.state;
    for y in -28..=-24 {
        let bid = biome_id_at_block(st, 103, y, -31);
        println!("biome_at (103,{y},-31)={bid}");
    }

    println!("\nvanilla cells around catalysts / first origin:");
    for &(x, y, z) in &[
        (98, -43, -23),
        (111, -36, -31),
        (103, -26, -31),
        (101, -30, -27),
        (99, -42, -24),
        (98, -43, -25),
        (109, -39, -30),
    ] {
        if (96..112).contains(&x) && (-32..-16).contains(&z) {
            let n = van[idx((x - 96) as usize, y, (z + 32) as usize)].as_str();
            println!("  van ({x},{y},{z}) = {n}");
        } else {
            println!("  van ({x},{y},{z}) out of chunk");
        }
    }
    println!("vanilla 3x3x3 around (98,-43,-23):");
    for y in -44..=-42 {
        for z in -24..=-22 {
            for x in 97..=99 {
                let n = van[idx((x - 96) as usize, y, (z + 32) as usize)].as_str();
                print!("  ({x},{y},{z})={n}");
            }
            println!();
        }
    }
    let _ = BlockId::Air;
}
