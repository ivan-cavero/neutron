//! neutron-parity: the single canonical Neutron-vs-vanilla comparison kit.
//!
//! One strict MCA decoder, one cell-exact diff engine, deterministic reports.
//! Replaces the per-example decoder zoo (24 private copies, 3 divergent
//! biome unpackers) with one auditable implementation.

pub mod cache;
pub mod compare;
pub mod refdata;
pub mod report;

pub use compare::{
    compare_chunk, compare_chunk_biomes, vanilla_resolves, BiomeChunkMetrics, ChunkMetrics,
    GapClass, GapKey, GapStat, LedgerRow, RegionAccumulator, Tally, Zone,
};
pub use refdata::{
    discover_dimension_dirs, BiomeGrid, BlockGrid, DimSpec, KNOWN_STRUCTURE_TYPES, ParityError,
    RefChunk, RegionSet, WORLD_DIM,
};
pub use report::{
    build_summary, gate_diff, print_stdout, write_json, write_ledger_csv, GapJson, RunMeta,
    Summary,
};
