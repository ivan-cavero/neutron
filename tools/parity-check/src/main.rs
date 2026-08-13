// Copyright (c) 2026 Neutron Contributors -- MIT License
//
// parity-check: Compare Neutron-generated chunks against vanilla golden data.
//
// Two comparison modes:
// 1. Raw hash: hash the block/biome arrays directly (for neutron-vs-neutron consistency)
// 2. NBT hash: serialize to vanilla-compatible NBT sections and hash (for vanilla parity)
//
// Also generates a "neutron golden data" JSON that can be compared with the
// vanilla golden data using `golden-data compare`.

#![forbid(unsafe_code)]

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use anyhow::{bail, Context, Result};
use clap::Parser;
use neutron_world::nbt::ussr_nbt::mutf8::MString;
use neutron_world::nbt::ussr_nbt::owned::{Compound, List, Nbt, Tag};
use neutron_world::nbt::{compound_insert, write_nbt};
use neutron_worldgen::generator::SECTIONS_PER_COLUMN;
use neutron_worldgen::{BlockId, ChunkGenerator};
use serde::{Deserialize, Serialize};
use xxhash_rust::xxh3::xxh3_64;

/// Compare Neutron-generated chunks against vanilla golden data.
#[derive(Parser, Debug)]
#[command(name = "parity-check", version, about)]
struct Cli {
    /// Path to golden data JSON file (vanilla).
    #[arg(long)]
    golden: Option<PathBuf>,

    /// World seed.
    #[arg(long)]
    seed: i64,

    /// Output JSON report path (optional).
    #[arg(long)]
    output: Option<PathBuf>,

    /// Generate neutron golden data JSON for later comparison.
    #[arg(long)]
    generate_neutron: Option<PathBuf>,

    /// Chunk radius around spawn to generate (default: 8).
    #[arg(long, default_value_t = 8)]
    radius: i32,

    /// Show detailed stats for first N chunks.
    #[arg(long, default_value_t = 5)]
    detail: usize,
}

/// Golden data JSON format (matching golden-data tool output).
#[derive(Debug, Deserialize)]
struct GoldenData {
    seed: i64,
    server: String,
    version: String,
    #[allow(dead_code)]
    generated_at: String,
    #[allow(dead_code)]
    hash_mode: Option<String>,
    chunks: Vec<GoldenChunk>,
    total_chunks: usize,
}

/// A single chunk entry from golden data.
#[derive(Debug, Deserialize)]
struct GoldenChunk {
    #[allow(dead_code)]
    region_x: i32,
    #[allow(dead_code)]
    region_z: i32,
    chunk_x: i32,
    chunk_z: i32,
    hash: String,
    size_bytes: usize,
}

/// Neutron golden data output format.
#[derive(Serialize)]
struct NeutronGoldenData {
    seed: i64,
    server: String,
    version: String,
    generated_at: String,
    hash_mode: String,
    chunks: Vec<NeutronChunkInfo>,
    total_chunks: usize,
}

/// A chunk entry in neutron golden data.
#[derive(Serialize)]
struct NeutronChunkInfo {
    region_x: i32,
    region_z: i32,
    chunk_x: i32,
    chunk_z: i32,
    hash: String,
    size_bytes: usize,
}

/// Statistics for a single chunk.
#[derive(Serialize)]
struct ChunkStats {
    chunk_x: i32,
    chunk_z: i32,
    raw_hash: String,
    nbt_hash: String,
    block_counts: HashMap<String, usize>,
    total_blocks: usize,
    non_air_blocks: usize,
    heightmap_avg: f64,
    heightmap_min: i16,
    heightmap_max: i16,
    surface_block: String,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Create chunk generator
    let mut generator = ChunkGenerator::new(cli.seed);

    println!("Parity check for seed={}", cli.seed);
    println!("Generating chunks in radius {} around spawn...", cli.radius);

    // Generate chunks and compute stats
    let mut all_stats = Vec::new();
    let mut neutron_chunks = Vec::new();

