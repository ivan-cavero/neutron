//! ticket_sim_anyseed — any-seed validation of the deco_schedule ticket_sim
//! decorate order against mined decoration-precedence pairs
//! (deco_precedence_dump CSVs, seeds 424242 / 12345 / 777).
//!
//! Definitions (mirror of the original 424242 validation, score2.rs):
//! - A CSV row (winner, loser) means the winner pass ran LAST on the contested
//!   cell (the loser finished before it). A model is CONSISTENT with a row iff
//!   the model places the winner strictly after the loser.
//! - ticket_sim rank: first-occurrence index of an origin chunk in the global
//!   chronological decorate sequence of the canonical pregen
//!   (`deco_schedule::decorate_sequence`, seed-independent). Rows whose winner
//!   or loser is unranked (outside the simulated footprint) count as
//!   inconsistent; the count is reported.
//! - row baseline: ascending (cz, cx) z-major order.
//! - world_origin baseline: ascending (dist² of chunk centre (cx*16+8,
//!   cz*16+8) from world origin, cz, cx).
//! - interior subset: consumer chunk (ccx,ccz) in [-8..7]² — the forced
//!   square, identical to deco_precedence_dump's phase-0 report band.
//!
//! Usage:
//!   ticket_sim_anyseed <pairs.csv> [more.csv ...]
//! Appends a section per CSV to /tmp/opencode/consistency_report.txt.

use neutron_worldgen::deco_schedule;
use std::collections::{HashMap, HashSet};
use std::io::{BufRead, Write};

type P = (i32, i32);

struct Pair {
    w: P,
    l: P,
    ccx: i32,
    ccz: i32,
}

fn load(path: &str) -> Vec<Pair> {
    let f = std::fs::File::open(path).unwrap_or_else(|e| panic!("open {path}: {e}"));
    let mut out = Vec::new();
    for line in std::io::BufReader::new(f).lines() {
        let line = line.unwrap();
        if line.starts_with('#') || line.starts_with("ccx") || line.trim().is_empty() {
            continue;
        }
        let c: Vec<&str> = line.split(',').collect();
        assert!(c.len() >= 8, "bad row: {line}");
        let g = |i: usize| c[i].trim().parse::<i32>().unwrap();
        out.push(Pair {
            w: (g(2), g(3)),
            l: (g(5), g(6)),
            ccx: g(0),
            ccz: g(1),
        });
    }
    out
}

fn row_key(p: P) -> (i64, i64, i64) {
    (p.1 as i64, p.0 as i64, 0)
}

fn world_origin_key(p: P) -> (i64, i64, i64) {
    let wx = p.0 as i64 * 16 + 8;
    let wz = p.1 as i64 * 16 + 8;
    (wx * wx + wz * wz, p.1 as i64, p.0 as i64)
}

