//! Throwaway: dump BASE mismatch clusters (disks/strata/noise edges) with Y,
//! local cluster size and 6-neighborhood summary for one chunk.
use neutron_world::nbt::ussr_nbt::owned::{List, Tag};
use neutron_world::nbt::{compound_get, read_nbt};
use neutron_world::Region;
use neutron_worldgen::surface::BlockId;
use neutron_worldgen::{ChunkGenerator, NoiseCache};
use std::collections::HashMap;
use std::path::PathBuf;

fn load_vanilla(region_dir: &str, cx: i32, cz: i32) -> HashMap<(u8, i32, u8), String> {
    let (rx, rz) = (cx >> 5, cz >> 5);
    let path = PathBuf::from(format!("{region_dir}/r.{rx}.{rz}.mca"));
    let region = Region::open(&path).expect("open").with_coords(rx, rz);
    let data = region.get_chunk(cx & 31, cz & 31).expect("get").expect("present");
    let nbt = read_nbt(&data).expect("nbt");
    let sections = match compound_get(&nbt.compound, "sections") {
        Some(Tag::List(List::Compound(l))) => l,
        _ => panic!("no sections"),
    };
    let mut map = HashMap::new();
    for sec in sections {
        let y_sec = match compound_get(sec, "Y") {
            Some(Tag::Byte(y)) => *y as i8 as i32,
            _ => continue,
        };
        let Some(Tag::Compound(bs)) = compound_get(sec, "block_states") else { continue };
        let Some(Tag::List(List::Compound(palette))) = compound_get(bs, "palette") else { continue };
        let names: Vec<String> = palette
            .iter()
            .map(|pc| match compound_get(pc, "Name") {
                Some(Tag::String(s)) => s.to_string(),
                _ => "minecraft:air".into(),
            })
            .collect();
        if names.len() == 1 {
            for i in 0..4096u32 {
                map.insert(((i & 15) as u8, y_sec * 16 + (i >> 8) as i32, ((i >> 4) & 15) as u8), names[0].clone());
            }
            continue;
        }
        let bits = ((names.len() - 1).ilog2() + 1).max(4) as u32;
        let Some(Tag::LongArray(d)) = compound_get(bs, "data") else { continue };
        let longs: Vec<i64> = d.to_vec();
        let epl = 64 / bits;
        let mask = (1u64 << bits) - 1;
        for i in 0..4096u32 {
            let li = (i / epl) as usize;
            let bo = (i % epl) * bits;
            let idx = ((longs[li] as u64) >> bo) & mask;
            map.insert(
                ((i & 15) as u8, y_sec * 16 + (i >> 8) as i32, ((i >> 4) & 15) as u8),
                names.get(idx as usize).cloned().unwrap_or_default(),
            );
        }
    }
    map
}

fn main() {
    let seed: i64 = std::env::args().nth(1).and_then(|s| s.parse().ok()).unwrap_or(12345);
    let cx: i32 = std::env::args().nth(2).and_then(|s| s.parse().ok()).unwrap_or(6);
    let cz: i32 = std::env::args().nth(3).and_then(|s| s.parse().ok()).unwrap_or(-2);
    let region_dir = std::env::args().nth(4).unwrap_or_else(|| {
        format!("tools/nbt-ref/vanilla-fresh-{seed}/world/dimensions/minecraft/overworld/region")
    });
    let van = load_vanilla(&region_dir, cx, cz);
    let gen = ChunkGenerator::new(seed);
    let mut cache = NoiseCache::new();
    let chunk = gen.generate_chunk_cached(cx, cz, &mut cache);

    // collect non-veg mismatches excluding trees/sculk (those are cascade buckets)
    let mut rows: Vec<(String, i32, i32, i32)> = Vec::new();
    for y in -60..200 {
        for lz in 0..16u32 {
            for lx in 0..16u32 {
                let vn = van.get(&(lx as u8, y, lz as u8)).map(|s| s.as_str()).unwrap_or("minecraft:air");
                let nb = chunk.block_at(lx, y, lz).block_name();
                if vn == nb { continue; }
                let vshort = vn.trim_start_matches("minecraft:");
                let nshort = nb.trim_start_matches("minecraft:");
                // skip vegetation-ish and sculk (cascade buckets)
                let skip = |s: &str| {
                    matches!(
                        s,
                        "air" | "cave_air" | "dark_oak_leaves" | "dark_oak_log" | "oak_leaves"
                            | "oak_log" | "short_grass" | "leaf_litter" | "sculk" | "sculk_vein"
                            | "moss_block" | "pale_oak_leaves" | "pale_oak_log" | "grass_block"
                            | "dirt" | "azalea" | "flowering_azalea" | "hanging_roots"
                            | "large_amethyst_bud" | "glow_lichen" | "vine" | "moss_carpet"
                            | "brown_mushroom" | "red_mushroom" | "fern" | "snow"
                    )
                };
                if skip(vshort) || skip(nshort) { continue; }
                rows.push((format!("{vshort} -> {nshort}"), lx as i32, y, lz as i32));
            }
        }
    }
    println!("non-cascade mismatches: {}", rows.len());
    for (pair, lx, y, lz) in &rows {
        // 6-neigh count of vanilla blocks equal to either side (cluster feel)
        let get = |dx: i32, dy: i32, dz: i32| -> String {
            van.get(&((lx + dx) as u8, y + dy, (lz + dz) as u8))
                .map(|s| s.trim_start_matches("minecraft:").to_string())
                .unwrap_or_else(|| "?".into())
        };
        let same_van = [&get(1, 0, 0), &get(-1, 0, 0), &get(0, 1, 0), &get(0, -1, 0)]
            .iter()
            .filter(|s| Some(s.as_str()) == pair.split(" -> ").next())
            .count();
        println!("({lx:>2},{y:>4},{lz:>2}) {pair:<42} van-neigh-same={same_van}");
    }
}
