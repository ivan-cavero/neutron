//! Throwaway: diff vanilla mineshaft starts vs neutron generation.
//! Enumerates starts via References (packed longs), reads owner-chunk
//! Children BBs from any region file on disk, diffs vs generate_start.
use neutron_worldgen::mineshaft::{generate_start, is_mineshaft_chunk};
use neutron_world::nbt::ussr_nbt::owned::{List, Tag};
use neutron_world::nbt::{compound_get, read_nbt};
use neutron_world::Region;
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

type Bb = (i32, i32, i32, i32, i32, i32);

fn decode(packed: i64) -> (i32, i32) {
    // References store (z << 32) | (x & 0xFFFFFFFF) — NOT ChunkPos.asLong.
    let x = packed as i32;
    let z = (packed >> 32) as i32;
    (x, z)
}

fn load_chunk(region_dir: &str, cx: i32, cz: i32) -> Option<neutron_world::nbt::ussr_nbt::owned::Nbt> {
    let (rx, rz) = (cx >> 5, cz >> 5);
    let path = PathBuf::from(format!("{region_dir}/r.{rx}.{rz}.mca"));
    let region = Region::open(&path).ok()?.with_coords(rx, rz);
    let data = region.get_chunk(cx & 31, cz & 31).ok()??;
    read_nbt(&data).ok()
}

fn main() {
    let seed: i64 = std::env::args().nth(1).and_then(|s| s.parse().ok()).unwrap_or(424242);
    let rx: i32 = std::env::args().nth(2).and_then(|s| s.parse().ok()).unwrap_or(0);
    let rz: i32 = std::env::args().nth(3).and_then(|s| s.parse().ok()).unwrap_or(-1);
    let region_dir = std::env::args().nth(4).unwrap_or_else(|| {
        format!("tools/nbt-ref/vanilla-fresh-{seed}/world/dimensions/minecraft/overworld/region")
    });

    // 1) collect referenced starts across ALL region files present
    let mut refs: BTreeSet<(i32, i32)> = BTreeSet::new();
    for drx in -1..=1i32 {
        for drz in -2..=1i32 {
            let path = PathBuf::from(format!("{region_dir}/r.{drx}.{drz}.mca"));
            let Ok(region) = Region::open(&path) else { continue };
            let region = region.with_coords(drx, drz);
            for lx in 0..32i32 {
                for lz in 0..32i32 {
                    let Ok(Some(data)) = region.get_chunk(lx, lz) else { continue };
                    let Ok(nbt) = read_nbt(&data) else { continue };
                    let Some(Tag::Compound(structures)) = compound_get(&nbt.compound, "structures") else { continue };
                    let Some(Tag::Compound(references)) = compound_get(structures, "References") else { continue };
                    if let Some(Tag::LongArray(arr)) = compound_get(references, "minecraft:mineshaft") {
                        for packed in arr.to_vec() {
                            refs.insert(decode(packed));
                        }
                    }
                }
            }
        }
    }
    println!("referenced mine starts: {refs:?}");

    // 2) for each start, find its owner record (chunk whose starts.<id> has
    // matching ChunkX/ChunkZ) anywhere on disk
    for &(sx, sz) in &refs {
        let mut bbs: Vec<Bb> = Vec::new();
        let mut found_owner = false;
        // owner is usually within +-8 chunks; scan candidates' regions
        'owner: for dz in -8..=8i32 {
            for dx in -8..=8i32 {
                let ocx = sx + dx;
                let ocz = sz + dz;
                let Some(nbt) = load_chunk(&region_dir, ocx, ocz) else { continue };
                let Some(Tag::Compound(structures)) = compound_get(&nbt.compound, "structures") else { continue };
                let Some(Tag::Compound(starts)) = compound_get(structures, "starts") else { continue };
                for (_k, st) in &starts.tags {
                    let Tag::Compound(st) = st else { continue };
                    if !matches!(compound_get(st, "id"), Some(Tag::String(s)) if s.to_string() == "minecraft:mineshaft") {
                        continue;
                    }
                    let cx = match compound_get(st, "ChunkX") { Some(Tag::Int(v)) => *v, _ => continue };
                    let cz = match compound_get(st, "ChunkZ") { Some(Tag::Int(v)) => *v, _ => continue };
                    if std::env::var_os("MS_DEBUG").is_some() {
                        eprintln!("[owner-scan] at ({ocx},{ocz}) found start tag ChunkX={cx} ChunkZ={cz} looking for ({sx},{sz})");
                    }
                    if (cx, cz) != (sx, sz) {
                        continue;
                    }
                    found_owner = true;
                    if let Some(Tag::List(List::Compound(children))) = compound_get(st, "Children") {
                        if std::env::var_os("MS_DEBUG").is_some() {
                            if let Some(c0) = children.first() {
                                if let Some(Tag::Compound(bb)) = compound_get(c0, "BB") {
                                    let names: Vec<String> = bb
                                        .tags
                                        .iter()
                                        .map(|(n, v)| {
                                            let val = match v {
                                                Tag::Int(i) => format!("={i}"),
                                                _ => String::new(),
                                            };
                                            format!("{}{}", n.to_string(), val)
                                        })
                                        .collect();
                                    eprintln!("[child0 BB] {names:?}");
                                } else {
                                    match compound_get(c0, "BB") {
                                        Some(other) => {
                                            eprintln!("[child0 BB] non-compound: {other:?}")
                                        }
                                        None => eprintln!("[child0 BB] MISSING"),
                                    }
                                }
                            }
                        }
                        for c in children.iter() {
                            if let Some(Tag::IntArray(bb)) = compound_get(c, "BB") {
                                let v = bb.to_vec();
                                if v.len() == 6 {
                                    bbs.push((v[0], v[1], v[2], v[3], v[4], v[5]));
                                }
                            }
                        }
                    }
                    break 'owner;
                }
            }
        }
        let ours_ok = is_mineshaft_chunk(seed, sx, sz);
        let verdict = if !ours_ok {
            "NOT-generated-by-neutron".to_string()
        } else if !found_owner {
            "owner-not-on-disk".to_string()
        } else {
            let mut a = bbs.clone();
            a.sort();
            let mut b: Vec<Bb> = generate_start(seed, sx, sz)
                .iter()
                .map(|p| (p.bb.min_x, p.bb.min_y, p.bb.min_z, p.bb.max_x, p.bb.max_y, p.bb.max_z))
                .collect();
            b.sort();
            if a == b {
                format!("BB-IDENTICAL ({})", a.len())
            } else {
                let common = a.iter().filter(|x| b.contains(x)).count();
                format!(
                    "DIFF van={} neu={} common={} | van[0]={:?} neu[0]={:?} | van_sorted[0]={:?} neu_sorted[0]={:?}",
                    a.len(),
                    b.len(),
                    common,
                    bbs.first(),
                    b.first(),
                    a.first(),
                    b.first()
                )
            }
        };
        println!("start ({sx},{sz}) van_pieces={} -> {verdict}", bbs.len());
    }

    // 3) ours without any vanilla reference nearby
    let mut extra = vec![];
    for cz in ((rz * 32) - 8)..=((rz + 1) * 32 + 8) {
        for cx in ((rx * 32) - 8)..=((rx + 1) * 32 + 8) {
            if is_mineshaft_chunk(seed, cx, cz) && !refs.contains(&(cx, cz)) {
                extra.push((cx, cz));
            }
        }
    }
    println!("neutron-only starts nearby: {extra:?}");
}
