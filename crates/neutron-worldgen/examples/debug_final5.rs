use neutron_worldgen::{WorldgenState, density::{compute, DensityEnv, DensityRegistry}};
fn main() {
    let st = WorldgenState::overworld(42);
    let mut reg = DensityRegistry::build();
    let factor = reg.function("overworld/factor");
    let jaggedness = reg.function("overworld/jaggedness");
    let coords = [(100,40,200), (-57,63,31), (511,100,-200), (0,0,0)];
    for (x,y,z) in coords {
        let mut env = DensityEnv::new(x, y, z, st.noises.noises());
        let d = compute(&st.router.depth, &mut env);
        let f = compute(&factor, &mut env);
        let j = compute(&jaggedness, &mut env);
        let jag_noise = st.noises.get("jagged").get_value(x as f64 * 1500.0, y as f64 * 0.0, z as f64 * 1500.0);
        let hn = if jag_noise > 0.0 { jag_noise } else { jag_noise * 0.5 };
        let chain = j * hn;
        let x_val = (d + chain) * f;
        println!("({},{},{}) depth={:.17e} factor={:.17e} jaggedness={:.17e} jagNoise={:.17e} chain={:.17e} qnInner={:.17e}", x, y, z, d, f, j, jag_noise, chain, x_val);
    }
}
