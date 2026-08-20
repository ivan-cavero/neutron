//! run-058 T1: dump the vanilla ref terrain (3x3 chunks around a center) as
//! "x y z name" lines for ProbePaleFlow, with the center chunk's step-9
//! vegetal output stripped (trees->air, grass/flowers/carpet/hanging-moss->
//! air, pale_moss_block/moss_block->dirt) so it matches vanilla's draw-time
//! terrain for the center chunk.
//! Usage: dump_terrain <region_dir> <out> [center_cx] [center_cz]
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

fn strip_id(b: u16) -> u16 {
    let id = BlockId::from_u16(b).unwrap_or(BlockId::Air);
    match id {
        BlockId::PaleOakLog | BlockId::PaleOakLeaves => BlockId::Air.as_u16(),
        BlockId::ShortGrass | BlockId::LeafLitter | BlockId::PaleMossCarpet
        | BlockId::PaleHangingMoss | BlockId::Azalea | BlockId::FloweringAzalea => BlockId::Air.as_u16(),
        BlockId::PaleMossBlock | BlockId::MossBlock => BlockId::Dirt.as_u16(),
        _ => b,
    }
}

fn main() {
    let region_dir = std::env::args().nth(1).unwrap_or_else(|| "tools/nbt-ref/vanilla-fresh-424242/world/dimensions/minecraft/overworld/region".to_string());
    let out = std::env::args().nth(2).unwrap_or_else(|| "tmp-vanilla-terrain-3x3.txt".to_string());
    let center_cx: i32 = std::env::args().nth(3).and_then(|s| s.parse().ok()).unwrap_or(0);
    let center_cz: i32 = std::env::args().nth(4).and_then(|s| s.parse().ok()).unwrap_or(0);
    let wb = neutron_worldgen::generator::WORLD_BOTTOM;
    let mut lines: Vec<String> = Vec::new();
    for cz in center_cz-1..=center_cz+1 { for cx in center_cx-1..=center_cx+1 {
        let Some(blocks) = load_vanilla_blocks(&region_dir, cx, cz) else { eprintln!("missing chunk {cx},{cz}"); continue; };
        for ly in 0..384i32 {
            for lz in 0..16i32 { for lx in 0..16i32 {
                let bi = (ly * 256 + lz * 16 + lx) as usize;
                let mut b = blocks[bi];
                if cx == center_cx && cz == center_cz {
                    b = strip_id(b);
                }
                let name = BlockId::from_u16(b).map(|x| neutron_worldgen::surface::vanilla_name(x)).unwrap_or("minecraft:air");
                if name != "minecraft:air" {
                    lines.push(format!("{} {} {} {}", cx*16+lx, wb+ly, cz*16+lz, name));
                }
            }}
        }
    }}
    std::fs::write(&out, lines.join("\n")).unwrap();
    println!("wrote {} non-air cells to {out}", lines.len());
}
