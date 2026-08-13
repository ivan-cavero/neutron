use neutron_worldgen::{biome_source, density::DensityEnv, ChunkGenerator};
fn main() {
    let gen = ChunkGenerator::new(12345);
    let st = &gen.state;
    let x = 96i32;
    let z = -32i32;
    for y in [-50, 0, 32, 64, 100, 135, 200] {
        let mut env = DensityEnv::new(x, y, z, st.noises.noises());
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
            "Y={y:4} depth_q={} cont={} temp={} hum={} ero={} weird={} biome={}",
            c.depth, c.continentalness, c.temperature, c.humidity, c.erosion, c.weirdness, id
        );
    }
}