    for cz in -cli.radius..=cli.radius {
        for cx in -cli.radius..=cli.radius {
            let chunk = generator.generate_chunk(cx, cz);

            // Raw hash of block + biome data
            let raw_hash = hash_raw_chunk(&chunk);

            // NBT-based hash (vanilla-compatible format)
            let nbt_hash = hash_neutron_chunk(&chunk);

            // Block statistics
            let stats = compute_chunk_stats(cx, cz, &chunk, &raw_hash, &nbt_hash);
            all_stats.push(stats);

            // For neutron golden data output
            let region_x = cx.div_euclid(32);
            let region_z = cz.div_euclid(32);
            let nbt_bytes = serialize_chunk_to_nbt_bytes(&chunk);
            neutron_chunks.push(NeutronChunkInfo {
                region_x,
                region_z,
                chunk_x: cx,
                chunk_z: cz,
                hash: nbt_hash.clone(),
                size_bytes: nbt_bytes.len(),
            });
        }
    }

    // Print summary
    println!("\n=== Generated Chunk Statistics ===");
    println!("Total chunks: {}", all_stats.len());

    // Aggregate block counts
    let mut total_block_counts: HashMap<String, usize> = HashMap::new();
    let mut total_blocks = 0;
    let mut total_non_air = 0;
    for s in &all_stats {
        for (block, count) in &s.block_counts {
            *total_block_counts.entry(block.clone()).or_insert(0) += count;
        }
        total_blocks += s.total_blocks;
        total_non_air += s.non_air_blocks;
    }

    println!("\n--- Block Distribution (all chunks) ---");
    let mut sorted_blocks: Vec<_> = total_block_counts.iter().collect();
    sorted_blocks.sort_by(|a, b| b.1.cmp(a.1));
    for (block, count) in sorted_blocks.iter().take(20) {
        let pct = (**count as f64 / total_blocks as f64) * 100.0;
        println!("  {:30} {:>8} ({:.1}%)", block, count, pct);
    }

    println!("\n--- Heightmap Statistics ---");
    let avg_height: f64 =
        all_stats.iter().map(|s| s.heightmap_avg).sum::<f64>() / all_stats.len() as f64;
    let min_height = all_stats.iter().map(|s| s.heightmap_min).min().unwrap_or(0);
    let max_height = all_stats.iter().map(|s| s.heightmap_max).max().unwrap_or(0);
    println!("  Average surface height: {:.1}", avg_height);
    println!("  Min surface height: {}", min_height);
    println!("  Max surface height: {}", max_height);

    // Show detailed stats for first N chunks
    println!("\n--- Sample Chunk Details (first {}) ---", cli.detail);
    for s in all_stats.iter().take(cli.detail) {
        println!("  Chunk ({}, {}):", s.chunk_x, s.chunk_z);
        println!("    Raw hash:  {}", s.raw_hash);
        println!("    NBT hash:  {}", s.nbt_hash);
        println!(
            "    Blocks:    {} total, {} non-air",
            s.total_blocks, s.non_air_blocks
        );
        println!(
            "    Heightmap: avg={:.1} min={} max={}",
            s.heightmap_avg, s.heightmap_min, s.heightmap_max
        );
        println!("    Surface:   {}", s.surface_block);
        // Show top 5 block types
        let mut top_blocks: Vec<_> = s.block_counts.iter().collect();
        top_blocks.sort_by(|a, b| b.1.cmp(a.1));
        for (block, count) in top_blocks.iter().take(5) {
            println!("      {:20} {:>6}", block, count);
        }
    }

    // Compare with golden data if provided
    if let Some(golden_path) = &cli.golden {
        compare_with_golden(&golden_path, &neutron_chunks)?;
    }

    // Generate neutron golden data if requested
    if let Some(output_path) = &cli.generate_neutron {
        let neutron_golden = NeutronGoldenData {
            seed: cli.seed,
            server: "neutron".to_string(),
            version: "26.2".to_string(),
            generated_at: chrono::Utc::now().to_rfc3339(),
            hash_mode: "blocks".to_string(),
            chunks: neutron_chunks,
            total_chunks: all_stats.len(),
        };
        let json = serde_json::to_string_pretty(&neutron_golden)?;
        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(output_path, &json)
            .with_context(|| format!("Failed to write {}", output_path.display()))?;
        println!("\nNeutron golden data saved to {}", output_path.display());
        if let Some(ref golden_path) = cli.golden {
            println!(
                "Compare with: cargo run -p golden-data -- compare --left {} --right {}",
                golden_path.display(),
                output_path.display()
            );
        }
    }

    // Write output report
    if let Some(output_path) = &cli.output {
        let report = serde_json::to_string_pretty(&all_stats)?;
        fs::write(output_path, &report)
            .with_context(|| format!("Failed to write {}", output_path.display()))?;
        println!("\nReport saved to {}", output_path.display());
    }

