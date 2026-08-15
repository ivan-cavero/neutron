// Shared helper for sculk differential examples: load vanilla 3x3 chunks from
// the reference world and overlay the stripped (pre-sculk) version onto a
// neutron region. Included by sculk_vanworld.rs and sculk_veintrace.rs.

use neutron_world::nbt::ussr_nbt::owned::{List, Tag};
use neutron_world::nbt::{compound_get, read_nbt};
use neutron_world::Region;
use neutron_worldgen::region_buf::RegionBuf;
use neutron_worldgen::surface::BlockId;
use std::collections::HashMap;
use std::path::PathBuf;
use neutron_worldgen::generator::{WORLD_BOTTOM, WORLD_TOP};

pub fn name_to_block(n: &str) -> BlockId {
    match n {
        "minecraft:stone" => BlockId::Stone,
        "minecraft:granite" => BlockId::Granite,
        "minecraft:diorite" => BlockId::Diorite,
        "minecraft:andesite" => BlockId::Andesite,
        "minecraft:dirt" => BlockId::Dirt,
        "minecraft:gravel" => BlockId::Gravel,
        "minecraft:deepslate" => BlockId::Deepslate,
        "minecraft:tuff" => BlockId::Tuff,
        "minecraft:calcite" => BlockId::Calcite,
        "minecraft:water" => BlockId::Water,
        "minecraft:lava" => BlockId::Lava,
        "minecraft:clay" => BlockId::Clay,
        "minecraft:bedrock" => BlockId::Bedrock,
        "minecraft:coal_ore" => BlockId::CoalOre,
        "minecraft:iron_ore" => BlockId::IronOre,
        "minecraft:copper_ore" => BlockId::CopperOre,
        "minecraft:gold_ore" => BlockId::GoldOre,
        "minecraft:redstone_ore" => BlockId::RedstoneOre,
        "minecraft:lapis_ore" => BlockId::LapisOre,
        "minecraft:diamond_ore" => BlockId::DiamondOre,
        "minecraft:deepslate_coal_ore" => BlockId::DeepslateCoalOre,
        "minecraft:deepslate_iron_ore" => BlockId::DeepslateIronOre,
        "minecraft:deepslate_copper_ore" => BlockId::DeepslateCopperOre,
        "minecraft:deepslate_gold_ore" => BlockId::DeepslateGoldOre,
        "minecraft:deepslate_redstone_ore" => BlockId::DeepslateRedstoneOre,
        "minecraft:deepslate_lapis_ore" => BlockId::DeepslateLapisOre,
        "minecraft:deepslate_diamond_ore" => BlockId::DeepslateDiamondOre,
        "minecraft:raw_iron_block" => BlockId::RawIronBlock,
        "minecraft:raw_copper_block" => BlockId::RawCopperBlock,
        "minecraft:moss_block" => BlockId::MossBlock,
        "minecraft:grass_block" => BlockId::GrassBlock,
        "minecraft:oak_log" => BlockId::OakLog,
        "minecraft:dark_oak_log" => BlockId::DarkOakLog,
        "minecraft:oak_leaves" => BlockId::OakLeaves,
        "minecraft:dark_oak_leaves" => BlockId::DarkOakLeaves,
        "minecraft:short_grass" => BlockId::ShortGrass,
        "minecraft:leaf_litter" => BlockId::LeafLitter,
        "minecraft:oak_planks" => BlockId::OakPlanks,
        "minecraft:oak_fence" => BlockId::OakFence,
        "minecraft:cobblestone" => BlockId::Cobblestone,
        "minecraft:snow_block" => BlockId::Snow,
        "minecraft:coarse_dirt" => BlockId::CoarseDirt,
        "minecraft:podzol" => BlockId::Podzol,
        "minecraft:mycelium" => BlockId::Mycelium,
        "minecraft:red_sand" => BlockId::RedSand,
        "minecraft:sand" => BlockId::Sand,
        "minecraft:sandstone" => BlockId::Sandstone,
        "minecraft:red_sandstone" => BlockId::RedSandstone,
        "minecraft:ice" => BlockId::Ice,
        "minecraft:packed_ice" => BlockId::PackedIce,
        "minecraft:blue_ice" => BlockId::BlueIce,
        other => {
            if !matches!(
                other,
                "minecraft:birch_leaves"
                    | "minecraft:kelp_plant"
                    | "minecraft:red_mushroom_block"
                    | "minecraft:moss_carpet"
                    | "minecraft:cobweb"
                    | "minecraft:seagrass"
                    | "minecraft:tall_seagrass"
                    | "minecraft:brown_mushroom_block"
                    | "minecraft:mushroom_stem"
            ) {
                eprintln!("unmapped vanilla block {other:?} -> air");
            }
            BlockId::Air
        }
    }
}

