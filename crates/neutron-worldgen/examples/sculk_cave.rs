// Dump pre-sculk cave around (98,-43,-23) and replay one ChargeCursor patch.
// cargo run -p neutron-worldgen --example sculk_cave --release
//
// Writes tools/worldgen-probe/cave-98-43-23.txt for ProbeSculkPatch.

use neutron_world::nbt::ussr_nbt::owned::{List, Tag};
use neutron_world::nbt::{compound_get, read_nbt};
use neutron_world::Region;
use neutron_worldgen::generator::{WORLD_BOTTOM, WORLD_TOP};
use neutron_worldgen::sculk;
use neutron_worldgen::surface::BlockId;
use neutron_worldgen::ChunkGenerator;
use std::path::PathBuf;

fn load_van_chunk(region: &Region, cx: i32, cz: i32) -> Vec<String> {
    let data = region
        .get_chunk(cx.rem_euclid(32), cz.rem_euclid(32))
        .unwrap()
        .unwrap();
    let nbt = read_nbt(&data).unwrap();
    let sections = match compound_get(&nbt.compound, "sections") {
        Some(Tag::List(List::Compound(l))) => l,
        _ => panic!(),
    };
    let mut van = vec!["air".to_string(); 16 * 384 * 16];
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
                    .unwrap_or_else(|| "air".into())
            };
            let ly = (i >> 8) as i32;
            let lz = ((i >> 4) & 15) as usize;
            let lx = (i & 15) as usize;
            let y = y_sec * 16 + ly;
            let idx = ((y - WORLD_BOTTOM) as usize) * 256 + lz * 16 + lx;
            van[idx] = name;
        }
    }
    van
}

fn load_van_3x3() -> std::collections::HashMap<(i32, i32), Vec<String>> {
    let path = PathBuf::from(
        "tools/nbt-ref/vanilla1/world/dimensions/minecraft/overworld/region/r.0.-1.mca",
    );
    let region = Region::open(&path).unwrap().with_coords(0, -1);
    let mut out = std::collections::HashMap::new();
    for cz in -3..=-1 {
        for cx in 5..=7 {
            out.insert((cx, cz), load_van_chunk(&region, cx, cz));
        }
    }
    out
}

fn van_at(van: &std::collections::HashMap<(i32, i32), Vec<String>>, x: i32, y: i32, z: i32) -> &str {
    let cx = x.div_euclid(16);
    let cz = z.div_euclid(16);
    let Some(chunk) = van.get(&(cx, cz)) else {
        return "air";
    };
    let lx = x.rem_euclid(16) as usize;
    let lz = z.rem_euclid(16) as usize;
    let idx = ((y - WORLD_BOTTOM) as usize) * 256 + lz * 16 + lx;
    chunk.get(idx).map(|s| s.as_str()).unwrap_or("air")
}

fn name(b: BlockId) -> &'static str {
    match b {
        BlockId::Air => "air",
        BlockId::Stone => "stone",
        BlockId::Granite => "granite",
        BlockId::Diorite => "diorite",
        BlockId::Andesite => "andesite",
        BlockId::Dirt => "dirt",
        BlockId::Gravel => "gravel",
        BlockId::Deepslate => "deepslate",
        BlockId::Tuff => "tuff",
        BlockId::Calcite => "calcite",
        BlockId::Water => "water",
        BlockId::Lava => "lava",
        BlockId::Clay => "clay",
        BlockId::Sand => "sand",
        BlockId::CoalOre => "coal_ore",
        BlockId::IronOre => "iron_ore",
        BlockId::CopperOre => "copper_ore",
        BlockId::GoldOre => "gold_ore",
        BlockId::RedstoneOre => "redstone_ore",
        BlockId::LapisOre => "lapis_ore",
        BlockId::DiamondOre => "diamond_ore",
        BlockId::DeepslateCoalOre => "deepslate_coal_ore",
        BlockId::DeepslateIronOre => "deepslate_iron_ore",
        BlockId::DeepslateCopperOre => "deepslate_copper_ore",
        BlockId::DeepslateGoldOre => "deepslate_gold_ore",
        BlockId::DeepslateRedstoneOre => "deepslate_redstone_ore",
        BlockId::DeepslateLapisOre => "deepslate_lapis_ore",
        BlockId::DeepslateDiamondOre => "deepslate_diamond_ore",
        BlockId::SculkVein => "sculk_vein",
        BlockId::Sculk => "sculk",
        BlockId::Bedrock => "bedrock",
        BlockId::RawIronBlock => "raw_iron_block",
        other => {
            eprintln!("unmapped {other:?} -> stone");
            "stone"
        }
    }
}

