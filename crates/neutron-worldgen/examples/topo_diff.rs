//! Column-top parity: pre-decoration terrain (export_predecorate) vs the
//! stripped vanilla dump (NDEC1). Measures the ±1 surface diffs that
//! cascade into tree origin desyncs.
//!
//! Usage: topo_diff <seed> <ccx> <ccz> <dump_prefix> [region_dir]
use neutron_world::nbt::ussr_nbt::owned::{List, Tag};
use neutron_world::nbt::{compound_get, read_nbt};
use neutron_world::Region;
use neutron_worldgen::surface::{vanilla_name, BlockId};
use neutron_worldgen::generator::WORLD_BOTTOM;
use neutron_worldgen::ChunkGenerator;
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
    let wb = WORLD_BOTTOM;
    let mut blocks = vec![BlockId::Air.as_u16(); 16 * 384 * 16];
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
        let Some(Tag::LongArray(data)) = compound_get(bs, "data") else {
            continue;
        };
        if names.len() <= 1 {
            continue;
        }
        let bits = ((names.len() - 1).ilog2() + 1).max(4) as u32;
        let longs: Vec<i64> = data.to_vec();
        let epl = 64 / bits;
        let mask = (1u64 << bits) - 1;
        for i in 0..4096u32 {
            let li = (i / epl) as usize;
            let bo = (i % epl) * bits;
            let idxp = ((longs[li] as u64) >> bo) & mask;
            let ly = (i >> 8) as i32;
            let lz = ((i >> 4) & 15) as u8;
            let lx = (i & 15) as u8;
            let name = names.get(idxp as usize).cloned().unwrap_or_default();
            let bid = BlockId::from_name(name.strip_prefix("minecraft:").unwrap_or(&name))
                .map(|b| b.as_u16())
                .unwrap_or(BlockId::Air.as_u16());
            let bi = ((y_sec * 16 + ly - wb) * 256 + lz as i32 * 16 + lx as i32) as usize;
            blocks[bi] = bid;
        }
    }
    // Strip ONLY the tree/vegetation output whose height matters for the
    // pre-tree comparison, KEEPING moss_block/pale_moss_block: the pale_moss
    // patch (gif 14, AFTER trees) paints moss on the surface cell the surface
    // rule gave grass — same Y, so it marks the pre-tree terrain height.
    for v in blocks.iter_mut() {
        if let Some(b) = BlockId::from_u16(*v) {
            let strip = matches!(
                b,
                BlockId::OakLog
                    | BlockId::OakLeaves
                    | BlockId::DarkOakLog
                    | BlockId::DarkOakLeaves
                    | BlockId::PaleOakLog
                    | BlockId::PaleOakLeaves
                    | BlockId::BirchLog
                    | BlockId::BirchLeaves
                    | BlockId::SpruceLog
                    | BlockId::SpruceLeaves
                    | BlockId::ShortGrass
                    | BlockId::TallGrass
                    | BlockId::Fern
                    | BlockId::LargeFern
                    | BlockId::LeafLitter
                    | BlockId::MossCarpet
                    | BlockId::PaleMossCarpet
                    | BlockId::PaleHangingMoss
                    | BlockId::HangingRoots
                    | BlockId::Vine
                    | BlockId::GlowLichen
                    | BlockId::CaveVines
                    | BlockId::CaveVinesPlant
            );
            if strip {
                *v = BlockId::Air.as_u16();
            }
        }
    }
    Some(blocks)
}

fn top_motion(blocks: &[u16], lx: usize, lz: usize) -> i32 {
    for ly in (0..384usize).rev() {
        let b = BlockId::from_u16(blocks[ly * 256 + lz * 16 + lx]).unwrap_or(BlockId::Air);
        // OCEAN_FLOOR: blocksMotion only (plants/leaves excluded). Approximate
        // with the same non-veg rule the harness uses: solid & !plantish.
        match b {
            BlockId::Air
            | BlockId::CaveAir
            | BlockId::ShortGrass
            | BlockId::TallGrass
            | BlockId::Fern
            | BlockId::LargeFern
            | BlockId::LeafLitter
            | BlockId::PaleMossCarpet
            | BlockId::MossCarpet
            | BlockId::PaleHangingMoss
            | BlockId::Vine
            | BlockId::GlowLichen
            | BlockId::HangingRoots => {}
            _ => return WORLD_BOTTOM + ly as i32,
        }
    }
    WORLD_BOTTOM
}