    Ok(())
}

/// Compare neutron chunks with golden data.
fn compare_with_golden(golden_path: &PathBuf, neutron_chunks: &[NeutronChunkInfo]) -> Result<()> {
    let content = fs::read_to_string(golden_path)
        .with_context(|| format!("Failed to read {}", golden_path.display()))?;
    let golden: GoldenData = serde_json::from_str(&content)?;

    println!("\n=== Comparison with Golden Data ===");
    println!(
        "Golden: {} chunks from {} (seed={})",
        golden.total_chunks, golden.server, golden.seed
    );

    // Build lookup map for golden data
    let golden_map: HashMap<(i32, i32), &GoldenChunk> = golden
        .chunks
        .iter()
        .map(|c| ((c.chunk_x, c.chunk_z), c))
        .collect();

    let mut matching = 0;
    let mut different = 0;
    let mut missing_in_golden = 0;
    let mut only_in_golden = 0;

    // Check neutron chunks against golden
    for nc in neutron_chunks {
        match golden_map.get(&(nc.chunk_x, nc.chunk_z)) {
            Some(gc) => {
                if gc.hash == nc.hash {
                    matching += 1;
                } else {
                    different += 1;
                }
            }
            None => {
                missing_in_golden += 1;
            }
        }
    }

    // Check for golden chunks not in neutron
    for gc in &golden.chunks {
        if !neutron_chunks
            .iter()
            .any(|nc| nc.chunk_x == gc.chunk_x && nc.chunk_z == gc.chunk_z)
        {
            only_in_golden += 1;
        }
    }

    println!("Matching NBT hashes:    {}", matching);
    println!("Different NBT hashes:   {}", different);
    println!("Missing in golden:      {}", missing_in_golden);
    println!("Only in golden:         {}", only_in_golden);

    if different > 0 {
        println!("\nNote: NBT hash differences are expected because the serialization format");
        println!("differs between vanilla and neutron-worldgen. Use 'golden-data compare'");
        println!("for a proper NBT-level comparison after writing compatible .mca files.");
    }

    Ok(())
}

/// Compute statistics for a single chunk.
fn compute_chunk_stats(
    cx: i32,
    cz: i32,
    chunk: &neutron_worldgen::GeneratedChunk,
    raw_hash: &str,
    nbt_hash: &str,
) -> ChunkStats {
    let mut block_counts: HashMap<String, usize> = HashMap::new();
    let mut total_blocks = 0;
    let mut non_air_blocks = 0;

    for &block_id in &chunk.blocks {
        let name = block_id_to_name(block_id);
        *block_counts.entry(name.to_string()).or_insert(0) += 1;
        total_blocks += 1;
        if block_id != BlockId::Air.as_u16() {
            non_air_blocks += 1;
        }
    }

    // Heightmap stats
    let heights: Vec<i16> = chunk.heightmap.clone();
    let heightmap_avg = heights.iter().map(|&h| h as f64).sum::<f64>() / heights.len() as f64;
    let heightmap_min = *heights.iter().min().unwrap_or(&0);
    let heightmap_max = *heights.iter().max().unwrap_or(&0);

    // Find the most common surface block (at heightmap level)
    let mut surface_block_counts: HashMap<String, usize> = HashMap::new();
    for (i, &h) in heights.iter().enumerate() {
        let x = (i % 16) as u32;
        let z = (i / 16) as u32;
        let block = chunk.block_at(x, h as i32, z);
        let name = block_id_to_name(block.as_u16()).to_string();
        *surface_block_counts.entry(name).or_insert(0) += 1;
    }
    let surface_block = surface_block_counts
        .iter()
        .max_by_key(|(_, count)| *count)
        .map(|(name, _)| name.clone())
        .unwrap_or_else(|| "unknown".to_string());

    ChunkStats {
        chunk_x: cx,
        chunk_z: cz,
        raw_hash: raw_hash.to_string(),
        nbt_hash: nbt_hash.to_string(),
        block_counts,
        total_blocks,
        non_air_blocks,
        heightmap_avg,
        heightmap_min,
        heightmap_max,
        surface_block,
    }
}