fn is_sculk_family(n: &str) -> bool {
    n.contains("sculk")
}

fn main() {
    let g = ChunkGenerator::new(12345);
    let mut region = g.generate_ores_region(6, -2);
    let ox = 98;
    let oy = -43;
    let oz = -23;

    let van = load_van_3x3();
    let mut ore_mismatch = std::collections::HashMap::new();
    let mut compared = 0u32;
    for y in (oy - 15)..=(oy + 15) {
        for z in (oz - 15)..=(oz + 15) {
            for x in (ox - 15)..=(ox + 15) {
                let vn = van_at(&van, x, y, z);
                if is_sculk_family(vn) {
                    continue;
                }
                compared += 1;
                let nn = name(region.get(x, y, z));
                if nn != vn {
                    *ore_mismatch
                        .entry(format!("{vn}->{nn}"))
                        .or_insert(0u32) += 1;
                }
            }
        }
    }
    let mut van_names = std::collections::HashMap::new();
    for y in (oy - 15)..=(oy + 15) {
        for z in (oz - 15)..=(oz + 15) {
            for x in (ox - 15)..=(ox + 15) {
                *van_names.entry(van_at(&van, x, y, z).to_string()).or_insert(0u32) += 1;
            }
        }
    }
    let mut vn: Vec<_> = van_names.into_iter().collect();
    vn.sort_by(|a, b| b.1.cmp(&a.1));
    print!("van blocks in r=15 cube:");
    for (k, c) in vn.iter().take(16) {
        print!(" {k}={c}");
    }
    println!();

    println!("ores-only vs van 3x3 (non-sculk, r=15 cube): compared={compared}");
    let mut mm: Vec<_> = ore_mismatch.into_iter().collect();
    mm.sort_by(|a, b| b.1.cmp(&a.1));
    for (k, c) in mm.iter().take(12) {
        println!("  {c:>4}  {k}");
    }

    let ores_only = region.blocks.clone();
    let mut faces = sculk::probe_apply_vein_origin(&mut region, &g.state, 96, -32);
    let mut veins = Vec::new();
    for y in (oy - 15)..=(oy + 15) {
        for z in (oz - 15)..=(oz + 15) {
            for x in (ox - 15)..=(ox + 15) {
                if region.get(x, y, z) == BlockId::SculkVein {
                    veins.push((x, y, z));
                }
            }
        }
    }
    println!(
        "after vein, origin cell={:?}  veins in r=15: {} {:?}",
        region.get(ox, oy, oz),
        veins.len(),
        veins
    );
    for &(x, y, z) in &veins {
        println!("  vein ({x},{y},{z}) van={}", van_at(&van, x, y, z));
    }

    let mut extra = 0u32;
    for y in WORLD_BOTTOM..WORLD_TOP {
        for z in (oz - 15)..=(oz + 15) {
            for x in (ox - 15)..=(ox + 15) {
                let vn = van_at(&van, x, y, z);
                if vn != "deepslate" && vn != "tuff" && !is_sculk_family(vn) {
                    continue;
                }
                if vn.contains("vein") {
                    continue;
                }
                let nb = region.get(x, y, z);
                if is_ore(nb) {
                    extra += 1;
                    if extra <= 8 {
                        println!("  extra ({x},{y},{z}) van={vn} neu={nb:?}");
                    }
                }
            }
        }
    }
    println!("  extra ore cells in r=15 xz (all Y)={extra}");

    let mut repl_neu_only = 0u32;
    let mut repl_van_only = 0u32;
    let mut repl_same = 0u32;
    for y in WORLD_BOTTOM..WORLD_TOP {
        for z in (oz - 15)..=(oz + 15) {
            for x in (ox - 15)..=(ox + 15) {
                let dx = x - ox;
                let dz = z - oz;
                if dx * dx + dz * dz > 15 * 15 {
                    continue;
                }
                let vn = van_at(&van, x, y, z);
                let van_repl = van_was_replaceable(vn);
                let neu_repl = is_repl(region.get(x, y, z));
                match (neu_repl, van_repl) {
                    (true, true) | (false, false) => repl_same += 1,
                    (true, false) => repl_neu_only += 1,
                    (false, true) => {
                        repl_van_only += 1;
                        if repl_van_only <= 8 {
                            println!(
                                "  repl van-only ({x},{y},{z}) van={vn} neu={:?}",
                                region.get(x, y, z)
                            );
                        }
                    }
                }
            }
        }
    }
    println!(
        "replaceable xz-r<=15 3x3: same={repl_same} neu_only={repl_neu_only} van_only={repl_van_only}"
    );

    let (px, py, pz, roll, draws) =
        sculk::probe_real_first_patch(&mut region, &g.state, &mut faces, 96, -32);
    println!("real first patch ({px},{py},{pz}) roll={roll:.6} draws={draws}");

    // A: revert extra ores in the whole 3x3 (van deepslate/tuff/sculk-solid → not ore)
    region.blocks.copy_from_slice(&ores_only);
    let mut reverted = 0u32;
    for cz in -3..=-1 {
        for cx in 5..=7 {
            for y in WORLD_BOTTOM..WORLD_TOP {
                for lz in 0..16 {
                    for lx in 0..16 {
                        let x = cx * 16 + lx;
                        let z = cz * 16 + lz;
                        let vn = van_at(&van, x, y, z);
                        if is_ore(region.get(x, y, z)) && van_was_replaceable(vn) {
                            region.set(x, y, z, strip_to_substrate(vn));
                            reverted += 1;
                        }
                    }
                }
            }
        }
    }
    let mut faces_a = sculk::probe_apply_vein_origin(&mut region, &g.state, 96, -32);
    let (px, py, pz, roll, draws) =
        sculk::probe_real_first_patch(&mut region, &g.state, &mut faces_a, 96, -32);
    println!("A revert extra ores n={reverted}: ({px},{py},{pz}) roll={roll:.6} draws={draws}");

    // B: overlay stripped vanilla 3x3 (catalyst/sensor/shrieker/vein → air, sculk → deepslate)
    region.blocks.copy_from_slice(&ores_only);
    for cz in -3..=-1 {
        for cx in 5..=7 {
            for y in WORLD_BOTTOM..WORLD_TOP {
                for lz in 0..16 {
                    for lx in 0..16 {
                        let x = cx * 16 + lx;
                        let z = cz * 16 + lz;
                        region.set(x, y, z, strip_vanilla(van_at(&van, x, y, z)));
                    }
                }
            }
        }
    }
    println!(
        "B origin cell after strip={:?} van={}",
        region.get(ox, oy, oz),
        van_at(&van, ox, oy, oz)
    );
    let mut faces_b = std::collections::HashMap::new();
    let (px, py, pz, roll, draws) =
        sculk::probe_real_first_patch(&mut region, &g.state, &mut faces_b, 96, -32);
    println!("B overlay stripped van 3x3: ({px},{py},{pz}) roll={roll:.6} draws={draws}");

    // C: only open cells vanilla carved (cave_air/air/water) — tests carver/mineshaft air
    region.blocks.copy_from_slice(&ores_only);
    let mut opened = 0u32;
    let mut by_name = std::collections::HashMap::new();
    let mut opened_center = 0u32;
    for cz in -3..=-1 {
        for cx in 5..=7 {
            for y in WORLD_BOTTOM..16 {
                for lz in 0..16 {
                    for lx in 0..16 {
                        let x = cx * 16 + lx;
                        let z = cz * 16 + lz;
                        let vn = van_at(&van, x, y, z);
                        if matches!(vn, "cave_air" | "air" | "void_air" | "water")
                            && !matches!(
                                region.get(x, y, z),
                                BlockId::Air | BlockId::Water | BlockId::Lava
                            )
                        {
                            *by_name.entry(vn.to_string()).or_insert(0u32) += 1;
                            if cx == 6 && cz == -2 {
                                opened_center += 1;
                            }
                            region.set(
                                x,
                                y,
                                z,
                                if vn == "water" {
                                    BlockId::Water
                                } else {
                                    BlockId::Air
                                },
                            );
                            opened += 1;
                        }
                    }
                }
            }
        }
    }
    let mut bn: Vec<_> = by_name.into_iter().collect();
    bn.sort_by(|a, b| b.1.cmp(&a.1));
    print!("C opened by van name (center={opened_center}):");
    for (k, c) in &bn {
        print!(" {k}={c}");
    }
    println!();
    let mut near_shaft = 0u32;
    let mut planks = [0u32; 9];
    let mut i = 0usize;
    for cz in -3..=-1 {
        for cx in 5..=7 {
            for y in WORLD_BOTTOM..16 {
                for lz in 0..16 {
                    for lx in 0..16 {
                        let x = cx * 16 + lx;
                        let z = cz * 16 + lz;
                        let vn = van_at(&van, x, y, z);
                        if vn == "oak_planks" || vn == "oak_fence" || vn == "rail" {
                            planks[i] += 1;
                        }
                    }
                }
            }
            i += 1;
        }
    }
    println!(
        "mineshaft blocks y<16 by chunk (cx 5..7, cz -3..-1): {:?}",
        planks
    );
    for cz in -3..=-1 {
        for cx in 5..=7 {
            for y in WORLD_BOTTOM..16 {
                for lz in 0..16 {
                    for lx in 0..16 {
                        let x = cx * 16 + lx;
                        let z = cz * 16 + lz;
                        let vn = van_at(&van, x, y, z);
                        if vn != "cave_air" {
                            continue;
                        }
                        let mut adj = false;
                        for (dx, dy, dz) in [
                            (1, 0, 0),
                            (-1, 0, 0),
                            (0, 1, 0),
                            (0, -1, 0),
                            (0, 0, 1),
                            (0, 0, -1),
                        ] {
                            let n = van_at(&van, x + dx, y + dy, z + dz);
                            if n == "oak_planks" || n == "oak_fence" || n == "rail" {
                                adj = true;
                                break;
                            }
                        }
                        if adj {
                            near_shaft += 1;
                        }
                    }
                }
            }
        }
    }
    println!("missing cave_air adjacent to planks/fence/rail={near_shaft}");
    let mut faces_c = sculk::probe_apply_vein_origin(&mut region, &g.state, 96, -32);
    let (px, py, pz, roll, draws) =
        sculk::probe_real_first_patch(&mut region, &g.state, &mut faces_c, 96, -32);
    println!("C open van cave_air n={opened}: ({px},{py},{pz}) roll={roll:.6} draws={draws}");

    // D: only open missing cave_air next to mineshaft pieces
    region.blocks.copy_from_slice(&ores_only);
    let mut d_n = 0u32;
    for cz in -3..=-1 {
        for cx in 5..=7 {
            for y in WORLD_BOTTOM..16 {
                for lz in 0..16 {
                    for lx in 0..16 {
                        let x = cx * 16 + lx;
                        let z = cz * 16 + lz;
                        if van_at(&van, x, y, z) != "cave_air" {
                            continue;
                        }
                        if matches!(
                            region.get(x, y, z),
                            BlockId::Air | BlockId::Water | BlockId::Lava
                        ) {
                            continue;
                        }
                        let mut adj = false;
                        for (dx, dy, dz) in [
                            (1, 0, 0),
                            (-1, 0, 0),
                            (0, 1, 0),
                            (0, -1, 0),
                            (0, 0, 1),
                            (0, 0, -1),
                        ] {
                            let n = van_at(&van, x + dx, y + dy, z + dz);
                            if n == "oak_planks" || n == "oak_fence" || n == "rail" {
                                adj = true;
                                break;
                            }
                        }
                        if adj {
                            region.set(x, y, z, BlockId::Air);
                            d_n += 1;
                        }
                    }
                }
            }
        }
    }
    let mut faces_d = sculk::probe_apply_vein_origin(&mut region, &g.state, 96, -32);
    let (px, py, pz, roll, draws) =
        sculk::probe_real_first_patch(&mut region, &g.state, &mut faces_d, 96, -32);
    println!("D open shaft-adj cave_air n={d_n}: ({px},{py},{pz}) roll={roll:.6} draws={draws}");

    // Per-chunk missing air/cave_air (ores-only vs van), y<16.
    region.blocks.copy_from_slice(&ores_only);
    println!("missing openings y<16 per chunk (air/cave_air we still solid):");
    for cz in -3..=-1 {
        for cx in 5..=7 {
            let mut cave = 0u32;
            let mut air = 0u32;
            let mut water = 0u32;
            for y in WORLD_BOTTOM..16 {
                for lz in 0..16 {
                    for lx in 0..16 {
                        let x = cx * 16 + lx;
                        let z = cz * 16 + lz;
                        let vn = van_at(&van, x, y, z);
                        if matches!(
                            region.get(x, y, z),
                            BlockId::Air | BlockId::Water | BlockId::Lava
                        ) {
                            continue;
                        }
                        match vn {
                            "cave_air" => cave += 1,
                            "air" | "void_air" => air += 1,
                            "water" => water += 1,
                            _ => {}
                        }
                    }
                }
            }
            println!("  ({cx},{cz}) cave_air={cave} air={air} water={water}");
        }
    }

    // (6,-3) missing `air`: Y histogram + our block + xz bbox
    let mut yhist = std::collections::HashMap::new();
    let mut our_block = std::collections::HashMap::new();
    let mut minx = i32::MAX;
    let mut maxx = i32::MIN;
    let mut minz = i32::MAX;
    let mut maxz = i32::MIN;
    let mut in_r15 = 0u32;
    for y in WORLD_BOTTOM..16 {
        for lz in 0..16 {
            for lx in 0..16 {
                let x = 96 + lx;
                let z = -48 + lz;
                if van_at(&van, x, y, z) != "air" {
                    continue;
                }
                if matches!(
                    region.get(x, y, z),
                    BlockId::Air | BlockId::Water | BlockId::Lava
                ) {
                    continue;
                }
                *yhist.entry(y).or_insert(0u32) += 1;
                *our_block.entry(name(region.get(x, y, z))).or_insert(0u32) += 1;
                minx = minx.min(x);
                maxx = maxx.max(x);
                minz = minz.min(z);
                maxz = maxz.max(z);
                let dx = x - ox;
                let dz = z - oz;
                if dx * dx + dz * dz <= 15 * 15 {
                    in_r15 += 1;
                }
            }
        }
    }
    let mut yh: Vec<_> = yhist.into_iter().collect();
    yh.sort_by_key(|(y, _)| *y);
    print!("(6,-3) missing air Y:");
    for (y, c) in &yh {
        print!(" {y}:{c}");
    }
    println!();
    println!(
        "(6,-3) missing air our-block={our_block:?} bbox=[{minx}..{maxx},{minz}..{maxz}] in_r15={in_r15}"
    );

    // E: open all missing air/cave_air/water in mineshaft chunks (5,-2) and (5,-1)
    region.blocks.copy_from_slice(&ores_only);
    let mut e_n = 0u32;
    let mut e_r15 = 0u32;
    for &(cx, cz) in &[(5, -2), (5, -1)] {
        for y in WORLD_BOTTOM..16 {
            for lz in 0..16 {
                for lx in 0..16 {
                    let x = cx * 16 + lx;
                    let z = cz * 16 + lz;
                    let vn = van_at(&van, x, y, z);
                    if !matches!(vn, "cave_air" | "air" | "void_air" | "water") {
                        continue;
                    }
                    if matches!(
                        region.get(x, y, z),
                        BlockId::Air | BlockId::Water | BlockId::Lava
                    ) {
                        continue;
                    }
                    region.set(
                        x,
                        y,
                        z,
                        if vn == "water" {
                            BlockId::Water
                        } else {
                            BlockId::Air
                        },
                    );
                    e_n += 1;
                    let dx = x - ox;
                    let dz = z - oz;
                    if dx * dx + dz * dz <= 15 * 15 {
                        e_r15 += 1;
                    }
                }
            }
        }
    }
    let mut faces_e = sculk::probe_apply_vein_origin(&mut region, &g.state, 96, -32);
    let (px, py, pz, roll, draws) =
        sculk::probe_real_first_patch(&mut region, &g.state, &mut faces_e, 96, -32);
    println!(
        "E open mineshaft-chunk air n={e_n} in_r15={e_r15}: ({px},{py},{pz}) roll={roll:.6} draws={draws}"
    );
}

