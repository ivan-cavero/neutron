// Run the FULL sculk region decoration (9 origins, vein feature + patches) on
// the VANILLA pre-sculk world (vanilla chunks with sculk* stripped), then
// compare the center chunk against the real vanilla chunk.
// If mismatches collapse, the divergence is input terrain, not the algorithm.
//
// cargo run -p neutron-worldgen --example sculk_vanworld --release [seed] [cx] [cz]

use neutron_worldgen::sculk;
use neutron_worldgen::ChunkGenerator;

include!("sculk_vanworld_world.rs");

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

    // Neutron region pre-sculk, then overlay the stripped vanilla world.
    // REGION5=1 builds a 5x5 so every origin's ±15 patch range sees real
    // blocks (vanilla decorates each origin within its own 3x3 region).
    let g = ChunkGenerator::new(seed);
    let mut region;
    if std::env::var_os("REGION5").is_some() {
        region = neutron_worldgen::region_buf::RegionBuf::new(cx, cz, 2);
        let mut placed = std::collections::HashSet::new();
        for ccz in (cz - 2..=cz + 2).step_by(2) {
            for ccx in (cx - 2..=cx + 2).step_by(2) {
                let sub = g.generate_ores_region(ccx, ccz);
                for dz in -1..=1 {
                    for dx in -1..=1 {
                        let ncx = ccx + dx;
                        let ncz = ccz + dz;
                        if (ncx - cx).abs() > 2 || (ncz - cz).abs() > 2 || !placed.insert((ncx, ncz)) {
                            continue;
                        }
                        let (b, h) = sub.take_chunk(ncx, ncz);
                        region.put_chunk(ncx, ncz, &b, &h);
                    }
                }
            }
        }
        // Overlay vanilla 5x5.
        let region_dir = if seed == 12345 {
            "tools/nbt-ref/vanilla-fresh-12345/world/dimensions/minecraft/overworld/region"
        } else {
            "tools/nbt-ref/vanilla-fresh-424242/world/dimensions/minecraft/overworld/region"
        };
        let mut overlaid = 0u32;
        for ncz in cz - 2..=cz + 2 {
            for ncx in cx - 2..=cx + 2 {
                let rxx = ncx.div_euclid(32);
                let rzz = ncz.div_euclid(32);
                let p = std::path::PathBuf::from(format!("{region_dir}/r.{rxx}.{rzz}.mca"));
                let vreg = Region::open(&p).expect("open region").with_coords(rxx, rzz);
                let ch = load_van_chunk(&vreg, ncx, ncz);
                for ((lx, y, lz), n) in &ch {
                    let b = match n.as_str() {
                        "minecraft:sculk" => neutron_worldgen::surface::BlockId::Deepslate,
                        "minecraft:sculk_vein"
                        | "minecraft:sculk_catalyst"
                        | "minecraft:sculk_sensor"
                        | "minecraft:sculk_shrieker" => neutron_worldgen::surface::BlockId::Air,
                        "minecraft:air" | "minecraft:cave_air" | "minecraft:void_air" => {
                            neutron_worldgen::surface::BlockId::Air
                        }
                        "minecraft:water" => neutron_worldgen::surface::BlockId::Water,
                        other => name_to_block(other),
                    };
                    if *y >= WORLD_BOTTOM && *y < WORLD_TOP {
                        region.set(ncx * 16 + lx, *y, ncz * 16 + lz, b);
                        overlaid += 1;
                    }
                }
            }
        }
        eprintln!("REGION5: overlaid {overlaid} cells");
    } else {
        region = g.generate_ores_region(cx, cz);
        overlay_vanilla_stripped(&mut region, cx, cz, seed);
    }

    // Ground truth: real vanilla chunk (unstripped).
    let region_dir = if seed == 12345 {
        "tools/nbt-ref/vanilla-fresh-12345/world/dimensions/minecraft/overworld/region"
    } else {
        "tools/nbt-ref/vanilla-fresh-424242/world/dimensions/minecraft/overworld/region"
    };
    let rx = cx.div_euclid(32);
    let rz = cz.div_euclid(32);
    let path = std::path::PathBuf::from(format!("{region_dir}/r.{rx}.{rz}.mca"));
    let vregion = Region::open(&path).expect("open vanilla region").with_coords(rx, rz);
    let van_c = load_van_chunk(&vregion, cx, cz);

    // Optional: gate the biome checks with vanilla's real 3D deep_dark biomes.
    let use_van_biomes = std::env::var_os("VAN_BIOMES").is_some();
    if std::env::var_os("GATE_ALL").is_some() {
        sculk::set_biome_gate_override(Some(std::sync::Arc::new(|_x, _y, _z| true)));
        println!("GATE_ALL: biome gate always true");
    }
    if use_van_biomes {
        let mut quarts = std::collections::HashSet::new();
        for dz in -1..=1 {
            for dx in -1..=1 {
                quarts.extend(load_van_deep_dark_quarts(&vregion, cx + dx, cz + dz));
            }
        }
        let quarts = std::sync::Arc::new(quarts);
        println!("vanilla deep_dark quarts={}", quarts.len());
        sculk::set_biome_gate_override(Some(std::sync::Arc::new(
            move |x: i32, y: i32, z: i32| {
                let (qx, qy, qz) = (x.div_euclid(4), y.div_euclid(4), z.div_euclid(4));
                quarts.contains(&(qx, qy, qz))
            },
        )));
    }

    // Full sculk decoration for the 3x3 (identical path to generate_chunk).
    if std::env::var_os("ONE_FLOW").is_some() {
        std::env::set_var("NEUTRON_SCULK_PATCHES", "1");
        std::env::set_var("NEUTRON_SCULK_ONE_ORIGIN", "1");
    }
    sculk::apply_sculk_region(&mut region, &g.state);

    let mut mismatch_kinds: HashMap<String, u32> = HashMap::new();
    let mut sculk_mismatch = 0u32;
    let mut total = 0u32;
    for y in WORLD_BOTTOM..WORLD_TOP {
        for lz in 0..16i32 {
            for lx in 0..16i32 {
                let nb = region.get(cx * 16 + lx, y, cz * 16 + lz);
                let nname = neutron_worldgen::surface::vanilla_name(nb);
                let vname = van_c
                    .get(&(lx, y, lz))
                    .map(|s| s.as_str())
                    .unwrap_or("minecraft:air");
                total += 1;
                if nname != vname {
                    *mismatch_kinds.entry(format!("{vname} -> {nname}")).or_insert(0) += 1;
                    if nname.contains("sculk") || vname.contains("sculk") {
                        sculk_mismatch += 1;
                    }
                }
            }
        }
    }
    println!(
        "ALL mismatched: {} of {total}",
        mismatch_kinds.values().sum::<u32>()
    );
    println!("sculk* mismatches = {sculk_mismatch}");
    if std::env::var_os("DUMP_MISSING").is_some() {
        // where are vanilla-only sculk cells? bucket by nearest chunk origin
        let mut by_origin: HashMap<String, u32> = HashMap::new();
        let mut by_y: HashMap<i32, u32> = HashMap::new();
        for y in WORLD_BOTTOM..WORLD_TOP {
            for lz in 0..16i32 {
                for lx in 0..16i32 {
                    let vname = van_c.get(&(lx, y, lz)).map(|s| s.as_str()).unwrap_or("minecraft:air");
                    let nb = region.get(cx * 16 + lx, y, cz * 16 + lz);
                    let nname = neutron_worldgen::surface::vanilla_name(nb);
                    if vname == "minecraft:sculk" && nname != "minecraft:sculk" {
                        let wx = cx * 16 + lx;
                        let wz = cz * 16 + lz;
                        let ocx = wx.div_euclid(16);
                        let ocz = wz.div_euclid(16);
                        *by_origin.entry(format!("{ocx},{ocz}")).or_insert(0) += 1;
                        *by_y.entry(y).or_insert(0) += 1;
                    }
                }
            }
        }
        let mut o: Vec<_> = by_origin.into_iter().collect();
        o.sort_by(|a, b| b.1.cmp(&a.1));
        println!("vanilla-only sculk by chunk cell: {o:?}");
        let mut yy: Vec<_> = by_y.into_iter().collect();
        yy.sort();
        println!("vanilla-only sculk by y: {yy:?}");
    }
    let mut kinds: Vec<_> = mismatch_kinds.into_iter().collect();
    kinds.sort_by(|a, b| b.1.cmp(&a.1));
    for (k, c) in kinds.iter().take(14) {
        println!("  {c:>6}  {k}");
    }
}
