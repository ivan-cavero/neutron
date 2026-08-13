use neutron_worldgen::{biome_source, density::DensityEnv, ChunkGenerator};
fn main() {
    let gen = ChunkGenerator::new(12345);
    let st = &gen.state;
    for (x, z) in [(96i32, -32), (100, -27), (111, -17)] {
        let mut env = DensityEnv::new(x, 64, z, st.noises.noises());
        let c = biome_source::climate_at_block(
            &mut env,
            &st.router.temperature,
            &st.router.vegetation,
            &st.router.continents,
            &st.router.erosion,
            &st.router.depth,
            &st.router.ridges,
        );
        let id = biome_source::find_biome(&c);
        println!(
            "pos ({x},64,{z}) temp={} hum={} cont={} ero={} depth={} weird={} -> biome {}",
            c.temperature, c.humidity, c.continentalness, c.erosion, c.depth, c.weirdness, id
        );
    }
    // also check biomes array in generated chunk
    let chunk = gen.generate_chunk(6, -2);
    let mut counts = std::collections::HashMap::new();
    for &b in &chunk.biomes {
        *counts.entry(b).or_insert(0u32) += 1;
    }
    println!("biome array counts: {:?}", counts);
}
