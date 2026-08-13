use neutron_worldgen::{
    density::{compute, DensityEnv},
    WorldgenState,
};
fn main() {
    let st = WorldgenState::overworld(42);
    let coords = [
        (100, 40, 200),
        (-57, 63, 31),
        (12, -40, 300),
        (511, 100, -200),
        (0, 0, 0),
    ];
    for (x, y, z) in coords {
        let mut env = DensityEnv::new(x, y, z, st.noises.noises());
        let t = compute(&st.router.temperature, &mut env);
        let c = compute(&st.router.continents, &mut env);
        let r = compute(&st.router.ridges, &mut env);
        // shift via offset noise directly
        let off = st.noises.get("offset");
        let sx = off.get_value(x as f64 * 0.25, 0.0, z as f64 * 0.25) * 4.0;
        let sz = off.get_value(z as f64 * 0.25, 0.0, x as f64 * 0.25) * 4.0;
        println!("({},{},{}) temp={:.17e} continents={:.17e} ridges={:.17e} shiftX={:.17e} shiftZ={:.17e}", x,y,z,t,c,r,sx,sz);
    }
}
