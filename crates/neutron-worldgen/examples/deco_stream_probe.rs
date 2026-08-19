//! B4 T3 diagnostic: run the REAL neutron placement chain (placement
//! modifiers + tree dispatch) against the VANILLA terrain loaded from the
//! reference .mca files. The tree placement consumes RNG based on the
//! region's blocks (isAirOrLeaves checks), so running it against the vanilla
//! terrain yields the EXACT vanilla RNG consumption — the tool to derive the
//! vanilla decoration stream and the origin order (T3 desync).
//!
//! Usage: deco_stream_probe [seed] [cx] [cz] [placed_feature] [region_dir]
//! Set NEUTRON_TRACE_TREES=1 to see per-draw accept/reject.
use neutron_world::nbt::ussr_nbt::owned::{List, Tag};
use neutron_world::nbt::{compound_get, read_nbt};
use neutron_world::Region;
use neutron_worldgen::feature_rng::FeatureRandom;
use neutron_worldgen::region_buf::RegionBuf;
use neutron_worldgen::surface::BlockId;
use neutron_worldgen::ChunkGenerator;
use std::path::PathBuf;

/// Vanilla pale_oak trunk base positions for a chunk (ground truth for the
/// draw->tree mapping in the before-set derivation).
fn vanilla_trunks(region_dir: &str, cx: i32, cz: i32) -> Vec<(i32, i32, i32)> {
    let Some(blocks) = load_vanilla_blocks(region_dir, cx, cz) else {
        return Vec::new();
    };
    let wb = neutron_worldgen::generator::WORLD_BOTTOM;
    let mut out = Vec::new();
    for lz in 0..16i32 {
        for lx in 0..16i32 {
            for ly in 0..384i32 {
                let bi = (ly * 256 + lz * 16 + lx) as usize;
                if blocks[bi] == BlockId::PaleOakLog.as_u16() {
                    let y = wb + ly;
                    // find the base: lowest log in the column
                    let mut by = y;
                    while by > wb
                        && blocks[((by - 1 - wb) * 256 + lz * 16 + lx) as usize]
                            == BlockId::PaleOakLog.as_u16()
                    {
                        by -= 1;
                    }
                    out.push((cx * 16 + lx, by, cz * 16 + lz));
                    break;
                }
            }
        }
    }
    out
}

