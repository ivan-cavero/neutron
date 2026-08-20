//! run-059 T1 diagnostic: sweep the global FeatureSorter index for
//! `pale_garden_vegetation` and find which one reproduces the vanilla trunk
//! positions in the ref chunk when the FULL neutron placement chain is run
//! against the vanilla terrain.
//!
//! If some index yields a high position-match with the vanilla trunks, the
//! vanilla draw stream is recoverable and a wrong catalog index (not terrain)
//! is the (0,0) over-acceptance root cause. If no index matches, the chain
//! logic (or the terrain strip procedure) diverges — not the index.
//!
//! Terrain: vanilla ref 5x5 with the center's step-9 vegetal output stripped
//! (same procedure as pale_stream_probe; moss blocks mapped to dirt).
//!
//! Usage: pale_idx_scan <seed> <cx> <cz> <region_dir> [idx_lo] [idx_hi]

use neutron_world::nbt::ussr_nbt::owned::{List, Tag};
use neutron_world::nbt::{compound_get, read_nbt};
use neutron_world::Region;
use neutron_worldgen::feature_rng::FeatureRandom;
use neutron_worldgen::region_buf::RegionBuf;
use neutron_worldgen::surface::BlockId;
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
    let wb = neutron_worldgen::generator::WORLD_BOTTOM;
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
        if names.is_empty() {
            continue;
        }
        let bits = if names.len() <= 1 {
            0
        } else {
            ((names.len() - 1).ilog2() + 1).max(4) as u32
        };
        match compound_get(bs, "data") {
            Some(Tag::LongArray(data)) => {
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
            _ => {
                let bid = names[0]
                    .strip_prefix("minecraft:")
                    .and_then(BlockId::from_name)
                    .map(|b| b.as_u16())
                    .unwrap_or(BlockId::Air.as_u16());
                for ly in 0..16 {
                    for lz in 0..16 {
                        for lx in 0..16 {
                            let bi = ((y_sec * 16 + ly - wb) * 256 + lz * 16 + lx) as usize;
                            blocks[bi] = bid;
                        }
                    }
                }
            }
        }
    }
    Some(blocks)
}

fn vanilla_trunks(region_dir: &str, cx: i32, cz: i32) -> Vec<(i32, i32, i32)> {
    let Some(blocks) = load_vanilla_blocks(region_dir, cx, cz) else {
        return Vec::new();
    };
    let wb = neutron_worldgen::generator::WORLD_BOTTOM;
    let mut out = Vec::new();
    for lz in 0..16i32 {
        for lx in 0..16i32 {
            for ly in 0..384i32 {
                let bi = (ly * 256 + lz * 16 + lx) as usize;
                if blocks[bi] == BlockId::PaleOakLog.as_u16() {
                    out.push((cx * 16 + lx, wb + ly, cz * 16 + lz));
                    break;
                }
            }
        }
    }
    out
}

/// Columns (x,z) of placed logs in the center chunk of a RegionBuf.
fn trunk_xz(region: &RegionBuf, cx: i32, cz: i32) -> Vec<(i32, i32)> {
    let wb = neutron_worldgen::generator::WORLD_BOTTOM;
    let mut out = Vec::new();
    for lz in 0..16i32 {
        for lx in 0..16i32 {
            for y in (wb..wb + 384).rev() {
                if region.get(cx * 16 + lx, y, cz * 16 + lz) == BlockId::PaleOakLog {
                    out.push((cx * 16 + lx, cz * 16 + lz));
                    break;
                }
            }
        }
    }
    out
}

/// Strip the center's step-9 vegetal output (same mapping as pale_stream_probe).
fn strip_center(region: &mut RegionBuf, cx: i32, cz: i32) -> usize {
    let wb = neutron_worldgen::generator::WORLD_BOTTOM;
    let mut stripped = 0usize;
    for y in wb..neutron_worldgen::generator::WORLD_TOP {
        for z in cz * 16..cz * 16 + 16 {
            for x in cx * 16..cx * 16 + 16 {
                let b = region.get(x, y, z);
                let replacement = match b {
                    BlockId::PaleOakLog | BlockId::PaleOakLeaves => Some(BlockId::Air),
                    BlockId::ShortGrass
                    | BlockId::LeafLitter
                    | BlockId::PaleMossCarpet
                    | BlockId::PaleHangingMoss
                    | BlockId::Azalea
                    | BlockId::FloweringAzalea => Some(BlockId::Air),
                    BlockId::PaleMossBlock | BlockId::MossBlock => Some(BlockId::Dirt),
                    _ => None,
                };
                if let Some(r) = replacement {
                    region.set(x, y, z, r);
                    stripped += 1;
                }
            }
        }
    }
    stripped
}

fn fresh_region(region_dir: &str, cx: i32, cz: i32) -> Option<RegionBuf> {
    let mut reg = RegionBuf::new(cx, cz, 2);
    for dz in -2..=2 {
        for dx in -2..=2 {
            let ncx = cx + dx;
            let ncz = cz + dz;
            match load_vanilla_blocks(region_dir, ncx, ncz) {
                Some(b) => reg.put_chunk(ncx, ncz, &b, &vec![0i16; 256]),
                None => return None,
            }
        }
    }
    strip_center(&mut reg, cx, cz);
    Some(reg)
}

fn main() {
    let mut args = std::env::args().skip(1);
    let seed: i64 = args.next().and_then(|s| s.parse().ok()).unwrap_or(424242);
    let cx: i32 = args.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    let cz: i32 = args.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    let region_dir = args.next().unwrap_or_else(|| {
        "tools/nbt-ref/vanilla-fresh-424242/world/dimensions/minecraft/overworld/region"
            .to_string()
    });
    let lo: i32 = args.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    let hi: i32 = args.next().and_then(|s| s.parse().ok()).unwrap_or(39);

    let gen = ChunkGenerator::new(seed);
    let vanilla = vanilla_trunks(&region_dir, cx, cz);
    let vanilla_xz: Vec<(i32, i32)> = vanilla.iter().map(|(x, _, z)| (*x, *z)).collect();
    println!("vanilla trunks: {} columns", vanilla.len());

    for idx in lo..=hi {
        let Some(mut reg) = fresh_region(&region_dir, cx, cz) else {
            eprintln!("missing chunks");
            return;
        };
        let mut rng = FeatureRandom::new(seed);
        let dec = rng.set_decoration_seed(seed, cx * 16, cz * 16);
        rng.set_feature_seed(dec, idx, 9);
        neutron_worldgen::feature_dispatch::place_placed_feature(
            &mut rng,
            &mut reg,
            &gen.state,
            cx * 16,
            cz * 16,
            "minecraft:pale_garden_vegetation",
        );
        let probe_xz = trunk_xz(&reg, cx, cz);
        let matched = probe_xz
            .iter()
            .filter(|p| vanilla_xz.contains(p))
            .count();
        let has_vanilla = vanilla_xz
            .iter()
            .filter(|v| probe_xz.contains(v))
            .count();
        println!("idx={idx:3} probe_cols={:2} matched={matched:2} vanilla_hit={has_vanilla}", probe_xz.len());
    }
    println!(
        "catalog index = {:?}",
        neutron_worldgen::feature_catalog::global_feature_index(9, "minecraft:pale_garden_vegetation")
    );
    let _ = vanilla;
}