/// Hash raw block + biome data (for neutron-vs-neutron consistency).
fn hash_raw_chunk(chunk: &neutron_worldgen::GeneratedChunk) -> String {
    let mut data = Vec::new();
    // Serialize blocks as little-endian u16 bytes
    for &block in &chunk.blocks {
        data.extend_from_slice(&block.to_le_bytes());
    }
    // Append biomes
    data.extend_from_slice(&chunk.biomes);
    let hash = xxh3_64(&data);
    format!("{:016x}", hash)
}

/// Hash a neutron-worldgen chunk using NBT serialization (vanilla-compatible format).
fn hash_neutron_chunk(chunk: &neutron_worldgen::GeneratedChunk) -> String {
    let nbt_bytes = serialize_chunk_to_nbt_bytes(chunk);
    let hash = xxh3_64(&nbt_bytes);
    format!("{:016x}", hash)
}

/// Serialize a GeneratedChunk to NBT bytes (sections format for hashing).
fn serialize_chunk_to_nbt_bytes(chunk: &neutron_worldgen::GeneratedChunk) -> Vec<u8> {
    let sections_nbt = serialize_chunk_to_sections_nbt(chunk);

    // Wrap in a compound and serialize (matching golden-data's hash format)
    let mut wrapper = Compound { tags: Vec::new() };
    wrapper
        .tags
        .push((MString::from("sections"), Tag::List(sections_nbt)));

    // Chunk-level fields matching vanilla format.
    // Heightmaps: MOTION_BLOCKING packed 9-bit values.
    let heightmap_data = pack_heightmap(&chunk.heightmap);
    let mut heightmaps = Compound { tags: Vec::new() };
    compound_insert(
        &mut heightmaps,
        "MOTION_BLOCKING",
        Tag::LongArray(heightmap_data.into()),
    );
    compound_insert(&mut wrapper, "Heightmaps", Tag::Compound(heightmaps));

    // Empty block entities list.
    compound_insert(&mut wrapper, "block_entities", Tag::List(List::Empty));

    // DataVersion for Minecraft 26.2.
    compound_insert(&mut wrapper, "DataVersion", Tag::Int(3955));

    let root = Nbt {
        name: MString::new(),
        compound: wrapper,
    };

    write_nbt(&root)
}

/// Serialize a GeneratedChunk into vanilla-compatible NBT sections format.
///
/// Vanilla 26.2 writes ALL 24 sections (including all-air ones) at any
/// generation status >= structure_starts.
fn serialize_chunk_to_sections_nbt(chunk: &neutron_worldgen::GeneratedChunk) -> List {
    let mut sections = Vec::new();

    for section_idx in 0..SECTIONS_PER_COLUMN {
        let y = (section_idx as i32 - 4) as i8;
        let section_start = section_idx * 16 * 16 * 16;
        let section_end = section_start + 16 * 16 * 16;
        let section_blocks = &chunk.blocks[section_start..section_end];

        // Build block palette
        let (block_palette, block_palette_map) = build_block_palette(section_blocks);
        let block_data = pack_block_data(section_blocks, &block_palette_map);

        // Extract biomes for this section.
        // Our storage has 16 entries per section (4x4 grid).
        // Vanilla stores 64 entries per section (4x4x4 grid, by4 * 4 + bz4 * 4 + bx4).
        // Since our generator stores the same biome for all y-levels, expand 16 -> 64.
        let biome_start = section_idx * 16;
        let biome_end = biome_start + 16;
        let section_biomes_16 = &chunk.biomes[biome_start..biome_end];

        // Expand to 64 entries: repeat each of the 16 entries 4 times (for by4=0,1,2,3).
        let section_biomes_64: Vec<u8> = section_biomes_16
            .iter()
            .flat_map(|&b| std::iter::repeat(b).take(4))
            .collect();

        // Build biome palette
        let (biome_palette, biome_palette_map) = build_biome_palette(&section_biomes_64);
        let biome_data = pack_biome_data(&section_biomes_64, &biome_palette_map);

        // Build section compound
        let mut section = Compound { tags: Vec::new() };
        compound_insert(&mut section, "Y", Tag::Byte(y as u8));

        // Block states
        let mut block_states = Compound { tags: Vec::new() };
        block_states
            .tags
            .push((MString::from("palette"), Tag::List(block_palette)));
        // Omit "data" array when palette has only 1 entry (single-value optimization).
        if !block_data.is_empty() {
            block_states
                .tags
                .push((MString::from("data"), Tag::LongArray(block_data.into())));
        }
        compound_insert(&mut section, "block_states", Tag::Compound(block_states));

        // Biomes (vanilla uses LongArray, not IntArray)
        let mut biomes_compound = Compound { tags: Vec::new() };
        biomes_compound
            .tags
            .push((MString::from("palette"), Tag::List(biome_palette)));
        // Omit "data" array when palette has only 1 entry (single-value optimization).
        if !biome_data.is_empty() {
            biomes_compound
                .tags
                .push((MString::from("data"), Tag::LongArray(biome_data.into())));
        }
        compound_insert(&mut section, "biomes", Tag::Compound(biomes_compound));

        sections.push(section);
    }

    List::Compound(sections)
}