/// Load one vanilla chunk's blocks (names) into a 16×384×16 u16 vec.
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
        if names.is_empty() {
            continue;
        }
        let bits = if names.len() <= 1 {
            0
        } else {
            ((names.len() - 1).ilog2() + 1).max(4) as u32
        };
        match compound_get(bs, "data") {
            Some(Tag::LongArray(data)) => {
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
                    let bid = BlockId::from_name(name.strip_prefix("minecraft:").unwrap_or(&name))
                        .map(|b| b.as_u16())
                        .unwrap_or(BlockId::Air.as_u16());
                    let bi = ((y_sec * 16 + ly - wb) * 256 + lz as i32 * 16 + lx as i32) as usize;
                    blocks[bi] = bid;
                }
            }
            _ => {
                let bid = names[0]
                    .strip_prefix("minecraft:")
                    .and_then(BlockId::from_name)
                    .map(|b| b.as_u16())
                    .unwrap_or(BlockId::Air.as_u16());
                for ly in 0..16 {
                    for lz in 0..16 {
                        for lx in 0..16 {
                            let bi = ((y_sec * 16 + ly - wb) * 256 + lz * 16 + lx) as usize;
                            blocks[bi] = bid;
                        }
                    }
                }
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
    let placed_id = args
        .next()
        .unwrap_or_else(|| "minecraft:pale_garden_vegetation".to_string());
    let region_dir = args.next().unwrap_or_else(|| {
        "tools/nbt-ref/vanilla-fresh-424242/world/dimensions/minecraft/overworld/region".to_string()
    });

    let gen = ChunkGenerator::new(seed);
    // NEUTRON_DECO_SURFACE=1: diff the surface columns (vanilla vs neutron)
    // for the center chunk; print the mismatch columns + biomes.
    if std::env::var("NEUTRON_DECO_SURFACE").is_ok() {
        let Some(van) = load_vanilla_blocks(&region_dir, cx, cz) else {
            eprintln!("no vanilla blocks");
            return;
        };
        let chunk = gen.generate_chunk_cached(cx, cz, &mut neutron_worldgen::NoiseCache::new());
        let wb = neutron_worldgen::generator::WORLD_BOTTOM;
        let mut n = 0usize;
        for lz in 0..16i32 {
            for lx in 0..16i32 {
                // find the top solid in vanilla and neutron
                let mut vy = wb;
                for y in (wb..wb + 384).rev() {
                    let bi = ((y - wb) * 256 + lz * 16 + lx) as usize;
                    if van[bi] != BlockId::Air.as_u16() {
                        vy = y;
                        break;
                    }
                }
                let mut ny = wb;
                for y in (wb..wb + 384).rev() {
                    let b = chunk.block_at(lx as u32, y, lz as u32);
                    if b != BlockId::Air {
                        ny = y;
                        break;
                    }
                }
                let vb = BlockId::from_u16(van[((vy - wb) * 256 + lz * 16 + lx) as usize])
                    .unwrap_or(BlockId::Air);
                let nb = chunk.block_at(lx as u32, ny, lz as u32);
                if vb != nb || (vy - ny).abs() > 1 {
                    let vbi = neutron_worldgen::biome_source::biome_id_at_block(
                        &gen.state,
                        cx * 16 + lx,
                        vy,
                        cz * 16 + lz,
                    );
                    let nbi = neutron_worldgen::biome_source::biome_id_at_block(
                        &gen.state,
                        cx * 16 + lx,
                        ny,
                        cz * 16 + lz,
                    );
                    println!(
                        "({},{}) van={}@{vy} neu={}@{ny} vbiome={vbi} nbiome={nbi}",
                        cx * 16 + lx,
                        cz * 16 + lz,
                        neutron_worldgen::surface::vanilla_name(vb),
                        neutron_worldgen::surface::vanilla_name(nb),
                    );
                    n += 1;
                }
            }
        }
        eprintln!("surface mismatches: {n}");
        return;
    }
    // NEUTRON_DECO_COL=lx,lz: print the neutron column blocks.
    if let Ok(c) = std::env::var("NEUTRON_DECO_COL") {
        let mut it = c.split(',');
        let lx: i32 = it.next().unwrap().parse().unwrap();
        let lz: i32 = it.next().unwrap().parse().unwrap();
        let chunk = gen.generate_chunk_cached(cx, cz, &mut neutron_worldgen::NoiseCache::new());
        let Some(van) = load_vanilla_blocks(&region_dir, cx, cz) else { return; };
        for y in 95..120 {
            let vb = BlockId::from_u16(van[((y - neutron_worldgen::generator::WORLD_BOTTOM) * 256 + lz * 16 + lx) as usize]).unwrap_or(BlockId::Air);
            let nb = chunk.block_at(lx as u32, y, lz as u32);
            println!("y={y} van={} neu={}", neutron_worldgen::surface::vanilla_name(vb), neutron_worldgen::surface::vanilla_name(nb));
        }
        return;
    }
    // NEUTRON_DECO_CLIMATE=1: print the neutron climate targets at (1,y,1)
    // for y in 56..=104 step 4 (compare vs ProbeClimateTarget).
    if std::env::var("NEUTRON_DECO_CLIMATE").is_ok() {
        for y in (56..=104).step_by(4) {
            let qy = y >> 2;
            let t = neutron_worldgen::biome_manager::climate_at(
                &gen.state,
                (1 << 2),
                (qy << 2),
                (1 << 2),
            );
            let id = neutron_worldgen::biome_source::biome_id_at_block(&gen.state, 4, y, 4);
            println!(
                "y={y} t={} h={} c={} e={} d={} w={} biome_id={id}",
                t.temperature, t.humidity, t.continentalness, t.erosion, t.depth, t.weirdness
            );
        }
        return;
    }
    // Optional: print the vanilla trunk bases (2x2 NW-corner candidates) for
    // the center chunk (the ground truth for the draw->tree mapping.
    if std::env::var("NEUTRON_DECO_TRUNKS").is_ok() {
        let trunks = vanilla_trunks(&region_dir, cx, cz);
        for (x, y, z) in trunks {
            println!("trunk ({x},{y},{z})");
        }
        return;
    }
    // B4 T3 before-set derivation: strip vegetal output of selected 3×3
    // neighbours ("after" chunks — at CARVERS in vanilla when the center
    // decorates) plus optionally the center's own trees (absent at its draw
    // time), then run the placement chain and compare accepted draws against
    // the vanilla trunks of the center chunk.
    //
    // NEUTRON_DECO_STRIP_AFTER="dx,dz;dx,dz;..." — neighbour offsets whose
    // vegetal-family blocks are masked to base terrain (stone/deepslate).
    // NEUTRON_DECO_STRIP_CENTER_TREES=1 — mask the center's own logs+leaves
    // (keeps moss/carpets: placed earlier in the same origin's step-9 pass).
    // Output: per-draw accept with NEUTRON_TRACE_TREES=1, then a per-position
    // comparison against the vanilla trunks (present / missing / extra).
    if std::env::var("NEUTRON_DECO_STRIP_AFTER").is_ok() || std::env::var("NEUTRON_DECO_STRIP_CENTER_TREES").is_ok() {
        let strip_after: Vec<(i32, i32)> = std::env::var("NEUTRON_DECO_STRIP_AFTER")
            .map(|s| {
                s.split(';')
                    .filter(|p| !p.is_empty())
                    .map(|p| {
                        let mut it = p.split(',');
                        let dx: i32 = it.next().unwrap().parse().unwrap();
                        let dz: i32 = it.next().unwrap().parse().unwrap();
                        (dx, dz)
                    })
                    .collect()
            })
            .unwrap_or_default();
        let strip_center = std::env::var("NEUTRON_DECO_STRIP_CENTER_TREES").is_ok();

        let mut region = RegionBuf::new(cx, cz, 2);
        let mut missing = Vec::new();
        for dz in -2..=2 {
            for dx in -2..=2 {
                let ncx = cx + dx;
                let ncz = cz + dz;
                match load_vanilla_blocks(&region_dir, ncx, ncz) {
                    Some(b) => region.put_chunk(ncx, ncz, &b, &vec![0i16; 256]),
                    None => missing.push((ncx, ncz)),
                }
            }
        }
        if !missing.is_empty() {
            eprintln!("missing chunks: {missing:?}");
            return;
        }

        let wb = neutron_worldgen::generator::WORLD_BOTTOM;
        let mut stripped = 0usize;
        for &(dx, dz) in &strip_after {
            let x0 = cx * 16 + dx * 16;
            let z0 = cz * 16 + dz * 16;
            for y in wb..neutron_worldgen::generator::WORLD_TOP {
                for z in z0..z0 + 16 {
                    for x in x0..x0 + 16 {
                        let b = region.get(x, y, z);
                        if neutron_worldgen::sculk::is_vegetal_family(b) {
                            region.set(
                                x,
                                y,
                                z,
                                if y < 0 { BlockId::Deepslate } else { BlockId::Stone },
                            );
                            stripped += 1;
                        }
                    }
                }
            }
        }
        if strip_center {
            // Center's own vegetal output is placed AFTER the trees in the
            // step-9 sorter (pale_moss_patch idx 14 > pale_garden_vegetation
            // idx 13, flowers/grass higher) — absent at tree-draw time.
            // Glow_lichen (idx 0) is not in the vegetal family (kept).
            let x0 = cx * 16;
            let z0 = cz * 16;
            for y in wb..neutron_worldgen::generator::WORLD_TOP {
                for z in z0..z0 + 16 {
                    for x in x0..x0 + 16 {
                        let b = region.get(x, y, z);
                        if neutron_worldgen::sculk::is_vegetal_family(b) {
                            region.set(
                                x,
                                y,
                                z,
                                if y < 0 { BlockId::Deepslate } else { BlockId::Stone },
                            );
                            stripped += 1;
                        }
                    }
                }
            }
        }
        eprintln!("stripped blocks: {stripped}");

        let mut rng = FeatureRandom::new(seed);
        let dec = rng.set_decoration_seed(seed, cx * 16, cz * 16);
        let idx = neutron_worldgen::feature_catalog::global_feature_index(9, &placed_id)
            .expect("feature in step 9 sorter");
        rng.set_feature_seed(dec, idx, 9);
        eprintln!(
            "beforeset seed={seed} chunk=({cx},{cz}) placed={placed_id} index={idx} strip_after={strip_after:?} strip_center_trees={strip_center}"
        );
        neutron_worldgen::feature_dispatch::place_placed_feature(
            &mut rng,
            &mut region,
            &gen.state,
            cx * 16,
            cz * 16,
            &placed_id,
        );

        // Compare probe accepted tree positions vs vanilla trunks (center).
        let vanilla = vanilla_trunks(&region_dir, cx, cz);
        let wb = neutron_worldgen::generator::WORLD_BOTTOM;
        let mut accepted: Vec<(i32, i32)> = Vec::new();
        for z in 0..16i32 {
            for x in 0..16i32 {
                for y in (wb..wb + 384).rev() {
                    let b = region.get(cx * 16 + x, y, cz * 16 + z);
                    if b == BlockId::PaleOakLog {
                        accepted.push((cx * 16 + x, cz * 16 + z));
                        break;
                    }
                }
            }
        }
        let mut matched = 0usize;
        for (x, z) in &accepted {
            if vanilla.iter().any(|(vx, _, vz)| vx == x && vz == z) {
                matched += 1;
            }
        }
        println!("vanilla trunks: {}", vanilla.len());
        println!("probe trunks: {} (matched {matched})", accepted.len());
        for (x, z) in &accepted {
            let has = vanilla.iter().any(|(vx, _, vz)| vx == x && vz == z);
            println!("  probe ({x},{z}) {}", if has { "MATCH" } else { "extra" });
        }
        for (x, y, z) in &vanilla {
            if !accepted.iter().any(|(ax, az)| ax == x && az == z) {
                println!("  MISSING vanilla trunk ({x},{y},{z})");
            }
        }
        return;
    }
    // 5x5 buffer (FEATURE_RADIUS 2) loaded from the vanilla reference.
    let mut region = RegionBuf::new(cx, cz, 2);
    let mut missing = Vec::new();
    for dz in -2..=2 {
        for dx in -2..=2 {
            let ncx = cx + dx;
            let ncz = cz + dz;
            match load_vanilla_blocks(&region_dir, ncx, ncz) {
                Some(b) => region.put_chunk(ncx, ncz, &b, &vec![0i16; 256]),
                None => missing.push((ncx, ncz)),
            }
        }
    }
    if !missing.is_empty() {
        eprintln!("missing chunks: {missing:?}");
        return;
    }

    let mut rng = FeatureRandom::new(seed);
    let dec = rng.set_decoration_seed(seed, cx * 16, cz * 16);
    // global index for the placed feature in step 9 (VEGETAL_DECORATION).
    let idx = neutron_worldgen::feature_catalog::global_feature_index(9, &placed_id)
        .expect("feature in step 9 sorter");
    rng.set_feature_seed(dec, idx, 9);
    eprintln!(
        "seed={seed} chunk=({cx},{cz}) placed={placed_id} index={idx} dec={dec} (terrain = vanilla ref)"
    );
    let count_clay = |region: &RegionBuf| -> usize {
        let wb = neutron_worldgen::generator::WORLD_BOTTOM;
        let mut n = 0;
        for y in wb..wb + 384 {
            for lz in 0..16 {
                for lx in 0..16 {
                    if region.get(cx * 16 + lx, y, cz * 16 + lz) == BlockId::Clay {
                        n += 1;
                    }
                }
            }
        }
        n
    };
    let before = count_clay(&region);
    neutron_worldgen::feature_dispatch::place_placed_feature(
        &mut rng,
        &mut region,
        &gen.state,
        cx * 16,
        cz * 16,
        &placed_id,
    );
    let after = count_clay(&region);
    eprintln!("[clay] before={before} after={after} placed={}", after - before);
}
