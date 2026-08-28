//! Closed two-sided dump of tree trunk bases: vanilla ref .mca vs the real
//! neutron pipeline, per log type (oak, dark_oak, pale_oak, birch, spruce).
//!
//! For every chunk in the window, extract each column's lowest log of each
//! type that starts a contiguous vertical run of height >= 2 (the trunk
//! base; a 2x2 dark oak records 4 bases) and diff the two sets at identical
//! (x,y,z). Missing vanilla bases are then matched to the nearest extra
//! neutron base of the same type (Chebyshev <= 8) with a (dx,dy,dz)
//! histogram: displaced same trees vs entirely different sets.
//!
//! Usage: tree_trunks_dump [seed] [cx] [cz] [radius] [region_dir]
use neutron_world::nbt::ussr_nbt::owned::{List, Tag};
use neutron_world::nbt::{compound_get, read_nbt};
use neutron_world::Region;
use neutron_worldgen::generator::WORLD_BOTTOM;
use neutron_worldgen::surface::{vanilla_name, BlockId};
use neutron_worldgen::ChunkGenerator;
use std::collections::{BTreeMap, HashSet};
use std::path::PathBuf;

const LOG_TYPES: [&str; 5] = ["oak", "dark_oak", "pale_oak", "birch", "spruce"];

fn log_type_of_u16(bid: u16) -> Option<&'static str> {
    let b = BlockId::from_u16(bid)?;
    match vanilla_name(b).strip_prefix("minecraft:")? {
        "oak_log" => Some("oak"),
        "dark_oak_log" => Some("dark_oak"),
        "pale_oak_log" => Some("pale_oak"),
        "birch_log" => Some("birch"),
        "spruce_log" => Some("spruce"),
        _ => None,
    }
}

/// Trunk bases per log type from a 16x384x16 u16 block vec laid out as
/// index = (y - WORLD_BOTTOM) * 256 + z * 16 + x (same layout for the
/// vanilla NBT decode and `GeneratedChunk.blocks`).
fn trunk_bases(blocks: &[u16], cx: i32, cz: i32) -> BTreeMap<&'static str, Vec<(i32, i32, i32)>> {
    let mut out: BTreeMap<&'static str, Vec<(i32, i32, i32)>> =
        LOG_TYPES.iter().map(|&t| (t, Vec::new())).collect();
    for lz in 0..16i32 {
        for lx in 0..16i32 {
            let col = |ly: i32| blocks[(ly * 256 + lz * 16 + lx) as usize];
            let mut ly = 0i32;
            while ly < 384 {
                let bid = col(ly);
                let Some(t) = log_type_of_u16(bid) else {
                    ly += 1;
                    continue;
                };
                let mut top = ly;
                while top + 1 < 384 && col(top + 1) == bid {
                    top += 1;
                }
                if top - ly >= 1 {
                    // contiguous run of height >= 2: record the base
                    out.get_mut(t)
                        .unwrap()
                        .push((cx * 16 + lx, WORLD_BOTTOM + ly, cz * 16 + lz));
                }
                ly = top + 1;
            }
        }
    }
    out
}

/// Load one vanilla chunk's blocks (names) into a 16x384x16 u16 vec.
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

fn empty_bases() -> BTreeMap<&'static str, Vec<(i32, i32, i32)>> {
    LOG_TYPES.iter().map(|&t| (t, Vec::new())).collect()
}

