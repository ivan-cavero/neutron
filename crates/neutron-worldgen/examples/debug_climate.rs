use neutron_worldgen::{
    biome_source::{climate_at_block, find_biome, ClimateTarget},
    density::DensityEnv,
    WorldgenState,
};
fn main() {
    let st = WorldgenState::overworld(42);
    for (x, y, z) in [
        (0, 0, 0),
        (100, 40, 200),
        (500, 0, 0),
        (16, 63, 16),
        (-57, 63, 31),
    ] {
        let mut env = DensityEnv::new(x, y, z, st.noises.noises());
        let ct = climate_at_block(
            &mut env,
            &st.router.temperature,
            &st.router.vegetation,
            &st.router.continents,
            &st.router.erosion,
            &st.router.depth,
            &st.router.ridges,
        );
        let biome = find_biome(&ct);
        println!(
            "({},{},{}) T={} H={} C={} E={} D={} W={} → biome={}",
            x,
            y,
            z,
            ct.temperature,
            ct.humidity,
            ct.continentalness,
            ct.erosion,
            ct.depth,
            ct.weirdness,
            biome
        );
    }
}
