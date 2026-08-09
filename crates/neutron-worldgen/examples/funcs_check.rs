use neutron_worldgen::{WorldgenState, density::{compute, DensityEnv}};
use std::collections::HashMap;
fn main() {
    let mut reg = neutron_worldgen::density::DensityRegistry::build();
    let keys = ["overworld/offset", "overworld/factor", "overworld/jaggedness", "overworld/sloped_cheese", "overworld/depth", "overworld/caves/entrances", "overworld/caves/noodle", "overworld/caves/spaghetti_2d"];
    let mut funcs: Vec<(&str, _)> = keys.iter().map(|k| (*k, reg.function(k))).collect();
    let st = WorldgenState::overworld(42);
    let coords = [(0,0,0), (100,40,200), (-57,63,31), (12,-40,300)];
    for (x,y,z) in coords {
        println!("-- ({},{},{}) --", x, y, z);
        let mut env = DensityEnv::new(x, y, z, st.noises.noises());
        for (k, f) in &funcs {
            println!("{}={:.17e}", k, compute(f, &mut env));
        }
    }
}
