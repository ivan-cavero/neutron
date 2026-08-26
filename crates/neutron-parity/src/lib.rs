//! neutron-parity: the single canonical Neutron-vs-vanilla comparison kit.
//!
//! One strict MCA decoder, one cell-exact diff engine, deterministic reports.
//! Replaces the per-example decoder zoo (24 private copies, 3 divergent
//! biome unpackers) with one auditable implementation.

pub mod compare;
pub mod refdata;
pub mod report;

pub use compare::{
    compare_chunk, compare_chunk_biomes, vanilla_resolves, BiomeChunkMetrics, ChunkMetrics,
    GapClass, GapKey, GapStat, LedgerRow, RegionAccumulator, Tally, Zone,
};
pub use refdata::{BiomeGrid, BlockGrid, ParityError, RefChunk, RegionSet, CHUNK_CELLS, QUARTS_Y, WORLD_BOTTOM, WORLD_TOP};
pub use report::{build_summary, print_stdout, write_json, write_ledger_csv, RunMeta, Summary};
