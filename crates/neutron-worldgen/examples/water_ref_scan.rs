// Investigate the water/terrain puzzle in ref chunk (0,0) of a vanilla 26.2
// reference world. Prints:
//   - the block at a given probe point (default (12,1,15))
//   - water + cave_air positions grouped by y-band and by border zone
//     (local x/z in 0..3 or 12..15 = "border", 4..11 = "interior")
//   - full water/cave_air position lists (optional)
//
// Usage:
//   cargo run -p neutron-worldgen --example water_ref_scan -- \
//       <region_dir> <cx> <cz> [wx wy wz] [--list]
// e.g.
//   cargo run -p neutron-worldgen --example water_ref_scan -- \
//       tools/nbt-ref/vanilla-fresh-424242/world/dimensions/minecraft/overworld/region 0 0

use neutron_world::nbt::ussr_nbt::owned::{List, Tag};
use neutron_world::nbt::{compound_get, read_nbt};
use neutron_world::Region;
use std::collections::BTreeMap;
use std::path::PathBuf;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let region_dir = args.get(1).cloned().unwrap_or_else(|| {
        "tools/nbt-ref/vanilla-fresh-424242/world/dimensions/minecraft/overworld/region"
            .to_string()
    });
    let cx: i32 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(0);
    let cz: i32 = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(0);
    let probe: Option<(i32, i32, i32)> = match (args.get(4), args.get(5), args.get(6)) {
        (Some(x), Some(y), Some(z)) => {
            Some((x.parse().unwrap(), y.parse().unwrap(), z.parse().unwrap()))
        }
        _ => None,
    };
    let list = args.iter().any(|a| a == "--list");

    let rx = cx.div_euclid(32);
    let rz = cz.div_euclid(32);
    let lcx = cx.rem_euclid(32);
    let lcz = cz.rem_euclid(32);
    let path = PathBuf::from(format!("{region_dir}/r.{rx}.{rz}.mca"));
    println!("region: {}", path.display());
    let region = Region::open(&path)
        .expect("open region")
        .with_coords(rx, rz);
    let data = region
        .get_chunk(lcx, lcz)
        .expect("get")
        .expect("chunk present");
    let nbt = read_nbt(&data).expect("nbt");

    let sections = match compound_get(&nbt.compound, "sections") {
        Some(Tag::List(List::Compound(list))) => list,
        _ => panic!("no sections"),
    };

    // block map: (lx, y, lz) -> name
    let mut blocks: BTreeMap<(u8, i32, u8), String> = BTreeMap::new();
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
                let lz = ((i >> 4) & 15) as u8;
                let lx = (i & 15) as u8;
                blocks.insert((lx, y_sec * 16 + ly, lz), names[0].clone());
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
            let idx = ((longs[li] as u64) >> bo) & mask;
            let ly = (i >> 8) as i32;
            let lz = ((i >> 4) & 15) as u8;
            let lx = (i & 15) as u8;
            let name = names
                .get(idx as usize)
                .cloned()
                .unwrap_or_else(|| "minecraft:air".into());
            blocks.insert((lx, y_sec * 16 + ly, lz), name);
        }
    }

    if let Some((wx, wy, wz)) = probe {
        let lx = wx.rem_euclid(16) as u8;
        let lz = wz.rem_euclid(16) as u8;
        println!(
            "probe world ({wx},{wy},{wz}) local ({lx},{wy},{lz}) -> {}",
            blocks.get(&(lx, wy, lz)).map(String::as_str).unwrap_or("(not stored)")
        );
    }

    let mut water: Vec<(i32, u8, u8, i32)> = Vec::new(); // (y, lx, lz, world_z)
    let mut cave_air: Vec<(i32, u8, u8, i32)> = Vec::new();
    for (&(lx, y, lz), name) in &blocks {
        let wz = cz * 16 + lz as i32;
        if name == "minecraft:water" {
            water.push((y, lx, lz, wz));
        } else if name == "minecraft:cave_air" {
            cave_air.push((y, lx, lz, wz));
        }
    }

    let band = |y: i32| -> &'static str {
        if y < -16 {
            "<-16"
        } else if y < 0 {
            "-16..-1"
        } else if y < 16 {
            "0..15"
        } else if y < 32 {
            "16..31"
        } else if y < 64 {
            "32..63"
        } else {
            ">=64"
        }
    };
    let zone = |x: u8, z: u8| -> &'static str {
        if x < 4 || x > 11 || z < 4 || z > 11 {
            "border"
        } else {
            "interior"
        }
    };

    println!("\nWATER by y-band x zone (chunk {cx},{cz}):");
    let mut wb: BTreeMap<(&str, &str), usize> = BTreeMap::new();
    for &(y, lx, lz, _) in &water {
        *wb.entry((band(y), zone(lx, lz))).or_insert(0) += 1;
    }
    for ((b, z), c) in &wb {
        println!("  {b:>10} {z:>8}: {c}");
    }
    let wtotal: usize = wb.values().sum();
    println!("  water total: {wtotal}");

    println!("\nCAVE_AIR by y-band x zone:");
    let mut cb: BTreeMap<(&str, &str), usize> = BTreeMap::new();
    for &(y, lx, lz, _) in &cave_air {
        *cb.entry((band(y), zone(lx, lz))).or_insert(0) += 1;
    }
    for ((b, z), c) in &cb {
        println!("  {b:>10} {z:>8}: {c}");
    }
    let ctotal: usize = cb.values().sum();
    println!("  cave_air total: {ctotal}");

    // water y-histogram
    println!("\nWATER y-histogram:");
    let mut wh: BTreeMap<i32, usize> = BTreeMap::new();
    for &(y, _, _, _) in &water {
        *wh.entry(y).or_insert(0) += 1;
    }
    for (y, c) in &wh {
        println!("  y={y:>4}: {c}");
    }

    if list {
        use neutron_worldgen::surface::vanilla_name;
        use neutron_worldgen::ChunkGenerator;
        let gen = ChunkGenerator::new(424242);
        let chunk = gen.generate_chunk(cx, cz);
        let nb = |x: i32, y: i32, z: i32| {
            vanilla_name(chunk.block_at(x.rem_euclid(16) as u32, y, z.rem_euclid(16) as u32))
                .to_string()
        };
        println!("\nWATER positions (world x,y,z)  ref -> neutron:");
        for &(y, lx, lz, wz) in &water {
            println!(
                "  ({},{},{})  {} -> {}",
                cx * 16 + lx as i32,
                y,
                wz,
                blocks.get(&(lx, y, lz)).map(String::as_str).unwrap_or("?"),
                nb(cx * 16 + lx as i32, y, wz)
            );
        }
        println!("\nCAVE_AIR positions (world x,y,z)  ref -> neutron:");
        for &(y, lx, lz, wz) in &cave_air {
            println!(
                "  ({},{},{})  {} -> {}",
                cx * 16 + lx as i32,
                y,
                wz,
                blocks.get(&(lx, y, lz)).map(String::as_str).unwrap_or("?"),
                nb(cx * 16 + lx as i32, y, wz)
            );
        }
    }
}