fn main() {
    let mut args = std::env::args().skip(1);
    let seed: i64 = args.next().and_then(|s| s.parse().ok()).unwrap_or(424242);
    let cx0: i32 = args.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    let cz0: i32 = args.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    let radius: i32 = args.next().and_then(|s| s.parse().ok()).unwrap_or(2);
    let region_dir = args.next().unwrap_or_else(|| {
        "tools/nbt-ref/vanilla-fresh-424242/world/dimensions/minecraft/overworld/region".to_string()
    });

    let mut chunks: Vec<(i32, i32)> = Vec::new();
    for dz in -radius..=radius {
        for dx in -radius..=radius {
            chunks.push((cx0 + dx, cz0 + dz));
        }
    }
    chunks.sort();

    let gen = ChunkGenerator::new(seed);
    let mut cache = neutron_worldgen::NoiseCache::new();

    // per type: [van, neu, match, missing, extra]
    let mut totals: BTreeMap<&'static str, [usize; 5]> =
        LOG_TYPES.iter().map(|&t| (t, [0usize; 5])).collect();
    let mut miss_map = empty_bases();
    let mut extra_map = empty_bases();

    for &(cx, cz) in &chunks {
        let van_blocks = load_vanilla_blocks(&region_dir, cx, cz);
        let van_bases = match &van_blocks {
            Some(b) => trunk_bases(b, cx, cz),
            None => {
                eprintln!("CHUNK ({cx},{cz}) no vanilla ref chunk");
                empty_bases()
            }
        };
        let chunk = gen.generate_chunk_cached(cx, cz, &mut cache);
        let neu_bases = trunk_bases(&chunk.blocks, cx, cz);
        for t in LOG_TYPES {
            let v: HashSet<(i32, i32, i32)> = van_bases[t].iter().copied().collect();
            let n: HashSet<(i32, i32, i32)> = neu_bases[t].iter().copied().collect();
            let matched = v.intersection(&n).count();
            let missing: Vec<(i32, i32, i32)> = v.difference(&n).copied().collect();
            let extra: Vec<(i32, i32, i32)> = n.difference(&v).copied().collect();
            println!(
                "CHUNK ({cx},{cz}) {t}: van={} neu={} match={matched} missing={} extra={}",
                v.len(),
                n.len(),
                missing.len(),
                extra.len()
            );
            let tot = totals.get_mut(t).unwrap();
            tot[0] += v.len();
            tot[1] += n.len();
            tot[2] += matched;
            tot[3] += missing.len();
            tot[4] += extra.len();
            miss_map.get_mut(t).unwrap().extend(missing);
            extra_map.get_mut(t).unwrap().extend(extra);
        }
    }

    println!(
        "REGION SUMMARY seed={seed} center=({cx0},{cz0}) radius={radius} chunks={}",
        chunks.len()
    );
    for t in LOG_TYPES {
        let [van, neu, matched, missing, extra] = totals[t];
        println!("  {t}: van={van} neu={neu} match={matched} missing={missing} extra={extra}");
    }

    println!("DISPLACEMENT (nearest same-type extra base, Chebyshev<=8, dx=-2..2 dy=-2..2 dz=-2..2 -> counts):");
    for t in LOG_TYPES {
        let missing = &miss_map[t];
        let extras = &extra_map[t];
        if missing.is_empty() {
            println!("  DISP {t} -> none missing");
            continue;
        }
        let mut offs: BTreeMap<(i32, i32, i32), usize> = BTreeMap::new();
        let mut orphan = 0usize;
        for &(x, y, z) in missing {
            let mut best: Option<((i32, i32, i32), i32)> = None;
            for &(ex, ey, ez) in extras {
                let d = (ex - x, ey - y, ez - z);
                let cheb = d.0.abs().max(d.1.abs()).max(d.2.abs());
                if cheb <= 8 && best.map_or(true, |(_, bc)| cheb < bc) {
                    best = Some((d, cheb));
                }
            }
            match best {
                Some((d, _)) => *offs.entry(d).or_default() += 1,
                None => orphan += 1,
            }
        }
        let nearby: usize = offs.values().sum();
        let parts: Vec<String> = offs
            .iter()
            .map(|((dx, dy, dz), n)| format!("({dx},{dy},{dz})={n}"))
            .collect();
        println!(
            "  DISP {t} -> counts: {} orphan={orphan} (nearby={nearby}/{} missing)",
            parts.join(" "),
            missing.len()
        );
    }
}
