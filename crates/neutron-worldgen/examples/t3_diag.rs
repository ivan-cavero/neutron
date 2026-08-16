// T3 diagnostic: dump divergent ore/vein cells for chunk (6,-2) seed 12345
// and cluster iron blobs in vanilla vs neutron.

use neutron_world::nbt::ussr_nbt::owned::{List, Tag};
use neutron_world::nbt::{compound_get, read_nbt};
use neutron_world::Region;
use neutron_worldgen::surface::BlockId;
use neutron_worldgen::{generator::WORLD_BOTTOM, ChunkGenerator};
use std::collections::HashMap;

fn main() {
    let seed: i64 = 12345;
    let cx: i32 = 6;
    let cz: i32 = -2;

    let rx = cx.div_euclid(32);
    let rz = cz.div_euclid(32);
    let lcx = cx.rem_euclid(32);
    let lcz = cz.rem_euclid(32);
    let path = std::path::PathBuf::from(format!("tools/nbt-ref/vanilla1/world/dimensions/minecraft/overworld/region/r.{rx}.{rz}.mca"));
    let region = Region::open(&path).expect("open region").with_coords(rx, rz);
    let data = region.get_chunk(lcx, lcz).expect("get").expect("chunk");
    let nbt = read_nbt(&data).expect("nbt");

    let sections = match compound_get(&nbt.compound, "sections") {
        Some(Tag::List(List::Compound(list))) => list,
        _ => panic!("no sections"),
    };

    let mut vanilla: HashMap<(u8, i32, u8), String> = HashMap::new();
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
                vanilla.insert((lx, y_sec * 16 + ly, lz), names[0].clone());
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
            let name = names
                .get(idx as usize)
                .cloned()
                .unwrap_or_else(|| "minecraft:air".into());
            vanilla.insert((lx, y_sec * 16 + ly, lz), name);
        }
    }

    let gen = ChunkGenerator::new(seed);
    let chunk = gen.generate_chunk(cx, cz);

    // ---- T3-relevant mismatch dump ----
    let interesting = |v: &str, n: &str| {
        let t3 = |s: &str| {
            s.contains("ore") || s == "minecraft:tuff" || s == "minecraft:deepslate" || s == "minecraft:clay"
        };
        t3(v) || t3(n)
    };
    println!("== T3 mismatch cells (x=local, y, z=local; vanilla -> neutron) ==");
    let mut by_kind: HashMap<String, Vec<(i32, i32, i32)>> = HashMap::new();
    for y in WORLD_BOTTOM..320 {
        for z in 0..16u8 {
            for x in 0..16u8 {
                let nb = chunk.block_at(x as u32, y, z as u32);
                let nname = block_to_name(nb);
                let vname = vanilla
                    .get(&(x, y, z))
                    .map(|s| s.as_str())
                    .unwrap_or("minecraft:air");
                if nname != vname && interesting(vname, nname) {
                    by_kind
                        .entry(format!("{vname} -> {nname}"))
                        .or_default()
                        .push((x as i32, y, z as i32));
                }
            }
        }
    }
    let mut kinds: Vec<_> = by_kind.iter().collect();
    kinds.sort_by(|a, b| b.1.len().cmp(&a.1.len()));
    for (k, cells) in &kinds {
        println!("-- {k} ({} cells)", cells.len());
        // print y histogram + a few coords
        let mut yh: HashMap<i32, usize> = HashMap::new();
        for (_, y, _) in cells.iter() {
            *yh.entry(*y).or_default() += 1;
        }
        let mut ys: Vec<_> = yh.into_iter().collect();
        ys.sort_by_key(|(y, _)| *y);
        let ymin = ys.first().map(|(y, _)| *y).unwrap_or(0);
        let ymax = ys.last().map(|(y, _)| *y).unwrap_or(0);
        println!("   y range [{ymin}..{ymax}], hist: {ys:?}");
        for c in cells.iter().take(40) {
            println!("   ({},{},{})", c.0, c.1, c.2);
        }
        if cells.len() > 40 {
            println!("   ... +{} more", cells.len() - 40);
        }
    }

    // ---- Iron blob clustering (6-connected) vanilla vs neutron ----
    let iron_ores: [(&str, fn(&str) -> bool); 2] = [
        ("iron_ore", |n: &str| n == "minecraft:iron_ore"),
        (
            "deepslate_iron_ore",
            |n: &str| n == "minecraft:deepslate_iron_ore",
        ),
    ];
    for (label, is_iron) in iron_ores {
        for side in ["vanilla", "neutron"] {
            let mut cells: Vec<(i32, i32, i32)> = Vec::new();
            for y in WORLD_BOTTOM..320 {
                for z in 0..16u8 {
                    for x in 0..16u8 {
                        let name = if side == "vanilla" {
                            vanilla
                                .get(&(x, y, z))
                                .map(|s| s.as_str())
                                .unwrap_or("minecraft:air")
                        } else {
                            block_to_name(chunk.block_at(x as u32, y, z as u32))
                        };
                        if is_iron(name) {
                            cells.push((x as i32, y, z as i32));
                        }
                    }
                }
            }
            let clusters = cluster(&cells);
            println!(
                "== {side} {label}: {} cells, {} clusters",
                cells.len(),
                clusters.len()
            );
            let mut cs = clusters.clone();
            cs.sort_by_key(|c| (c[0].1, c[0].0, c[0].2));
            for c in cs {
                let ys: Vec<i32> = c.iter().map(|p| p.1).collect();
                let (mn, mx) = (ys.iter().min().unwrap(), ys.iter().max().unwrap());
                println!(
                    "   blob size {} at x {}..{} y {mn}..{mx} z {}..{}",
                    c.len(),
                    c.iter().map(|p| p.0).min().unwrap(),
                    c.iter().map(|p| p.0).max().unwrap(),
                    c.iter().map(|p| p.2).min().unwrap(),
                    c.iter().map(|p| p.2).max().unwrap(),
                );
            }
        }
    }
}

fn cluster(cells: &[(i32, i32, i32)]) -> Vec<Vec<(i32, i32, i32)>> {
    let set: std::collections::HashSet<(i32, i32, i32)> = cells.iter().copied().collect();
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for &c in cells {
        if seen.contains(&c) {
            continue;
        }
        let mut stack = vec![c];
        let mut comp = Vec::new();
        seen.insert(c);
        while let Some(p) = stack.pop() {
            comp.push(p);
            for d in [
                (1, 0, 0),
                (-1, 0, 0),
                (0, 1, 0),
                (0, -1, 0),
                (0, 0, 1),
                (0, 0, -1),
            ] {
                let q = (p.0 + d.0, p.1 + d.1, p.2 + d.2);
                if set.contains(&q) && !seen.contains(&q) {
                    seen.insert(q);
                    stack.push(q);
                }
            }
        }
        out.push(comp);
    }
    out
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
        BlockId::OakPlanks => "minecraft:oak_planks",
        BlockId::OakFence => "minecraft:oak_fence",
        _ => "minecraft:unknown",
    }
}
