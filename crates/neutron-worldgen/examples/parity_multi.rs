//! Multi-chunk / multi-biome density-phase + block parity report.
//! Compares Neutron vs vanilla MCA at several chunk coords (seed 12345).

use neutron_world::nbt::ussr_nbt::owned::{List, Tag};
use neutron_world::nbt::{compound_get, read_nbt};
use neutron_world::Region;
use neutron_worldgen::surface::BlockId;
use neutron_worldgen::{generator::WORLD_BOTTOM, ChunkGenerator};
use std::path::PathBuf;

fn dens_van(n: &str) -> u8 {
    let n = n.strip_prefix("minecraft:").unwrap_or(n);
    if n == "air" || n == "cave_air" || n == "void_air" {
        return 0;
    }
    if n == "water" || n == "lava" {
        return 1;
    }
    if n.contains("sculk")
        || n.contains("leaves")
        || n.contains("log")
        || n == "moss_block"
        || n == "short_grass"
        || n == "fern"
        || n.contains("mushroom")
        || n == "vine"
        || n == "glow_lichen"
    {
        return 0;
    }
    2
}

fn dens_neu(b: BlockId) -> u8 {
    match b {
        BlockId::Air
        | BlockId::Sculk
        | BlockId::SculkVein
        | BlockId::SculkCatalyst
        | BlockId::SculkSensor
        | BlockId::SculkShrieker
        | BlockId::MossBlock
        | BlockId::OakLeaves
        | BlockId::DarkOakLeaves
        | BlockId::ShortGrass
        | BlockId::LeafLitter
        | BlockId::OakLog
        | BlockId::DarkOakLog => 0,
        BlockId::Water | BlockId::Lava => 1,
        _ => 2,
    }
}

fn load_vanilla_chunk(cx: i32, cz: i32) -> Option<(Vec<String>, String)> {
    // Region coords
    let rx = cx.div_euclid(32);
    let rz = cz.div_euclid(32);
    let lx = cx.rem_euclid(32);
    let lz = cz.rem_euclid(32);
    let path = PathBuf::from(format!(
        "tools/nbt-ref/vanilla1/world/dimensions/minecraft/overworld/region/r.{rx}.{rz}.mca"
    ));
    if !path.exists() {
        return None;
    }
    let region = Region::open(&path).ok()?.with_coords(rx, rz);
    let data = region.get_chunk(lx, lz).ok()??;
    let nbt = read_nbt(&data).ok()?;
    let status = match compound_get(&nbt.compound, "Status") {
        Some(Tag::String(s)) => s.to_string(),
        _ => "?".into(),
    };
    // Only compare fully (or nearly fully) generated chunks.
    // Proto statuses (structure_starts, biomes, carvers-only) are mostly empty
    // and must not be used as ground truth.
    let ok = status.contains("full")
        || status.contains("initialize_light")
        || status.contains("light")
        || status.contains("spawn");
    if !ok {
        return None;
    }
    let sections = match compound_get(&nbt.compound, "sections") {
        Some(Tag::List(List::Compound(l))) => l,
        _ => return None,
    };
    let mut names = vec!["minecraft:air".to_string(); 98304];
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
        let pal: Vec<String> = palette
            .iter()
            .map(|pc| match compound_get(pc, "Name") {
                Some(Tag::String(s)) => s.to_string(),
                _ => "minecraft:air".into(),
            })
            .collect();
        let nstates = pal.len();
        for i in 0..4096u32 {
            let name = if nstates == 1 {
                pal[0].clone()
            } else {
                let bits = ((nstates - 1).ilog2() + 1).max(4) as u32;
                let Tag::LongArray(data) = compound_get(bs, "data").unwrap() else {
                    panic!()
                };
                let longs: Vec<i64> = data.to_vec();
                let epl = 64 / bits;
                let mask = (1u64 << bits) - 1;
                let li = (i / epl) as usize;
                let bo = (i % epl) * bits;
                if li >= longs.len() {
                    continue;
                }
                let idx = ((longs[li] as u64) >> bo) & mask;
                pal.get(idx as usize)
                    .cloned()
                    .unwrap_or_else(|| "minecraft:air".into())
            };
            let ly = (i >> 8) as i32;
            let lz = ((i >> 4) & 15) as usize;
            let lx = (i & 15) as usize;
            let y = y_sec * 16 + ly;
            let idx = ((y - WORLD_BOTTOM) as usize) * 256 + lz * 16 + lx;
            if idx < names.len() {
                names[idx] = name;
            }
        }
    }
    Some((names, status))
}

