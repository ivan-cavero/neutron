use neutron_worldgen::density::{compute, DF, DFNode, DensityEnv, MarkerKind, MarkerState};
use neutron_worldgen::generator::lerp;
use neutron_worldgen::surface::BlockId;
use neutron_worldgen::ChunkGenerator;

fn find_interp(df: &DF) -> DF {
    match &**df {
        DFNode::Marker(MarkerKind::Interpolated, inner) => inner.clone(),
        _ => {
            for c in df.children() {
                if let Some(x) = find_interp_opt(c) { return x; }
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
                if let Some(x) = find_interp_opt(c) { return Some(x); }
            }
            None
        }
    }
}
fn squeeze(v: f64) -> f64 {
    let c = v.clamp(-1.0, 1.0);
    c / 2.0 - c * c * c / 24.0
}

fn main() {
    let gen = ChunkGenerator::new(12345);
    let st = &gen.state;
    let chunk = gen.generate_chunk(6, -2);
    let cx = 6i32; let cz = -2i32;
    let (a_part, noodle_part) = match &*st.router.final_density {
        DFNode::Min(a, b) => (a.clone(), b.clone()),
        _ => panic!(),
    };
    let interp_wrapped = find_interp(&a_part);
    let cell_width = st.cell_width;
    let cell_height = st.cell_height;
    let cell_count_xz = 16 / cell_width;
    let cell_count_y = st.height / cell_height;
    let cell_noise_min_y = st.min_y.div_euclid(cell_height);
    let first_cell_x = (cx * 16).div_euclid(cell_width);
    let first_cell_z = (cz * 16).div_euclid(cell_width);
    let stride_xz = (cell_count_xz + 1) as usize;
    let mut samples_g = vec![0f64; stride_xz * (cell_count_y as usize + 1) * stride_xz];
    for iy in 0..=cell_count_y {
        let grid_y = (cell_noise_min_y + iy as i32) * cell_height;
        for iz in 0..=cell_count_xz {
            let grid_z = (first_cell_z + iz as i32) * cell_width;
            for ix in 0..=cell_count_xz {
                let grid_x = (first_cell_x + ix as i32) * cell_width;
                let mut env = DensityEnv::new(grid_x, grid_y, grid_z, st.noises.noises());
                let v = compute(&interp_wrapped, &mut env);
                let si = (iy as usize * stride_xz + iz as usize) * stride_xz + ix as usize;
                samples_g[si] = v;
            }
        }
    }

    // Vanilla base OPEN coords
    let pts = [
        (102, -41, -26), (96, -41, -24), (103, -40, -25), (103, -39, -28),
        (108, -38, -30), (98, -38, -24),
    ];
    for (pos_x, y, pos_z) in pts {
        let lx = pos_x - cx * 16;
        let lz = pos_z - cz * 16;
        let block = if (0..16).contains(&lx) && (0..16).contains(&lz) {
            format!("{:?}", chunk.block_at(lx as u32, y, lz as u32))
        } else {
            "out_of_chunk".into()
        };

        let cell_x = pos_x.div_euclid(cell_width) - first_cell_x;
        let cell_z = pos_z.div_euclid(cell_width) - first_cell_z;
        let cell_y = y.div_euclid(cell_height) - cell_noise_min_y;
        let fx = pos_x.rem_euclid(cell_width) as f64 / cell_width as f64;
        let fy = y.rem_euclid(cell_height) as f64 / cell_height as f64;
        let fz = pos_z.rem_euclid(cell_width) as f64 / cell_width as f64;
        let idx = |cix: i32, ciy: i32, ciz: i32| -> f64 {
            samples_g[(ciy as usize * stride_xz + ciz as usize) * stride_xz + cix as usize]
        };
        // generator order: Y then X then Z
        let n000 = idx(cell_x, cell_y, cell_z);
        let n100 = idx(cell_x + 1, cell_y, cell_z);
        let n010 = idx(cell_x, cell_y + 1, cell_z);
        let n110 = idx(cell_x + 1, cell_y + 1, cell_z);
        let n001 = idx(cell_x, cell_y, cell_z + 1);
        let n101 = idx(cell_x + 1, cell_y, cell_z + 1);
        let n011 = idx(cell_x, cell_y + 1, cell_z + 1);
        let n111 = idx(cell_x + 1, cell_y + 1, cell_z + 1);
        let v_xz00 = lerp(fy, n000, n010);
        let v_xz10 = lerp(fy, n100, n110);
        let v_xz01 = lerp(fy, n001, n011);
        let v_xz11 = lerp(fy, n101, n111);
        let v_z0 = lerp(fx, v_xz00, v_xz10);
        let v_z1 = lerp(fx, v_xz01, v_xz11);
        let interpolated = lerp(fz, v_z0, v_z1);
        let a_value = squeeze(interpolated);
        let mut marker = MarkerState::new(cell_width as usize, cell_height as usize);
        let mut nenv = DensityEnv::with_markers(pos_x, y, pos_z, st.noises.noises(), &mut marker);
        let noodle_v = compute(&noodle_part, &mut nenv);
        let gen_fd = a_value.min(noodle_v);

        // point sample of interp inner (no squeeze)
        let mut env2 = DensityEnv::new(pos_x, y, pos_z, st.noises.noises());
        let point_inner = compute(&interp_wrapped, &mut env2);
        let point_a = squeeze(point_inner);

        println!(
            "({pos_x},{y},{pos_z}) block={block} gen_fd={gen_fd:.6} a={a_value:.6} noodle={noodle_v:.6} point_a={point_a:.6} corners=[{n000:.4},{n100:.4}/{n010:.4},{n110:.4} | {n001:.4},{n101:.4}/{n011:.4},{n111:.4}] f=({fx:.2},{fy:.2},{fz:.2})"
        );
    }
}