/// Build a palette for block IDs in a section.
///
/// Vanilla palette entries include block state Properties when the block has them.
/// For example, `minecraft:grass_block` requires `{"snowy": "false"}`.
fn build_block_palette(blocks: &[u16]) -> (List, HashMap<u16, usize>) {
    let mut seen = Vec::new();
    let mut palette_map = HashMap::new();

    for &block_id in blocks {
        if !palette_map.contains_key(&block_id) {
            palette_map.insert(block_id, seen.len());
            seen.push(block_id);
        }
    }

    let palette_entries: Vec<Compound> = seen
        .iter()
        .map(|&id| {
            let mut compound = Compound { tags: Vec::new() };
            let block_name = block_id_to_name(id);
            compound_insert(
                &mut compound,
                "Name",
                Tag::String(MString::from(block_name)),
            );
            // Add Properties if this block has state properties.
            if let Some(props) = block_id_to_properties(id) {
                let mut properties = Compound { tags: Vec::new() };
                for (key, value) in props {
                    compound_insert(&mut properties, key, Tag::String(MString::from(value)));
                }
                compound_insert(&mut compound, "Properties", Tag::Compound(properties));
            }
            compound
        })
        .collect();

    (List::Compound(palette_entries), palette_map)
}

/// Return block state properties for blocks that have them.
///
/// Vanilla requires ALL properties (including defaults) in the palette entry.
/// Returns `None` if the block has no properties.
fn block_id_to_properties(id: u16) -> Option<Vec<(&'static str, &'static str)>> {
    match BlockId::from_u16(id) {
        Some(BlockId::GrassBlock) => Some(vec![("snowy", "false")]),
        Some(BlockId::Podzol) => Some(vec![("snowy", "false")]),
        Some(BlockId::Sandstone) => Some(vec![("type", "bottom")]),
        Some(BlockId::RedSandstone) => Some(vec![("type", "bottom")]),
        Some(BlockId::OakLog) => Some(vec![("axis", "y")]),
        Some(BlockId::OakLeaves) => Some(vec![
            ("distance", "7"),
            ("persistent", "false"),
            ("waterlogged", "false"),
        ]),
        Some(BlockId::Water) => Some(vec![("level", "0"), ("waterlogged", "false")]),
        Some(BlockId::Lava) => Some(vec![("level", "0"), ("waterlogged", "false")]),
        Some(BlockId::Deepslate) => Some(vec![("axis", "y")]),
        Some(BlockId::Clay) => Some(vec![("level", "0"), ("waterlogged", "false")]),
        Some(BlockId::Ice) => Some(vec![]),
        Some(BlockId::PackedIce) => Some(vec![]),
        Some(BlockId::BlueIce) => Some(vec![]),
        _ => None,
    }
}

/// Build a palette for biome IDs in a section.
fn build_biome_palette(biomes: &[u8]) -> (List, HashMap<u8, usize>) {
    let mut seen = Vec::new();
    let mut palette_map = HashMap::new();

    for &biome_id in biomes {
        if !palette_map.contains_key(&biome_id) {
            palette_map.insert(biome_id, seen.len());
            seen.push(biome_id);
        }
    }

    let palette_entries: Vec<MString> = seen
        .iter()
        .map(|&id| {
            let biome_name = biome_id_to_name(id);
            MString::from(biome_name)
        })
        .collect();

    (List::String(palette_entries), palette_map)
}