/// Vanilla chunk as (local x, y, z) -> full block name.
pub fn load_van_chunk(
    region: &Region,
    cx: i32,
    cz: i32,
) -> HashMap<(i32, i32, i32), String> {
    let data = region
        .get_chunk(cx.rem_euclid(32), cz.rem_euclid(32))
        .unwrap()
        .unwrap();
    let nbt = read_nbt(&data).unwrap();
    let sections = match compound_get(&nbt.compound, "sections") {
        Some(Tag::List(List::Compound(l))) => l,
        _ => panic!("no sections"),
    };
    let mut out = HashMap::new();
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
                let lz = ((i >> 4) & 15) as i32;
                let lx = (i & 15) as i32;
                out.insert((lx, y_sec * 16 + ly, lz), names[0].clone());
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
            out.insert(
                (i as i32 & 15, y_sec * 16 + ((i >> 8) as i32), ((i >> 4) & 15) as i32),
                names
                    .get(idx as usize)
                    .cloned()
                    .unwrap_or_else(|| "minecraft:air".into()),
            );
        }
    }
    out
}

/// Vanilla deep_dark biomes at quart resolution for one chunk, from NBT.
/// Returns a set of (quart_x, quart_y_global, quart_z) that are deep_dark.
pub fn load_van_deep_dark_quarts(
    region: &Region,
    cx: i32,
    cz: i32,
) -> std::collections::HashSet<(i32, i32, i32)> {
    let data = region
        .get_chunk(cx.rem_euclid(32), cz.rem_euclid(32))
        .unwrap()
        .unwrap();
    let nbt = read_nbt(&data).unwrap();
    let sections = match compound_get(&nbt.compound, "sections") {
        Some(Tag::List(List::Compound(l))) => l,
        _ => panic!("no sections"),
    };
    let mut out = std::collections::HashSet::new();
    for sec in sections {
        let y_sec = match compound_get(sec, "Y") {
            Some(Tag::Byte(y)) => *y as i8 as i32,
            Some(Tag::Int(y)) => *y,
            _ => continue,
        };
        let Some(Tag::Compound(bio)) = compound_get(sec, "biomes") else {
            continue;
        };
        let Some(Tag::List(List::String(pal))) = compound_get(bio, "palette") else {
            continue;
        };
        let dd_index: Vec<usize> = pal
            .iter()
            .enumerate()
            .filter(|(_, s)| s.to_string() == "minecraft:deep_dark")
            .map(|(i, _)| i)
            .collect();
        if dd_index.is_empty() {
            continue;
        }
        if pal.len() == 1 {
            // whole section deep_dark: quarts y 0..4 of this section
            for qy in 0..4 {
                for qz in 0..4 {
                    for qx in 0..4 {
                        out.insert((
                            cx * 4 + qx,
                            y_sec * 4 + qy,
                            cz * 4 + qz,
                        ));
                    }
                }
            }
            continue;
        }
        // Biome palettes have NO 4-bit minimum: 2 entries pack at 1 bit.
        let bits = if pal.len() > 1 {
            ((pal.len() - 1).ilog2() + 1) as u32
        } else {
            0
        };
        let Some(Tag::LongArray(data)) = compound_get(bio, "data") else {
            continue;
        };
        let longs: Vec<i64> = data.to_vec();
        let epl = 64 / bits;
        let mask = (1u64 << bits) - 1;
        for i in 0..64u32 {
            let li = (i / epl) as usize;
            let bo = (i % epl) * bits;
            let idx = ((longs[li] as u64) >> bo) & mask;
            if dd_index.contains(&(idx as usize)) {
                let qy = (i / 16) as i32;
                let qz = ((i % 16) / 4) as i32;
                let qx = (i % 4) as i32;
                out.insert((cx * 4 + qx, y_sec * 4 + qy, cz * 4 + qz));
            }
        }
    }
    out
}

/// Overlay the vanilla pre-sculk world (sculk* stripped) onto the region.
pub fn overlay_vanilla_stripped(region: &mut RegionBuf, cx: i32, cz: i32, seed: i64) {
    let region_dir = if seed == 12345 {
        "tools/nbt-ref/vanilla-fresh-12345/world/dimensions/minecraft/overworld/region"
    } else {
        "tools/nbt-ref/vanilla-fresh-424242/world/dimensions/minecraft/overworld/region"
    };
    let rx = cx.div_euclid(32);
    let rz = cz.div_euclid(32);
    let path = PathBuf::from(format!("{region_dir}/r.{rx}.{rz}.mca"));
    let vregion = Region::open(&path).expect("open vanilla region").with_coords(rx, rz);
    let mut overlaid = 0u32;
    for dz in -1..=1 {
        for dx in -1..=1 {
            let ch = load_van_chunk(&vregion, cx + dx, cz + dz);
            for ((lx, y, lz), n) in &ch {
                let b = match n.as_str() {
                    "minecraft:sculk" => BlockId::Deepslate,
                    "minecraft:sculk_vein"
                    | "minecraft:sculk_catalyst"
                    | "minecraft:sculk_sensor"
                    | "minecraft:sculk_shrieker" => BlockId::Air,
                    "minecraft:air" | "minecraft:cave_air" | "minecraft:void_air" => BlockId::Air,
                    "minecraft:water" => BlockId::Water,
                    other => name_to_block(other),
                };
                let x = (cx + dx) * 16 + lx;
                let z = (cz + dz) * 16 + lz;
                if *y >= WORLD_BOTTOM && *y < WORLD_TOP {
                    region.set(x, *y, z, b);
                    overlaid += 1;
                }
            }
        }
    }
    eprintln!("overlaid {overlaid} cells of vanilla-stripped world");
}
