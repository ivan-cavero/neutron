//! vanilladiff — vanilla-vs-vanilla determinism check + race-mask builder.
//!
//! Diff mode: compares two vanilla reference region dirs block-by-block
//! over their common full-status chunks. Answers: "do two vanilla
//! generations of the same seed+procedure produce identical blocks?"
//!
//! Usage:
//!   vanilladiff --a DIR --b DIR [--limit N]
//!
//! Mask mode: builds a race-cell mask from N samples of the same
//! seed+procedure. A cell lands in the mask iff NOT all samples agree on
//! it — those cells are order/timing-sensitive (scheduler races) and must
//! be excluded from deterministic-parity measurement (`parity
//! --race-mask FILE`). Samples must be tick-frozen (randomTickSpeed 0,
//! no weather) so post-gen growth doesn't pollute the mask.
//!
//! Usage:
//!   vanilladiff --samples A,B,C --mask-out MASK.txt
//!
//! Mask format: `x,y,z` (world coords) one per line, sorted. Exit 0 always
//! (a divergent mask is a finding, not a failure); prints chunk coverage
//! and mask size. Diff mode exits 0 iff cell-identical.

use neutron_parity::refdata::{DimSpec, RegionSet};
use std::path::PathBuf;

fn usage() -> ! {
    eprintln!("usage: vanilladiff --a DIR --b DIR [--limit N]");
    eprintln!("   or: vanilladiff --samples A,B,C --mask-out MASK.txt");
    std::process::exit(64);
}

fn main() {
    let mut a: Option<PathBuf> = None;
    let mut b: Option<PathBuf> = None;
    let mut samples: Option<String> = None;
    let mut mask_out: Option<PathBuf> = None;
    let mut limit: usize = 5;
    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--a" => a = Some(PathBuf::from(it.next().unwrap_or_else(|| usage()))),
            "--b" => b = Some(PathBuf::from(it.next().unwrap_or_else(|| usage()))),
            "--samples" => samples = Some(it.next().unwrap_or_else(|| usage())),
            "--mask-out" => mask_out = Some(PathBuf::from(it.next().unwrap_or_else(|| usage()))),
            "--limit" => {
                limit = it
                    .next()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or_else(|| usage())
            }
            _ => usage(),
        }
    }
    if let (Some(s), Some(o)) = (samples, mask_out) {
        return run_mask_mode(&s, &o);
    }
    let (adir, bdir) = match (a, b) {
        (Some(a), Some(b)) => (a, b),
        _ => usage(),
    };
    let dim = DimSpec::OVERWORLD;
    let mut ra = RegionSet::open(&adir).unwrap_or_else(|e| {
        eprintln!("vanilladiff: --a: {e}");
        std::process::exit(2);
    });
    let mut rb = RegionSet::open(&bdir).unwrap_or_else(|e| {
        eprintln!("vanilladiff: --b: {e}");
        std::process::exit(2);
    });
    let da = ra.discover().unwrap_or_else(|e| {
        eprintln!("vanilladiff: discover --a: {e}");
        std::process::exit(2);
    });
    let db = rb.discover().unwrap_or_else(|e| {
        eprintln!("vanilladiff: discover --b: {e}");
        std::process::exit(2);
    });
    let setb: std::collections::BTreeSet<(i32, i32)> = db.full.into_iter().collect();
    let na_full = da.full.len();
    let common: Vec<(i32, i32)> = da
        .full
        .into_iter()
        .filter(|c| setb.contains(c))
        .collect();
    println!(
        "vanilladiff: a={na_full} full chunks, b={} full chunks, common={}",
        setb.len(),
        common.len()
    );
    let mut chunks_compared = 0usize;
    let mut chunks_identical = 0usize;
    let mut cells_total: u64 = 0;
    let mut cells_diff: u64 = 0;
    let mut biome_cells_diff: u64 = 0;
    for (cx, cz) in &common {
        let ca = match ra.load_chunk(*cx, *cz, dim) {
            Ok(Some(c)) => c,
            Ok(None) => {
                println!("{cx:>5},{cz:>4}  A-missing");
                continue;
            }
            Err(e) => {
                eprintln!("vanilladiff: load A {cx},{cz}: {e}");
                std::process::exit(2);
            }
        };
        let cb = match rb.load_chunk(*cx, *cz, dim) {
            Ok(Some(c)) => c,
            Ok(None) => {
                println!("{cx:>5},{cz:>4}  B-missing");
                continue;
            }
            Err(e) => {
                eprintln!("vanilladiff: load B {cx},{cz}: {e}");
                std::process::exit(2);
            }
        };
        chunks_compared += 1;
        let mut ndiff = 0usize;
        let mut shown = 0usize;
        let n = ca.blocks.names.len().min(cb.blocks.names.len());
        cells_total += n as u64;
        for (i, (na, nb)) in ca.blocks.names.iter().zip(cb.blocks.names.iter()).enumerate() {
            if na != nb {
                ndiff += 1;
                if shown < limit {
                    let y = dim.bottom() + (i / 256) as i32;
                    let rem = i % 256;
                    let (lx, lz) = ((rem % 16) as i32, (rem / 16) as i32);
                    println!("  diff ({},{},{}): A={na} B={nb}", cx * 16 + lx, y, cz * 16 + lz);
                    shown += 1;
                }
            }
        }
        if ca.blocks.names.len() != cb.blocks.names.len() {
            println!(
                "  GRIDLEN A={} B={}",
                ca.blocks.names.len(),
                cb.blocks.names.len()
            );
            ndiff += ca.blocks.names.len().abs_diff(cb.blocks.names.len());
        }
        match (&ca.biomes, &cb.biomes) {
            (Some(ga), Some(gb)) => {
                for (ba, bb) in ga.names.iter().zip(gb.names.iter()) {
                    if ba != bb {
                        biome_cells_diff += 1;
                    }
                }
            }
            _ => {}
        }
        cells_diff += ndiff as u64;
        if ndiff == 0 {
            chunks_identical += 1;
        } else {
            println!("{cx:>5},{cz:>4}  {ndiff} cells differ");
        }
    }
    println!("== VANILLADIFF ==");
    println!("chunks compared {chunks_compared}, identical {chunks_identical}");
    println!("cells total {cells_total}, differ {cells_diff}");
    println!("biome cells differ {biome_cells_diff}");
    if cells_diff == 0 && biome_cells_diff == 0 {
        println!("VANILLADIFF PASS: cell-identical");
    } else {
        println!("VANILLADIFF FAIL: worlds diverge");
        std::process::exit(1);
    }
}

