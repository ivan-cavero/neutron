// Multi-chunk parity: neutron vs vanilla fresh reference across a chunk
// radius, with core/border split (border cells carry the vanilla thread-
// scheduling noise; core must be deterministic).
// Usage:
//   region_parity [seed] [cx] [cz] [radius] [region_dir]      # 3×3 window
//   PARITY_SCAN=[step] region_parity <seed> 0 0 0 <region_dir># all ref chunks
// Env: PARITY_LEDGER=<csv> cell-exact gap list · PARITY_HISTO=1 class histogram
//      PARITY_WORKERS=<n> generation pool size (default = cores - 2)
use neutron_world::nbt::ussr_nbt::owned::{List, Tag};
use neutron_world::nbt::{compound_get, read_nbt};
use neutron_world::Region;
use neutron_worldgen::surface::{is_vegetation_name, vanilla_name, BlockId};
use neutron_worldgen::{ChunkGenerator, NoiseCache};
use std::collections::{BTreeMap, HashMap};
use std::io::Write;
use std::path::PathBuf;
use std::sync::mpsc;
use std::sync::Arc;

fn load_vanilla_chunk(
    regions: &mut HashMap<(i32, i32), Region>,
    region_dir: &str,
    cx: i32,
    cz: i32,
) -> Option<HashMap<(u8, i32, u8), String>> {
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
    if let Some(Tag::String(s)) = compound_get(&nbt.compound, "Status") {
        let st = s.to_string();
        if !st.ends_with("full") {
            return None; // stub chunk (biomes-only etc.): not comparable
        }
    } else {
        return None;
    }    let sections = match compound_get(&nbt.compound, "sections") {
        Some(Tag::List(List::Compound(l))) => l,
        _ => return None,
    };
    let mut map = HashMap::new();
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
                map.insert((lx, y_sec * 16 + ly, lz), names[0].clone());
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
            let idxp = ((longs[li] as u64) >> bo) & mask;
            let ly = (i >> 8) as i32;
            let lz = ((i >> 4) & 15) as u8;
            let lx = (i & 15) as u8;
            let name = names.get(idxp as usize).cloned().unwrap_or_default();
            map.insert((lx, y_sec * 16 + ly, lz), name);
        }
    }
    Some(map)
}

/// All full-status chunks present in the ref region dir, sorted. Coverage =
/// whatever the vanilla server generated there (spawn area on a fresh world;
/// pregenerate more in-game to widen the audit).
fn discover_chunks(
    regions: &mut HashMap<(i32, i32), Region>,
    region_dir: &str,
) -> Vec<(i32, i32)> {
    let mut rcoords: Vec<(i32, i32)> = std::fs::read_dir(region_dir)
        .expect("region dir readable")
        .filter_map(|e| {
            let name = e.ok()?.file_name().into_string().ok()?;
            let rest = name.strip_prefix("r.")?.strip_suffix(".mca")?;
            let mut it = rest.split('.');
            let rx = it.next()?.parse().ok()?;
            let rz = it.next()?.parse().ok()?;
            Some((rx, rz))
        })
        .collect();
    rcoords.sort();
    let mut coords = Vec::new();
    for (rx, rz) in rcoords {
        let path = PathBuf::from(format!("{region_dir}/r.{rx}.{rz}.mca"));
        let Ok(region) = Region::open(&path).map(|r| r.with_coords(rx, rz)) else {
            continue;
        };
        for lz in 0..32u32 {
            for lx in 0..32u32 {
                let Ok(Some(data)) = region.get_chunk(lx as i32, lz as i32) else {
                    continue;
                };
                let full = read_nbt(&data).ok().is_some_and(|nbt| {
                    match compound_get(&nbt.compound, "Status") {
                        Some(Tag::String(s)) => s.to_string().ends_with("full"),
                        _ => false,
                    }
                });
                if full {
                    coords.push((rx * 32 + lx as i32, rz * 32 + lz as i32));
                }
            }
        }
        regions.insert((rx, rz), region);
    }
    coords.sort();
    coords
}

/// Cell-exact gap accounting: streams every mismatch to CSV and accumulates
/// per-gap-key stats (count, example cell, bbox) so the report says not only
/// WHAT fails but WHERE.
struct Gaps {
    out: Option<std::fs::File>,
    rows: u64,
    map: HashMap<String, GapStat>,
}

