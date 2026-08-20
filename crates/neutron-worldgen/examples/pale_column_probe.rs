//! run-059 T1 diagnostic: for a set of (x,z) draw positions, dump the ref
//! column top ~6 blocks + the top block under any vegetal, so we can see what
//! vanilla's would_survive/OCEAN_FLOOR actually saw at draw time.
//! Usage: pale_column_probe <region_dir> <cx> <cz> <x1,z1;x2,z2;...>

use neutron_world::nbt::ussr_nbt::owned::{List, Tag};
use neutron_world::nbt::{compound_get, read_nbt};
use neutron_world::Region;
use neutron_worldgen::surface::BlockId;
use std::path::PathBuf;

fn load_vanilla_blocks(region_dir: &str, cx: i32, cz: i32) -> Option<Vec<u16>> {
    let (rx, rz) = (cx >> 5, cz >> 5);
    let path = PathBuf::from(format!("{region_dir}/r.{rx}.{rz}.mca"));
    let region = Region::open(&path).ok()?.with_coords(rx, rz);
    let data = region.get_chunk(cx & 31, cz & 31).ok()??;
    let nbt = read_nbt(&data).ok()?;
    let sections = match compound_get(&nbt.compound, "sections") {
        Some(Tag::List(List::Compound(l))) => l,
        _ => return None,
    };
    let wb = neutron_worldgen::generator::WORLD_BOTTOM;
    let mut blocks = vec![BlockId::Air.as_u16(); 16 * 384 * 16];
    for sec in sections {
        let y_sec = match compound_get(sec, "Y") {
            Some(Tag::Byte(y)) => *y as i8 as i32,
            Some(Tag::Int(y)) => *y,
            _ => continue,
        };
        let Some(Tag::Compound(bs)) = compound_get(sec, "block_states") else { continue; };
        let Some(Tag::List(List::Compound(palette))) = compound_get(bs, "palette") else { continue; };
        let names: Vec<String> = palette.iter().map(|pc| match compound_get(pc, "Name") {
            Some(Tag::String(s)) => s.to_string(), _ => "minecraft:air".into() }).collect();
        if names.is_empty() { continue; }
        let bits = if names.len() <= 1 { 0 } else { ((names.len()-1).ilog2()+1).max(4) as u32 };
        match compound_get(bs, "data") {
            Some(Tag::LongArray(data)) => {
                let longs: Vec<i64> = data.to_vec();
                let epl = 64/bits; let mask = (1u64<<bits)-1;
                for i in 0..4096u32 {
                    let li=(i/epl) as usize; let bo=(i%epl)*bits;
                    let idxp=((longs[li] as u64)>>bo)&mask;
                    let ly=(i>>8) as i32; let lz=((i>>4)&15) as u8; let lx=(i&15) as u8;
                    let name=names.get(idxp as usize).cloned().unwrap_or_default();
                    let bid=BlockId::from_name(name.strip_prefix("minecraft:").unwrap_or(&name)).map(|b|b.as_u16()).unwrap_or(BlockId::Air.as_u16());
                    let bi=((y_sec*16+ly-wb)*256+lz as i32*16+lx as i32) as usize;
                    blocks[bi]=bid;
                }
            }
            _ => {
                let bid = names[0].strip_prefix("minecraft:").and_then(BlockId::from_name).map(|b|b.as_u16()).unwrap_or(BlockId::Air.as_u16());
                for ly in 0..16 { for lz in 0..16 { for lx in 0..16 {
                    let bi=((y_sec*16+ly-wb)*256+lz*16+lx) as usize; blocks[bi]=bid;
                }}}
            }
        }
    }
    Some(blocks)
}

fn name(b: u16) -> &'static str {
    BlockId::from_u16(b).map(neutron_worldgen::surface::vanilla_name).unwrap_or("???")
}

fn main() {
    let mut args = std::env::args().skip(1);
    let region_dir = args.next().unwrap_or("F:/neutron/tools/nbt-ref/vanilla-fresh-424242/world/dimensions/minecraft/overworld/region".into());
    let cx: i32 = args.next().unwrap_or("0".into()).parse().unwrap();
    let cz: i32 = args.next().unwrap_or("0".into()).parse().unwrap();
    let positions = args.next().unwrap_or_default();
    let wb = neutron_worldgen::generator::WORLD_BOTTOM;
    let blocks = load_vanilla_blocks(&region_dir, cx, cz).expect("vanilla chunk");

    let mut cols: Vec<(i32, i32)> = Vec::new();
    if positions.is_empty() {
        for lz in 0..16i32 { for lx in 0..16i32 { cols.push((lx, lz)); } }
    } else {
        for p in positions.split(';') {
            let mut it = p.split(',');
            let x: i32 = it.next().unwrap_or("0").parse().unwrap_or(0);
            let z: i32 = it.next().unwrap_or("0").parse().unwrap_or(0);
            cols.push((x, z));
        }
    }

    for (lx0, lz0) in cols {
        // top 6 blocks of the column (from WORLD_TOP down)
        let mut line = Vec::new();
        for ly in (0..384i32).rev() {
            let bi = (ly * 256 + lz0 * 16 + lx0) as usize;
            let b = blocks[bi];
            if b != BlockId::Air.as_u16() || line.len() <= 6 {
                let blk_name = name(b);
                if line.len() < 8 {
                    line.push(format!("{blk_name}@{}", wb + ly));
                }
                if b != BlockId::Air.as_u16() && line.len() >= 6 { break; }
            }
        }
        println!("col ({lx0},{lz0}): {}", line.join(" "));
    }
}