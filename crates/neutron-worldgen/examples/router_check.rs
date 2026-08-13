use neutron_worldgen::density::compute;
use neutron_worldgen::density::DensityEnv;
use neutron_worldgen::WorldgenState;

fn main() {
    let seed: i64 = std::env::args()
        .nth(1)
        .unwrap_or("42".into())
        .parse()
        .unwrap();
    let st = WorldgenState::overworld(seed);
    let names = [
        "barrier",
        "fluid_floodedness",
        "fluid_spread",
        "lava",
        "temperature",
        "vegetation",
        "continents",
        "erosion",
        "depth",
        "ridges",
        "preliminary_surface",
        "final_density",
        "vein_toggle",
        "vein_ridged",
        "vein_gap",
    ];
    let funcs = [
        st.router.barrier.clone(),
        st.router.fluid_level_floodedness.clone(),
        st.router.fluid_level_spread.clone(),
        st.router.lava.clone(),
        st.router.temperature.clone(),
        st.router.vegetation.clone(),
        st.router.continents.clone(),
        st.router.erosion.clone(),
        st.router.depth.clone(),
        st.router.ridges.clone(),
        st.router.preliminary_surface_level.clone(),
        st.router.final_density.clone(),
        st.router.vein_toggle.clone(),
        st.router.vein_ridged.clone(),
        st.router.vein_gap.clone(),
    ];
    let coords = [
        (0, 0, 0),
        (100, 40, 200),
        (-57, 63, 31),
        (12, -40, 300),
        (511, 100, -200),
    ];
    for (x, y, z) in coords {
        println!("-- coord ({},{},{}) --", x, y, z);
        for (i, f) in funcs.iter().enumerate() {
            let mut env = DensityEnv::new(x, y, z, st.noises.noises());
            let v = compute(f, &mut env);
            println!("{}={:.17e}", names[i], v);
        }
    }
}
