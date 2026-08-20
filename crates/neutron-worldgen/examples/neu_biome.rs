//! run-058 T1: print Neutron's biome ids at chunk centers for a seed.
use neutron_worldgen::biome_source;
use neutron_worldgen::ChunkGenerator;
fn main() {
    let seed: i64 = std::env::args().nth(1).and_then(|s| s.parse().ok()).unwrap_or(424242);
    let gen = ChunkGenerator::new(seed);
    for (cx, cz) in [(0i32,0i32),(11,11),(5,3),(8,9),(2,2),(7,7),(10,2),(3,9),(4,10),(1,1)] {
        let id = biome_source::biome_id_at_block(&gen.state, cx*16+8, 100, cz*16+8);
        println!("chunk ({cx},{cz}) neutron biome id={id}");
    }
}
