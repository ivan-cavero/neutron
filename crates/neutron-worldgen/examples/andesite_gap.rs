// Extra andesite vs vanilla: Y histogram + blob clustering.
// cargo run -p neutron-worldgen --example andesite_gap --release

use neutron_world::nbt::ussr_nbt::owned::{List, Tag};
use neutron_world::nbt::{compound_get, read_nbt};
use neutron_world::Region;
use neutron_worldgen::surface::BlockId;
use neutron_worldgen::{generator::WORLD_BOTTOM, ChunkGenerator};
use std::path::PathBuf;

fn main() {
    let path = PathBuf::from(std::env::args().nth(1).unwrap_or_else(|| {
        "tools/nbt-ref/vanilla1/world/dimensions/minecraft/overworld/region/r.0.-1.mca"
            .to_string()
    }));
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
                Some(Tag::String(s)) => s.to_string(),
                _ => "minecraft:air".into(),
            })
            .collect();
        let nstates = names.len();
        if nstates == 1 {
            for i in 0..4096u32 {
                let ly = (i >> 8) as i32;
                let lz = ((i >> 4) & 15) as usize;
                let lx = (i & 15) as usize;
                let y = y_sec * 16 + ly;
                let idx = ((y - WORLD_BOTTOM) as usize) * 256 + lz * 16 + lx;
                van[idx] = names[0].clone();
            }
            continue;
        }
        let bits = ((nstates - 1).ilog2() + 1).max(4) as u32;
        let Some(Tag::LongArray(data)) = compound_get(bs, "data") else {
            continue;
        };
        let longs: Vec<i64> = data.to_vec();
        let epl = 64 / bits;
        let mask = (1u64 << bits) - 1;
        for i in 0..4096u32 {
            let li = (i / epl) as usize;
            let bo = (i % epl) * bits;
            let idxp = ((longs[li] as u64) >> bo) & mask;
            let ly = (i >> 8) as i32;
            let lz = ((i >> 4) & 15) as usize;
            let lx = (i & 15) as usize;
            let y = y_sec * 16 + ly;
            let idx = ((y - WORLD_BOTTOM) as usize) * 256 + lz * 16 + lx;
            van[idx] = names
                .get(idxp as usize)
                .cloned()
                .unwrap_or_else(|| "minecraft:air".into());
        }
    }

    let chunk = ChunkGenerator::new(12345).generate_chunk(6, -2);
    let mut extra_y = [0u32; 24]; // 16-block bands from -64
    let mut miss_y = [0u32; 24];
    let mut extra_pos = Vec::new();
    let mut neu_and = 0u32;
    let mut van_and = 0u32;
    for y in WORLD_BOTTOM..320 {
        for z in 0..16u32 {
            for x in 0..16u32 {
                let nb = chunk.block_at(x, y, z);
                let idx = ((y - WORLD_BOTTOM) as usize) * 256 + (z as usize) * 16 + (x as usize);
                let vn = van[idx].as_str();
                if vn == "minecraft:andesite" {
                    van_and += 1;
                }
                if nb == BlockId::Andesite {
                    neu_and += 1;
                }
                let extra = nb == BlockId::Andesite && vn != "minecraft:andesite";
                let miss = vn == "minecraft:andesite" && nb != BlockId::Andesite;
                if extra || miss {
                    let band = ((y - WORLD_BOTTOM) / 16) as usize;
                    if extra {
                        extra_y[band] += 1;
                        extra_pos.push((x as i32, y, z as i32));
                    }
                    if miss {
                        miss_y[band] += 1;
                    }
                }
            }
        }
    }
    let mut van_and_y = [0u32; 24];
    for y in WORLD_BOTTOM..320 {
        for z in 0..16u32 {
            for x in 0..16u32 {
                let idx = ((y - WORLD_BOTTOM) as usize) * 256 + (z as usize) * 16 + (x as usize);
                if van[idx] == "minecraft:andesite" {
                    van_and_y[((y - WORLD_BOTTOM) / 16) as usize] += 1;
                }
            }
        }
    }
    println!("vanilla andesite by Y section:");
    for (i, c) in van_and_y.iter().enumerate() {
        if *c > 0 {
            let y0 = WORLD_BOTTOM + (i as i32) * 16;
            println!("  Y {y0}..{}  van={c}", y0 + 15);
        }
    }
    println!("andesite van={van_and} neu={neu_and} extra={} miss={}", extra_pos.len(), miss_y.iter().sum::<u32>());
    println!("extra by Y section:");
    for (i, c) in extra_y.iter().enumerate() {
        if *c > 0 {
            let y0 = WORLD_BOTTOM + (i as i32) * 16;
            println!("  Y {y0}..{}  extra={c}  miss={}", y0 + 15, miss_y[i]);
        }
    }
    if !extra_pos.is_empty() {
        let (sx, sy, sz) = extra_pos.iter().fold((0i64, 0i64, 0i64), |a, p| {
            (a.0 + p.0 as i64, a.1 + p.1 as i64, a.2 + p.2 as i64)
        });
        let n = extra_pos.len() as i64;
        println!(
            "extra centroid ~ ({:.1}, {:.1}, {:.1})",
            sx as f64 / n as f64,
            sy as f64 / n as f64,
            sz as f64 / n as f64
        );
        let mut miny = i32::MAX;
        let mut maxy = i32::MIN;
        for p in &extra_pos {
            miny = miny.min(p.1);
            maxy = maxy.max(p.1);
        }
        println!("extra Y range {miny}..{maxy}");
    }

    println!("\ncolumn lx=9 lz=6  Y=80..160 (van / neu):");
    for y in (80..=160).rev() {
        let nb = chunk.block_at(9, y, 6);
        let idx = ((y - WORLD_BOTTOM) as usize) * 256 + 6 * 16 + 9;
        let vn = van[idx].strip_prefix("minecraft:").unwrap_or(&van[idx]);
        let nn = format!("{nb:?}").to_ascii_lowercase();
        if vn != "air" || nn != "air" {
            let mark = if vn == "andesite" && nn != "andesite" {
                " MISS"
            } else if vn != "andesite" && nn == "andesite" {
                " EXTRA"
            } else {
                ""
            };
            println!("  Y{y:3}  {vn:16} {nn:16}{mark}");
        }
    }
}
