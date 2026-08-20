// Quick neighborhood dump: y-slices x vs z for chunk (0,0), ref + neutron
use neutron_world::nbt::ussr_nbt::owned::{List, Tag};
use neutron_world::nbt::{compound_get, read_nbt};
use neutron_world::Region;
use std::collections::BTreeMap;
use std::path::PathBuf;
use neutron_worldgen::surface::vanilla_name;
use neutron_worldgen::ChunkGenerator;

fn main() {
    let region_dir = "F:/neutron/tools/nbt-ref/vanilla-fresh-424242/world/dimensions/minecraft/overworld/region".to_string();
    let path = PathBuf::from(format!("{region_dir}/r.0.0.mca"));
    let region = Region::open(&path).unwrap().with_coords(0, 0);
    let data = region.get_chunk(0, 0).unwrap().unwrap();
    let nbt = read_nbt(&data).unwrap();
    let sections = match compound_get(&nbt.compound, "sections") {
        Some(Tag::List(List::Compound(list))) => list,
        _ => panic!("no sections"),
    };
    let mut refb: BTreeMap<(u8, i32, u8), String> = BTreeMap::new();
    for sec in sections {
        let y_sec = match compound_get(sec, "Y") {
            Some(Tag::Byte(y)) => *y as i8 as i32,
            Some(Tag::Int(y)) => *y,
            _ => continue,
        };
        let Some(Tag::Compound(bs)) = compound_get(sec, "block_states") else { continue };
        let Some(Tag::List(List::Compound(palette))) = compound_get(bs, "palette") else { continue };
        let names: Vec<String> = palette.iter().map(|pc| match compound_get(pc, "Name") {
            Some(Tag::String(s)) => s.to_string(), _ => "minecraft:air".into(),
        }).collect();
        let nstates = names.len();
        if nstates == 1 {
            for i in 0..4096u32 {
                let ly = (i >> 8) as i32; let lz = ((i >> 4) & 15) as u8; let lx = (i & 15) as u8;
                refb.insert((lx, y_sec * 16 + ly, lz), names[0].clone());
            }
            continue;
        }
        let bits = ((nstates - 1).ilog2() + 1).max(4) as u32;
        let Some(Tag::LongArray(data)) = compound_get(bs, "data") else { continue };
        let longs: Vec<i64> = data.to_vec();
        let epl = 64 / bits; let mask = (1u64 << bits) - 1;
        for i in 0..4096u32 {
            let li = (i / epl) as usize; let bo = (i % epl) * bits;
            let idx = ((longs[li] as u64) >> bo) & mask;
            let ly = (i >> 8) as i32; let lz = ((i >> 4) & 15) as u8; let lx = (i & 15) as u8;
            refb.insert((lx, y_sec * 16 + ly, lz), names.get(idx as usize).cloned().unwrap_or_else(|| "minecraft:air".into()));
        }
    }
    let gen = ChunkGenerator::new(424242);
    let chunk = gen.generate_chunk(0, 0);

    let short = |n: &str| -> String {
        match n {
            "minecraft:water" => "W".into(),
            "minecraft:cave_air" => "c".into(),
            "minecraft:air" => ".".into(),
            "minecraft:clay" => "C".into(),
            "minecraft:moss_block" => "M".into(),
            "minecraft:deepslate" => "#".into(),
            "minecraft:stone" => "+".into(),
            "minecraft:dirt" => "d".into(),
            "minecraft:grass_block" => "g".into(),
            "minecraft:andesite" => "a".into(),
            "minecraft:gravel" => "v".into(),
            "minecraft:granite" => "n".into(),
            "minecraft:diorite" => "o".into(),
            "minecraft:tuff" => "t".into(),
            "minecraft:bedrock" => "B".into(),
            _ => n.trim_start_matches("minecraft:").chars().next().unwrap_or('?').to_string(),
        }
    };
    for y in (0..=6).rev() {
        println!("=== ref  y={y} (z rows 15..0):");
        for z in (13..=15).rev() {
            let mut row = String::new();
            for x in 0..16u8 {
                row.push_str(&short(refb.get(&(x, y, z)).map(String::as_str).unwrap_or("?")));
            }
            println!("  z={z} {row}");
        }
        println!("=== neu  y={y}:");
        for z in (13..=15).rev() {
            let mut row = String::new();
            for x in 0..16u8 {
                row.push_str(&short(vanilla_name(chunk.block_at(x as u32, y, z as u32))));
            }
            println!("  z={z} {row}");
        }
    }
}
