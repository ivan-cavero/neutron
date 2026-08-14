// Dump sculk patch origins + Y histogram vs vanilla.
// cargo run -p neutron-worldgen --example sculk_patches --release

use neutron_world::nbt::ussr_nbt::owned::{List, Tag};
use neutron_world::nbt::{compound_get, read_nbt};
use neutron_world::Region;
use neutron_worldgen::feature_catalog::{self, step};
use neutron_worldgen::feature_rng::FeatureRandom;
use neutron_worldgen::generator::WORLD_BOTTOM;
use neutron_worldgen::sculk::{
    SCULK_BIOME_OK, SCULK_PLACED, SCULK_SPREAD_OK, SCULK_TRIES,
};
use neutron_worldgen::{biome_source::biome_id_at_block, surface::BlockId, ChunkGenerator};
use std::path::PathBuf;
use std::sync::atomic::Ordering;

fn main() {
    for a in [&SCULK_TRIES, &SCULK_BIOME_OK, &SCULK_SPREAD_OK, &SCULK_PLACED] {
        a.store(0, Ordering::Relaxed);
    }
    let g = ChunkGenerator::new(12345);
    // Replay only the *first* origin's 256 attempts (center-first = (6,-2)).
    let ox = 96;
    let oz = -32;
    let idx = feature_catalog::global_feature_index(step::UNDERGROUND_DECORATION, "sculk_patch_deep_dark")
        .unwrap_or(1);
    let mut rng = FeatureRandom::new(12345);
    let dec = rng.set_decoration_seed(12345, ox, oz);
    rng.set_feature_seed(dec, idx, step::UNDERGROUND_DECORATION);
    println!("center origin attempts (biome only; can_spread needs live world):");
    let mut biome_pos = Vec::new();
    for i in 0..256 {
        let x = ox + rng.next_int(16);
        let z = oz + rng.next_int(16);
        let y = WORLD_BOTTOM + rng.next_int(256 - WORLD_BOTTOM + 1);
        let bid = biome_id_at_block(&g.state, x, y, z);
        if bid == neutron_worldgen::biome_source::biome_id::DEEP_DARK {
            biome_pos.push((i, x, y, z));
        }
    }
    println!("  deep_dark samples from center origin: {}", biome_pos.len());
    print!("  Ys:");
    for &(_, _, y, _) in &biome_pos {
        print!(" {y}");
    }
    println!();

    let ch = g.generate_chunk(6, -2);
    let mut neu_y = [0u32; 24];
    let mut neu_sculk = 0u32;
    let mut neu_cat = 0u32;
    for y in WORLD_BOTTOM..320 {
        for z in 0..16u32 {
            for x in 0..16u32 {
                match ch.block_at(x, y, z) {
                    BlockId::Sculk => {
                        neu_sculk += 1;
                        neu_y[((y - WORLD_BOTTOM) as usize) / 16] += 1;
                    }
                    BlockId::SculkCatalyst => neu_cat += 1,
                    _ => {}
                }
            }
        }
    }

    let path = PathBuf::from(
        "tools/nbt-ref/vanilla1/world/dimensions/minecraft/overworld/region/r.0.-1.mca",
    );
    let region = Region::open(&path).unwrap().with_coords(0, -1);
    let data = region.get_chunk(6, 30).unwrap().unwrap();
    let nbt = read_nbt(&data).unwrap();
    let sections = match compound_get(&nbt.compound, "sections") {
        Some(Tag::List(List::Compound(l))) => l,
        _ => panic!(),
    };
    let mut van_y = [0u32; 24];
    let mut van_sculk = 0u32;
    let mut van_cat = 0u32;
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
        for i in 0..4096u32 {
            let name = if nstates == 1 {
                names[0].clone()
            } else {
                let bits = ((nstates - 1).ilog2() + 1).max(4) as u32;
                let Some(Tag::LongArray(data)) = compound_get(bs, "data") else {
                    continue;
                };
                let longs: Vec<i64> = data.to_vec();
                let epl = 64 / bits;
                let mask = (1u64 << bits) - 1;
                let li = (i / epl) as usize;
                let bo = (i % epl) * bits;
                let idx = ((longs[li] as u64) >> bo) & mask;
                names.get(idx as usize).cloned().unwrap_or_default()
            };
            if name.ends_with(":sculk") {
                van_sculk += 1;
                let ly = (i >> 8) as i32;
                let y = y_sec * 16 + ly;
                van_y[((y - WORLD_BOTTOM) as usize) / 16] += 1;
            } else if name.contains("sculk_catalyst") {
                van_cat += 1;
            }
        }
    }
    println!(
        "global tries={} biome_ok={} spread_ok={} placed_ops={}",
        SCULK_TRIES.load(Ordering::Relaxed),
        SCULK_BIOME_OK.load(Ordering::Relaxed),
        SCULK_SPREAD_OK.load(Ordering::Relaxed),
        SCULK_PLACED.load(Ordering::Relaxed)
    );
    println!("neu sculk={neu_sculk} cat={neu_cat}  van sculk={van_sculk} cat={van_cat}");
    println!("Y-section histogram (secY  neu  van):");
    for s in 0..24 {
        if neu_y[s] + van_y[s] > 0 {
            let sec = s as i32 - 4;
            println!("  Ysec={sec:3}  neu={:4}  van={:4}", neu_y[s], van_y[s]);
        }
    }

    // Reload van names for Y=-32..-17 miss map
    let mut van_name = vec!["air".to_string(); 16 * 384 * 16];
    for sec in sections {
        let y_sec = match compound_get(sec, "Y") {
            Some(Tag::Byte(y)) => *y as i8 as i32,
            Some(Tag::Int(y)) => *y,
            _ => continue,
        };
        if y_sec != -2 {
            continue;
        }
        let Some(Tag::Compound(bs)) = compound_get(sec, "block_states") else {
            continue;
        };
        let Some(Tag::List(List::Compound(palette))) = compound_get(bs, "palette") else {
            continue;
        };
        let names: Vec<String> = palette
            .iter()
            .map(|pc| match compound_get(pc, "Name") {
                Some(Tag::String(s)) => s
                    .to_string()
                    .strip_prefix("minecraft:")
                    .unwrap_or(&s.to_string())
                    .to_string(),
                _ => "air".into(),
            })
            .collect();
        let nstates = names.len();
        for i in 0..4096u32 {
            let name = if nstates == 1 {
                names[0].clone()
            } else {
                let bits = ((nstates - 1).ilog2() + 1).max(4) as u32;
                let Some(Tag::LongArray(data)) = compound_get(bs, "data") else {
                    continue;
                };
                let longs: Vec<i64> = data.to_vec();
                let epl = 64 / bits;
                let mask = (1u64 << bits) - 1;
                let li = (i / epl) as usize;
                let bo = (i % epl) * bits;
                names
                    .get((((longs[li] as u64) >> bo) & mask) as usize)
                    .cloned()
                    .unwrap_or_default()
            };
            let ly = (i >> 8) as i32;
            let lz = ((i >> 4) & 15) as usize;
            let lx = (i & 15) as usize;
            let y = y_sec * 16 + ly;
            van_name[(((y - WORLD_BOTTOM) as usize) * 256) + lz * 16 + lx] = name;
        }
    }
    let st = &g.state;
    let mut miss = 0u32;
    let mut biome_hist = std::collections::HashMap::new();
    let mut neu_hist = std::collections::HashMap::new();
    for y in -32..-16 {
        for z in 0..16u32 {
            for x in 0..16u32 {
                let idx = ((y - WORLD_BOTTOM) as usize) * 256 + (z as usize) * 16 + x as usize;
                if van_name[idx] != "sculk" {
                    continue;
                }
                let nb = ch.block_at(x, y, z);
                if nb == BlockId::Sculk {
                    continue;
                }
                miss += 1;
                let bid = biome_id_at_block(st, 96 + x as i32, y, -32 + z as i32);
                *biome_hist.entry(bid).or_insert(0u32) += 1;
                *neu_hist.entry(format!("{nb:?}")).or_insert(0u32) += 1;
            }
        }
    }
    println!("miss van-sculk in Y=-32..-17: {miss}");
    println!("  neu block at miss: {neu_hist:?}");
    println!("  biome_id at miss: {biome_hist:?} (31=deep_dark 34=lush)");
}
