// Density at the 9 ref water positions in chunk (0,0), seed 424242
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
        DFNode::Marker(MarkerKind::Interpolated, inner, _) => inner.clone(),
        _ => { for c in df.children() { if let Some(x) = find_interp_opt(c) { return x; } } panic!("no interp"); }
    }
}
fn find_interp_opt(df: &DF) -> Option<DF> {
    match &**df {
        DFNode::Marker(MarkerKind::Interpolated, inner, _) => Some(inner.clone()),
        _ => { for c in df.children() { if let Some(x) = find_interp_opt(c) { return Some(x); } } None }
    }
}
fn density_at(st: &WorldgenState, cx: i32, cz: i32, px: i32, py: i32, pz: i32) -> (f64, f64, f64) {
    let (a_part, noodle_part) = match &*st.router.final_density {
        DFNode::Min(a, b) => (a.clone(), b.clone()),
        _ => panic!(),
    };
    let interp_wrapped = find_interp(&a_part);
    let cw = st.cell_width; let ch = st.cell_height;
    let cell_noise_min_y = st.min_y.div_euclid(ch);
    let first_cell_x = (cx * 16).div_euclid(cw);
    let first_cell_z = (cz * 16).div_euclid(cw);
    let cell_x = px.div_euclid(cw) - first_cell_x;
    let cell_z = pz.div_euclid(cw) - first_cell_z;
    let cell_y = py.div_euclid(ch) - cell_noise_min_y;
    let mut corners = [[[0f64; 2]; 2]; 2];
    for dy in 0..2i32 { for dz in 0..2i32 { for dx in 0..2i32 {
        let gx = (first_cell_x + cell_x + dx) * cw;
        let gy = (cell_noise_min_y + cell_y + dy) * ch;
        let gz = (first_cell_z + cell_z + dz) * cw;
        let mut env = DensityEnv::new(gx, gy, gz, st.noises.noises());
        corners[dx as usize][dy as usize][dz as usize] = compute(&interp_wrapped, &mut env);
    }}}
    let fx = px.rem_euclid(cw) as f64 / cw as f64;
    let fy = py.rem_euclid(ch) as f64 / ch as f64;
    let fz = pz.rem_euclid(cw) as f64 / cw as f64;
    let v00 = lerp(fy, corners[0][0][0], corners[0][1][0]);
    let v10 = lerp(fy, corners[1][0][0], corners[1][1][0]);
    let v01 = lerp(fy, corners[0][0][1], corners[0][1][1]);
    let v11 = lerp(fy, corners[1][0][1], corners[1][1][1]);
    let v0 = lerp(fx, v00, v10);
    let v1 = lerp(fx, v01, v11);
    let interp = lerp(fz, v0, v1);
    let a = squeeze(interp);
    let mut marker = MarkerState::new(cw as usize, ch as usize, st.reg.cache_slot_count());
    let mut env = DensityEnv::with_markers(px, py, pz, st.noises.noises(), &mut marker);
    let noodle = compute(&noodle_part, &mut env);
    (interp, a, a.min(noodle))
}
fn main() {
    let gen = ChunkGenerator::new(424242);
    let st = &gen.state;
    // (chunk_z, x, y, z) — points are the ref's water cells in chunks (0,0) and (0,1)
    let pts = [
        (0, 1, 5, 15), (0, 2, 5, 14), (0, 2, 5, 15), (0, 5, 5, 14), (0, 5, 5, 15),
        (0, 8, 3, 14), (0, 8, 3, 15), (0, 10, 2, 15), (0, 12, 1, 15),
        (1, 0, 5, 17), (1, 1, 5, 17), (1, 0, 5, 18), (1, 1, 6, 19), (1, 0, 6, 20),
        (1, 1, 6, 21), (1, 2, 6, 22), (1, 3, 6, 23), (1, 0, 5, 24), (1, 1, 5, 25),
        (1, 1, 4, 28), (1, 2, 4, 28), (1, 3, 4, 28),
    ];
    println!("  x, y, z   raw_interp  squeezed  final  solid?");
    for &(cz, x, y, z) in &pts {
        let (i, a, f) = density_at(st, 0, cz, x, y, z);
        println!("({x:2},{y:2},{z:2})  {i:+.6}  {a:+.6}  {f:+.6}  {}", f > 0.0);
    }
}
