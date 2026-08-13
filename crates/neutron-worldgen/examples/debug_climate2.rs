use neutron_worldgen::{
    density::{compute, DensityEnv},
    WorldgenState,
};
fn main() {
    let st = WorldgenState::overworld(42);
    for (x, y, z) in [(0, 0, 0), (100, 40, 200), (500, 0, 0)] {
        let mut env = DensityEnv::new(x, y, z, st.noises.noises());
        let t = compute(&st.router.temperature, &mut env);
        let v = compute(&st.router.vegetation, &mut env);
        let c = compute(&st.router.continents, &mut env);
        let e = compute(&st.router.erosion, &mut env);
        let d = compute(&st.router.depth, &mut env);
        let r = compute(&st.router.ridges, &mut env);
        println!(
            "({},{},{}) T={:.6} V={:.6} C={:.6} E={:.6} D={:.6} R={:.6}",
            x, y, z, t, v, c, e, d, r
        );
    }
}