#[derive(Default)]
struct GapStat {
    n: u64,
    ex: (i32, i32, i32),
    bb: [i32; 6], // minx,miny,minz,maxx,maxy,maxz
}

impl Gaps {
    #[allow(clippy::too_many_arguments)]
    fn row(&mut self, ccx: i32, ccz: i32, x: u32, y: i32, z: u32, d: i32, vn: &str, nn: &str) {
        let class = if vn == "minecraft:air" {
            "extra"
        } else if nn == "minecraft:air" {
            "missing"
        } else {
            "wrong"
        };
        let zone = if d >= 5 { "core" } else { "border" };
        let wx = ccx * 16 + x as i32;
        let wz = ccz * 16 + z as i32;
        self.rows += 1;
        if let Some(f) = self.out.as_mut() {
            let _ = writeln!(f, "{wx},{y},{wz},{class},{zone},{vn},{nn}");
        }
        let key = match class {
            "missing" => format!("missing {vn}"),
            "extra" => format!("extra {nn}"),
            _ => format!("wrong {vn} <- {nn}"),
        };
        let e = self.map.entry(key).or_insert_with(|| GapStat {
            ex: (wx, y, wz),
            ..Default::default()
        });
        e.n += 1;
        if e.n == 1 {
            e.bb = [wx, y, wz, wx, y, wz];
        } else {
            e.bb[0] = e.bb[0].min(wx);
            e.bb[1] = e.bb[1].min(y);
            e.bb[2] = e.bb[2].min(wz);
            e.bb[3] = e.bb[3].max(wx);
            e.bb[4] = e.bb[4].max(y);
            e.bb[5] = e.bb[5].max(wz);
        }
    }

    fn report(&self, worst: &HashMap<(i32, i32), u64>) {
        let mut v: Vec<_> = self.map.iter().collect();
        v.sort_by_key(|(_, s)| std::cmp::Reverse(s.n));
        let tot = self.rows as f64;
        let mut cum = 0u64;
        println!(
            "GAPS (core rows are deterministic; border rows carry vanilla scheduler noise):"
        );
        for (k, s) in v.iter().take(30) {
            cum += s.n;
            let b = s.bb;
            println!(
                "GAP {:>6} {:>5.1}% cum {:>5.1}%  {k:<52} e.g.({}, {}, {})  bbox x {}..{}, y {}..{}, z {}..{}",
                s.n,
                100.0 * s.n as f64 / tot,
                100.0 * cum as f64 / tot,
                s.ex.0,
                s.ex.1,
                s.ex.2,
                b[0], b[3], b[1], b[4], b[2], b[5],
            );
        }
        let mut w: Vec<_> = worst.iter().collect();
        w.sort_by_key(|(_, n)| std::cmp::Reverse(**n));
        println!("WORST CHUNKS:");
        for ((cx, cz), n) in w.iter().take(10) {
            println!("WORST ({cx:>4},{cz:>4}) {n} cells");
        }
    }
}

