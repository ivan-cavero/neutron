// Diagnose surface-rule diffs: coords (x,y,z), Y histogram, per-column pattern,
// and biome comparison (vanilla NBT biomes vs neutron sampled biome).
// Usage: cargo run -p neutron-worldgen --example surface_diag -- [seed] [cx] [cz] [region_dir]

use neutron_world::nbt::ussr_nbt::owned::{List, Tag};
use neutron_world::nbt::{compound_get, read_nbt};
use neutron_world::Region;
use neutron_worldgen::surface::BlockId;
use neutron_worldgen::{generator::WORLD_BOTTOM, ChunkGenerator};
use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;

fn unpack(longs: &[i64], bits: u32, i: u32) -> u32 {
    let epl = 64 / bits;
    let li = (i / epl) as usize;
    let bo = (i % epl) * bits;
    (((longs[li] as u64) >> bo) & ((1u64 << bits) - 1)) as u32
}

fn main() {
    let seed: i64 = std::env::args().nth(1).and_then(|s| s.parse().ok()).unwrap_or(424242);
    let cx: i32 = std::env::args().nth(2).and_then(|s| s.parse().ok()).unwrap_or(0);
    let cz: i32 = std::env::args().nth(3).and_then(|s| s.parse().ok()).unwrap_or(0);
    let region_dir = std::env::args()
        .nth(4)
        .unwrap_or_else(|| "tools/nbt-ref/vanilla-fresh-424242/world/dimensions/minecraft/overworld/region".to_string());

    let rx = cx.div_euclid(32);
    let rz = cz.div_euclid(32);
    let lcx = cx.rem_euclid(32);
    let lcz = cz.rem_euclid(32);
    let path = PathBuf::from(format!("{region_dir}/r.{rx}.{rz}.mca"));
    let region = Region::open(&path).expect("open region").with_coords(rx, rz);
    let data = region.get_chunk(lcx, lcz).expect("get").expect("chunk present");
    let nbt = read_nbt(&data).expect("nbt");

    let sections = match compound_get(&nbt.compound, "sections") {
        Some(Tag::List(List::Compound(list))) => list,
        _ => panic!("no sections"),
    };

    // Vanilla blocks: (lx, y, lz) -> name
    let mut vanilla: HashMap<(u8, i32, u8), String> = HashMap::new();
    // Vanilla biome per quart (qx, qy, qz) -> name  (quart = block >> 2)
    let mut vanilla_biome: HashMap<(i32, i32, i32), String> = HashMap::new();

    for sec in sections.iter() {
        let y_sec = match compound_get(sec, "Y") {
            Some(Tag::Byte(y)) => *y as i8 as i32,
            Some(Tag::Int(y)) => *y,
            _ => continue,
        };
        if let Some(Tag::Compound(bs)) = compound_get(sec, "block_states") {
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
                    vanilla.insert((lx, y_sec * 16 + ly, lz), names[0].clone());
                }
            } else {
                let bits = ((nstates - 1).ilog2() + 1).max(4) as u32;
                let Some(Tag::LongArray(data)) = compound_get(bs, "data") else {
                    continue;
                };
                let longs: Vec<i64> = data.to_vec();
                for i in 0..4096u32 {
                    let idx = unpack(&longs, bits, i);
                    let ly = (i >> 8) as i32;
                    let lz = ((i >> 4) & 15) as u8;
                    let lx = (i & 15) as u8;
                    let name = names.get(idx as usize).cloned().unwrap_or_else(|| "minecraft:air".into());
                    vanilla.insert((lx, y_sec * 16 + ly, lz), name);
                }
            }
        }
        if let Some(Tag::Compound(bio)) = compound_get(sec, "biomes") {
            // 26.2 biome palettes are plain string lists
            let names: Vec<String> = match compound_get(bio, "palette") {
                Some(Tag::List(List::String(palette))) => {
                    palette.iter().map(|s| s.to_string()).collect()
                }
                Some(Tag::List(List::Compound(palette))) => palette
                    .iter()
                    .map(|pc| match compound_get(pc, "Name") {
                        Some(Tag::String(s)) => s.to_string(),
                        _ => "minecraft:plains".into(),
                    })
                    .collect(),
                _ => continue,
            };
            let nstates = names.len();
            // quart local index i: y4 = i>>4, z4 = (i>>2)&3, x4 = i&3
            if nstates == 1 {
                for i in 0..64u32 {
                    let qy = y_sec * 4 + (i >> 4) as i32;
                    let qz = (i >> 2) & 3;
                    let qx = i & 3;
                    vanilla_biome.insert((qx as i32, qy, qz as i32), names[0].clone());
                }
            } else {
                // biome palettes use minimal bit width (no 4-bit floor)
                let bits = ((nstates - 1).ilog2() + 1).max(1) as u32;
                let Some(Tag::LongArray(data)) = compound_get(bio, "data") else {
                    continue;
                };
                let longs: Vec<i64> = data.to_vec();
                for i in 0..64u32 {
                    let idx = unpack(&longs, bits, i);
                    let qy = y_sec * 4 + (i >> 4) as i32;
                    let qz = (i >> 2) & 3;
                    let qx = i & 3;
                    let name = names.get(idx as usize).cloned().unwrap_or_else(|| "minecraft:plains".into());
                    vanilla_biome.insert((qx as i32, qy, qz as i32), name);
                }
            }
        }
    }

    let gen = ChunkGenerator::new(seed);
    let chunk = gen.generate_chunk(cx, cz);

    // Summary of vanilla biomes present (quart-level) in this chunk
    {
        let mut names: std::collections::BTreeSet<&String> = std::collections::BTreeSet::new();
        for v in vanilla_biome.values() {
            names.insert(v);
        }
        println!("vanilla biomes in chunk: {names:?}");
        let mut neu: std::collections::BTreeMap<String, u32> = std::collections::BTreeMap::new();
        for z in 0..16usize {
            for x in 0..16usize {
                let wx = cx * 16 + x as i32;
                let wz = cz * 16 + z as i32;
                let id = neutron_worldgen::biome_manager::biome_id_at_block(&gen.state, wx, 96, wz);
                *neu.entry(biome_name(id)).or_insert(0) += 1;
            }
        }
        println!("neutron biomes at y=96: {neu:?}");
    }

    let targets: &[(&str, &str)] = &[
        ("minecraft:stone", "minecraft:sandstone"),
        ("minecraft:dirt", "minecraft:sand"),
        ("minecraft:grass_block", "minecraft:sand"),
        ("minecraft:pale_moss_block", "minecraft:sand"),
    ];

    for (vname, nname) in targets {
        let mut diffs: Vec<(u8, i32, u8)> = Vec::new();
        for y in WORLD_BOTTOM..320 {
            for z in 0..16u8 {
                for x in 0..16u8 {
                    let nb = chunk.block_at(x as u32, y, z as u32);
                    let nn = neutron_worldgen::surface::vanilla_name(nb);
                    let vn = vanilla.get(&(x, y, z)).map(|s| s.as_str()).unwrap_or("minecraft:air");
                    if vn == *vname && nn == *nname {
                        diffs.push((x, y, z));
                    }
                }
            }
        }
        if diffs.is_empty() {
            println!("\n=== {vname} -> {nname}: 0 diffs ===");
            continue;
        }
        println!("\n=== {vname} -> {nname}: {} diffs ===", diffs.len());

        // Y histogram
        let mut yhist: BTreeMap<i32, u32> = BTreeMap::new();
        for &(_, y, _) in &diffs {
            *yhist.entry(y).or_insert(0) += 1;
        }
        println!("Y histogram:");
        for (y, c) in &yhist {
            println!("  y={y:>4}: {c}");
        }

        // Per-column
        let mut cols: BTreeMap<(u8, u8), (u32, i32, i32)> = BTreeMap::new(); // (lx,lz) -> (count, ymin, ymax)
        for &(x, y, z) in &diffs {
            let e = cols.entry((x, z)).or_insert((0, y, y));
            e.0 += 1;
            e.1 = e.1.min(y);
            e.2 = e.2.max(y);
        }
        println!("columns affected: {}", cols.len());
        for ((x, z), (c, ymin, ymax)) in cols.iter().take(60) {
            let wx = cx * 16 + *x as i32;
            let wz = cz * 16 + *z as i32;
            // surface biome (vanilla) at the column's top diff quart
            let top_qy = (*ymax) >> 2;
            let vb_top = vanilla_biome
                .get(&((*x as i32) >> 2, top_qy, (*z as i32) >> 2))
                .cloned()
                .unwrap_or_else(|| "?".into());
            let vb_low = vanilla_biome
                .get(&((*x as i32) >> 2, (*ymin) >> 2, (*z as i32) >> 2))
                .cloned()
                .unwrap_or_else(|| "?".into());
            println!(
                "  col (wx={wx}, wz={wz}) n={c} y[{ymin}..{ymax}] vbiome(top)={vb_top} vbiome(low)={vb_low}"
            );
        }

        // Sample first 12 diffs in detail with neutron biome at that y
        println!("sample cells (x,y,z | vanilla biome quart | neutron biome@y | neutron biome@surface):");
        let mut shown = 0;
        for &(x, y, z) in &diffs {
            if shown >= 12 {
                break;
            }
            shown += 1;
            let wx = cx * 16 + x as i32;
            let wz = cz * 16 + z as i32;
            // find column surface: topmost non-air in neutron chunk
            let mut surf_y = 320;
            while surf_y > WORLD_BOTTOM
                && chunk.block_at(x as u32, surf_y - 1, z as u32).is_air()
            {
                surf_y -= 1;
            }
            let surf_y = surf_y - 1;
            let nb_at = biome_name(neutron_worldgen::biome_manager::biome_id_at_block(&gen.state, wx, y, wz));
            let nb_surf = biome_name(neutron_worldgen::biome_manager::biome_id_at_block(&gen.state, wx, surf_y, wz));
            let vq = vanilla_biome
                .get(&((x as i32) >> 2, y >> 2, (z as i32) >> 2))
                .cloned()
                .unwrap_or_else(|| "?".into());
            println!("  ({wx},{y},{wz}) | vanilla_q={vq} | neutron@y={nb_at} | neutron@surf({surf_y})={nb_surf}");
        }
    }
}

