use neutron_worldgen::{WorldgenState, density::{compute, DensityEnv, DensityRegistry}};
use serde_json::Value;
fn main() {
    let st = WorldgenState::overworld(42);
    let mut reg = DensityRegistry::build();
    let json = neutron_worldgen::datapack_data::datapack_json("noise_settings_overworld.json").unwrap();
    let v: Value = serde_json::from_str(json).unwrap();
    let fd = &v["noise_router"]["final_density"];
    // A = argument1 (squeeze chain); descend: squeeze -> interpolated -> mul(0.64, blend_density(slide))
    let a = reg.parse(&fd["argument1"]);
    let interp = reg.parse(&fd["argument1"]["argument"]["argument"]);
    let mul64 = reg.parse(&fd["argument1"]["argument"]["argument"]);
    let slide = reg.parse(&fd["argument1"]["argument"]["argument"]["argument2"]["argument"]);
    let noodle = reg.parse(&fd["argument2"]);
    let coords = [(100,40,200), (-57,63,31), (511,100,-200)];
    for (x,y,z) in coords {
        let mut env = DensityEnv::new(x, y, z, st.noises.noises());
        println!("({},{},{}) A={:.17e} interp={:.17e} slide={:.17e} noodle={:.17e}", x, y, z,
            compute(&a, &mut env), compute(&interp, &mut env), compute(&slide, &mut env), compute(&noodle, &mut env));
    }
}
