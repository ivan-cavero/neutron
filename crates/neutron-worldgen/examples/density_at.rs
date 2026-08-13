// Dump neutron INTERPOLATED final density (same path as generator) at coords.
use neutron_worldgen::density::{compute, DFNode, DensityEnv, MarkerKind, MarkerState, DF};
use neutron_worldgen::generator::lerp;
use neutron_worldgen::worldgen::WorldgenState;
use neutron_worldgen::ChunkGenerator;

fn squeeze(v: f64) -> f64 {
    let c = v.clamp(-1.0, 1.0);
    c / 2.0 - c * c * c / 24.0
}

fn find_interp(df: &DF) -> DF {
    match &**df {
        DFNode::Marker(MarkerKind::Interpolated, inner) => inner.clone(),
        _ => {
            for c in df.children() {
                if let Some(x) = find_interp_opt(c) {
                    return x;
                }
            }
            panic!("no interp");
        }
    }
}

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

fn density_at(
    st: &WorldgenState,
    cx: i32,
    cz: i32,
    pos_x: i32,
    pos_y: i32,
    pos_z: i32,
) -> (f64, f64, f64) {
    let (a_part, noodle_part) = match &*st.router.final_density {
        DFNode::Min(a, b) => (a.clone(), b.clone()),
        _ => panic!(),
    };
    let interp_wrapped = find_interp(&a_part);
    let cell_width = st.cell_width;
    let cell_height = st.cell_height;
    let cell_noise_min_y = st.min_y.div_euclid(cell_height);
    let first_cell_x = (cx * 16).div_euclid(cell_width);
    let first_cell_z = (cz * 16).div_euclid(cell_width);
    let cell_x = pos_x.div_euclid(cell_width) - first_cell_x;
    let cell_z = pos_z.div_euclid(cell_width) - first_cell_z;
    let cell_y = pos_y.div_euclid(cell_height) - cell_noise_min_y;
    let mut corners = [[[0f64; 2]; 2]; 2];
    for dy in 0..2i32 {
        for dz in 0..2i32 {
            for dx in 0..2i32 {
                let gx = (first_cell_x + cell_x + dx) * cell_width;
                let gy = (cell_noise_min_y + cell_y + dy) * cell_height;
                let gz = (first_cell_z + cell_z + dz) * cell_width;
                let mut env = DensityEnv::new(gx, gy, gz, st.noises.noises());
                corners[dx as usize][dy as usize][dz as usize] = compute(&interp_wrapped, &mut env);
            }
        }
    }
    let x_in = pos_x.rem_euclid(cell_width);
    let y_in = pos_y.rem_euclid(cell_height);
    let z_in = pos_z.rem_euclid(cell_width);
    let fx = x_in as f64 / cell_width as f64;
    let fy = y_in as f64 / cell_height as f64;
    let fz = z_in as f64 / cell_width as f64;
    let v00 = lerp(fy, corners[0][0][0], corners[0][1][0]);
    let v10 = lerp(fy, corners[1][0][0], corners[1][1][0]);
    let v01 = lerp(fy, corners[0][0][1], corners[0][1][1]);
    let v11 = lerp(fy, corners[1][0][1], corners[1][1][1]);
    let v0 = lerp(fx, v00, v10);
    let v1 = lerp(fx, v01, v11);
    let interp = lerp(fz, v0, v1);
    let a = squeeze(interp);
    let mut marker = MarkerState::new(cell_width as usize, cell_height as usize);
    let mut env = DensityEnv::with_markers(pos_x, pos_y, pos_z, st.noises.noises(), &mut marker);
    let noodle = compute(&noodle_part, &mut env);
    let final_d = a.min(noodle);
    (interp, a, final_d)
}

fn main() {
    let gen = ChunkGenerator::new(12345);
    let st = &gen.state;
    let cx = 6;
    let cz = -2;
    let pts = [
        (96, -47, -20),
        (96, -46, -23),
        (97, -46, -21),
        (98, -45, -22),
        (102, -41, -26),
        (96, -41, -24),
        (103, -40, -25),
        (106, -39, -29),
        (100, 0, -20),
        (100, 200, -20),
        (100, 135, -20),
    ];
    println!("wx,y,wz  squeezed  final  solid?");
    for &(wx, y, wz) in &pts {
        let (_i, a, f) = density_at(st, cx, cz, wx, y, wz);
        println!("{wx:4},{y:4},{wz:4}  {a:.8}  {f:.8}  {}", f > 0.0);
    }
}