/// Pack block data into a long array using vanilla's bit-packing scheme.
fn pack_block_data(blocks: &[u16], palette_map: &HashMap<u16, usize>) -> Vec<i64> {
    let palette_size = palette_map.len();
    if palette_size <= 1 {
        return Vec::new();
    }

    let bits_per_entry = ((palette_size as f64).log2().ceil() as usize).max(4);
    let entries_per_long = 64 / bits_per_entry;
    let total_longs = (4096 + entries_per_long - 1) / entries_per_long;

    let mut data = vec![0i64; total_longs];

    for (i, block) in blocks.iter().enumerate() {
        let index = palette_map[block];
        let long_idx = i / entries_per_long;
        let bit_offset = (i % entries_per_long) * bits_per_entry;
        data[long_idx] |= (index as i64) << bit_offset;
    }

    data
}

/// Pack biome data into a long array using vanilla's bit-packing scheme.
///
/// Vanilla uses LongArray (not IntArray) for biome data in chunk sections.
/// Each section has 64 biome entries (4x4x4 grid).
fn pack_biome_data(biomes: &[u8], palette_map: &HashMap<u8, usize>) -> Vec<i64> {
    let palette_size = palette_map.len();
    if palette_size <= 1 {
        return Vec::new();
    }

    let bits_per_entry = ((palette_size as f64).log2().ceil() as usize).max(4);
    let entries_per_long = 64 / bits_per_entry;
    let total_longs = (biomes.len() + entries_per_long - 1) / entries_per_long;

    let mut data = vec![0i64; total_longs];

    for (i, biome) in biomes.iter().enumerate() {
        let index = palette_map[biome];
        let long_idx = i / entries_per_long;
        let bit_offset = (i % entries_per_long) * bits_per_entry;
        data[long_idx] |= (index as i64) << bit_offset;
    }

    data
}

/// Pack heightmap values into a long array using vanilla's 9-bit packing scheme.
///
/// Vanilla stores `(absoluteY + 1 - minY)` per column (minY = -64 for overworld),
/// i.e. `absolute_solid_y + 65`. 256 values (16×16), 9 bits each, 7 per long.
fn pack_heightmap(heights: &[i16]) -> Vec<i64> {
    const MIN_Y: i32 = -64;
    let bits_per_entry = 9;
    let entries_per_long = 64 / bits_per_entry; // 7
    let total_longs = (heights.len() + entries_per_long - 1) / entries_per_long;

    let mut data = vec![0i64; total_longs];

    for (i, &h) in heights.iter().enumerate() {
        // packed = (absoluteY + 1) - minY
        let value = (h as i32 + 1 - MIN_Y) as u64;
        let long_idx = i / entries_per_long;
        let bit_offset = (i % entries_per_long) * bits_per_entry;
        data[long_idx] |= (value as i64) << bit_offset;
    }

    data
}

