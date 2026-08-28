//! Closed dump: FIRST divergent tree draw per origin (seed 424242 window).
//!
//! Both sides run the SAME protocol: step-9 (VEGETAL_DECORATION) replay over
//! a 5x5 stripped terrain buffer, origins in ticket_sim order, per-feature
//! seeds = set_feature_seed(dec, gif, 9). Vanilla side terrain = reference
//! .mca minus vegetal-family output (exported as NDEC1 for
//! ProbeTreeFirstFlip); neutron side = real generated chunks minus vegetal.
//! NEUTRON_TRACE_TREES / NEUTRON_DECO_TREE_TRACE go to stderr (parse
//! offline); this file emits the stream summary + gate-input dumps.
//!
//! Modes:
//!   order   <seed> <ccx> <ccz>
//!       print the ticket_sim inner-3x3 origin order (world chunk coords).
//!   vanndec <seed> <ccx> <ccz> <out_prefix> [region_dir]
//!       write <out_prefix>.ndec: stripped vanilla .mca 5x5 + neutron biome
//!       grids (NDEC1, same layout decorate_oracle exports).
//!   replay  <seed> <ccx> <ccz> <van|neu> [region_dir]
//!       run the replay protocol over the stripped buffer (van = .mca refs,
//!       neu = generated chunks); stdout: origin order, biome unions, step-9
//!       feature lists; stderr: per-draw traces (env set inside).
//!   real    <seed> <ocx> <ocz>
//!       run the REAL pipeline generate_chunk_cached for one region (stderr
//!       traces; the target origin is the region center).
//!   gates   <seed> <ccx> <ccz> <van|neu> <ocx> <ocz> <gif> <draw_k> <x> <y> <z>
//!           [cells_file] [region_dir]
//!       rebuild the buffer state just before draw <draw_k> of placed feature
//!       <gif> at origin (ocx,ocz), dump gate inputs at (x,y,z), evaluate
//!       would_survive + max_free_tree_height (heights 6..9) on the state
//!       WITHOUT (A) and WITH (B) the earlier draws' tree cells.
use neutron_world::nbt::ussr_nbt::owned::{List, Tag};
use neutron_world::nbt::{compound_get, read_nbt};
use neutron_world::Region;
use neutron_worldgen::deco_schedule::window_order;
use neutron_worldgen::feature_catalog::{features_at_step, global_feature_index};
use neutron_worldgen::feature_dispatch::{biome_id_to_name, place_placed_feature};
use neutron_worldgen::feature_rng::FeatureRandom;
use neutron_worldgen::generator::WORLD_BOTTOM;
use neutron_worldgen::region_buf::RegionBuf;
use neutron_worldgen::surface::{vanilla_name, BlockId};
use neutron_worldgen::{ChunkGenerator, NoiseCache};
use std::collections::HashSet;
use std::io::Write;
use std::path::PathBuf;

const REGION_DIR: &str = "tools/nbt-ref/vanilla-fresh-424242/world/dimensions/minecraft/overworld/region";

/// Load one vanilla chunk's blocks (names) into a 16x384x16 u16 vec
/// (index = (y - WORLD_BOTTOM) * 256 + z * 16 + x). Same as
/// tree_trunks_dump::load_vanilla_blocks.
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
    let wb = WORLD_BOTTOM;
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
                            let bi = ((y_sec * 16 + ly - wb) * 256 + lz * 16 + lx as i32) as usize;
                            blocks[bi] = bid;
                        }
                    }
                }
            }
        }
    }
    Some(blocks)
}

