//! Deterministic report artifacts. Same inputs -> byte-identical outputs:
//! BTreeMaps everywhere, sort keys fully specified, no timestamps in the
//! payload (run identity comes from seed + ref dir + chunk counts).

use crate::compare::{BiomeChunkMetrics, ChunkMetrics, GapKey, GapStat, LedgerRow, Zone};
use serde::Serialize;
use std::io::Write;

#[derive(Debug, Clone, Serialize, serde::Deserialize)]
pub struct RunMeta {
    pub seed: i64,
    /// "window" or "scan"
    pub mode: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub center: Option<[i32; 2]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub radius: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scan_step: Option<usize>,
    pub ref_dir: String,
    pub chunks_compared: u64,
    pub chunks_missing: u64,
    pub protos_skipped: usize,
}

#[derive(Debug, Clone, Serialize, serde::Deserialize)]
pub struct ZonePct {
    pub pct: f64,
    pub equal: u64,
    pub mismatch: u64,
}

impl ZonePct {
    pub fn of(t: &crate::compare::Tally) -> Self {
        ZonePct { pct: t.pct(), equal: t.equal, mismatch: t.mismatch }
    }
}

#[derive(Debug, Clone, Serialize, serde::Deserialize)]
pub struct MetricsJson {
    pub all: ZonePct,
    pub base: ZonePct,
    pub core: ZonePct,
    pub border: ZonePct,
}

impl MetricsJson {
    pub fn of(m: &ChunkMetrics) -> Self {
        MetricsJson {
            all: ZonePct::of(&m.all),
            base: ZonePct::of(&m.base),
            core: ZonePct::of(&m.core),
            border: ZonePct::of(&m.border),
        }
    }
}

#[derive(Debug, Clone, Serialize, serde::Deserialize)]
pub struct BiomesJson {
    pub pct: f64,
    pub equal: u64,
    pub mismatch: u64,
}

impl BiomesJson {
    pub fn of(m: &BiomeChunkMetrics) -> Self {
        BiomesJson {
            pct: m.quarts.pct(),
            equal: m.quarts.equal,
            mismatch: m.quarts.mismatch,
        }
    }
}

#[derive(Debug, Clone, Serialize, serde::Deserialize)]
pub struct GapJson {
    pub class: crate::compare::GapClass,
    pub vanilla: String,
    pub neutron: String,
    pub n: u64,
    pub share_pct: f64,
    pub example: [i32; 3],
    pub bbox: [i32; 6],
}

#[derive(Debug, Clone, Serialize, serde::Deserialize)]
pub struct WorstChunkJson {
    pub cx: i32,
    pub cz: i32,
    pub mismatches: u64,
}

#[derive(Debug, Clone, Serialize, serde::Deserialize)]
pub struct Summary {
    pub meta: RunMeta,
    pub blocks: MetricsJson,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub biomes: Option<BiomesJson>,
    pub gaps: Vec<GapJson>,
    pub worst_chunks: Vec<WorstChunkJson>,
    /// Vanilla names our palette cannot represent — the upgrade-day signal.
    /// Empty list == no version drift detected.
    pub unmapped_vanilla_names: Vec<String>,
}

/// Build the serializable summary from an accumulator.
///
/// `gaps` are ordered by count desc then key asc; `worst` by count desc then
/// coords asc. Both orderings are total -> deterministic.
pub fn build_summary(
    meta: RunMeta,
    acc: &crate::compare::RegionAccumulator,
    top_gaps: usize,
    top_worst: usize,
) -> Summary {
    let total_rows: u64 = acc.gaps.values().map(|g| g.n).sum();
    let mut gaps: Vec<(&GapKey, &GapStat)> = acc.gaps.iter().collect();
    gaps.sort_by(|a, b| b.1.n.cmp(&a.1.n).then_with(|| a.0.cmp(b.0)));
    let mut worst: Vec<(&(i32, i32), &u64)> = acc.worst.iter().collect();
    worst.sort_by(|a, b| b.1.cmp(a.1).then_with(|| a.0.cmp(b.0)));
    Summary {
        blocks: MetricsJson::of(&acc.totals),
        biomes: acc.biome_totals.as_ref().map(BiomesJson::of),
        gaps: gaps
            .into_iter()
            .take(top_gaps)
            .map(|(k, s)| GapJson {
                class: k.class,
                vanilla: k.vanilla.clone(),
                neutron: k.neutron.clone(),
                n: s.n,
                share_pct: 100.0 * s.n as f64 / total_rows.max(1) as f64,
                example: s.example,
                bbox: s.bbox,
            })
            .collect(),
        worst_chunks: worst
            .into_iter()
            .take(top_worst)
            .map(|(&(cx, cz), n)| WorstChunkJson { cx, cz, mismatches: *n })
            .collect(),
        unmapped_vanilla_names: acc.unmapped_vanilla.iter().cloned().collect(),
        meta,
    }
}

pub fn write_json(path: &std::path::Path, s: &Summary) -> std::io::Result<()> {
    let mut f = std::fs::File::create(path)?;
    serde_json::to_writer_pretty(&mut f, s)?;
    f.write_all(b"\n")
}

/// Historical ledger format kept byte-compatible:
/// `x,y,z,class,zone,vanilla,neutron` with class in {missing,extra,wrong}.
pub fn write_ledger_csv(path: &std::path::Path, rows: &[LedgerRow]) -> std::io::Result<u64> {
    use std::io::{BufWriter, Write};
    let f = std::fs::File::create(path)?;
    let mut w = BufWriter::new(f);
    writeln!(w, "x,y,z,class,zone,vanilla,neutron")?;
    for r in rows {
        writeln!(
            w,
            "{},{},{},{},{},{},{}",
            r.wx,
            r.y,
            r.wz,
            r.class.as_str(),
            match r.zone {
                Zone::Core => "core",
                Zone::Border => "border",
            },
            r.vanilla,
            r.neutron
        )?;
    }
    w.flush()?;
    Ok(rows.len() as u64)
}