fn is_ore(b: BlockId) -> bool {
    matches!(
        b,
        BlockId::DeepslateRedstoneOre
            | BlockId::DeepslateIronOre
            | BlockId::DeepslateDiamondOre
            | BlockId::DeepslateGoldOre
            | BlockId::DeepslateLapisOre
            | BlockId::DeepslateCopperOre
            | BlockId::DeepslateCoalOre
            | BlockId::RedstoneOre
            | BlockId::IronOre
            | BlockId::DiamondOre
            | BlockId::GoldOre
            | BlockId::LapisOre
            | BlockId::CopperOre
            | BlockId::CoalOre
    )
}

fn van_was_replaceable(vn: &str) -> bool {
    if vn.contains("vein") {
        return false;
    }
    is_sculk_family(vn)
        || matches!(
            vn,
            "stone"
                | "granite"
                | "diorite"
                | "andesite"
                | "dirt"
                | "gravel"
                | "sand"
                | "clay"
                | "calcite"
                | "tuff"
                | "deepslate"
        )
}

fn strip_to_substrate(vn: &str) -> BlockId {
    if vn == "tuff" {
        BlockId::Tuff
    } else {
        BlockId::Deepslate
    }
}

fn strip_vanilla(vn: &str) -> BlockId {
    if vn.contains("vein")
        || vn == "sculk_catalyst"
        || vn == "sculk_sensor"
        || vn == "sculk_shrieker"
    {
        return BlockId::Air;
    }
    if vn == "sculk" {
        return BlockId::Deepslate;
    }
    from_name(vn)
}

