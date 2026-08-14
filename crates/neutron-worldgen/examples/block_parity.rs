// Block-level parity: Neutron vs vanilla .mca for one chunk.
// Usage: cargo run -p neutron-worldgen --example block_parity -- [seed] [cx] [cz]

use neutron_world::nbt::ussr_nbt::owned::{List, Tag};
use neutron_world::nbt::{compound_get, read_nbt};
use neutron_world::Region;
use neutron_worldgen::surface::BlockId;
use neutron_worldgen::{generator::WORLD_BOTTOM, ChunkGenerator};
use std::collections::HashMap;
use std::path::PathBuf;

fn main() {
    let seed: i64 = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(12345);
    let cx: i32 = std::env::args()
        .nth(2)
        .and_then(|s| s.parse().ok())
        .unwrap_or(6);
    let cz: i32 = std::env::args()
        .nth(3)
        .and_then(|s| s.parse().ok())
        .unwrap_or(-2);

    let rx = cx.div_euclid(32);
    let rz = cz.div_euclid(32);
    let lcx = cx.rem_euclid(32);
    let lcz = cz.rem_euclid(32);
    let region_dir = std::env::args().nth(4).unwrap_or_else(|| {
        "tools/nbt-ref/vanilla1/world/dimensions/minecraft/overworld/region".to_string()
    });
    let path = PathBuf::from(format!("{region_dir}/r.{rx}.{rz}.mca"));
    let region = Region::open(&path)
        .expect("open region")
        .with_coords(rx, rz);
    let data = region
        .get_chunk(lcx, lcz)
        .expect("get")
        .expect("chunk present");
    let nbt = read_nbt(&data).expect("nbt");

    let sections = match compound_get(&nbt.compound, "sections") {
        Some(Tag::List(List::Compound(list))) => list,
        _ => panic!("no sections"),
    };

    let mut vanilla: HashMap<(u8, i32, u8), String> = HashMap::new();
    let mut vanilla_counts: HashMap<String, u32> = HashMap::new();

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
                let lz = ((i >> 4) & 15) as u8;
                let lx = (i & 15) as u8;
                let y = y_sec * 16 + ly;
                let name = names[0].clone();
                *vanilla_counts.entry(name.clone()).or_insert(0) += 1;
                vanilla.insert((lx, y, lz), name);
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
            let lz = ((i >> 4) & 15) as u8;
            let lx = (i & 15) as u8;
            let y = y_sec * 16 + ly;
            let name = names
                .get(idx as usize)
                .cloned()
                .unwrap_or_else(|| "minecraft:air".into());
            *vanilla_counts.entry(name.clone()).or_insert(0) += 1;
            vanilla.insert((lx, y, lz), name);
        }
    }

    let gen = ChunkGenerator::new(seed);
    let chunk = gen.generate_chunk(cx, cz);

    let mut match_all = 0u32;
    let mut total = 0u32;
    let mut match_base = 0u32;
    let mut total_base = 0u32;
    let mut mismatch_kinds: HashMap<String, u32> = HashMap::new();
    let mut neu_counts: HashMap<String, u32> = HashMap::new();

    for y in WORLD_BOTTOM..320 {
        for z in 0..16u8 {
            for x in 0..16u8 {
                total += 1;
                let nb = chunk.block_at(x as u32, y, z as u32);
                let nname = block_to_name(nb);
                *neu_counts.entry(nname.to_string()).or_insert(0) += 1;
                let vname = vanilla
                    .get(&(x, y, z))
                    .map(|s| s.as_str())
                    .unwrap_or("minecraft:air");
                if nname == vname {
                    match_all += 1;
                } else {
                    let key = format!("{vname} -> {nname}");
                    *mismatch_kinds.entry(key).or_insert(0) += 1;
                }

                if is_veg(vname) || is_veg(nname) {
                    continue;
                }
                total_base += 1;
                if nname == vname {
                    match_base += 1;
                }
            }
        }
    }

    println!("seed={seed} chunk=({cx},{cz})");
    println!(
        "ALL blocks:    match {match_all}/{total} ({:.2}%)",
        100.0 * match_all as f64 / total as f64
    );
    println!(
        "BASE (no veg): match {match_base}/{total_base} ({:.2}%)",
        100.0 * match_base as f64 / total_base as f64
    );

    let mut kinds: Vec<_> = mismatch_kinds.into_iter().collect();
    kinds.sort_by(|a, b| b.1.cmp(&a.1));
    println!("\nTop mismatches (vanilla -> neutron):");
    for (k, c) in kinds.iter().take(30) {
        println!("  {c:>6}  {k}");
    }

    println!("\nVanilla top:");
    let mut vc: Vec<_> = vanilla_counts.into_iter().collect();
    vc.sort_by(|a, b| b.1.cmp(&a.1));
    for (n, c) in vc.iter().take(12) {
        println!("  {c:>6}  {n}");
    }
    println!("\nNeutron top:");
    let mut nc: Vec<_> = neu_counts.into_iter().collect();
    nc.sort_by(|a, b| b.1.cmp(&a.1));
    for (n, c) in nc.iter().take(12) {
        println!("  {c:>6}  {n}");
    }
}