/// Region buffer around (ccx,ccz) radius 2, filled per side.
/// van: vanilla .mca refs; neu: real generated chunks (biome grids attached).
fn build_stripped_buffer(
    gen: &ChunkGenerator,
    ccx: i32,
    ccz: i32,
    side: &str,
    region_dir: &str,
) -> RegionBuf {
    let mut region = RegionBuf::new(ccx, ccz, 2);
    for dz in -2..=2 {
        for dx in -2..=2 {
            let (ncx, ncz) = (ccx + dx, ccz + dz);
            match side {
                "van" => {
                    let b = load_vanilla_blocks(region_dir, ncx, ncz)
                        .unwrap_or_else(|| panic!("missing vanilla chunk ({ncx},{ncz})"));
                    region.put_chunk(ncx, ncz, &b, &vec![0i16; 256]);
                }
                "neu" => {
                    let g = gen.generate_chunk_cached(
                        ncx,
                        ncz,
                        &mut NoiseCache::new(),
                    );
                    region.put_chunk(ncx, ncz, &g.blocks, &g.heightmap);
                    let mut grid = [0u8; 1536];
                    for (i, v) in g.biomes.iter().take(1536).enumerate() {
                        grid[i] = *v;
                    }
                    region.put_chunk_biomes(ncx, ncz, &grid);
                }
                s => panic!("unknown side {s}"),
            }
        }
    }
    strip_vegetal(&mut region);
    region
}

fn strip_vegetal(region: &mut RegionBuf) {
    for y in WORLD_BOTTOM..neutron_worldgen::generator::WORLD_TOP {
        for z in region.origin_z..region.origin_z + region.side {
            for x in region.origin_x..region.origin_x + region.side {
                if neutron_worldgen::sculk::is_vegetal_family(region.get(x, y, z)) {
                    region.set(x, y, z, BlockId::Air);
                }
            }
        }
    }
}

/// Inner 3x3 of the 5x5 window centered (ccx,ccz), ticket_sim order, as
/// world chunk coords (the same order decorate_region_origin_major uses).
fn inner_order(ccx: i32, ccz: i32) -> Vec<(i32, i32)> {
    let order = window_order(5, (ccx - 2) * 16, (ccz - 2) * 16);
    order
        .into_iter()
        .filter(|&(cxl, czl)| (cxl - 2).abs() <= 1 && (czl - 2).abs() <= 1)
        .map(|(cxl, czl)| (ccx - 2 + cxl, ccz - 2 + czl))
        .collect()
}

/// Replicates feature_dispatch::origin_biome_union: biomes present in the
/// quart grids of the 3x3 chunks around the origin (clamped to the buffer),
/// sampled on the stored 4x4x24 per-chunk grid with noise fallback.
fn biome_union(region: &RegionBuf, state: &neutron_worldgen::worldgen::WorldgenState, ox0: i32, oz0: i32) -> Vec<String> {
    let cxl = (ox0 - region.origin_x) / 16;
    let czl = (oz0 - region.origin_z) / 16;
    let mut names: Vec<String> = Vec::new();
    let push = |id: u8, names: &mut Vec<String>| {
        let n = biome_id_to_name(id).to_string();
        if !names.iter().any(|x| x == &n) {
            names.push(n);
        }
    };
    for dz in -1..=1i32 {
        for dx in -1..=1i32 {
            let ncx = cxl + dx;
            let ncz = czl + dz;
            if ncx < 0 || ncz < 0 || ncx >= region.chunks || ncz >= region.chunks {
                continue;
            }
            let cx0 = region.origin_x + ncx * 16;
            let cz0 = region.origin_z + ncz * 16;
            for section in 0..24i32 {
                let base_y_q = (WORLD_BOTTOM + section * 16) >> 2;
                for sy4 in 0..4i32 {
                    for bz4 in 0..4i32 {
                        for bx4 in 0..4i32 {
                            let (qx, qy, qz) = (cx0 / 4 + bx4, base_y_q + sy4, cz0 / 4 + bz4);
                            let id = region
                                .stored_noise_biome(qx, qy, qz)
                                .unwrap_or_else(|| {
                                    neutron_worldgen::biome_manager::noise_biome_at_quart(
                                        state, qx, qy, qz,
                                    )
                                });
                            push(id, &mut names);
                        }
                    }
                }
            }
        }
    }
    names
}

