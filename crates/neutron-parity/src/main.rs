//! parity — the single Neutron-vs-vanilla comparison CLI.
//!
//! Usage:
//!   parity --ref DIR [--seed N] [--center CX,CZ] [--radius N]   # window audit
//!   parity --ref DIR --scan [STEP]                              # whole-ref audit
//!   parity --ref DIR --biomes ...                               # + quart biome diff
//!   parity --ledger FILE.csv --json FILE.json --strict --min-core 98.0
//!
//! Exit codes: 0 ok · 1 --min-core threshold · 2 --strict violations /
//! decode errors. Deterministic: identical inputs -> identical stdout, JSON,
//! CSV (gap order is total: count desc then key asc).

use neutron_parity::compare::{compare_chunk, compare_chunk_biomes};
use neutron_parity::refdata::RegionSet;
use neutron_parity::{
    build_summary, print_stdout, write_json, RegionAccumulator, RunMeta,
};
use neutron_worldgen::{ChunkGenerator, NoiseCache};
use std::io::Write;
use std::path::PathBuf;

struct Args {
    seed: i64,
    center: (i32, i32),
    radius: i32,
    scan_step: usize,
    biomes: bool,
    json: Option<PathBuf>,
    ledger: Option<PathBuf>,
    strict: bool,
    min_core: Option<f64>,
    top_gaps: usize,
    refs: String,
}

fn usage() -> ! {
    eprintln!(
        "usage: parity [--ref DIR] [--seed N] [--center CX,CZ] [--radius N]\n\
              \x20             [--scan [STEP]] [--biomes] [--json FILE.json] [--ledger FILE.csv]\n\
              \x20             [--strict] [--min-core PCT] [--top-gaps N]"
    );
    std::process::exit(64);
}

fn parse_args() -> Args {
    let mut a = Args {
        seed: 424242,
        center: (0, 0),
        radius: 1,
        scan_step: 0,
        biomes: false,
        json: None,
        ledger: None,
        strict: false,
        min_core: None,
        top_gaps: 30,
        refs: "tools/nbt-ref/vanilla-fresh-424242/world/dimensions/minecraft/overworld/region"
            .into(),
    };
    let mut it = std::env::args().skip(1).peekable();
    while let Some(arg) = it.next() {
        let mut val = || it.next().unwrap_or_else(|| usage());
        match arg.as_str() {
            "--ref" => a.refs = val(),
            "--seed" => a.seed = val().parse().unwrap_or_else(|_| usage()),
            "--center" => {
                let v = val();
                let (cx, cz) = v.split_once(',').unwrap_or_else(|| usage());
                a.center = (cx.parse().unwrap_or_else(|_| usage()), cz.parse().unwrap_or_else(|_| usage()));
            }
            "--radius" => a.radius = val().parse().unwrap_or_else(|_| usage()),
            "--scan" => {
                a.scan_step = it
                    .peek()
                    .and_then(|v| v.parse::<usize>().ok())
                    .map(|v| {
                        it.next();
                        v
                    })
                    .unwrap_or(1);
            }
            "--biomes" => a.biomes = true,
            "--json" => a.json = Some(PathBuf::from(val())),
            "--ledger" => a.ledger = Some(PathBuf::from(val())),
            "--strict" => a.strict = true,
            "--min-core" => a.min_core = Some(val().parse().unwrap_or_else(|_| usage())),
            "--top-gaps" => a.top_gaps = val().parse().unwrap_or_else(|_| usage()),
            _ => usage(),
        }
    }
    a
}