fn is_veg(name: &str) -> bool {
    let n = name.strip_prefix("minecraft:").unwrap_or(name);
    n.contains("leaves")
        || n.contains("log")
        || n.contains("wood")
        || n == "leaf_litter"
        || n == "vine"
        || n == "short_grass"
        || n == "tall_grass"
        || n == "grass"
        || n == "fern"
        || n == "large_fern"
        || n.contains("orchid")
        || n.contains("tulip")
        || n.contains("daisy")
        || n.contains("lilac")
        || n.contains("rose")
        || n.contains("peony")
        || n.contains("azalea")
        || n.contains("mushroom")
        || n == "dandelion"
        || n == "poppy"
        || n == "cornflower"
        || n == "oxeye_daisy"
        || n == "lily_of_the_valley"
        || n == "pink_petals"
        || n == "moss_carpet"
        || n == "sculk"
        || n == "sculk_vein"
        || n == "sculk_sensor"
        || n == "sculk_catalyst"
        || n == "sculk_shrieker"
        || n == "glow_lichen"
        || n.contains("sapling")
}

fn block_to_name(b: BlockId) -> &'static str {
    match b {
        BlockId::Air => "minecraft:air",
        BlockId::Stone => "minecraft:stone",
        BlockId::Granite => "minecraft:granite",
        BlockId::Diorite => "minecraft:diorite",
        BlockId::Andesite => "minecraft:andesite",
        BlockId::Dirt => "minecraft:dirt",
        BlockId::CoarseDirt => "minecraft:coarse_dirt",
        BlockId::GrassBlock => "minecraft:grass_block",
        BlockId::Podzol => "minecraft:podzol",
        BlockId::Mycelium => "minecraft:mycelium",
        BlockId::Cobblestone => "minecraft:cobblestone",
        BlockId::Sand => "minecraft:sand",
        BlockId::RedSand => "minecraft:red_sand",
        BlockId::Gravel => "minecraft:gravel",
        BlockId::GoldOre => "minecraft:gold_ore",
        BlockId::IronOre => "minecraft:iron_ore",
        BlockId::CoalOre => "minecraft:coal_ore",
        BlockId::CopperOre => "minecraft:copper_ore",
        BlockId::DeepslateIronOre => "minecraft:deepslate_iron_ore",
        BlockId::DeepslateCoalOre => "minecraft:deepslate_coal_ore",
        BlockId::DeepslateGoldOre => "minecraft:deepslate_gold_ore",
        BlockId::DeepslateCopperOre => "minecraft:deepslate_copper_ore",
        BlockId::DeepslateDiamondOre => "minecraft:deepslate_diamond_ore",
        BlockId::DeepslateRedstoneOre => "minecraft:deepslate_redstone_ore",
        BlockId::DeepslateLapisOre => "minecraft:deepslate_lapis_ore",
        BlockId::DiamondOre => "minecraft:diamond_ore",
        BlockId::RedstoneOre => "minecraft:redstone_ore",
        BlockId::LapisOre => "minecraft:lapis_ore",
        BlockId::RawIronBlock => "minecraft:raw_iron_block",
        BlockId::RawCopperBlock => "minecraft:raw_copper_block",
        BlockId::Bedrock => "minecraft:bedrock",
        BlockId::OakLog => "minecraft:oak_log",
        BlockId::OakLeaves => "minecraft:oak_leaves",
        BlockId::Water => "minecraft:water",
        BlockId::Lava => "minecraft:lava",
        BlockId::Sandstone => "minecraft:sandstone",
        BlockId::RedSandstone => "minecraft:red_sandstone",
        BlockId::Ice => "minecraft:ice",
        BlockId::Snow => "minecraft:snow_block",
        BlockId::Clay => "minecraft:clay",
        BlockId::PackedIce => "minecraft:packed_ice",
        BlockId::PowderSnow => "minecraft:powder_snow",
        BlockId::Terracotta => "minecraft:terracotta",
        BlockId::WhiteTerracotta => "minecraft:white_terracotta",
        BlockId::OrangeTerracotta => "minecraft:orange_terracotta",
        BlockId::BrownTerracotta => "minecraft:brown_terracotta",
        BlockId::BlackTerracotta => "minecraft:black_terracotta",
        BlockId::YellowTerracotta => "minecraft:yellow_terracotta",
        BlockId::RedTerracotta => "minecraft:red_terracotta",
        BlockId::LightGrayTerracotta => "minecraft:light_gray_terracotta",
        BlockId::Mud => "minecraft:mud",
        BlockId::Deepslate => "minecraft:deepslate",
        BlockId::Tuff => "minecraft:tuff",
        BlockId::Calcite => "minecraft:calcite",
        BlockId::BlueIce => "minecraft:blue_ice",
        BlockId::Cinnabar => "minecraft:cinnabar",
        BlockId::Sulfur => "minecraft:sulfur",
        BlockId::Sculk => "minecraft:sculk",
        BlockId::SculkCatalyst => "minecraft:sculk_catalyst",
        BlockId::SculkVein => "minecraft:sculk_vein",
        BlockId::SculkSensor => "minecraft:sculk_sensor",
        BlockId::SculkShrieker => "minecraft:sculk_shrieker",
        BlockId::MossBlock => "minecraft:moss_block",
        BlockId::ShortGrass => "minecraft:short_grass",
        BlockId::LeafLitter => "minecraft:leaf_litter",
        BlockId::DarkOakLog => "minecraft:dark_oak_log",
        BlockId::DarkOakLeaves => "minecraft:dark_oak_leaves",
    }
}
