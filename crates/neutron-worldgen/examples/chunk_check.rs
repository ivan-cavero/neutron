use neutron_worldgen::surface::BlockId;
use neutron_worldgen::{
    generator::{WORLD_BOTTOM, WORLD_TOP},
    ChunkGenerator,
};
fn main() {
    let seed: i64 = std::env::args()
        .nth(1)
        .unwrap_or("12345".into())
        .parse()
        .unwrap();
    let mut gen = ChunkGenerator::new(seed);
    let cx: i32 = std::env::args()
        .nth(2)
        .unwrap_or("0".into())
        .parse()
        .unwrap();
    let cz: i32 = std::env::args()
        .nth(3)
        .unwrap_or("0".into())
        .parse()
        .unwrap();
    let chunk = gen.generate_chunk(cx, cz);
    let mut counts = std::collections::HashMap::new();
    for &b in &chunk.blocks {
        *counts
            .entry(BlockId::from_u16(b).unwrap_or(BlockId::Air))
            .or_insert(0u32) += 1;
    }
    let mut sorted: Vec<_> = counts.into_iter().collect();
    sorted.sort_by(|a, b| b.1.cmp(&a.1));
    println!("chunk ({cx},{cz}) block distribution:");
    for (b, c) in sorted.iter().take(8) {
        println!("  {b:?}: {c}");
    }
    let hs: Vec<i16> = chunk.heightmap.clone();
    let avg = hs.iter().map(|&h| h as f64).sum::<f64>() / hs.len() as f64;
    println!(
        "heightmap: avg={:.1} min={} max={}",
        avg,
        hs.iter().min().unwrap(),
        hs.iter().max().unwrap()
    );
    // sample some heights
    for (x, z) in [(0, 0), (5, 5), (15, 15), (10, 3)] {
        println!("  height at ({x},{z}) = {}", chunk.heightmap[z * 16 + x]);
    }
}