fn is_repl(b: BlockId) -> bool {
    matches!(
        b,
        BlockId::Stone
            | BlockId::Granite
            | BlockId::Diorite
            | BlockId::Andesite
            | BlockId::Dirt
            | BlockId::Gravel
            | BlockId::Sand
            | BlockId::Clay
            | BlockId::Calcite
            | BlockId::Tuff
            | BlockId::Deepslate
    )
}

fn from_name(n: &str) -> BlockId {
    match n {
        "air" => BlockId::Air,
        "stone" => BlockId::Stone,
        "granite" => BlockId::Granite,
        "diorite" => BlockId::Diorite,
        "andesite" => BlockId::Andesite,
        "dirt" => BlockId::Dirt,
        "gravel" => BlockId::Gravel,
        "deepslate" => BlockId::Deepslate,
        "tuff" => BlockId::Tuff,
        "calcite" => BlockId::Calcite,
        "water" => BlockId::Water,
        "lava" => BlockId::Lava,
        "clay" => BlockId::Clay,
        "sand" => BlockId::Sand,
        "coal_ore" => BlockId::CoalOre,
        "iron_ore" => BlockId::IronOre,
        "copper_ore" => BlockId::CopperOre,
        "gold_ore" => BlockId::GoldOre,
        "redstone_ore" => BlockId::RedstoneOre,
        "lapis_ore" => BlockId::LapisOre,
        "diamond_ore" => BlockId::DiamondOre,
        "deepslate_coal_ore" => BlockId::DeepslateCoalOre,
        "deepslate_iron_ore" => BlockId::DeepslateIronOre,
        "deepslate_copper_ore" => BlockId::DeepslateCopperOre,
        "deepslate_gold_ore" => BlockId::DeepslateGoldOre,
        "deepslate_redstone_ore" => BlockId::DeepslateRedstoneOre,
        "deepslate_lapis_ore" => BlockId::DeepslateLapisOre,
        "deepslate_diamond_ore" => BlockId::DeepslateDiamondOre,
        "bedrock" => BlockId::Bedrock,
        "raw_iron_block" => BlockId::RawIronBlock,
        "cave_air" | "void_air" => BlockId::Air,
        "moss_block" => BlockId::MossBlock,
        "dark_oak_leaves" => BlockId::DarkOakLeaves,
        "dark_oak_log" => BlockId::DarkOakLog,
        "oak_leaves" => BlockId::OakLeaves,
        "oak_log" => BlockId::OakLog,
        "sculk" => BlockId::Sculk,
        "sculk_vein" => BlockId::SculkVein,
        "sculk_catalyst" => BlockId::SculkCatalyst,
        "sculk_sensor" => BlockId::SculkSensor,
        "sculk_shrieker" => BlockId::SculkShrieker,
        other => {
            eprintln!("unmapped van '{other}' -> stone");
            BlockId::Stone
        }
    }
}
