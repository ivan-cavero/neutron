//! Biome-grid parity: vanilla ref stored quart biomes vs neutron climate
//! sampler, over every quart of a chunk radius. Answers "is the boundary
//! shifted?" for feature-dispatch flips (trees/patches) without guessing.
//! Usage: biome_grid_parity <seed> <cx> <cz> <radius> <region_dir>
use neutron_world::nbt::ussr_nbt::owned::{List, Tag};
use neutron_world::nbt::{compound_get, read_nbt};
use neutron_world::Region;
use neutron_worldgen::feature_dispatch::biome_id_to_name;
use neutron_worldgen::{biome_source, ChunkGenerator};
use std::collections::HashMap;
use std::path::PathBuf;

fn load_ref_biomes(
    regions: &mut HashMap<(i32, i32), Region>,
    region_dir: &str,
    cx: i32,
    cz: i32,
) -> Option<HashMap<(i32, i32, i32), String>> {
    // key: quart coords (qx, qy, qz) in world quart space
    let (rx, rz) = (cx >> 5, cz >> 5);
    let key = (rx, rz);
    if !regions.contains_key(&key) {
        let path = PathBuf::from(format!("{region_dir}/r.{rx}.{rz}.mca"));
        let region = Region::open(&path).ok()?.with_coords(rx, rz);
        regions.insert(key, region);
    }
    let region = regions.get(&key)?;
    let data = region.get_chunk(cx & 31, cz & 31).ok()??;
    let nbt = read_nbt(&data).ok()?;
    let sections = match compound_get(&nbt.compound, "sections") {
        Some(Tag::List(List::Compound(l))) => l,
        _ => return None,
    };
    let mut map = HashMap::new();
    if std::env::var_os("BGP_DEBUG").is_some() {
        for sec in sections.iter().take(3) {
            let y = compound_get(sec, "Y");
            let bs = compound_get(sec, "biomes");
            let dl = match bs {
                Some(Tag::Compound(b)) => match compound_get(b, "data") {
                    Some(Tag::LongArray(d)) => Some(d.len()),
                    _ => None,
                },
                _ => None,
            };
            let pl = match bs {
                Some(Tag::Compound(b)) => match compound_get(b, "palette") {
                    Some(Tag::List(List::Compound(p))) => Some(p.len()),
                    _ => None,
                },
                _ => None,
            };
            let keys = match bs {
                Some(Tag::Compound(b)) => Some(
                    b.tags
                        .iter()
                        .map(|(k, t)| {
                            let tv = match t {
                                Tag::List(l) => match l {
                                    List::Compound(v) => format!("List[Compound×{}]", v.len()),
                                    List::String(v) => format!("List[String×{}]", v.len()),
                                    List::Int(v) => format!("List[Int×{}]", v.len()),
                                    List::Byte(_) => "List[Byte]".into(),
                                    _ => "List[other]".into(),
                                },
                                Tag::Compound(_) => "Compound".to_string(),
                                Tag::LongArray(_) => "LongArray".to_string(),
                                Tag::String(s) => format!("String={}", s.to_string()),
                                _ => "?".to_string(),
                            };
                            format!("{}({})", k.to_string(), tv)
                        })
                        .collect::<Vec<_>>(),
                ),
                _ => None,
            };
            eprintln!("DBG sec Y={y:?} biomes={} keys={keys:?} data_longs={dl:?}",
                bs.is_some());
        }
    }
    for sec in sections {
        let y_sec = match compound_get(sec, "Y") {
            Some(Tag::Byte(b)) => *b as i8 as i32,
            Some(Tag::Int(b)) => *b,
            _ => continue,
        };
        let Some(Tag::Compound(bs)) = compound_get(sec, "biomes") else { continue };
        // NOTE: this ref world stores biome palettes as plain strings
        // (List[String]), not List[Compound{Name}] like block palettes.
        let names: Vec<String> = match compound_get(bs, "palette") {
            Some(Tag::List(List::String(v))) => v
                .iter()
                .map(|s| {
                    let s = s.to_string();
                    s.trim_start_matches("minecraft:").to_string()
                })
                .collect(),
            Some(Tag::List(List::Compound(palette))) => palette
                .iter()
                .filter_map(|pc| match compound_get(pc, "Name") {
                    Some(Tag::String(s)) => Some({
                        let s = s.to_string();
                        s.trim_start_matches("minecraft:").to_string()
                    }),
                    _ => None,
                })
                .collect(),
            _ => continue,
        };
        if names.is_empty() {
            continue;
        }
        // This post-processed ref packs biome quarts with NO 4-bit minimum:
        // 2 biomes -> 1 bit -> 64 entries in exactly 1 long (verified:
        // pal=2 sections carry data_longs=1).
        let bits = if names.len() <= 1 {
            0
        } else {
            (names.len() - 1).ilog2() + 1
        };
        let base_qy = y_sec * 4; // first quart y of this section
        match compound_get(bs, "data") {
            None => {
                // single-value palette: fill uniformly
                for sy in 0i32..4 {
                    for bz in 0i32..4 {
                        for bx in 0i32..4 {
                            map.insert(
                                (cx * 4 + bx, base_qy + sy, cz * 4 + bz),
                                names[0].clone(),
                            );
                        }
                    }
                }
            }
            Some(Tag::LongArray(data)) => {
                let longs: Vec<i64> = data.to_vec();
                let epl = (64 / bits) as usize;
                let mask = (1u64 << bits) - 1;
                for sy in 0i32..4 {
                    for bz in 0i32..4 {
                        for bx in 0i32..4 {
                            let idx = (sy * 16 + bz * 4 + bx) as usize;
                            let li = idx / epl;
                            let sh = ((idx % epl) * bits as usize) as u32;
                            let v = if bits == 0 {
                                    0
                                } else {
                                    (((longs[li] as u64) >> sh) & mask) as usize
                                };
                            map.insert(
                                (cx * 4 + bx, base_qy + sy, cz * 4 + bz),
                                names.get(v).cloned().unwrap_or_else(|| "?".into()),
                            );
                        }
                    }
                }
            }
            _ => continue,
        }
    }
    Some(map)
}