fn main() {
    let args = parse_args();
    let mut regions = match RegionSet::open(&args.refs) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("parity: {e}");
            std::process::exit(2);
        }
    };

    let mut protos_skipped: usize = 0;
    let coords: Vec<(i32, i32)> = if args.scan_step > 0 {
        let d = match regions.discover() {
            Ok(d) => d,
            Err(e) => {
                eprintln!("parity: {e}");
                std::process::exit(2);
            }
        };
        let protos = d.protos.len();
        protos_skipped = protos;
        if protos > 0 {
            eprintln!("parity: {} proto/stub chunks present (skipped, not measurement targets)", protos);
        }
        let mut c = d.full;
        if args.scan_step > 1 {
            c = c.into_iter().step_by(args.scan_step).collect();
        }
        println!("seed={} SCAN {} comparable chunks (step {})", args.seed, c.len(), args.scan_step);
        c
    } else {
        println!(
            "seed={} center=({},{}) radius={}",
            args.seed, args.center.0, args.center.1, args.radius
        );
        (args.center.1 - args.radius..=args.center.1 + args.radius)
            .flat_map(|z| (args.center.0 - args.radius..=args.center.0 + args.radius).map(move |x| (x, z)))
            .collect()
    };

    // Ledger streams incrementally: bounded memory even for the full-ref run.
    let ledger_file = args
        .ledger
        .as_ref()
        .map(|p| std::fs::File::create(p).expect("ledger path"));
    let mut ledger_writer = ledger_file.map(std::io::BufWriter::new);
    if let Some(w) = ledger_writer.as_mut() {
        writeln!(w, "x,y,z,class,zone,vanilla,neutron").unwrap();
    }

    let gen = ChunkGenerator::new(args.seed);
    let mut acc = RegionAccumulator::default();
    let mut ledger_rows: u64 = 0;
    const BATCH: usize = 64;

    println!(
        "{:>10} {:>9} {:>9} {:>9} {:>9}",
        "chunk", "ALL", "BASE", "core", "border"
    );

    for batch in coords.chunks(BATCH) {
        let generated: Vec<(i32, i32, neutron_worldgen::GeneratedChunk)> =
            std::thread::scope(|s| {
                let gen = &gen;
                let mut handles = Vec::with_capacity(batch.len());
                for &(ccx, ccz) in batch {
                    handles.push(s.spawn(move || {
                        let mut cache = NoiseCache::new();
                        let chunk = gen.generate_chunk_cached(ccx, ccz, &mut cache);
                        (ccx, ccz, chunk)
                    }));
                }
                handles.into_iter().map(|h| h.join().unwrap()).collect()
            });
        if args.scan_step > 0 {
            eprintln!("scan {}/{}", acc.chunks_compared as usize + generated.len(), coords.len());
        }
        for (ccx, ccz, chunk) in generated {
            let van = match regions.load_chunk(ccx, ccz) {
                Ok(v) => v,
                Err(e) => {
                    eprintln!("parity: chunk {ccx},{ccz}: {e}");
                    std::process::exit(2); // strict decode: never compare garbage
                }
            };
            let Some(van) = van else {
                acc.chunks_missing += 1;
                println!("{ccx:>5},{ccz:>4}     missing");
                continue;
            };
            let m = compare_chunk(&mut acc, ccx, ccz, &chunk, &van, ledger_writer.is_some());
            if let (Some(w), true) = (ledger_writer.as_mut(), !acc.rows.is_empty()) {
                use std::io::Write;
                let rows = std::mem::take(&mut acc.rows);
                ledger_rows += rows.len() as u64;
                for r in &rows {
                    writeln!(
                        w,
                        "{},{},{},{},{},{},{}",
                        r.wx,
                        r.y,
                        r.wz,
                        r.class.as_str(),
                        match r.zone {
                            neutron_parity::Zone::Core => "core",
                            neutron_parity::Zone::Border => "border",
                        },
                        r.vanilla,
                        r.neutron
                    )
                    .unwrap();
                }
            } else {
                acc.rows.clear();
            }
            if args.biomes {
                compare_chunk_biomes(&mut acc, &gen, ccx, ccz, &van);
            }
            let pct = |t: &neutron_parity::Tally| t.pct();
            println!(
                "{ccx:>5},{ccz:>4} {:>8.2}% {:>8.2}% {:>8.2}% {:>8.2}%",
                pct(&m.all),
                pct(&m.base),
                pct(&m.core),
                pct(&m.border)
            );
        }
    }
    drop(ledger_writer);

    let meta = RunMeta {
        seed: args.seed,
        mode: if args.scan_step > 0 { "scan".into() } else { "window".into() },
        center: (args.scan_step == 0).then(|| [args.center.0, args.center.1]),
        radius: (args.scan_step == 0).then_some(args.radius),
        scan_step: (args.scan_step > 0).then_some(args.scan_step),
        ref_dir: args.refs.clone(),
        chunks_compared: acc.chunks_compared,
        chunks_missing: acc.chunks_missing,
        protos_skipped,
    };
    let summary = build_summary(meta, &acc, args.top_gaps, 10);
    print_stdout(&summary);

    if let Some(p) = &args.json {
        write_json(p, &summary).expect("write json");
        println!("JSON -> {}", p.display());
    }
    if let Some(p) = &args.ledger {
        println!("LEDGER {} cells -> {}", ledger_rows, p.display());
    }

    let mut exit = 0;
    if let Some(min) = args.min_core {
        if summary.blocks.core.pct < min {
            eprintln!(
                "parity: CORE {:.2}% below --min-core {min}",
                summary.blocks.core.pct
            );
            exit = 1;
        }
    }
    if args.strict {
        if !summary.unmapped_vanilla_names.is_empty() {
            eprintln!(
                "parity: --strict violated: {} unmapped vanilla names (version drift)",
                summary.unmapped_vanilla_names.len()
            );
            exit = 2;
        }
        if summary.meta.chunks_missing > 0 {
            eprintln!(
                "parity: --strict violated: {} ref chunks missing/incomplete",
                summary.meta.chunks_missing
            );
            exit = 2;
        }
    }
    std::process::exit(exit);
}
