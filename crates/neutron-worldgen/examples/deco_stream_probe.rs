//! B4 T3 diagnostic: run the REAL neutron placement chain (placement
//! modifiers + tree dispatch) against the VANILLA terrain loaded from the
//! reference .mca files. The tree placement consumes RNG based on the
//! region's blocks (isAirOrLeaves checks), so running it against the vanilla
//! terrain yields the EXACT vanilla RNG consumption — the tool to derive the
//! vanilla decoration stream and the origin order (T3 desync).
//!
//! Usage: deco_stream_probe [seed] [cx] [cz] [placed_feature] [region_dir]
//! Set NEUTRON_TRACE_TREES=1 to see per-draw accept/reject.
use neutron_world::nbt::ussr_nbt::owned::{List, Tag};
use neutron_world::nbt::{compound_get, read_nbt};
use neutron_world::Region;
use neutron_worldgen::feature_rng::FeatureRandom;
use neutron_worldgen::region_buf::RegionBuf;
use neutron_worldgen::surface::BlockId;
use neutron_worldgen::ChunkGenerator;
use std::path::PathBuf;

/// Load one vanilla chunk's blocks (names) into a 16×384×16 u16 vec.
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

fn main() {
    let mut args = std::env::args().skip(1);
    let seed: i64 = args.next().and_then(|s| s.parse().ok()).unwrap_or(424242);
    let cx: i32 = args.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    let cz: i32 = args.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    let placed_id = args
        .next()
        .unwrap_or_else(|| "minecraft:pale_garden_vegetation".to_string());
    let region_dir = args.next().unwrap_or_else(|| {
        "tools/nbt-ref/vanilla-fresh-424242/world/dimensions/minecraft/overworld/region".to_string()
    });

    let gen = ChunkGenerator::new(seed);
    // Optional: print the vanilla trunk bases (2x2 NW-corner candidates) for
    // the center chunk (the ground truth for the draw->tree mapping.
    if std::env::var("NEUTRON_DECO_TRUNKS").is_ok() {
        let Some(blocks) = load_vanilla_blocks(&region_dir, cx, cz) else {
            eprintln!("no vanilla blocks");
            return;
        };
        let wb = neutron_worldgen::generator::WORLD_BOTTOM;
        for lz in 0..16i32 {
            for lx in 0..16i32 {
                for ly in 0..384i32 {
                    let bi = (ly * 256 + lz * 16 + lx) as usize;
                    if blocks[bi] == BlockId::PaleOakLog.as_u16() {
                        let y = wb + ly;
                        // find the base: lowest log in the column
                        let mut by = y;
                        while by > wb
                            && blocks[((by - 1 - wb) * 256 + lz * 16 + lx) as usize]
                                == BlockId::PaleOakLog.as_u16()
                        {
                            by -= 1;
                        }
                        println!("trunk ({},{},{})", cx * 16 + lx, by, cz * 16 + lz);
                        break;
                    }
                }
            }
        }
        return;
    }
    // 5x5 buffer (FEATURE_RADIUS 2) loaded from the vanilla reference.
    let mut region = RegionBuf::new(cx, cz, 2);
    let mut missing = Vec::new();
    for dz in -2..=2 {
        for dx in -2..=2 {
            let ncx = cx + dx;
            let ncz = cz + dz;
            match load_vanilla_blocks(&region_dir, ncx, ncz) {
                Some(b) => region.put_chunk(ncx, ncz, &b, &vec![0i16; 256]),
                None => missing.push((ncx, ncz)),
            }
        }
    }
    if !missing.is_empty() {
        eprintln!("missing chunks: {missing:?}");
        return;
    }

    let mut rng = FeatureRandom::new(seed);
    let dec = rng.set_decoration_seed(seed, cx * 16, cz * 16);
    // global index for the placed feature in step 9 (VEGETAL_DECORATION).
    let idx = neutron_worldgen::feature_catalog::global_feature_index(9, &placed_id)
        .expect("feature in step 9 sorter");
    rng.set_feature_seed(dec, idx, 9);
    eprintln!(
        "seed={seed} chunk=({cx},{cz}) placed={placed_id} index={idx} dec={dec} (terrain = vanilla ref)"
    );
    let count_clay = |region: &RegionBuf| -> usize {
        let wb = neutron_worldgen::generator::WORLD_BOTTOM;
        let mut n = 0;
        for y in wb..wb + 384 {
            for lz in 0..16 {
                for lx in 0..16 {
                    if region.get(cx * 16 + lx, y, cz * 16 + lz) == BlockId::Clay {
                        n += 1;
                    }
                }
            }
        }
        n
    };
    let before = count_clay(&region);
    neutron_worldgen::feature_dispatch::place_placed_feature(
        &mut rng,
        &mut region,
        &gen.state,
        cx * 16,
        cz * 16,
        &placed_id,
    );
    let after = count_clay(&region);
    eprintln!("[clay] before={before} after={after} placed={}", after - before);
}
