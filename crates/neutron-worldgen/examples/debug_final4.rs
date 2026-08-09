use neutron_worldgen::{WorldgenState, density::{compute, DensityEnv, DensityRegistry}};
use serde_json::Value;
fn main() {
    let st = WorldgenState::overworld(42);
    let mut reg = DensityRegistry::build();
    let sloped = reg.function("overworld/sloped_cheese");
    let entrances = reg.function("overworld/caves/entrances");
    let spaghetti = reg.function("overworld/caves/spaghetti_2d");
    let rough = reg.function("overworld/caves/spaghetti_roughness_function");
    let pillars = reg.function("overworld/caves/pillars");
    let coords = [(100,40,200), (-57,63,31), (511,100,-200)];
    for (x,y,z) in coords {
        let mut env = DensityEnv::new(x, y, z, st.noises.noises());
        println!("({},{},{}) sloped={:.17e} entrances={:.17e} spaghetti2d={:.17e} rough={:.17e} pillars={:.17e}", x, y, z,
            compute(&sloped, &mut env), compute(&entrances, &mut env), compute(&spaghetti, &mut env), compute(&rough, &mut env), compute(&pillars, &mut env));
    }
}
