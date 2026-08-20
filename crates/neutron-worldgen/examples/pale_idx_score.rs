//! run-059 T1 diagnostic: for every candidate global FeatureSorter index,
//! score how many of the 16 in_square draw positions land on a vanilla pale
//! oak 2x2 base (the positions vanilla ACTUALLY grew trees at). Purely RNG —
//! no world-loading. The vanilla index should score ~ the number of vanilla
//! trees (~9-10) while wrong indices score ~0-3.
//! Usage: pale_draw_scan <seed> <cx> <cz> <region_dir>

use neutron_world::nbt::ussr_nbt::owned::{List, Tag};
use neutron_world::nbt::{compound_get, read_nbt};
use neutron_world::Region;
use neutron_worldgen::feature_rng::FeatureRandom;
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
    let mut args = std::env::args().skip(1);
    let seed: i64 = args.next().and_then(|s| s.parse().ok()).unwrap_or(424242);
    let cx: i32 = args.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    let cz: i32 = args.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    let region_dir = args.next().unwrap_or("F:/neutron/tools/nbt-ref/vanilla-fresh-424242/world/dimensions/minecraft/overworld/region".into());
    let hi: i32 = args.next().and_then(|s| s.parse().ok()).unwrap_or(105);

    let wb = neutron_worldgen::generator::WORLD_BOTTOM;
    let blocks = load_vanilla_blocks(&region_dir, cx, cz).expect("vanilla chunk");
    // trunk column bitmask per (x,z)
    let mut is_trunk = vec![false; 16 * 16];
    for lz in 0..16i32 {
        for lx in 0..16i32 {
            for ly in 0..384i32 {
                if blocks[(ly * 256 + lz * 16 + lx) as usize] == BlockId::PaleOakLog.as_u16() {
                    is_trunk[(lz * 16 + lx) as usize] = true;
                    break;
                }
            }
        }
    }
    let trunk_count = is_trunk.iter().filter(|b| **b).count();

    // A draw at (x,z) is a dark oak 2x2 base (x..x+1, z..z+1) + up to 2-block
    // lean. Score a hit when >=2 of the 2x2 base columns are vanilla trunks.
    let hits_for = |x: i32, z: i32| -> i32 {
        let mut hit = 0;
        for dx in 0..2 {
            for dz in 0..2 {
                let (mx, mz) = (x + dx, z + dz);
                if mx >= 0 && mx < 16 && mz >= 0 && mz < 16 && is_trunk[(mz * 16 + mx) as usize] {
                    hit += 1;
                }
            }
        }
        hit
    };

    println!("vanilla trunk columns: {trunk_count}");
    let mut scored: Vec<(i32, i32, i32)> = Vec::new(); // (score, idx, base_hits)
    for idx in 0..=105 {
        let mut rng = FeatureRandom::new(seed);
        let dec = rng.set_decoration_seed(seed, cx * 16, cz * 16);
        rng.set_feature_seed(dec, idx, 9);
        let mut draws = Vec::new();
        for _ in 0..16 {
            draws.push((rng.next_int(16), rng.next_int(16)));
        }
        let mut score = 0;
        let mut base_hits = 0;
        for &(x, z) in &draws {
            let h = hits_for_point(x, z, &is_trunk);
            base_hits += h;
            if h >= 2 {
                score += 1;
            }
        }
        scored.push((score, base_hits, idx));
    }
    scored.sort_by(|a, b| b.0.cmp(&a.0).then(b.1.cmp(&a.1)).then(a.2.cmp(&b.2)));
    for (score, base_hits, idx) in scored.iter().take(15) {
        println!("idx={idx:3} scored_draws={score} base_hits={base_hits}");
    }
    println!("---");
    for (score, base_hits, idx) in scored.iter().rev().take(5) {
        println!("idx={idx:3} scored_draws={score} base_hits={base_hits}");
    }
    println!(
        "catalog index = {:?}",
        neutron_worldgen::feature_catalog::global_feature_index(9, "minecraft:pale_garden_vegetation")
    );

    fn hits_for_point(x: i32, z: i32, is_trunk: &[bool]) -> i32 {
        let mut hit = 0;
        for dx in 0..2 {
            for dz in 0..2 {
                let (mx, mz) = (x + dx, z + dz);
                if mx >= 0 && mx < 16 && mz >= 0 && mz < 16 && is_trunk[(mz * 16 + mx) as usize] {
                    hit += 1;
                }
            }
        }
        hit
    }
    let _ = wb;
}