//! Cell-exact comparison of a generated Neutron chunk against a decoded
//! vanilla reference chunk. Deterministic by construction: gap keys live in
//! a BTreeMap ordered by (class, vanilla, neutron); worst chunks order by
//! (count desc, coords asc). Same input -> byte-identical report.

use crate::refdata::{RefChunk, DimSpec};
use neutron_worldgen::surface::{is_vegetation_name, vanilla_name, BlockId};
use neutron_worldgen::GeneratedChunk;
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GapClass {
    /// Vanilla has a block here, Neutron has air.
    Missing,
    /// Neutron has a block here, vanilla has air.
    Extra,
    /// Both non-air, different blocks.
    Wrong,
}

impl GapClass {
    pub fn as_str(&self) -> &'static str {
        match self {
            GapClass::Missing => "missing",
            GapClass::Extra => "extra",
            GapClass::Wrong => "wrong",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Zone {
    Core,
    Border,
}

/// min distance to the chunk edge; core = d >= 5 (interior 6x6 columns),
/// border carries the vanilla thread-scheduler noise documented in
/// new-mc-version.sh.
pub fn zone_of(x: u32, z: u32) -> Zone {
    let d = (x as i32).min(15 - x as i32).min(z as i32).min(15 - z as i32);
    if d >= 5 { Zone::Core } else { Zone::Border }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LedgerRow {
    pub wx: i32,
    pub y: i32,
    pub wz: i32,
    pub class: GapClass,
    /// True when the vanilla side name is not resolvable by our palette —
    /// flagged in reports/JSON; the CSV keeps its historical shape classes.
    pub unmapped: bool,
    pub zone: Zone,
    pub vanilla: String,
    pub neutron: String,
}

#[derive(Debug, Default, Clone, Copy, Serialize)]
pub struct Tally {
    #[serde(skip)]
    pub mismatch: u64,
    #[serde(skip)]
    pub equal: u64,
}

impl Tally {
    fn add(&mut self, m: bool) {
        if m { self.equal += 1 } else { self.mismatch += 1 }
    }
    pub fn total(&self) -> u64 {
        self.mismatch + self.equal
    }
    pub fn pct(&self) -> f64 {
        100.0 * self.equal as f64 / self.total().max(1) as f64
    }
}

#[derive(Debug, Default, Clone, Copy, Serialize)]
pub struct ChunkMetrics {
    pub all: Tally,
    /// Excludes cells whose *vanilla* block is vegetation (same classifier
    /// semantics as the historical meter).
    pub base: Tally,
    pub core: Tally,
    pub border: Tally,
}

#[derive(Debug, Default, Clone, Serialize)]
pub struct BiomeChunkMetrics {
    pub quarts: Tally,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct GapKey {
    pub class: GapClass,
    pub vanilla: String,
    pub neutron: String,
}

#[derive(Debug, Default, Clone, Serialize)]
pub struct GapStat {
    pub n: u64,
    pub example: [i32; 3],
    pub bbox: [i32; 6],
}

#[derive(Debug, Default)]
pub struct RegionAccumulator {
    pub totals: ChunkMetrics,
    pub biome_totals: Option<BiomeChunkMetrics>,
    pub gaps: BTreeMap<GapKey, GapStat>,
    pub worst: BTreeMap<(i32, i32), u64>,
    pub rows: Vec<LedgerRow>,
    pub unmapped_vanilla: std::collections::BTreeSet<String>,
    pub chunks_compared: u64,
    pub chunks_missing: u64,
}

pub fn classify(vn: &str, nn: &str) -> GapClass {
    if vn == "minecraft:air" {
        GapClass::Extra
    } else if nn == "minecraft:air" {
        GapClass::Missing
    } else {
        GapClass::Wrong
    }
}

/// Compare one generated chunk against one ref chunk.
///
/// `collect_rows`: push every mismatch into `rows` (ledger mode). Without it,
/// only aggregates accumulate — cheap enough for wide scans.
pub fn compare_chunk(
    acc: &mut RegionAccumulator,
    cx: i32,
    cz: i32,
    chunk: &GeneratedChunk,
    van: &RefChunk,
    collect_rows: bool,
) -> ChunkMetrics {
    debug_assert_eq!(van.blocks.names.len(), van.blocks.dim.cells());
    let dim: DimSpec = van.blocks.dim;
    let mut m = ChunkMetrics::default();
    for y in dim.bottom()..dim.top() {
        for z in 0..16u32 {
            for x in 0..16u32 {
                let b = chunk.block_at(x, y, z);
                let nn = vanilla_name(b);
                let vn = van.blocks.get(x, y, z);
                let matched = nn == vn;
                let zone = zone_of(x, z);
                m.all.add(matched);
                if !is_vegetation_name(vn) {
                    m.base.add(matched);
                }
                match zone {
                    Zone::Core => m.core.add(matched),
                    Zone::Border => m.border.add(matched),
                }
                if matched {
                    continue;
                }
                // Version-drift tripwire: a vanilla name our palette cannot
                // represent makes every such cell diff regardless of what
                // the generator does. Flag it loudly instead of letting it
                // pollute feature-gap rankings.
                let unmapped = vn != "minecraft:air" && !vanilla_resolves(vn);
                if unmapped {
                    acc.unmapped_vanilla.insert(vn.to_string());
                }
                let e = acc
                    .gaps
                    .entry(GapKey {
                        class: classify(vn, nn),
                        vanilla: vn.to_string(),
                        neutron: nn.to_string(),
                    })
                    .or_default();
                e.n += 1;
                let (wx, wz) = (cx * 16 + x as i32, cz * 16 + z as i32);
                if e.n == 1 {
                    e.example = [wx, y, wz];
                    e.bbox = [wx, y, wz, wx, y, wz];
                } else {
                    e.bbox[0] = e.bbox[0].min(wx);
                    e.bbox[1] = e.bbox[1].min(y);
                    e.bbox[2] = e.bbox[2].min(wz);
                    e.bbox[3] = e.bbox[3].max(wx);
                    e.bbox[4] = e.bbox[4].max(y);
                    e.bbox[5] = e.bbox[5].max(wz);
                }
                *acc.worst.entry((cx, cz)).or_insert(0) += 1;
                if collect_rows {
                    acc.rows.push(LedgerRow {
                        wx,
                        y,
                        wz,
                        class: classify(vn, nn),
                        unmapped,
                        zone,
                        vanilla: vn.to_string(),
                        neutron: nn.to_string(),
                    });
                }
            }
        }
    }
    acc.totals.all.mismatch += m.all.mismatch;
    acc.totals.all.equal += m.all.equal;
    acc.totals.base.mismatch += m.base.mismatch;
    acc.totals.base.equal += m.base.equal;
    acc.totals.core.mismatch += m.core.mismatch;
    acc.totals.core.equal += m.core.equal;
    acc.totals.border.mismatch += m.border.mismatch;
    acc.totals.border.equal += m.border.equal;
    acc.chunks_compared += 1;
    m
}

/// A vanilla name we cannot map into our palette: every cell carrying it is
/// guaranteed to diff no matter what our generator does. Air variants are
/// always resolvable.
pub fn vanilla_resolves(name: &str) -> bool {
    if name == "minecraft:air" || name == "minecraft:cave_air" || name == "minecraft:void_air" {
        return true;
    }
    BlockId::from_name(name).is_some()
}

/// Compare stored quart biomes vs the climate sampler at quart centers.
pub fn compare_chunk_biomes(
    acc: &mut RegionAccumulator,
    gen: &neutron_worldgen::ChunkGenerator,
    cx: i32,
    cz: i32,
    van: &RefChunk,
) -> Option<BiomeChunkMetrics> {
    let biomes = van.biomes.as_ref()?;
    let mut m = BiomeChunkMetrics::default();
    for qy in 0i32..biomes.dim.quarts_y() {
        for qz in 0u32..4 {
            for qx in 0u32..4 {
                let want = biomes.get(qx, qy, qz);
                let wx = cx * 4 + qx as i32 * 4 + 2;
                let wy = (qy * 4 + 2) + biomes.dim.min_y;
                let wz = cz * 4 + qz as i32 * 4 + 2;
                let got = neutron_worldgen::feature_dispatch::biome_id_to_name(
                    neutron_worldgen::biome_source::biome_id_at_block(&gen.state, wx, wy, wz),
                );
                m.quarts.add(got == want);
            }
        }
    }
    match &mut acc.biome_totals {
        Some(t) => {
            t.quarts.mismatch += m.quarts.mismatch;
            t.quarts.equal += m.quarts.equal;
        }
        t @ None => {
            *t = Some(BiomeChunkMetrics {
                quarts: Tally {
                    mismatch: m.quarts.mismatch,
                    equal: m.quarts.equal,
                },
            })
        }
    }
    Some(m)
}
