use neutron_worldgen::{WorldgenState, density::{compute, DensityEnv, DensityRegistry}};
fn main() {
    let st = WorldgenState::overworld(42);
    let mut reg = DensityRegistry::build();
    let jaggedness = reg.function("overworld/jaggedness");
    let factor = reg.function("overworld/factor");
    let sloped = reg.function("overworld/sloped_cheese");
    let noodle = reg.function("overworld/caves/noodle");
    let (x, y, z) = (0i32, 0i32, 0i32);
    let mut env = DensityEnv::new(x, y, z, st.noises.noises());
    let j = compute(&jaggedness, &mut env);
    let f = compute(&factor, &mut env);
    let d = compute(&st.router.depth, &mut env);
    let s = compute(&sloped, &mut env);
    let n = compute(&noodle, &mut env);
    println!("depth={:.17e} factor={:.17e} jaggedness={:.17e} sloped_cheese={:.17e} noodle={:.17e}", d, f, j, s, n);
    // base_3d at 0,0,0
    let bn = st.blended_noise();
    println!("base_3d={:.17e}", bn.compute(0, 0, 0));
}