fn main() {
    let mut args = std::env::args().skip(1);
    let seed: i64 = args.next().and_then(|s| s.parse().ok()).unwrap_or(424242);
    let ccx: i32 = args.next().and_then(|s| s.parse().ok()).unwrap_or(2);
    let ccz: i32 = args.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    let prefix = args.next().unwrap_or_else(|| "/tmp/opencode/tff_ndec.txt".into());
    let region_dir = args.next().unwrap_or_else(|| {
        "tools/nbt-ref/vanilla-fresh-424242/world/dimensions/minecraft/overworld/region".into()
    });

    let gen = ChunkGenerator::new(seed);
    let (chunk_pals, blocks_arr, _bio, _names) = gen.export_predecorate(ccx, ccz);

    // Load stripped vanilla for the same 5x5.
    let mut van: Vec<Vec<u16>> = Vec::new();
    for dz in -2..=2 {
        for dx in -2..=2 {
            van.push(load_vanilla_blocks(&region_dir, ccx + dx, ccz + dz).expect("van chunk"));
        }
    }

    // Compare column tops over the INNER 3x3 chunks (the decorating origins).
    let mut exact = 0u32;
    let mut off1 = 0u32;
    let mut off_more = 0u32;
    let mut examples: Vec<(i32, i32, i32, i32, String, String)> = Vec::new();
    for czl in -1..=1 {
        for cxl in -1..=1 {
            let vidx = ((czl + 2) * 5 + (cxl + 2)) as usize;
            let van_blocks = &van[vidx];
            let neu_chunk = &blocks_arr[vidx]; // palette-index grid, same layout
            let pal = &chunk_pals[vidx];
            for lz in 0..16usize {
                for lx in 0..16usize {
                    let wx = (ccx + cxl) * 16 + lx as i32;
                    let wz = (ccz + czl) * 16 + lz as i32;
                    let vt = top_motion(van_blocks, lx, lz);
                    // neutron: decode via palette
                    let mut nt = WORLD_BOTTOM;
                    for ly in (0..384usize).rev() {
                        let idx = ly * 256 + lz * 16 + lx;
                        let name = pal.get(neu_chunk[idx] as usize).cloned().unwrap_or_default();
                        let b = BlockId::from_name(name.strip_prefix("minecraft:").unwrap_or(&name))
                            .unwrap_or(BlockId::Air);
                        match b {
                            BlockId::Air
                            | BlockId::CaveAir
                            | BlockId::ShortGrass
                            | BlockId::TallGrass
                            | BlockId::Fern
                            | BlockId::LargeFern
                            | BlockId::LeafLitter
                            | BlockId::PaleMossCarpet
                            | BlockId::MossCarpet
                            | BlockId::PaleHangingMoss
                            | BlockId::Vine
                            | BlockId::GlowLichen
                            | BlockId::HangingRoots => {}
                            _ => {
                                nt = WORLD_BOTTOM + ly as i32;
                                break;
                            }
                        }
                    }
                    let d = nt - vt;
                    if d == 0 {
                        exact += 1;
                    } else if d.abs() == 1 {
                        off1 += 1;
                        if examples.len() < 12 {
                            examples.push((wx, wz, vt, nt, String::new(), String::new()));
                        }
                    } else {
                        off_more += 1;
                        if examples.len() < 12 {
                            examples.push((wx, wz, vt, nt, String::new(), String::new()));
                        }
                    }
                    let _ = vanilla_name(BlockId::Air);
                }
            }
        }
    }
    let total = exact + off1 + off_more;
    println!(
        "columns={total} exact={exact} ({:.2}%) off_by_1={off1} ({:.2}%) off_ge_2={off_more} ({:.2}%)",
        100.0 * exact as f64 / total as f64,
        100.0 * off1 as f64 / total as f64,
        100.0 * off_more as f64 / total as f64
    );
    for (wx, wz, vt, nt, _, _) in &examples {
        println!("  DIFF ({wx},{wz}) van_top={vt} neu_top={nt} d={}", nt - vt);
    }
}