fn discover_ready_chunks() -> Vec<(i32, i32)> {
    let mut out = Vec::new();
    for rx in -2i32..=2 {
        for rz in -2i32..=2 {
            let path = PathBuf::from(format!(
                "tools/nbt-ref/vanilla1/world/dimensions/minecraft/overworld/region/r.{rx}.{rz}.mca"
            ));
            if !path.exists() {
                continue;
            }
            let Ok(region) = Region::open(&path) else {
                continue;
            };
            let region = region.with_coords(rx, rz);
            for lz in 0..32i32 {
                for lx in 0..32i32 {
                    let Ok(Some(data)) = region.get_chunk(lx, lz) else {
                        continue;
                    };
                    if data.len() < 20_000 {
                        continue;
                    }
                    let Ok(nbt) = read_nbt(&data) else {
                        continue;
                    };
                    let status = match compound_get(&nbt.compound, "Status") {
                        Some(Tag::String(s)) => s.to_string(),
                        _ => continue,
                    };
                    if status.contains("full")
                        || status.contains("initialize_light")
                        || status.contains("light")
                        || status.contains("spawn")
                    {
                        out.push((rx * 32 + lx, rz * 32 + lz));
                    }
                }
            }
        }
    }
    out.sort();
    out.dedup();
    out
}

fn main() {
    let gen = ChunkGenerator::new(12345);
    let mut chunks = discover_ready_chunks();
    if chunks.is_empty() {
        // Fallback known set from pre-pregen world
        chunks = vec![
            (6, -2),
            (5, -3),
            (7, -1),
            (6, -3),
            (6, -1),
            (7, -2),
            (7, -3),
            (5, -2),
            (5, -1),
        ];
    }
    // Cap for runtime: prefer spread, max 24
    if chunks.len() > 24 {
        let step = chunks.len() / 24;
        chunks = chunks.into_iter().step_by(step.max(1)).take(24).collect();
    }
    println!("comparing {} ready chunks", chunks.len());
    println!("chunk     status(short) dens_shape  pure_ex feat_ex miss  block%");
    let mut sum_dens = 0u64;
    let mut sum_block = 0u64;
    let mut n_ok = 0u32;
    for &(cx, cz) in &chunks {
        let Some((van, status)) = load_vanilla_chunk(cx, cz) else {
            println!("({cx:3},{cz:3})  SKIP (not full/light)");
            continue;
        };
        let st_short = status.rsplit(':').next().unwrap_or(&status);
        let chunk = gen.generate_chunk(cx, cz);
        let mut dens_m = 0u32;
        let mut pure_x = 0u32;
        let mut feat_x = 0u32;
        let mut miss = 0u32;
        let mut block_m = 0u32;
        let total = 98304u32;
        for y in WORLD_BOTTOM..320 {
            for z in 0..16usize {
                for x in 0..16usize {
                    let idx = ((y - WORLD_BOTTOM) as usize) * 256 + z * 16 + x;
                    let vn = &van[idx];
                    let nb = chunk.block_at(x as u32, y, z as u32);
                    let vc = dens_van(vn);
                    let nc = dens_neu(nb);
                    if vc == nc {
                        dens_m += 1;
                    } else if nc == 2 && vc == 0 {
                        let bare = vn.strip_prefix("minecraft:").unwrap_or(vn);
                        if bare == "air" || bare == "cave_air" {
                            pure_x += 1;
                        } else {
                            feat_x += 1;
                        }
                    } else if nc == 0 && vc == 2 {
                        miss += 1;
                    }
                    // block name match (rough)
                    if let Some(id) = BlockId::from_name(vn) {
                        if id == nb {
                            block_m += 1;
                        }
                    } else if dens_van(vn) == dens_neu(nb) {
                        // unmapped feature treated as class match only
                    }
                }
            }
        }
        // biome at section mid y=0
        let dens_pct = 100.0 * dens_m as f64 / total as f64;
        let block_pct = 100.0 * block_m as f64 / total as f64;
        println!(
            "({cx:3},{cz:3})  {st_short:16} {dens_pct:7.3}%  pe={pure_x:5} fe={feat_x:4} ms={miss:4}  {block_pct:6.2}%"
        );
        sum_dens += dens_m as u64;
        sum_block += block_m as u64;
        n_ok += 1;
    }
    if n_ok > 0 {
        let cells = n_ok as u64 * 98304;
        println!(
            "AVERAGE dens_shape={:.4}%  block_name_match={:.2}%  over {n_ok} chunks",
            100.0 * sum_dens as f64 / cells as f64,
            100.0 * sum_block as f64 / cells as f64
        );
    }
}
