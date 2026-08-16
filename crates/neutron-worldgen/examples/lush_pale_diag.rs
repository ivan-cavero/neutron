// Diagnostic: compare a generated chunk vs vanilla, report lush/pale missing
// block coordinates and the biome at those coordinates (neutron sampling).
// Usage: lush_pale_diag [seed] [cx] [cz] [region_dir]
use neutron_world::nbt::ussr_nbt::owned::{List, Tag};
use neutron_world::nbt::{compound_get, read_nbt};
use neutron_world::Region;
use neutron_worldgen::biome_source::biome_id_at_block;
use neutron_worldgen::feature_catalog;
use neutron_worldgen::surface::vanilla_name;
use neutron_worldgen::{generator::WORLD_BOTTOM, ChunkGenerator};
use std::collections::HashMap;
use std::path::PathBuf;

const LUSH_PALE: &[&str] = &[
    "moss_block", "moss_carpet", "clay", "cave_vines", "cave_vines_plant", "azalea",
    "flowering_azalea", "pale_oak_log", "pale_oak_leaves", "pale_moss_block",
    "pale_moss_carpet", "pale_hanging_moss", "big_dripleaf", "big_dripleaf_stem",
];

fn is_lp(name: &str) -> bool {
    let n = name.strip_prefix("minecraft:").unwrap_or(name);
    LUSH_PALE.contains(&n)
}

fn load_vanilla(region_dir: &str, cx: i32, cz: i32) -> Option<HashMap<(u8, i32, u8), String>> {
    let (rx, rz) = (cx >> 5, cz >> 5);
    let path = PathBuf::from(format!("{region_dir}/r.{rx}.{rz}.mca"));
    let region = Region::open(&path).ok()?.with_coords(rx, rz);
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
        if nstates == 1 {
            for i in 0..4096u32 {
                let ly = (i >> 8) as i32;
                map.insert(((i & 15) as u8, y_sec * 16 + ly, ((i >> 4) & 15) as u8), names[0].clone());
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
            let idx = ((longs[li] as u64) >> bo) & mask;
            let ly = (i >> 8) as i32;
            map.insert(((i & 15) as u8, y_sec * 16 + ly, ((i >> 4) & 15) as u8),
                names.get(idx as usize).cloned().unwrap_or_default());
        }
    }
    Some(map)
}

fn main() {
    let mut args = std::env::args().skip(1);
    let seed: i64 = args.next().and_then(|s| s.parse().ok()).unwrap_or(424242);
    let cx: i32 = args.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    let cz: i32 = args.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    let region_dir = args.next().unwrap_or_else(|| {
        "tools/nbt-ref/vanilla-fresh-424242/world/dimensions/minecraft/overworld/region".to_string()
    });

    // Check global FeatureSorter presence + index for key lush/pale features.
    let step9 = feature_catalog::features_per_step_at(9);
    println!("step9 global list len={}", step9.len());
    for want in ["lush_caves_clay", "lush_caves_vegetation", "cave_vines", "pale_garden_vegetation",
                 "pale_moss_patch", "flower_pale_garden", "pale_garden_flowers"] {
        let idx = step9.iter().position(|f| f == want);
        println!("  global[9] {} = {:?}", want, idx);
    }
    // Which features are in the union for chunk (0,0)?
    let biomes = vec!["lush_caves", "pale_garden"];
    let mut union: Vec<(i32, String)> = Vec::new();
    for b in &biomes {
        for f in feature_catalog::features_at_step(b, 9) {
            if let Some(idx) = feature_catalog::global_feature_index(9, &f) {
                if !union.iter().any(|(_, s)| s == &f) {
                    union.push((idx, f));
                }
            }
        }
    }
    union.sort_by_key(|(i, _)| *i);
    println!("union (lush_caves+pale_garden) step9 count={}", union.len());
    for (i, f) in &union {
        println!("  idx={i} {f}");
    }

    let gen = ChunkGenerator::new(seed);
    let chunk = gen.generate_chunk(cx, cz);
    let Some(van) = load_vanilla(&region_dir, cx, cz) else {
        println!("no vanilla chunk");
        return;
    };

    let mut by_block: HashMap<String, i64> = HashMap::new();
    let mut sample: HashMap<String, Vec<(i32, i32, i32, u8)>> = HashMap::new();
    for y in WORLD_BOTTOM..320 {
        for z in 0..16u8 {
            for x in 0..16u8 {
                let nn = vanilla_name(chunk.block_at(x as u32, y, z as u32));
                let vn = van.get(&(x, y, z)).map(|s| s.as_str()).unwrap_or("minecraft:air");
                if is_lp(vn) && nn != vn {
                    let b = vn.to_string();
                    *by_block.entry(b.clone()).or_insert(0) += 1;
                    if sample.len() < 200 {
                        let bio = biome_id_at_block(&gen.state, cx * 16 + x as i32, y, cz * 16 + z as i32);
                        sample.entry(b.clone()).or_default().push((x as i32, y, z as i32, bio));
                    }
                }
            }
        }
    }
    println!("chunk ({cx},{cz})");
    let mut sorted: Vec<_> = by_block.into_iter().collect();
    sorted.sort_by(|a, b| b.1.cmp(&a.1));
    println!("missing by vanilla block:");
    for (k, c) in sorted.iter().take(15) {
        println!("  {c:>6}  {k}");
    }
    println!("sample coords (x,y,z,biomeid):");
    for (k, coords) in sample.iter().take(15) {
        println!("  {k}:");
        for (x, y, z, bio) in coords.iter().take(5) {
            println!("    ({x},{y},{z}) biome={bio}");
        }
    }
}