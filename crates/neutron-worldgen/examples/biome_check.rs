use neutron_worldgen::{ChunkGenerator, biome_source::biome_id};
use neutron_worldgen::surface::BlockId;
use std::collections::HashMap;
fn main() {
    let seed: i64 = std::env::args().nth(1).unwrap_or("42".into()).parse().unwrap();
    let mut gen = ChunkGenerator::new(seed);
    let mut biome_counts: HashMap<u8, usize> = HashMap::new();
    let mut block_counts: HashMap<BlockId, usize> = HashMap::new();
    // Generate chunks around (0,0)
    for cx in -2..=2 { for cz in -2..=2 {
        let chunk = gen.generate_chunk(cx, cz);
        for &b in &chunk.blocks {
            *block_counts.entry(BlockId::from_u16(b).unwrap_or(BlockId::Air)).or_insert(0) += 1;
        }
        // biomes: 16 sections × 16 entries = 256 per chunk
        for &b in &chunk.biomes {
            *biome_counts.entry(b).or_insert(0) += 1;
        }
    }}
    println!("=== Block Distribution (25 chunks) ===");
    let mut blocks: Vec<_> = block_counts.iter().collect();
    blocks.sort_by(|a,b| b.1.cmp(a.1));
    for (b, c) in blocks.iter().take(12) {
        println!("  {:?}: {:.1}%", b, (**c as f64) / (25.0 * 16.0 * 384.0 * 16.0) * 100.0);
    }
    println!("=== Biome Distribution (25 chunks) ===");
    let mut biomes: Vec<_> = biome_counts.iter().collect();
    biomes.sort_by(|a,b| b.1.cmp(a.1));
    for (b, c) in biomes.iter().take(15) {
        println!("  biome {}: {:.1}%", b, (**c as f64) / (25.0 * 256.0) * 100.0);
    }
}
