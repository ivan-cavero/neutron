//! vanilladiff — vanilla-vs-vanilla determinism check.
//!
//! Compares two vanilla reference region dirs block-by-block over their
//! common full-status chunks. Answers: "do two vanilla generations of the
//! same seed+procedure produce identical blocks?"
//!
//! Usage:
//!   vanilladiff --a DIR --b DIR [--limit N]
//!
//! Exit 0 iff every common full chunk is cell-identical (blocks; biomes
//! when present on both sides). Prints per-chunk mismatch counts, the
//! first few diffs per chunk, and totals. Deterministic output for
//! identical inputs (sorted chunk order, capped diff dump).

use neutron_parity::refdata::{DimSpec, RegionSet};
use std::path::PathBuf;

fn usage() -> ! {
    eprintln!("usage: vanilladiff --a DIR --b DIR [--limit N]");
    std::process::exit(64);
}

fn main() {
    let mut a: Option<PathBuf> = None;
    let mut b: Option<PathBuf> = None;
    let mut limit: usize = 5;
    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--a" => a = Some(PathBuf::from(it.next().unwrap_or_else(|| usage()))),
            "--b" => b = Some(PathBuf::from(it.next().unwrap_or_else(|| usage()))),
            "--limit" => {
                limit = it
                    .next()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or_else(|| usage())
            }
            _ => usage(),
        }
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
