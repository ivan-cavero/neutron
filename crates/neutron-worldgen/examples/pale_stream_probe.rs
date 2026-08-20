//! run-058 T1 diagnostic: run the REAL neutron placement chain for
//! `pale_garden_vegetation` against the vanilla reference terrain with the
//! center chunk's OWN trees removed (logs+leaves only — ground kept), which
//! is the terrain vanilla sees when the center's vegetal step runs.
//!
//! If the accept/reject stream + placed trunks match the vanilla reference,
//! the RNG stream is correct and the (0,0) excess is terrain coupling. If
//! not, this pinpoints where the stream diverges.
//!
//! Usage: cargo run --release -p neutron-worldgen --example pale_stream_probe -- <seed> <cx> <cz> <region_dir>
//! Env: NEUTRON_TRACE_TREES=1 (per-draw), NEUTRON_DECO_TREE_TRACE=1 (placed blocks).

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

/// Vanilla pale_oak trunk base positions for a chunk (ground truth).
fn vanilla_trunks(region_dir: &str, cx: i32, cz: i32) -> Vec<(i32, i32, i32)> {
    let Some(blocks) = load_vanilla_blocks(region_dir, cx, cz) else {
        return Vec::new();
    };
    let wb = neutron_worldgen::generator::WORLD_BOTTOM;
    let mut out = Vec::new();
    for lz in 0..16i32 {
        for lx in 0..16i32 {
            let mut base = None;
            for ly in 0..384i32 {
                let bi = (ly * 256 + lz * 16 + lx) as usize;
                if blocks[bi] == BlockId::PaleOakLog.as_u16() {
                    if base.is_none() {
                        base = Some((cx * 16 + lx, wb + ly, cz * 16 + lz));
                    }
                }
            }
            if let Some(b) = base {
                out.push(b);
            }
        }
    }
    out
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

    let gen = ChunkGenerator::new(seed);
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

    // Strip the center's own step-9 vegetal output so the terrain matches
    // what vanilla sees when the center's vegetal pass runs: trees absent,
    // grass/flowers/carpet/hanging-moss absent, and pale_moss_block restored
    // to the dirt it replaced (moss patch runs after the trees).
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
    eprintln!("stripped center step-9 vegetal blocks: {stripped}");

    let mut rng = FeatureRandom::new(seed);
    let dec = rng.set_decoration_seed(seed, cx * 16, cz * 16);
    let idx = neutron_worldgen::feature_catalog::global_feature_index(9, "minecraft:pale_garden_vegetation")
        .expect("feature in step 9 sorter");
    rng.set_feature_seed(dec, idx, 9);
    eprintln!(
        "seed={seed} chunk=({cx},{cz}) placed=pale_garden_vegetation index={idx} dec={dec}"
    );
    neutron_worldgen::feature_dispatch::place_placed_feature(
        &mut rng,
        &mut region,
        &gen.state,
        cx * 16,
        cz * 16,
        "minecraft:pale_garden_vegetation",
    );

    // Compare probe placed trunks vs vanilla trunks (center chunk).
    let vanilla = vanilla_trunks(&region_dir, cx, cz);
    let mut accepted: Vec<(i32, i32)> = Vec::new();
    for z in 0..16i32 {
        for x in 0..16i32 {
            for y in (wb..wb + 384).rev() {
                let b = region.get(cx * 16 + x, y, cz * 16 + z);
                if b == BlockId::PaleOakLog {
                    accepted.push((cx * 16 + x, cz * 16 + z));
                    break;
                }
            }
        }
    }
    let mut matched = 0usize;
    for (x, z) in &accepted {
        if vanilla.iter().any(|(vx, _, vz)| vx == x && vz == z) {
            matched += 1;
        }
    }
    println!("vanilla trunks: {}", vanilla.len());
    println!("probe trunks: {} (matched {matched})", accepted.len());
    for (x, z) in &accepted {
        let has = vanilla.iter().any(|(vx, _, vz)| vx == x && vz == z);
        println!("  probe ({x},{z}) {}", if has { "MATCH" } else { "extra" });
    }
    for (x, y, z) in &vanilla {
        if !accepted.iter().any(|(ax, az)| ax == x && az == z) {
            println!("  MISSING vanilla trunk ({x},{y},{z})");
        }
    }
}