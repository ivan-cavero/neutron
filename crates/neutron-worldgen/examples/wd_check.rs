//! run-058 T1: check water depth + heightmaps at a column in the vanilla ref.
use neutron_world::nbt::ussr_nbt::owned::{List, Tag};
use neutron_world::nbt::{compound_get, read_nbt};
use neutron_world::Region;
use neutron_worldgen::region_buf::RegionBuf;
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

fn main() {
    let region_dir = std::env::args().nth(1).unwrap_or_else(|| "tools/nbt-ref/vanilla-fresh-424242/world/dimensions/minecraft/overworld/region".to_string());
    let mut region = RegionBuf::new(0, 0, 1);
    for dz in -1..=1 { for dx in -1..=1 {
        let ncx = 0+dx; let ncz = 0+dz;
        if let Some(b) = load_vanilla_blocks(&region_dir, ncx, ncz) {
            region.put_chunk(ncx, ncz, &b, &vec![0i16;256]);
        }
    }}
    for (lx, lz) in [(3u32,6u32),(10,8),(5,8),(13,0),(9,11),(11,3),(5,14),(7,2),(2,10),(4,0),(15,11),(12,2),(7,6),(4,4),(9,4),(4,7)] {
        let x = lx as i32; let z = lz as i32;
        let mut surface = None; let mut floor = None;
        for y in (-64..320).rev() {
            let b = region.get(x, y, z);
            if !b.is_air() && surface.is_none() { surface = Some(y+1); }
            if !b.is_air() && !b.is_fluid() && floor.is_none() { floor = Some(y+1); break; }
        }
        let depth = match (surface, floor) { (Some(s),Some(f)) => (s-f).max(0), _ => 0 };
        // top solid
        let mut top = None;
        for y in (-64..320).rev() { let b = region.get(x,y,z); if !b.is_air() && !b.is_fluid() { top = Some(y); break; } }
        println!("({lx},{lz}) depth={depth} top_solid={top:?} blocks 100-108: {:?}",
            (100..=108).map(|y| neutron_worldgen::surface::vanilla_name(region.get(x,y,z))).collect::<Vec<_>>());
    }
}