/// Convert a BlockId to its Minecraft resource location.
fn block_id_to_name(id: u16) -> &'static str {
    match BlockId::from_u16(id) {
        Some(BlockId::Air) => "minecraft:air",
        Some(BlockId::Stone) => "minecraft:stone",
        Some(BlockId::Granite) => "minecraft:granite",
        Some(BlockId::Diorite) => "minecraft:diorite",
        Some(BlockId::Andesite) => "minecraft:andesite",
        Some(BlockId::Dirt) => "minecraft:dirt",
        Some(BlockId::CoarseDirt) => "minecraft:coarse_dirt",
        Some(BlockId::GrassBlock) => "minecraft:grass_block",
        Some(BlockId::Podzol) => "minecraft:podzol",
        Some(BlockId::Cobblestone) => "minecraft:cobblestone",
        Some(BlockId::Bedrock) => "minecraft:bedrock",
        Some(BlockId::Sand) => "minecraft:sand",
        Some(BlockId::RedSand) => "minecraft:red_sand",
        Some(BlockId::Gravel) => "minecraft:gravel",
        Some(BlockId::GoldOre) => "minecraft:gold_ore",
        Some(BlockId::IronOre) => "minecraft:iron_ore",
        Some(BlockId::CoalOre) => "minecraft:coal_ore",
        Some(BlockId::CopperOre) => "minecraft:copper_ore",
        Some(BlockId::DeepslateIronOre) => "minecraft:deepslate_iron_ore",
        Some(BlockId::DeepslateCoalOre) => "minecraft:deepslate_coal_ore",
        Some(BlockId::DeepslateGoldOre) => "minecraft:deepslate_gold_ore",
        Some(BlockId::DeepslateCopperOre) => "minecraft:deepslate_copper_ore",
        Some(BlockId::DeepslateDiamondOre) => "minecraft:deepslate_diamond_ore",
        Some(BlockId::DeepslateRedstoneOre) => "minecraft:deepslate_redstone_ore",
        Some(BlockId::DeepslateLapisOre) => "minecraft:deepslate_lapis_ore",
        Some(BlockId::DiamondOre) => "minecraft:diamond_ore",
        Some(BlockId::RedstoneOre) => "minecraft:redstone_ore",
        Some(BlockId::LapisOre) => "minecraft:lapis_ore",
        Some(BlockId::RawIronBlock) => "minecraft:raw_iron_block",
        Some(BlockId::RawCopperBlock) => "minecraft:raw_copper_block",
        Some(BlockId::OakLog) => "minecraft:oak_log",
        Some(BlockId::OakLeaves) => "minecraft:oak_leaves",
        Some(BlockId::Water) => "minecraft:water",
        Some(BlockId::Lava) => "minecraft:lava",
        Some(BlockId::Sandstone) => "minecraft:sandstone",
        Some(BlockId::RedSandstone) => "minecraft:red_sandstone",
        Some(BlockId::Ice) => "minecraft:ice",
        Some(BlockId::Snow) => "minecraft:snow_block",
        Some(BlockId::Clay) => "minecraft:clay",
        Some(BlockId::PackedIce) => "minecraft:packed_ice",
        Some(BlockId::PowderSnow) => "minecraft:powder_snow",
        Some(BlockId::BlueIce) => "minecraft:blue_ice",
        Some(BlockId::Terracotta) => "minecraft:terracotta",
        Some(BlockId::WhiteTerracotta) => "minecraft:white_terracotta",
        Some(BlockId::OrangeTerracotta) => "minecraft:orange_terracotta",
        Some(BlockId::BrownTerracotta) => "minecraft:brown_terracotta",
        Some(BlockId::BlackTerracotta) => "minecraft:black_terracotta",
        Some(BlockId::YellowTerracotta) => "minecraft:yellow_terracotta",
        Some(BlockId::RedTerracotta) => "minecraft:red_terracotta",
        Some(BlockId::LightGrayTerracotta) => "minecraft:light_gray_terracotta",
        Some(BlockId::Mud) => "minecraft:mud",
        Some(BlockId::Deepslate) => "minecraft:deepslate",
        Some(BlockId::Tuff) => "minecraft:tuff",
        Some(BlockId::Calcite) => "minecraft:calcite",
        Some(BlockId::Mycelium) => "minecraft:mycelium",
        Some(BlockId::Cinnabar) => "minecraft:cinnabar",
        Some(BlockId::Sulfur) => "minecraft:sulfur",
        None => "minecraft:unknown",
    }
}

/// Convert a BiomeId to its Minecraft resource location.
fn biome_id_to_name(id: u8) -> &'static str {
    match id {
        0 => "minecraft:ocean",
        1 => "minecraft:plains",
        2 => "minecraft:desert",
        3 => "minecraft:forest",
        4 => "minecraft:taiga",
        5 => "minecraft:swamp",
        6 => "minecraft:river",
        7 => "minecraft:beach",
        8 => "minecraft:deep_ocean",
        9 => "minecraft:snowy_plains",
        10 => "minecraft:jungle",
        11 => "minecraft:savanna",
        12 => "minecraft:dark_forest",
        13 => "minecraft:stony_shore",
        14 => "minecraft:meadow",
        15 => "minecraft:frozen_ocean",
        16 => "minecraft:frozen_river",
        17 => "minecraft:ice_spikes",
        18 => "minecraft:old_growth_birch_forest",
        19 => "minecraft:old_growth_pine_forest",
        20 => "minecraft:windswept_hills",
        21 => "minecraft:grove",
        22 => "minecraft:snowy_slopes",
        23 => "minecraft:jagged_peaks",
        24 => "minecraft:frozen_peaks",
        25 => "minecraft:stony_peaks",
        26 => "minecraft:badlands",
        27 => "minecraft:eroded_badlands",
        28 => "minecraft:wooded_badlands",
        29 => "minecraft:mushroom_fields",
        30 => "minecraft:cherry_grove",
        31 => "minecraft:deep_dark",
        32 => "minecraft:mangrove_swamp",
        33 => "minecraft:birch_forest",
        34 => "minecraft:lush_caves",
        35 => "minecraft:dripstone_caves",
        _ => "minecraft:unknown",
    }
}
