use neutron_worldgen::{surface::BlockId, ChunkGenerator};
fn main() {
    let gen = ChunkGenerator::new(12345);
    let chunk = gen.generate_chunk(6, -2);
    // print occupancy as 0/1 for solid (non-air non-fluid) for all positions - dump compact
    // Actually dump surface type counts and deepslate/bedrock counts + solid mask hash
    use std::collections::HashMap;
    let mut c: HashMap<&str, u32> = HashMap::new();
    for y in -64..320 {
        for z in 0..16u32 {
            for x in 0..16u32 {
                let b = chunk.block_at(x, y, z);
                let name = match b {
                    BlockId::Air => "air",
                    BlockId::Stone => "stone",
                    BlockId::Deepslate => "deepslate",
                    BlockId::Dirt => "dirt",
                    BlockId::GrassBlock => "grass_block",
                    BlockId::Bedrock => "bedrock",
                    BlockId::Water => "water",
                    BlockId::Gravel => "gravel",
                    BlockId::Sand => "sand",
                    _ => "other",
                };
                *c.entry(name).or_insert(0) += 1;
            }
        }
    }
    let mut v: Vec<_> = c.into_iter().collect();
    v.sort_by(|a, b| b.1.cmp(&a.1));
    for (n, k) in v {
        println!("{n}: {k}");
    }
}
