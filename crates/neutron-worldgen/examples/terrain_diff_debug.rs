//! Compare point-sample vs cell-interpolated density.
//! Shows why block placement differs from density_at output.
//! Usage: terrain_diff_debug [seed] [x] [y0] [y1] [z]
use neutron_worldgen::density::{compute, DensityEnv, MarkerState};
use neutron_worldgen::surface::BlockId;
use neutron_worldgen::ChunkGenerator;

fn lerp(t: f64, a: f64, b: f64) -> f64 {
    a + t * (b - a)
}

fn main() {
    let mut args = std::env::args().skip(1);
    let seed: i64 = args.next().and_then(|s| s.parse().ok()).unwrap_or(424242);
    let wx: i32 = args.next().and_then(|s| s.parse().ok()).unwrap_or(-1);
    let y0: i32 = args.next().and_then(|s| s.parse().ok()).unwrap_or(-50);
    let y1: i32 = args.next().and_then(|s| s.parse().ok()).unwrap_or(-30);
    let wz: i32 = args.next().and_then(|s| s.parse().ok()).unwrap_or(-15);

    let gen = ChunkGenerator::new(seed);
    let st = &gen.state;

    let cx = wx.div_euclid(16);
    let cz = wz.div_euclid(16);
    let (blocks, _heightmap, _biomes) = gen.generate_noise_and_surface(cx, cz);

    let lx = wx - cx * 16;
    let lz = wz - cz * 16;

    let cell_w = st.cell_width;
    let cell_h = st.cell_height;

    println!("y     density_pt  density_cell  block      interp_diff");
    for y in y0..=y1 {
        let idx = ((y - neutron_worldgen::generator::WORLD_BOTTOM) as usize) * 256
            + (lz as usize) * 16
            + lx as usize;
        let block = BlockId::from_u16(blocks[idx]).unwrap_or(BlockId::Air);

        // Point-sample density
        let mut marker = MarkerState::new(cell_w as usize, cell_h as usize, st.reg.cache_slot_count());
        let mut env = DensityEnv::with_markers(wx, y, wz, st.noises.noises(), &mut marker);
        let density_pt = compute(&st.router.final_density, &mut env);

        // Cell-interpolated density (trilinear lerp on 4x8x4 grid)
        let cell_x = wx.div_euclid(cell_w);
        let cell_y = y.div_euclid(cell_h);
        let cell_z = wz.div_euclid(cell_w);
        let fx = wx.rem_euclid(cell_w) as f64 / cell_w as f64;
        let fy = y.rem_euclid(cell_h) as f64 / cell_h as f64;
        let fz = wz.rem_euclid(cell_w) as f64 / cell_w as f64;

        let sample = |dx: i32, dy: i32, dz: i32| -> f64 {
            let gx = (cell_x + dx) * cell_w;
            let gy = (cell_y + dy) * cell_h;
            let gz = (cell_z + dz) * cell_w;
            let mut m = MarkerState::new(cell_w as usize, cell_h as usize, st.reg.cache_slot_count());
            let mut e = DensityEnv::with_markers(gx, gy, gz, st.noises.noises(), &mut m);
            compute(&st.router.final_density, &mut e)
        };

        let n000 = sample(0, 0, 0);
        let n100 = sample(1, 0, 0);
        let n010 = sample(0, 1, 0);
        let n110 = sample(1, 1, 0);
        let n001 = sample(0, 0, 1);
        let n101 = sample(1, 0, 1);
        let n011 = sample(0, 1, 1);
        let n111 = sample(1, 1, 1);

        let v_xz00 = lerp(fy, n000, n010);
        let v_xz10 = lerp(fy, n100, n110);
        let v_xz01 = lerp(fy, n001, n011);
        let v_xz11 = lerp(fy, n101, n111);
        let v_z0 = lerp(fx, v_xz00, v_xz10);
        let v_z1 = lerp(fx, v_xz01, v_xz11);
        let density_cell = lerp(fz, v_z0, v_z1);

        let diff = density_cell - density_pt;
        let sign_diff = if (density_pt > 0.0) != (density_cell > 0.0) {
            " *** SIGN MISMATCH ***"
        } else {
            ""
        };

        println!(
            "{y:4}  {density_pt:+.8}  {density_cell:+.8}  {:?}  {diff:+.8}{sign_diff}",
            block
        );
    }
}
