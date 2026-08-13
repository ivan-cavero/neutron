use neutron_worldgen::{
    density::{compute, DensityEnv, DensityRegistry},
    WorldgenState,
};
use serde_json::Value;
fn main() {
    let st = WorldgenState::overworld(42);
    let mut reg = DensityRegistry::build();
    // Parse final_density's min arguments directly
    let json =
        neutron_worldgen::datapack_data::datapack_json("noise_settings_overworld.json").unwrap();
    let v: Value = serde_json::from_str(json).unwrap();
    let fd = &v["noise_router"]["final_density"];
    let arg1 = reg.parse(&fd["argument1"]); // postProcess part
    let arg2 = reg.parse(&fd["argument2"]); // noodle
    let coords = [(0, 0, 0), (100, 40, 200), (-57, 63, 31), (511, 100, -200)];
    for (x, y, z) in coords {
        let mut env = DensityEnv::new(x, y, z, st.noises.noises());
        let a = compute(&arg1, &mut env);
        let n = compute(&arg2, &mut env);
        let full = compute(&st.router.final_density, &mut env);
        println!(
            "({},{},{}) A={:.17e} noodle={:.17e} final={:.17e}",
            x, y, z, a, n, full
        );
    }
}