/// Mask mode: `samples` = comma-separated region dirs (same seed+procedure,
/// tick-frozen). Writes every cell where the samples do NOT all agree as
/// `x,y,z` lines (sorted) to `out`. Chunks compared = intersection of full
/// chunks; a chunk full in some samples but not others is skipped loudly.
fn run_mask_mode(samples: &str, out: &std::path::Path) {
    use std::io::Write;
    let dim = DimSpec::OVERWORLD;
    let dirs: Vec<PathBuf> = samples.split(',').map(PathBuf::from).collect();
    if dirs.len() < 2 {
        eprintln!("vanilladiff: mask mode needs >= 2 samples");
        std::process::exit(64);
    }
    let mut sets: Vec<RegionSet> = dirs
        .iter()
        .map(|d| {
            RegionSet::open(d).unwrap_or_else(|e| {
                eprintln!("vanilladiff: open {}: {e}", d.display());
                std::process::exit(2);
            })
        })
        .collect();
    let mut common: Option<std::collections::BTreeSet<(i32, i32)>> = None;
    let mut counts = Vec::new();
    for rs in sets.iter_mut() {
        let d = rs.discover().unwrap_or_else(|e| {
            eprintln!("vanilladiff: discover: {e}");
            std::process::exit(2);
        });
        counts.push(d.full.len());
        let s: std::collections::BTreeSet<(i32, i32)> = d.full.into_iter().collect();
        common = Some(match common {
            None => s,
            Some(c) => c.intersection(&s).cloned().collect(),
        });
    }
    let common = common.unwrap_or_default();
    println!("vanilladiff mask: samples={} full={counts:?} common={}", dirs.len(), common.len());
    let mut mask: Vec<(i32, i32, i32)> = Vec::new();
    let mut cells_total: u64 = 0;
    for (cx, cz) in &common {
        let mut grids = Vec::with_capacity(sets.len());
        let mut missing = false;
        for rs in sets.iter_mut() {
            match rs.load_chunk(*cx, *cz, dim) {
                Ok(Some(c)) => grids.push(c.blocks.names),
                _ => {
                    missing = true;
                    break;
                }
            }
        }
        if missing || grids.is_empty() {
            println!("{cx:>5},{cz:>4}  skipped (load)");
            continue;
        }
        let n = grids[0].len();
        cells_total += n as u64;
        for (i, first) in grids[0].iter().enumerate() {
            if grids[1..].iter().any(|g| g.get(i) != Some(first)) {
                let y = dim.bottom() + (i / 256) as i32;
                let rem = i % 256;
                mask.push((cx * 16 + (rem % 16) as i32, y, cz * 16 + (rem / 16) as i32));
            }
        }
    }
    mask.sort_unstable();
    let f = std::fs::File::create(out).unwrap_or_else(|e| {
        eprintln!("vanilladiff: create {}: {e}", out.display());
        std::process::exit(2);
    });
    let mut w = std::io::BufWriter::new(f);
    for (x, y, z) in &mask {
        writeln!(w, "{x},{y},{z}").unwrap();
    }
    w.flush().unwrap();
    println!(
        "MASK: {} race cells / {} total ({:.3}%) -> {}",
        mask.len(),
        cells_total,
        100.0 * mask.len() as f64 / cells_total.max(1) as f64,
        out.display()
    );
}