fn main() {
    let mut args = std::env::args().skip(1);
    let seed: i64 = args.next().and_then(|s| s.parse().ok()).unwrap_or(12345);
    let cx: i32 = args.next().and_then(|s| s.parse().ok()).unwrap_or(6);
    let cz: i32 = args.next().and_then(|s| s.parse().ok()).unwrap_or(-2);
    let radius: i32 = args.next().and_then(|s| s.parse().ok()).unwrap_or(1);
    let region_dir = args.next().unwrap_or_else(|| {
        "tools/nbt-ref/vanilla-fresh-12345/world/dimensions/minecraft/overworld/region".to_string()
    });

    // PARITY_SCAN=<step>: audit EVERY comparable chunk in the ref (step = sample
    // every Nth chunk to trade coverage for time; PARITY_SCAN=1 = full audit).
    let scan_step: usize =
        std::env::var_os("PARITY_SCAN").map(|v| v.to_str().and_then(|s| s.parse().ok()).unwrap_or(1)).unwrap_or(0);

    // Fixed worker pool: default = cores - 2 so the box stays responsive
    // (PARITY_WORKERS overrides). Generated chunks stream out through a
    // bounded channel as they finish instead of materializing whole 64-chunk
    // batches, and vanilla NBT decoding is prefetched on a dedicated thread
    // so it overlaps generation.
    let n_workers: usize = std::env::var_os("PARITY_WORKERS")
        .and_then(|v| v.to_str().and_then(|s| s.parse::<usize>().ok()))
        .filter(|&n| n > 0)
        .unwrap_or_else(|| {
            std::thread::available_parallelism()
                .map(|n| n.get())
                .unwrap_or(4)
                .saturating_sub(2)
                .max(1)
        });
    println!("workers={n_workers} (cores-2; PARITY_WORKERS overrides)");

    let gen = ChunkGenerator::new(seed);
    let window: Option<Vec<(i32, i32)>> = if scan_step > 0 {
        None
    } else {
        Some(
            (cz - radius..=cz + radius)
                .flat_map(|z| (cx - radius..=cx + radius).map(move |x| (x, z)))
                .collect(),
        )
    };

    let mut histogram: Option<HashMap<String, u64>> =
        if std::env::var_os("PARITY_HISTO").is_some() {
            Some(Default::default())
        } else {
            None
        };
    let ledger_path = std::env::var_os("PARITY_LEDGER").map(std::path::PathBuf::from);
    let mut gaps = Gaps {
        out: ledger_path
            .as_ref()
            .map(|p| std::fs::File::create(p).expect("ledger path")),
        rows: 0,
        map: Default::default(),
    };
    if let Some(p) = ledger_path.as_ref() {
        writeln!(gaps.out.as_mut().unwrap(), "x,y,z,class,zone,vanilla,neutron").unwrap();
        println!("LEDGER -> {}", p.display());
    }

    let (coords_tx, coords_rx) = mpsc::channel::<std::sync::Arc<Vec<(i32, i32)>>>();
    let (van_tx, van_rx) =
        mpsc::sync_channel::<(usize, Option<HashMap<(u8, i32, u8), String>>)>(4);

    let wb = neutron_worldgen::generator::WORLD_BOTTOM;
    let wt = neutron_worldgen::generator::WORLD_TOP;
    let mut tot = [0u64; 2];
    let mut chunks = 0u64;
    let mut worst: HashMap<(i32, i32), u64> = HashMap::new();

    std::thread::scope(|s| {
        // Loader thread: owns the region-file cache, discovers the comparable
        // chunk list in scan mode, then decodes vanilla NBT a few chunks ahead
        // of the comparator (bounded channel) so decode overlaps generation.
        let scan_dir = region_dir.clone();
        s.spawn(move || {
            let mut regions: HashMap<(i32, i32), Region> = HashMap::new();
            let coords = std::sync::Arc::new(match window {
                Some(c) => c,
                None => {
                    let mut c = discover_chunks(&mut regions, &scan_dir);
                    if scan_step > 1 {
                        c = c.into_iter().step_by(scan_step).collect();
                    }
                    c
                }
            });
            if coords_tx.send(Arc::clone(&coords)).is_err() {
                return;
            }
            for (i, &(ccx, ccz)) in coords.iter().enumerate() {
                let map = load_vanilla_chunk(&mut regions, &scan_dir, ccx, ccz);
                if van_tx.send((i, map)).is_err() {
                    return;
                }
            }
        });

        let coords: std::sync::Arc<Vec<(i32, i32)>> = coords_rx.recv().expect("chunk discovery");
        if scan_step > 0 {
            println!(
                "seed={seed} SCAN {} comparable chunks (step {scan_step})",
                coords.len()
            );
        } else {
            println!("seed={seed} center=({cx},{cz}) radius={radius}");
        }
        println!(
            "{:>10} {:>9} {:>9} {:>9} {:>9}",
            "chunk", "ALL", "BASE", "core", "border"
        );

        // Generation pool: contiguous coord blocks per worker (sorted coords
        // make consecutive chunks share ~80% of their 5x5 noise neighbourhood,
        // so each worker keeps one persistent NoiseCache across its block and
        // refill work drops ~5x). Results stream out through a bounded
        // channel; comparison happens on this thread in coords order, so
        // output and ledger ordering are deterministic.
        let (gen_tx, gen_rx) = mpsc::sync_channel::<(usize, (i32, i32), neutron_worldgen::GeneratedChunk)>(
            n_workers * 2,
        );
        let gen_ref = &gen;
        let n = coords.len();
        let block = n.div_ceil(n_workers);
        for w in 0..n_workers {
            let tx = gen_tx.clone();
            let gen = gen_ref;
            let coords = Arc::clone(&coords);
            let lo = w * block;
            let hi = ((w + 1) * block).min(n);
            if lo >= hi {
                break;
            }
            s.spawn(move || {
                let mut cache = NoiseCache::with_cap(48);
                for i in lo..hi {
                    let (ccx, ccz) = coords[i];
                    let chunk = gen.generate_chunk_cached(ccx, ccz, &mut cache);
                    if tx.send((i, (ccx, ccz), chunk)).is_err() {
                        break;
                    }
                }
            });
        }        drop(gen_tx);

        let mut pending: BTreeMap<usize, (i32, i32, neutron_worldgen::GeneratedChunk)> =
            BTreeMap::new();
        let mut next_idx = 0usize;
        'compare: while next_idx < coords.len() {
            while !pending.contains_key(&next_idx) {
                match gen_rx.recv() {
                    Ok((i, (ccx, ccz), chunk)) => {
                        pending.insert(i, (ccx, ccz, chunk));
                    }
                    Err(_) => {
                        eprintln!(
                            "generation pool ended early at {next_idx}/{}",
                            coords.len()
                        );
                        break 'compare;
                    }
                }
            }
            let (ccx, ccz, chunk) = pending.remove(&next_idx).unwrap();
            let (_vi, van) = match van_rx.recv() {
                Ok(v) => v,
                Err(_) => {
                    eprintln!("vanilla loader ended early at {next_idx}/{}", coords.len());
                    break 'compare;
                }
            };
            let Some(van) = van else {
                println!("{ccx:>5},{ccz:>4}     missing");
                next_idx += 1;
                continue;
            };
            let mut all = [0u64; 2];
            let mut base = [0u64; 2];
            let mut core = [0u64; 2];
            let mut border = [0u64; 2];
            let hist = &mut histogram;
            for y in wb..wt {
                for z in 0..16u32 {
                    for x in 0..16u32 {
                        let b = chunk.block_at(x, y, z);
                        let nn = vanilla_name(b);
                        let vn = van
                            .get(&(x as u8, y, z as u8))
                            .map(|s| s.as_str())
                            .unwrap_or("minecraft:air");
                        let m = (nn == vn) as u64;
                        all[m as usize] += 1;
                        if !is_vegetation_name(vn) {
                            base[m as usize] += 1;
                        }
                        let d = (x as i32)
                            .min(15 - x as i32)
                            .min(z as i32)
                            .min(15 - z as i32);
                        if d >= 5 {
                            core[m as usize] += 1;
                        } else {
                            border[m as usize] += 1;
                        }
                        if m == 0 {
                            if let Some(h) = hist {
                                let cls = match (nn, vn) {
                                    ("minecraft:air", v) => format!("ours=air vanilla={v}"),
                                    (n, "minecraft:air") => format!("ours={n} vanilla=air"),
                                    (n, v) => format!("ours={n} vanilla={v}"),
                                };
                                *h.entry(cls).or_insert(0) += 1;
                            }
                            if gaps.out.is_some() || scan_step > 0 {
                                gaps.row(ccx, ccz, x, y, z, d, vn, nn);
                            }
                            *worst.entry((ccx, ccz)).or_insert(0) += 1;
                        }
                    }
                }
            }
            for i in 0..2 {
                tot[i] += all[i];
            }
            chunks += 1;
            let pct = |a: [u64; 2]| 100.0 * a[1] as f64 / (a[0] + a[1]) as f64;
            println!(
                "{ccx:>5},{ccz:>4} {:>8.2}% {:>8.2}% {:>8.2}% {:>8.2}%",
                pct(all),
                pct(base),
                pct(core),
                pct(border)
            );
            next_idx += 1;
            if scan_step > 0 {
                eprintln!("scan {}/{}", next_idx, coords.len());
            }
        }
        if next_idx < coords.len() {
            // Aborted early: unblock the loader (it may be parked on a full
            // van channel) and let remaining workers finish, then drain.
            while let Ok((_i, m)) = van_rx.recv() {
                let _ = m;
            }
        }
    });
    if tot[0] + tot[1] > 0 {
        println!(
            "REGION ALL: {:.2}% over {chunks} chunks",
            100.0 * tot[1] as f64 / (tot[0] + tot[1]) as f64
        );
    }
    if let Some(h) = histogram.as_ref() {
        let mut v: Vec<_> = h.iter().collect();
        v.sort_by_key(|(_, n)| std::cmp::Reverse(**n));
        for (cls, n) in v {
            println!("HISTO {n:>7} {cls}");
        }
    }
    if gaps.out.is_some() || scan_step > 0 {
        println!("LEDGER {} cells", gaps.rows);
        gaps.report(&worst);
    }
}