fn main() {
    let mut args = std::env::args().skip(1);
    let seed: i64 = args.next().and_then(|s| s.parse().ok()).unwrap_or(424242);
    let ccx: i32 = args.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    let ccz: i32 = args.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    let radius: i32 = args.next().and_then(|s| s.parse().ok()).unwrap_or(1);
    let region_dir = args
        .next()
        .unwrap_or_else(|| "tools/nbt-ref/vanilla-fresh-424242/world/dimensions/minecraft/overworld/region".into());

    let gen = ChunkGenerator::new(seed);
    let mut regions = HashMap::new();
    let mut total = 0u64;
    let mut bad = 0u64;
    let mut pairs: HashMap<(String, String), u64> = HashMap::new();
    let mut bad_by_band: HashMap<(i32, String, String), u64> = HashMap::new();
    for dz in -radius..=radius {
        for dx in -radius..=radius {
            let (cx, cz) = (ccx + dx, ccz + dz);
            let Some(refmap) = load_ref_biomes(&mut regions, &region_dir, cx, cz) else {
                println!("{cx:>5},{cz:>4}   missing");
                continue;
            };
            for (&(qx, qy, qz), want) in &refmap {
                let id = biome_source::biome_id_at_block(&gen.state, qx * 4 + 2, qy * 4 + 2, qz * 4 + 2);
                let got = biome_id_to_name(id).to_string();
                total += 1;
                if got != *want {
                    bad += 1;
                    *pairs.entry((want.clone(), got.clone())).or_insert(0) += 1;
                    let band = qy * 4; // approx world y of band start
                    *bad_by_band.entry((band / 32, want.clone(), got)).or_insert(0) += 1;
                }
            }
        }
    }
    if std::env::var_os("BGP_SETS").is_some() {
        let mut refset = std::collections::BTreeSet::new();
        let mut ourset = std::collections::BTreeSet::new();
        for dz2 in -radius..=radius {
            for dx2 in -radius..=radius {
                if let Some(m) = load_ref_biomes(&mut regions, &region_dir, ccx + dx2, ccz + dz2) {
                    for v in m.values() { refset.insert(v.clone()); }
                }
                for qx in ((ccx + dx2) * 4)..((ccx + dx2) * 4 + 4) {
                    for qz in ((ccz + dz2) * 4)..((ccz + dz2) * 4 + 4) {
                        for qy in (-64 >> 2)..(320 >> 2) {
                            ourset.insert(
                                neutron_worldgen::feature_dispatch::biome_id_to_name(
                                    biome_source::biome_id_at_block(&gen.state,
                                        qx * 4 + 2, qy * 4 + 2, qz * 4 + 2),
                                ).to_string(),
                            );
                        }
                    }
                }
            }
        }
        println!("REF set ({}) : {:?}", refset.len(), refset);
        println!("OUR set ({}) : {:?}", ourset.len(), ourset);
        let only_ref: Vec<_> = refset.difference(&ourset).collect();
        let only_our: Vec<_> = ourset.difference(&refset).collect();
        println!("only in REF: {:?}", only_ref);
        println!("only in OUR: {:?}", only_our);
    }
    println!(
        "BIOME GRID: {:.4}% match ({bad}/{total} quarts differ)",
        100.0 * (total - bad) as f64 / total.max(1) as f64
    );
    let mut v: Vec<_> = pairs.iter().collect();
    v.sort_by_key(|(_, n)| std::cmp::Reverse(**n));
    println!("top wrong pairs (vanilla -> ours):");
    for ((w, g), n) in v.iter().take(12) {
        println!("  {n:>7}  {w} -> {g}");
    }
}
