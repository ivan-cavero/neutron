// Diagnostic: count lush_caves / pale_garden block mismatches across a region.
// Usage: lush_pale_parity [seed] [cx] [cz] [radius] [region_dir]
use neutron_world::nbt::ussr_nbt::owned::{List, Tag};
use neutron_world::nbt::{compound_get, read_nbt};
use neutron_world::Region;
use neutron_worldgen::surface::{is_vegetation_name, vanilla_name, BlockId};
use neutron_worldgen::{ChunkGenerator, NoiseCache};
use std::collections::HashMap;
use std::path::PathBuf;

const LUSH_PALE: &[&str] = &[
    "moss_block",
    "moss_carpet",
    "clay",
    "cave_vines",
    "cave_vines_plant",
    "azalea",
    "flowering_azalea",
    "pale_oak_log",
    "pale_oak_leaves",
    "pale_moss_block",
    "pale_moss_carpet",
    "pale_hanging_moss",
    "big_dripleaf",
    "big_dripleaf_stem",
];

fn is_lush_pale(name: &str) -> bool {
    let n = name.strip_prefix("minecraft:").unwrap_or(name);
    LUSH_PALE.contains(&n)
}

fn load_vanilla(
    regions: &mut HashMap<(i32, i32), Region>,
    region_dir: &str,
    cx: i32,
    cz: i32,
) -> Option<HashMap<(u8, i32, u8), String>> {
    let (rx, rz) = (cx >> 5, cz >> 5);
    let key = (rx, rz);
    if !regions.contains_key(&key) {
        let path = PathBuf::from(format!("{region_dir}/r.{rx}.{rz}.mca"));
        let region = Region::open(&path).ok()?.with_coords(rx, rz);
        regions.insert(key, region);
    }
    let region = regions.get(&key)?;
    let data = region.get_chunk(cx & 31, cz & 31).ok()??;
    let nbt = read_nbt(&data).ok()?;
    let sections = match compound_get(&nbt.compound, "sections") {
        Some(Tag::List(List::Compound(l))) => l,
        _ => return None,
    };
    let mut map = HashMap::new();
    for sec in sections {
        let y_sec = match compound_get(sec, "Y") {
            Some(Tag::Byte(y)) => *y as i8 as i32,
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
        if nstates == 1 {
            for i in 0..4096u32 {
                let ly = (i >> 8) as i32;
                let lz = ((i >> 4) & 15) as u8;
                let lx = (i & 15) as u8;
                map.insert((lx, y_sec * 16 + ly, lz), names[0].clone());
            }
            continue;
        }
        let bits = ((nstates - 1).ilog2() + 1).max(4) as u32;
        let Some(Tag::LongArray(data)) = compound_get(bs, "data") else {
            continue;
        };
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
            map.insert((lx, y_sec * 16 + ly, lz), name);
        }
    }
    Some(map)
}

fn main() {
    let mut args = std::env::args().skip(1);
    let seed: i64 = args.next().and_then(|s| s.parse().ok()).unwrap_or(424242);
    let cx: i32 = args.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    let cz: i32 = args.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    let radius: i32 = args.next().and_then(|s| s.parse().ok()).unwrap_or(1);
    let region_dir = args.next().unwrap_or_else(|| {
        "tools/nbt-ref/vanilla-fresh-424242/world/dimensions/minecraft/overworld/region".to_string()
    });

    let gen = ChunkGenerator::new(seed);
    // Shared noise cache across the 9 chunk generations (see region_parity.rs:
    // the 5×5 noise buffers overlap; generate_noise_and_surface is pure, so
    // results are byte-identical — only wall-clock changes).
    let mut regions: HashMap<(i32, i32), Region> = HashMap::new();
    let mut by_block: HashMap<String, i64> = HashMap::new();
    let mut total_missing = 0i64;
    let mut total_wrong = 0i64;
    let mut total_van = 0i64;
    let mut total_neu = 0i64;
    let mut chunks = 0i64;
    // Generate chunks in parallel (each with its own noise cache), then
    // accumulate serially in deterministic order. Output is byte-identical to
    // the serial loop — only wall-clock changes.
    let coords: Vec<(i32, i32)> = (cz - radius..=cz + radius)
        .flat_map(|z| (cx - radius..=cx + radius).map(move |x| (x, z)))
        .collect();
    let generated: Vec<(i32, i32, neutron_worldgen::GeneratedChunk, u8)> =
        std::thread::scope(|s| {
            let gen = &gen;
            let mut handles = Vec::with_capacity(coords.len());
            for &(ccx, ccz) in &coords {
                handles.push(s.spawn(move || {
                    let mut cache = NoiseCache::new();
                    let chunk = gen.generate_chunk_cached(ccx, ccz, &mut cache);
                    let cid = neutron_worldgen::biome_source::biome_id_at_block(
                        &gen.state,
                        ccx * 16 + 8,
                        64,
                        ccz * 16 + 8,
                    );
                    (ccx, ccz, chunk, cid)
                }));
            }
            handles.into_iter().map(|h| h.join().unwrap()).collect()
        });
    for (ccx, ccz, chunk, cid) in generated {
        let Some(van) = load_vanilla(&mut regions, &region_dir, ccx, ccz) else {
            continue;
        };
        println!("chunk ({ccx},{ccz}) center biome id={cid}");
        let wb = neutron_worldgen::generator::WORLD_BOTTOM;
        let wt = neutron_worldgen::generator::WORLD_TOP;
        for y in wb..wt {
            for z in 0..16u32 {
                for x in 0..16u32 {
                    let b = chunk.block_at(x, y, z);
                    let nn = vanilla_name(b);
                    let vn = van
                        .get(&(x as u8, y, z as u8))
                        .map(|s| s.as_str())
                        .unwrap_or("minecraft:air");
                    let v_lp = is_lush_pale(vn);
                    let n_lp = is_lush_pale(nn);
                    if v_lp {
                        total_van += 1;
                    }
                    if n_lp {
                        total_neu += 1;
                    }
                    if v_lp && nn != vn {
                        total_missing += 1;
                        *by_block.entry(vn.to_string()).or_insert(0) += 1;
                    }
                    if n_lp && nn != vn {
                        total_wrong += 1;
                    }
                }
            }
        }
        chunks += 1;
    }
    println!("seed={seed} center=({cx},{cz}) radius={radius} chunks={chunks}");
    println!("vanilla lush/pale cells: {total_van}");
    println!("neutron lush/pale cells: {total_neu}");
    println!("lush/pale MISSING (vanilla block != neutron): {total_missing}");
    println!("lush/pale WRONG (neutron placed where != vanilla): {total_wrong}");
    if total_van > 0 {
        println!(
            "lush/pale recall: {:.2}% (missing {:.2}%)",
            100.0 * (total_van - total_missing) as f64 / total_van as f64,
            100.0 * total_missing as f64 / total_van as f64
        );
    }
    let mut sorted: Vec<_> = by_block.into_iter().collect();
    sorted.sort_by(|a, b| b.1.cmp(&a.1));
    println!("missing by vanilla block:");
    for (k, c) in sorted.iter().take(20) {
        println!("  {c:>6}  {k}");
    }
}