/// consistent = winner keyed strictly after loser; interior = cc in [-8..7]².
fn pct(pairs: &[Pair], key: &dyn Fn(P) -> (i64, i64, i64)) -> (usize, usize, usize, usize) {
    let (mut ok, mut ni, mut oki) = (0usize, 0usize, 0usize);
    for p in pairs {
        let c = key(p.w) > key(p.l);
        if c {
            ok += 1;
        }
        if (-8..=7).contains(&p.ccx) && (-8..=7).contains(&p.ccz) {
            ni += 1;
            if c {
                oki += 1;
            }
        }
    }
    (ok, pairs.len(), oki, ni)
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    assert!(args.len() >= 2, "usage: ticket_sim_anyseed <pairs.csv> [...]");
    let t0 = std::time::Instant::now();

    let batches = deco_schedule::canonical_batches();
    let seq = deco_schedule::simulate_canonical_pregen();
    let mut dup = 0usize;
    let mut rank: HashMap<P, i64> = HashMap::with_capacity(seq.len());
    for (i, &p) in seq.iter().enumerate() {
        if rank.insert(p, i as i64).is_some() {
            dup += 1;
        }
    }
    let bbox = seq.iter().fold((i32::MAX, i32::MIN, i32::MAX, i32::MIN), |a, &(x, z)| {
        (a.0.min(x), a.1.max(x), a.2.min(z), a.3.max(z))
    });
    let mut out: Vec<String> = Vec::new();
    out.push(format!(
        "=== ticket_sim_anyseed run args={} elapsed={:.1}s",
        args[1..].join(" "),
        t0.elapsed().as_secs_f64()
    ));
    out.push(format!(
        "== ticket_sim decorate sequence: batches={:?} events={} duplicate_chunks={} bbox=x[{}..{}] z[{}..{}]",
        batches
            .iter()
            .map(|b| format!("rect({},{},{},{})", b.x1, b.z1, b.x2, b.z2))
            .collect::<Vec<_>>(),
        seq.len(),
        dup,
        bbox.0,
        bbox.1,
        bbox.2,
        bbox.3
    ));

    for csv in &args[1..] {
        let pairs = load(csv);
        let n = pairs.len();
        let ccbb = pairs.iter().fold((i32::MAX, i32::MIN, i32::MAX, i32::MIN), |a, p| {
            (a.0.min(p.ccx), a.1.max(p.ccx), a.2.min(p.ccz), a.3.max(p.ccz))
        });
        let orbb = pairs.iter().fold((i32::MAX, i32::MIN, i32::MAX, i32::MIN), |a, p| {
            (
                a.0.min(p.w.0.min(p.l.0)),
                a.1.max(p.w.0.max(p.l.0)),
                a.2.min(p.w.1.min(p.l.1)),
                a.3.max(p.w.1.max(p.l.1)),
            )
        });
        out.push(format!(
            "== seed csv={csv} pairs={n} cc_bbox=x[{}..{}] z[{}..{}] origin_bbox=x[{}..{}] z[{}..{}]",
            ccbb.0, ccbb.1, ccbb.2, ccbb.3, orbb.0, orbb.1, orbb.2, orbb.3
        ));

        let unranked = pairs
            .iter()
            .filter(|p| !rank.contains_key(&p.w) || !rank.contains_key(&p.l))
            .count();
        let (mut ok, mut ni, mut oki, mut ok_rev) = (0usize, 0usize, 0usize, 0usize);
        for p in &pairs {
            let (rw, rl) = (rank[&p.w], rank[&p.l]);
            let c = rw > rl;
            if c {
                ok += 1;
            } else if rw < rl {
                ok_rev += 1;
            }
            if (-8..=7).contains(&p.ccx) && (-8..=7).contains(&p.ccz) {
                ni += 1;
                if c {
                    oki += 1;
                }
            }
        }
        let pc = |k: usize, d: usize| if d == 0 { f64::NAN } else { k as f64 / d as f64 * 100.0 };
        out.push(format!(
            "  ticket_sim      all={:.2}% (unranked_rows={unranked})  interior(cc in [-8..7]^2)={:.2}% (n={ni})",
            pc(ok, n),
            pc(oki, ni)
        ));
        out.push(format!(
            "    [calibration] reversed-direction(winner earlier) all={:.2}%",
            pc(ok_rev, n)
        ));

        let (ra, _, ri, rni) = pct(&pairs, &row_key);
        out.push(format!(
            "  row (cz,cx asc) all={:.2}%  interior={:.2}% (n={rni})",
            pc(ra, n),
            pc(ri, rni)
        ));
        let (wa, _, wi, wni) = pct(&pairs, &world_origin_key);
        out.push(format!(
            "  world_origin    all={:.2}%  interior={:.2}% (n={wni})",
            pc(wa, n),
            pc(wi, wni)
        ));
        out.push(format!(
            "  ticket_sim - row: all={:+.2}pp interior={:+.2}pp",
            pc(ok, n) - pc(ra, n),
            pc(oki, ni) - pc(ri, rni)
        ));

        let mut dirs: HashMap<(P, P), u64> = HashMap::new();
        for p in &pairs {
            *dirs.entry((p.w, p.l)).or_insert(0) += 1;
        }
        let und: HashSet<(P, P)> =
            dirs.keys().map(|&(a, b)| if a < b { (a, b) } else { (b, a) }).collect();
        let (mut conflicted, mut weak_rows, mut worst) = (0u64, 0u64, None);
        for &(a, b) in &und {
            let ab = dirs.get(&(a, b)).copied().unwrap_or(0);
            let ba = dirs.get(&(b, a)).copied().unwrap_or(0);
            if ab > 0 && ba > 0 {
                conflicted += 1;
                weak_rows += ab.min(ba);
                if worst.map_or(true, |(_, _, w)| ab.min(ba) > w) {
                    worst = Some((a, b, ab.min(ba)));
                }
            }
        }
        out.push(format!(
            "  conflicts: directed_edges={} unordered_pairs={} bidirectional_pairs={conflicted} ({:.2}% of pairs) weak_side_rows={weak_rows} ({:.2}% of rows)",
            dirs.len(),
            und.len(),
            conflicted as f64 / und.len().max(1) as f64 * 100.0,
            weak_rows as f64 / n as f64 * 100.0,
        ));
        if let Some((a, b, m)) = worst {
            out.push(format!(
                "    worst mutual pair ({},{})-({},{}): weaker side {m}",
                a.0, a.1, b.0, b.1
            ));
        }

        const TRI_MIN: u64 = 5;
        let mut adj: HashMap<P, Vec<P>> = HashMap::new();
        for (&(a, b), &c) in &dirs {
            if c >= TRI_MIN {
                adj.entry(a).or_default().push(b);
            }
        }
        let mut cycles = 0u64;
        let nodes: Vec<P> = adj.keys().copied().collect();
        for &a in &nodes {
            for &b in adj.get(&a).into_iter().flatten() {
                if !(a < b) {
                    continue;
                }
                for &c in adj.get(&b).into_iter().flatten() {
                    if !(b < c) {
                        continue;
                    }
                    if adj.get(&c).map_or(false, |l| l.binary_search(&a).is_ok()) {
                        cycles += 1;
                    }
                }
            }
        }
        out.push(format!(
            "  transitivity: oriented triangle cycles (edges support>={TRI_MIN}): {cycles}"
        ));
    }

    for l in &out {
        println!("{l}");
    }
    let mut rep = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open("/tmp/opencode/consistency_report.txt")
        .expect("open report");
    for l in &out {
        writeln!(rep, "{l}").unwrap();
    }
}
