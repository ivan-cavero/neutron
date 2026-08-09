use neutron_worldgen::density::{compute, DF, DFNode, DensityEnv, MarkerKind, MarkerState};
use neutron_worldgen::ChunkGenerator;

fn find_interp_opt(df: &DF) -> Option<DF> {
    match &**df {
        DFNode::Marker(MarkerKind::Interpolated, inner) => Some(inner.clone()),
        _ => {
            for c in df.children() {
                if let Some(x) = find_interp_opt(c) {
                    return Some(x);
                }
            }
            None
        }
    }
}

fn main() {
    let gen = ChunkGenerator::new(12345);
    let st = &gen.state;
    let _ = find_interp_opt;
    let samples = [
        (0i32, -47, 12),
        (0, -46, 9),
        (2, -44, 7),
        (8, -40, 8),
        (4, -36, 8),
        (0, 20, 0),
    ];
    let cx = 6i32;
    let cz = -2i32;
    for (lx, y, lz) in samples {
        let pos_x = cx * 16 + lx;
        let pos_z = cz * 16 + lz;
        let mut env = DensityEnv::new(pos_x, y, pos_z, st.noises.noises());
        let mut marker = MarkerState::new(st.cell_width as usize, st.cell_height as usize);
        env.marker_state = Some(&mut marker);
        let fd = compute(&st.router.final_density, &mut env);
        println!(
            "({pos_x},{y},{pos_z}) final_density={fd:.5} solid={}",
            fd > 0.0
        );
    }
}
