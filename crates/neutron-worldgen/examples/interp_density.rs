//! Density with cell interpolation (same as actual generation).
//! Usage: interp_density [seed] [x] [y0] [y1] [z]
use neutron_worldgen::density::{
    compute, interpolated_wrapped, CellInterpRuntime, DensityEnv, MarkerState, DF,
};
use neutron_worldgen::generator::{CHUNK_BLOCK_VOLUME, WORLD_BOTTOM};
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

    // Generate the chunk to get actual blocks
    let cx = wx.div_euclid(16);
    let cz = wz.div_euclid(16);
    let (blocks, _heightmap, _biomes) = gen.generate_noise_and_surface(cx, cz);

    let lx = wx - cx * 16;
    let lz = wz - cz * 16;

    let cell_w = st.cell_width as i32;
    let cell_h = st.cell_height as i32;

    // Sample interpolated markers on cell grid (same as actual generation)
    let cell_x = wx.div_euclid(cell_w);
    let cell_z = wz.div_euclid(cell_w);

    // Collect interpolated markers
    let mut interp_markers: Vec<DF> = Vec::new();
    neutron_worldgen::density::collect_interpolated(&st.router.final_density, &mut interp_markers);

    let stride_xz = 2; // 2x2 grid around the target
    let grid_len = 3 * 3 * stride_xz * stride_xz; // (cell_count_y+1) * stride * stride

    println!("Building cell grid for {} interpolated markers...", interp_markers.len());

    // For each y, build cell grid and compute interpolated density
    println!("y     density_interp  density_pt  block      notes");
    for y in y0..=y1 {
        let idx = ((y - WORLD_BOTTOM) as usize) * 256 + (lz as usize) * 16 + lx as usize;
        let block = BlockId::from_u16(blocks[idx]).unwrap_or(BlockId::Air);

        // Build cell grid for this y (sample at cell corners)
        let cell_y = y.div_euclid(cell_h);
        let fy = y.rem_euclid(cell_h) as f64 / cell_h as f64;
        let fx = wx.rem_euclid(cell_w) as f64 / cell_w as f64;
        let fz = wz.rem_euclid(cell_w) as f64 / cell_w as f64;

        // Sample each interpolated marker at 8 corners of the cell
        let sample_corner = |marker: &DF, dx: i32, dy: i32, dz: i32| -> f64 {
            let gx = (cell_x + dx) * cell_w;
            let gy = (cell_y + dy) * cell_h;
            let gz = (cell_z + dz) * cell_w;
            let wrapped = interpolated_wrapped(marker);
            let mut m = MarkerState::new(cell_w as usize, cell_h as usize, st.reg.cache_slot_count());
            let mut e = DensityEnv::with_markers(gx, gy, gz, st.noises.noises(), &mut m);
            compute(&wrapped, &mut e)
        };

        // Compute interpolated value for the first marker (simplified)
        let mut density_interp = 0.0;
        if let Some(marker) = interp_markers.first() {
            let n000 = sample_corner(marker, 0, 0, 0);
            let n100 = sample_corner(marker, 1, 0, 0);
            let n010 = sample_corner(marker, 0, 1, 0);
            let n110 = sample_corner(marker, 1, 1, 0);
            let n001 = sample_corner(marker, 0, 0, 1);
            let n101 = sample_corner(marker, 1, 0, 1);
            let n011 = sample_corner(marker, 0, 1, 1);
            let n111 = sample_corner(marker, 1, 1, 1);

            let v_xz00 = lerp(fy, n000, n010);
            let v_xz10 = lerp(fy, n100, n110);
            let v_xz01 = lerp(fy, n001, n011);
            let v_xz11 = lerp(fy, n101, n111);
            let v_z0 = lerp(fx, v_xz00, v_xz10);
            let v_z1 = lerp(fx, v_xz01, v_xz11);
            density_interp = lerp(fz, v_z0, v_z1);
        }

        // Point-sample density (like density_at)
        let mut marker_pt = MarkerState::new(cell_w as usize, cell_h as usize, st.reg.cache_slot_count());
        let mut env_pt = DensityEnv::with_markers(wx, y, wz, st.noises.noises(), &mut marker_pt);
        let density_pt = compute(&st.router.final_density, &mut env_pt);

        let notes = if block == BlockId::Air && density_interp > 0.0 {
            "AIR BUT INTERP>0"
        } else if block != BlockId::Air && density_interp <= 0.0 {
            "SOLID BUT INTERP<=0"
        } else {
            ""
        };

        println!(
            "{y:4}  {density_interp:+.8}  {density_pt:+.8}  {:?}  {notes}",
            block
        );
    }
}