/// Refactor-gate comparison: two summaries must be IDENTICAL for a refactor
/// to be parity-neutral. Every divergence is reported with its numbers so the
/// reviewer sees exactly what moved. Order-stable output.
pub fn gate_diff(base: &Summary, new: &Summary) -> Vec<String> {
    let mut d = Vec::new();
    let b = &base.meta;
    let n = &new.meta;
    if b.chunks_compared != n.chunks_compared {
        d.push(format!(
            "meta.chunks_compared: {} -> {}",
            b.chunks_compared, n.chunks_compared
        ));
    }
    if b.chunks_missing != n.chunks_missing {
        d.push(format!(
            "meta.chunks_missing: {} -> {}",
            b.chunks_missing, n.chunks_missing
        ));
    }
    let pb = &base.blocks;
    let pn = &new.blocks;
    for (name, a, c) in [
        ("core", pb.core.pct, pn.core.pct),
        ("base", pb.base.pct, pn.base.pct),
        ("all", pb.all.pct, pn.all.pct),
        ("border", pb.border.pct, pn.border.pct),
    ] {
        if a.to_bits() != c.to_bits() {
            d.push(format!("blocks.{name}%: {a:.4} -> {c:.4}"));
        }
    }
    match (&base.biomes, &new.biomes) {
        (Some(x), Some(y)) => {
            if x.pct.to_bits() != y.pct.to_bits() {
                d.push(format!("biomes%: {:.4} -> {:.4}", x.pct, y.pct));
            }
        }
        (None, Some(_)) => d.push("biomes: absent -> present".into()),
        (Some(_), None) => d.push("biomes: present -> absent".into()),
        (None, None) => {}
    }
    let key = |g: &GapJson| format!("{:?}|{}|{}", g.class, g.vanilla, g.neutron);
    let bm: std::collections::BTreeMap<String, &GapJson> =
        base.gaps.iter().map(|g| (key(g), g)).collect();
    let nm: std::collections::BTreeMap<String, &GapJson> =
        new.gaps.iter().map(|g| (key(g), g)).collect();
    for (k, bg) in &bm {
        match nm.get(k) {
            None => d.push(format!("gap disappeared: {k} (was {} cells)", bg.n)),
            Some(ng) => {
                if bg.n != ng.n {
                    d.push(format!("gap count: {k}: {} -> {}", bg.n, ng.n));
                }
            }
        }
    }
    for k in nm.keys() {
        if !bm.contains_key(k) {
            d.push(format!("gap appeared: {k}"));
        }
    }
    let bw: Vec<_> = base.worst_chunks.iter().map(|w| (w.cx, w.cz, w.mismatches)).collect();
    let nw: Vec<_> = new.worst_chunks.iter().map(|w| (w.cx, w.cz, w.mismatches)).collect();
    if bw != nw {
        d.push(format!("worst_chunks changed: {bw:?} -> {nw:?}"));
    }
    if base.unmapped_vanilla_names != new.unmapped_vanilla_names {
        d.push(format!(
            "unmapped_vanilla_names: {:?} -> {:?}",
            base.unmapped_vanilla_names, new.unmapped_vanilla_names
        ));
    }
    d
}

/// Human summary on stdout. Headline is CORE% (deterministic interior);
/// ALL/BASE include border scheduler noise and are printed for continuity
/// with the historical meter.
pub fn print_stdout(s: &Summary) {
    println!("== PARITY SUMMARY (seed {}) ==", s.meta.seed);
    println!(
        "chunks compared {} (missing {}, protos skipped {})",
        s.meta.chunks_compared, s.meta.chunks_missing, s.meta.protos_skipped
    );
    println!(
        "REGION CORE : {:>6.2}%   <-- primary metric (border noise excluded)",
        s.blocks.core.pct
    );
    println!(
        "REGION BASE : {:>6.2}%   (non-vegetation)",
        s.blocks.base.pct
    );
    println!(
        "REGION ALL  : {:>6.2}%   (includes vanilla scheduler noise)",
        s.blocks.all.pct
    );
    if let Some(b) = &s.biomes {
        println!("BIOME QUARTS: {:>6.2}%   ({} quarts differ)", b.pct, b.mismatch);
    }
    if !s.unmapped_vanilla_names.is_empty() {
        println!(
            "!! UNMAPPED VANILLA NAMES ({}): version drift — these cells diff by definition:",
            s.unmapped_vanilla_names.len()
        );
        for n in &s.unmapped_vanilla_names {
            println!("   {n}");
        }
    }
    println!("TOP GAPS (count desc, key asc):");
    for g in s.gaps.iter().take(15) {
        println!(
            "GAP {:>7} {:>5.1}%  {:<7} van={:<42} neu={}  e.g.({},{},{})",
            g.n,
            g.share_pct,
            format!("{:?}", g.class).to_lowercase(),
            g.vanilla,
            g.neutron,
            g.example[0],
            g.example[1],
            g.example[2]
        );
    }
    println!("WORST CHUNKS:");
    for wc in s.worst_chunks.iter().take(10) {
        println!("WORST ({:>4},{:>4}) {} cells", wc.cx, wc.cz, wc.mismatches);
    }
}