/// One origin's step-9 pass, exactly like feature_dispatch::place_feature_list
/// (merged biome-union feature list sorted by global index, per-feature seeds).
fn replay_origin_step9(
    region: &mut RegionBuf,
    gen: &ChunkGenerator,
    seed: i64,
    ocx: i32,
    ocz: i32,
) {
    let ox0 = ocx * 16;
    let oz0 = ocz * 16;
    let biomes = biome_union(region, &gen.state, ox0, oz0);
    let mut merged: Vec<(i32, String)> = Vec::new();
    for b in &biomes {
        for f in features_at_step(b, 9) {
            if let Some(idx) = global_feature_index(9, &f) {
                if !merged.iter().any(|(_, s)| s == &f) {
                    merged.push((idx, f));
                }
            }
        }
    }
    merged.sort_by_key(|(i, _)| *i);
    println!(
        "ORIGIN {ocx} {ocz} biomes=[{}]",
        biomes.join(",")
    );
    println!(
        "FEATURES {ocx} {ocz} {}",
        merged.iter().map(|(i, f)| format!("{i}:{f}")).collect::<Vec<_>>().join(",")
    );
    let mut rng = FeatureRandom::new(seed);
    let dec = rng.set_decoration_seed(seed, ox0, oz0);
    for (gif, placed) in &merged {
        rng.set_feature_seed(dec, *gif, 9);
        place_placed_feature(&mut rng, region, &gen.state, ox0, oz0, placed);
    }
}

// ---- gate-input evaluation helpers (replicate tree/mod.rs gates) ----

fn valid_tree_pos(b: BlockId) -> bool {
    // Mirror of crates/neutron-worldgen/src/tree/mod.rs valid_tree_pos
    // (26.2 replaceable_by_trees tag) — kept in sync by hand.
    matches!(
        b,
        BlockId::Air
            | BlockId::CaveAir
            | BlockId::OakLeaves
            | BlockId::DarkOakLeaves
            | BlockId::PaleOakLeaves
            | BlockId::BirchLeaves
            | BlockId::SpruceLeaves
            | BlockId::JungleLeaves
            | BlockId::AcaciaLeaves
            | BlockId::MangroveLeaves
            | BlockId::CherryLeaves
            | BlockId::AzaleaLeaves
            | BlockId::FloweringAzaleaLeaves
            | BlockId::ShortGrass
            | BlockId::Fern
            | BlockId::LargeFern
            | BlockId::TallGrass
            | BlockId::ShortDryGrass
            | BlockId::TallDryGrass
            | BlockId::Bush
            | BlockId::FireflyBush
            | BlockId::DeadBush
            | BlockId::Vine
            | BlockId::GlowLichen
            | BlockId::Seagrass
            | BlockId::TallSeagrass
            | BlockId::WarpedRoots
            | BlockId::CrimsonRoots
            | BlockId::NetherSprouts
            | BlockId::Dandelion
            | BlockId::Poppy
            | BlockId::BlueOrchid
            | BlockId::Allium
            | BlockId::AzureBluet
            | BlockId::RedTulip
            | BlockId::OrangeTulip
            | BlockId::WhiteTulip
            | BlockId::PinkTulip
            | BlockId::OxeyeDaisy
            | BlockId::Cornflower
            | BlockId::LilyOfTheValley
            | BlockId::Sunflower
            | BlockId::Lilac
            | BlockId::RoseBush
            | BlockId::Peony
            | BlockId::LeafLitter
            | BlockId::PaleMossCarpet
            | BlockId::HangingRoots
            | BlockId::Water
    )
}

fn is_log(b: BlockId) -> bool {
    matches!(
        b,
        BlockId::OakLog
            | BlockId::DarkOakLog
            | BlockId::PaleOakLog
            | BlockId::BirchLog
            | BlockId::SpruceLog
            | BlockId::JungleLog
            | BlockId::AcaciaLog
            | BlockId::MangroveLog
            | BlockId::CherryLog
    )
}

