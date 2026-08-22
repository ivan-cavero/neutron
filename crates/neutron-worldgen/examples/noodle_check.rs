// Neutron noodle function value at the ref's water cells (seed 424242).
// Vanilla (ProbeNoodle.java) gives noodle = -0.075000 (raw) at all 22 cells.
use neutron_worldgen::density::{compute, DFNode, DensityEnv, MarkerKind, MarkerState, DF};
use neutron_worldgen::ChunkGenerator;

fn main() {
    let gen = ChunkGenerator::new(424242);
    let st = &gen.state;
    let (a_part, noodle_part) = match &*st.router.final_density {
        DFNode::Min(a, b) => (a.clone(), b.clone()),
        _ => panic!(),
    };
    let pts = [(1, 5, 15), (12, 1, 15), (0, 5, 17), (0, 5, 18), (1, 6, 21), (3, 6, 23)];
    for &(x, y, z) in &pts {
        let mut marker = MarkerState::new(st.cell_width as usize, st.cell_height as usize, st.reg.cache_slot_count());
        let mut env = DensityEnv::with_markers(x, y, z, st.noises.noises(), &mut marker);
        let noodle = compute(&noodle_part, &mut env);
        let mut env2 = DensityEnv::new(x, y, z, st.noises.noises());
        let noodle_raw = compute(&noodle_part, &mut env2);
        println!("({x},{y},{z}) noodle_interp={noodle:+.6} noodle_raw={noodle_raw:+.6}");
    }
}