fn biome_name(id: u8) -> String {
    use neutron_worldgen::biome_source::biome_id as b;
    let n = match id {
        x if x == b::OCEAN => "ocean",
        x if x == b::DEEP_OCEAN => "deep_ocean",
        x if x == b::FROZEN_OCEAN => "frozen_ocean",
        x if x == b::DESERT => "desert",
        x if x == b::PLAINS => "plains",
        x if x == b::FOREST => "forest",
        x if x == b::TAIGA => "taiga",
        x if x == b::SWAMP => "swamp",
        x if x == b::RIVER => "river",
        x if x == b::FROZEN_RIVER => "frozen_river",
        x if x == b::BEACH => "beach",
        x if x == b::STONY_SHORE => "stony_shore",
        x if x == b::SAVANNA => "savanna",
        x if x == b::JUNGLE => "jungle",
        x if x == b::SNOWY_PLAINS => "snowy_plains",
        x if x == b::SNOWY_SLOPES => "snowy_slopes",
        x if x == b::JAGGED_PEAKS => "jagged_peaks",
        x if x == b::FROZEN_PEAKS => "frozen_peaks",
        x if x == b::STONY_PEAKS => "stony_peaks",
        x if x == b::GROVE => "grove",
        x if x == b::WINDSWEPT_HILLS => "windswept_hills",
        x if x == b::DARK_FOREST => "dark_forest",
        x if x == b::MEADOW => "meadow",
        x if x == b::ICE_SPIKES => "ice_spikes",
        x if x == b::OLD_GROWTH_PINE_FOREST => "old_growth_pine_taiga",
        x if x == b::OLD_GROWTH_BIRCH_FOREST => "old_growth_birch_forest",
        x if x == b::BIRCH_FOREST => "birch_forest",
        x if x == b::CHERRY_GROVE => "cherry_grove",
        x if x == b::BADLANDS => "badlands",
        x if x == b::ERODED_BADLANDS => "eroded_badlands",
        x if x == b::WOODED_BADLANDS => "wooded_badlands",
        x if x == b::DRIPSTONE_CAVES => "dripstone_caves",
        x if x == b::MANGROVE_SWAMP => "mangrove_swamp",
        x if x == b::DEEP_DARK => "deep_dark",
        x if x == b::LUSH_CAVES => "lush_caves",
        x if x == b::SULFUR_CAVES => "sulfur_caves",
        _ => "other",
    };
    n.to_string()
}