fn is_free(b: BlockId) -> bool {
    valid_tree_pos(b) || is_log(b)
}

fn supports_vegetation(b: BlockId) -> bool {
    matches!(
        b,
        BlockId::Dirt
            | BlockId::CoarseDirt
            | BlockId::RootedDirt
            | BlockId::GrassBlock
            | BlockId::Podzol
            | BlockId::Mycelium
            | BlockId::Mud
            | BlockId::MossBlock
            | BlockId::PaleMossBlock
    )
}

fn blocks_motion(b: BlockId) -> bool {
    !matches!(
        b,
        BlockId::Air
            | BlockId::CaveAir
            | BlockId::Water
            | BlockId::Lava
            | BlockId::ShortGrass
            | BlockId::TallGrass
            | BlockId::LeafLitter
            | BlockId::Vine
            | BlockId::MossCarpet
            | BlockId::PaleMossCarpet
            | BlockId::GlowLichen
            | BlockId::Snow
    )
}

/// Heightmap scan: first-available = top + 1 (top-down, first matching).
fn heightmap_top(region: &RegionBuf, x: i32, z: i32, ocean_floor: bool) -> Option<i32> {
    for y in (WORLD_BOTTOM..neutron_worldgen::generator::WORLD_TOP).rev() {
        let b = region.get(x, y, z);
        let ok = if ocean_floor {
            blocks_motion(b)
        } else {
            b != BlockId::Air && b != BlockId::CaveAir
        };
        if ok {
            return Some(y + 1);
        }
    }
    None
}

/// Replicated max_free_tree_height for pale/dark oak
/// (three_layers_feature_size upper_size=2: limit=1 upper_limit=1 lower=0
/// middle=1 upper=2).
fn max_free_height(region: &RegionBuf, x: i32, y: i32, z: i32, tree_height: i32) -> i32 {
    for yo in 0..=tree_height + 1 {
        let r: i32 = if yo < 1 {
            0
        } else if yo >= tree_height - 1 {
            2
        } else {
            1
        };
        for dx in -r..=r {
            for dz in -r..=r {
                if !is_free(region.get(x + dx, y + yo, z + dz)) {
                    return yo - 2;
                }
            }
        }
    }
    tree_height
}

fn dump_neighborhood(region: &RegionBuf, x: i32, y: i32, z: i32) {
    let mut cells: Vec<String> = Vec::new();
    for dy in -1..=11 {
        for dz in -2..=2 {
            for dx in -2..=2 {
                let b = region.get(x + dx, y + dy, z + dz);
                if b != BlockId::Air && b != BlockId::CaveAir {
                    cells.push(format!(
                        "NB dy={dy} dx={dx} dz={dz} {} {}",
                        (y + dy),
                        vanilla_name(b)
                    ));
                }
            }
        }
    }
    for c in cells {
        println!("{c}");
    }
}

fn eval_gates(region: &RegionBuf, x: i32, y: i32, z: i32, label: &str) {
    let below = region.get(x, y - 1, z);
    let at = region.get(x, y, z);
    let of = heightmap_top(region, x, z, true).unwrap_or(WORLD_BOTTOM);
    let ws = heightmap_top(region, x, z, false).unwrap_or(WORLD_BOTTOM);
    println!(
        "GATE[{label}] pos=({x},{y},{z}) at={} below={} supports_veg={} ws={ws} of={of} water_depth={}",
        vanilla_name(at),
        vanilla_name(below),
        supports_vegetation(below),
        ws.saturating_sub(of)
    );
    for h in 6..=9 {
        let clipped = max_free_height(region, x, y, z, h);
        println!(
            "GATE[{label}] tree_height={h} max_free={clipped} pass={}",
            clipped >= h
        );
    }
    dump_neighborhood(region, x, y, z);
}

