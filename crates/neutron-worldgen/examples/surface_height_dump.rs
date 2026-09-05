//! Surface-height parity dump for one chunk: vanilla ref vs neutron
//! post-noise+surface (pre-carver/decoration). Per column: the highest
//! non-air block (surface top) and the highest blocksMotion block (floor).
//! Usage: surface_height_dump <seed> <cx> <cz> <region_dir>
use neutron_world::nbt::ussr_nbt::owned::{List, Tag};
use neutron_world::nbt::{compound_get, read_nbt};
use neutron_world::Region;
use neutron_worldgen::surface::BlockId;
use neutron_worldgen::ChunkGenerator;
use std::path::PathBuf;

fn blocks_motion_pub(b: BlockId) -> bool {
    !matches!(
        b,
        BlockId::Air
            | BlockId::CaveAir
            | BlockId::Water
            | BlockId::Lava
            | BlockId::ShortGrass
            | BlockId::TallGrass
            | BlockId::LeafLitter
            | BlockId::Snow
            | BlockId::PowderSnow
            | BlockId::PaleMossCarpet
            | BlockId::PaleMossCarpetTopper
            | BlockId::MossCarpet
            | BlockId::CaveVines
            | BlockId::CaveVinesPlant
            | BlockId::PaleHangingMoss
            | BlockId::HangingRoots
            | BlockId::Vine
            | BlockId::GlowLichen
            | BlockId::Fern
            | BlockId::BrownMushroom
    )
}

fn load_vanilla(region_dir: &str, cx: i32, cz: i32) -> Option<Vec<u16>> {
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

/// (surface_top, floor, dirt_boundary_y) — dirt_boundary = the highest Y
/// where stone sits directly below a dirt-family block (the surface-rule
/// boundary). i32::MIN when the column has no dirt over stone.
fn column_tops(blocks: &[u16]) -> [(i32, i32, i32); 256] {
    let wb = neutron_worldgen::generator::WORLD_BOTTOM;
    let mut out = [(i32::MIN, i32::MIN, i32::MIN); 256];
    for lz in 0..16i32 {
        for lx in 0..16i32 {
            let mut surface = i32::MIN;
            let mut floor = i32::MIN;
            let mut boundary = i32::MIN;
            for y in (wb..wb + 384).rev() {
                let b = BlockId::from_u16(
                    blocks[((y - wb) as usize) * 256 + (lz * 16 + lx) as usize],
                )
                .unwrap_or(BlockId::Air);
                if surface == i32::MIN && !b.is_air() {
                    surface = y;
                }
                if floor == i32::MIN && blocks_motion_pub(b) {
                    floor = y;
                }
                if boundary == i32::MIN
                    && b == BlockId::Stone
                    && y + 1 < wb + 384
                {
                    let above = BlockId::from_u16(
                        blocks[((y + 1 - wb) as usize) * 256 + (lz * 16 + lx) as usize],
                    )
                    .unwrap_or(BlockId::Air);
                    if matches!(
                        above,
                        BlockId::Dirt
                            | BlockId::GrassBlock
                            | BlockId::CoarseDirt
                            | BlockId::Mud
                            | BlockId::MuddyMangroveRoots
                            | BlockId::Podzol
                            | BlockId::RootedDirt
                    ) {
                        boundary = y + 1;
                    }
                }
                if surface != i32::MIN && floor != i32::MIN && boundary != i32::MIN {
                    break;
                }
            }
            out[(lz * 16 + lx) as usize] = (surface, floor, boundary);
        }
    }
    out
}

fn main() {
    let mut a = std::env::args().skip(1);
    let seed: i64 = a.next().unwrap().parse().unwrap();
    let cx: i32 = a.next().unwrap().parse().unwrap();
    let cz: i32 = a.next().unwrap().parse().unwrap();
    let region_dir = a.next().unwrap();

    let van = load_vanilla(&region_dir, cx, cz).expect("vanilla chunk");
    let van_tops = column_tops(&van);

    let gen = ChunkGenerator::new(seed);
    let (blocks, _, _) = gen.generate_noise_and_surface(cx, cz);
    let mut neu_blocks = vec![0u16; 16 * 384 * 16];
    for y in 0..384i32 {
        for z in 0..16i32 {
            for x in 0..16i32 {
                let src = ((y * 16 + z) * 16 + x) as usize;
                neu_blocks[(y as usize) * 256 + (z * 16 + x) as usize] = blocks[src];
            }
        }
    }
    let neu_tops = column_tops(&neu_blocks);

    let mut diffs = 0usize;
    let mut boundary_diffs = 0usize;
    for lz in 0..16i32 {
        for lx in 0..16i32 {
            let (_, _, vb) = van_tops[(lz * 16 + lx) as usize];
            let (_, _, nb) = neu_tops[(lz * 16 + lx) as usize];
            if vb != nb {
                diffs += 1;
                if vb != i32::MIN && nb != i32::MIN {
                    boundary_diffs += 1;
                }
                if diffs <= 24 {
                    println!(
                        "col ({:3},{:3}) vanilla dirt-boundary={} | neutron dirt-boundary={}",
                        cx * 16 + lx,
                        cz * 16 + lz,
                        vb,
                        nb
                    );
                }
            }
        }
    }
    println!(
        "chunk ({cx},{cz}): {diffs}/256 columns differ ({boundary_diffs} both-side boundary)"
    );
}