fn write_ndec(
    gen: &ChunkGenerator,
    seed: i64,
    ccx: i32,
    ccz: i32,
    out_prefix: &str,
    region_dir: &str,
) {
    // vanilla blocks, stripped
    let mut chunks: Vec<Vec<u16>> = Vec::new();
    for dz in -2..=2 {
        for dx in -2..=2 {
            let b = load_vanilla_blocks(region_dir, ccx + dx, ccz + dz)
                .unwrap_or_else(|| panic!("missing vanilla chunk ({},{})", ccx + dx, ccz + dz));
            chunks.push(b);
        }
    }
    for c in &mut chunks {
        for v in c.iter_mut() {
            if let Some(b) = BlockId::from_u16(*v) {
                if neutron_worldgen::sculk::is_vegetal_family(b) {
                    *v = BlockId::Air.as_u16();
                }
            }
        }
    }
    // global palette by first-seen order over the chunk iteration order
    let mut pal: Vec<String> = vec!["minecraft:air".into()];
    let mut pal_map: std::collections::HashMap<u16, u16> = std::collections::HashMap::new();
    pal_map.insert(BlockId::Air.as_u16(), 0);
    let mut idx_chunks: Vec<Vec<u16>> = Vec::with_capacity(25);
    for c in &chunks {
        let mut idxs = vec![0u16; 16 * 384 * 16];
        for (i, v) in c.iter().enumerate() {
            if *v == BlockId::Air.as_u16() {
                continue;
            }
            let pi = match pal_map.get(v) {
                Some(p) => *p,
                None => {
                    let p = pal.len() as u16;
                    let name = BlockId::from_u16(*v)
                        .map(|b| vanilla_name(b).to_string())
                        .unwrap_or_else(|| "minecraft:air".into());
                    pal.push(name);
                    pal_map.insert(*v, p);
                    p
                }
            };
            idxs[i] = pi;
        }
        idx_chunks.push(idxs);
    }
    // biome grids: neutron noise+surface grids (biome parity 7679/7680)
    let mut bio_grids: Vec<[u8; 1536]> = Vec::new();
    let mut dense: Vec<String> = Vec::new();
    let mut remap: std::collections::HashMap<u8, u8> = std::collections::HashMap::new();
    for id in 0u8..=64u8 {
        let n = biome_id_to_name(id).to_string();
        let next = match dense.iter().position(|x| x == &n) {
            Some(p) => p as u8,
            None => {
                dense.push(n);
                (dense.len() - 1) as u8
            }
        };
        remap.insert(id, next);
    }
    for dz in -2..=2 {
        for dx in -2..=2 {
            let (_, _, biomes) = gen.generate_noise_and_surface(ccx + dx, ccz + dz);
            let mut g = [0u8; 1536];
            for (i, v) in biomes.iter().take(1536).enumerate() {
                g[i] = remap.get(v).copied().unwrap_or(0);
            }
            bio_grids.push(g);
        }
    }
    // NDEC1
    let dump_path = format!("{out_prefix}.ndec");
    let mut f = std::fs::File::create(&dump_path).unwrap();
    f.write_all(b"NDEC1").unwrap();
    f.write_all(&seed.to_le_bytes()).unwrap();
    f.write_all(&ccx.to_le_bytes()).unwrap();
    f.write_all(&ccz.to_le_bytes()).unwrap();
    f.write_all(&(dense.len() as u16).to_le_bytes()).unwrap();
    for n in &dense {
        f.write_all(&(n.len() as u16).to_le_bytes()).unwrap();
        f.write_all(n.as_bytes()).unwrap();
    }
    for ci in 0..25 {
        f.write_all(&(pal.len() as u16).to_le_bytes()).unwrap();
        for n in &pal {
            f.write_all(&(n.len() as u16).to_le_bytes()).unwrap();
            f.write_all(n.as_bytes()).unwrap();
        }
        for v in &idx_chunks[ci] {
            f.write_all(&v.to_le_bytes()).unwrap();
        }
        f.write_all(bio_grids[ci].as_slice()).unwrap();
    }
    println!("wrote {dump_path}");
}

fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let mode = args.remove(0);
    let seed: i64 = args[0].parse().unwrap();
    let ccx: i32 = args[1].parse().unwrap();
    let ccz: i32 = args[2].parse().unwrap();
    match mode.as_str() {
        "order" => {
            let seq = neutron_worldgen::deco_schedule::decorate_sequence();
            let mut rank: std::collections::HashMap<(i32, i32), usize> =
                std::collections::HashMap::new();
            for (i, &p) in seq.iter().enumerate() {
                rank.entry(p).or_insert(i);
            }
            for (ocx, ocz) in inner_order(ccx, ccz) {
                let r = rank.get(&(ocx, ocz));
                println!("ORIGIN {ocx} {ocz} seq_rank={}", r.map(|v| v.to_string()).unwrap_or_else(|| "UNRANKED".into()));
            }
            println!("seq_len={}", seq.len());
        }
        "vanndec" => {
            let out = args[3].clone();
            let region_dir = args.get(4).cloned().unwrap_or_else(|| REGION_DIR.into());
            let gen = ChunkGenerator::new(seed);
            write_ndec(&gen, seed, ccx, ccz, &out, &region_dir);
        }
        "replay" => {
            let side = args[3].clone();
            let region_dir = args.get(4).cloned().unwrap_or_else(|| REGION_DIR.into());
            std::env::set_var("NEUTRON_TRACE_TREES", "1");
            std::env::set_var("NEUTRON_DECO_TREE_TRACE", "1");
            let gen = ChunkGenerator::new(seed);
            let mut region = build_stripped_buffer(&gen, ccx, ccz, &side, &region_dir);
            let order = inner_order(ccx, ccz);
            println!(
                "ORDER {}",
                order
                    .iter()
                    .map(|(x, z)| format!("{x},{z}"))
                    .collect::<Vec<_>>()
                    .join(";")
            );
            for (ocx, ocz) in &order {
                replay_origin_step9(&mut region, &gen, seed, *ocx, *ocz);
            }
            println!("REPLAY DONE side={side} center=({ccx},{ccz})");
        }
        "real" => {
            let ocx = ccx;
            let ocz = ccz;
            std::env::set_var("NEUTRON_TRACE_TREES", "1");
            std::env::set_var("NEUTRON_DECO_TREE_TRACE", "1");
            let gen = ChunkGenerator::new(seed);
            let _ = gen.generate_chunk_cached(ocx, ocz, &mut NoiseCache::new());
            println!("REAL DONE origin=({ocx},{ocz})");
        }
        "gates" => {
            // tree_first_flip gates <seed> <ccx> <ccz> <side> <ocx> <ocz>
            //                   <gif> <draw_k> <x> <y> <z> [cells] [region_dir]
            let side = args[3].clone();
            let ocx: i32 = args[4].parse().unwrap();
            let ocz: i32 = args[5].parse().unwrap();
            let target_gif: i32 = args[6].parse().unwrap();
            let draw_k: i32 = args[7].parse().unwrap();
            let x: i32 = args[8].parse().unwrap();
            let y: i32 = args[9].parse().unwrap();
            let z: i32 = args[10].parse().unwrap();
            let cells_file = args.get(11).filter(|s| s.as_str() != "-").cloned();
            let region_dir = args
                .get(12)
                .cloned()
                .unwrap_or_else(|| REGION_DIR.into());
            let gen = ChunkGenerator::new(seed);
            let mut region = build_stripped_buffer(&gen, ccx, ccz, &side, &region_dir);
            let order = inner_order(ccx, ccz);
            let tpos = order
                .iter()
                .position(|&(a, b)| a == ocx && b == ocz)
                .unwrap_or_else(|| panic!("origin ({ocx},{ocz}) not in inner order"));
            for (i, &(ocx2, ocz2)) in order.iter().enumerate() {
                if i < tpos {
                    replay_origin_step9(&mut region, &gen, seed, ocx2, ocz2);
                }
            }
            // target origin: features with gif < target_gif
            {
                let ox0 = ocx * 16;
                let oz0 = ocz * 16;
                let biomes = biome_union(&region, &gen.state, ox0, oz0);
                let mut merged: Vec<(i32, String)> = Vec::new();
                for b in &biomes {
                    for f in features_at_step(b, 9) {
                        if let Some(idx) = global_feature_index(9, &f) {
                            if idx < target_gif && !merged.iter().any(|(_, s)| s == &f) {
                                merged.push((idx, f));
                            }
                        }
                    }
                }
                merged.sort_by_key(|(i, _)| *i);
                println!(
                    "PREFEATURES {ocx} {ocz} {}",
                    merged.iter().map(|(i, f)| format!("{i}:{f}")).collect::<Vec<_>>().join(",")
                );
                let mut rng = FeatureRandom::new(seed);
                let dec = rng.set_decoration_seed(seed, ox0, oz0);
                for (gif, placed) in &merged {
                    rng.set_feature_seed(dec, *gif, 9);
                    place_placed_feature(&mut rng, &mut region, &gen.state, ox0, oz0, placed);
                }
            }
            println!(
                "STATE pre-draw {draw_k} of gif {target_gif} origin ({ocx},{ocz}) side={side}"
            );
            eval_gates(&region, x, y, z, "A-pre");
            if let Some(cf) = cells_file {
                let text = std::fs::read_to_string(&cf).unwrap_or_else(|e| panic!("cells {cf}: {e}"));
                let mut applied: HashSet<(i32, i32, i32)> = HashSet::new();
                let mut n = 0usize;
                for line in text.lines() {
                    let it: Vec<&str> = line.split_whitespace().collect();
                    if it.len() != 4 {
                        continue;
                    }
                    let (Ok(cx), Ok(cy), Ok(cz)) = (
                        it[0].parse::<i32>(),
                        it[1].parse::<i32>(),
                        it[2].parse::<i32>(),
                    ) else {
                        continue;
                    };
                    let Some(b) = BlockId::from_name(it[3]) else { continue };
                    if applied.insert((cx, cy, cz)) {
                        region.set(cx, cy, cz, b);
                        n += 1;
                    }
                }
                println!("STATE applied {n} cells from {cf}");
                eval_gates(&region, x, y, z, "B-cells");
            }
        }
        "col" => {
            // tree_first_flip col <seed> <ccx> <ccz> <van|neu|both> <x> <z> [region_dir]
            let side = args[3].clone();
            let x: i32 = args[4].parse().unwrap();
            let z: i32 = args[5].parse().unwrap();
            let region_dir = args.get(6).cloned().unwrap_or_else(|| REGION_DIR.into());
            let gen = ChunkGenerator::new(seed);
            if side == "van" || side == "both" {
                let mut region = build_stripped_buffer(&gen, ccx, ccz, "van", &region_dir);
                for y in 60..130 {
                    let b = region.get(x, y, z);
                    if b != BlockId::Air && b != BlockId::CaveAir {
                        println!("VAN {x} {y} {z} {}", vanilla_name(b));
                    }
                }
                let _ = &mut region;
            }
            if side == "neu" || side == "both" {
                let mut region = build_stripped_buffer(&gen, ccx, ccz, "neu", &region_dir);
                for y in 60..130 {
                    let b = region.get(x, y, z);
                    if b != BlockId::Air && b != BlockId::CaveAir {
                        println!("NEU {x} {y} {z} {}", vanilla_name(b));
                    }
                }
            }
        }
        m => panic!("unknown mode {m}"),
    }
}
