//! Dump raw depth noise value at specific coordinates.
//! Usage: depth_noise_at [seed] [wx] [y0] [y1] [wz]
use neutron_worldgen::density::{compute, DensityEnv};
use neutron_worldgen::ChunkGenerator;

fn main() {
    let mut args = std::env::args().skip(1);
    let seed: i64 = args.next().and_then(|s| s.parse().ok()).unwrap_or(424242);
    let wx: i32 = args.next().and_then(|s| s.parse().ok()).unwrap_or(4);
    let y0: i32 = args.next().and_then(|s| s.parse().ok()).unwrap_or(56);
    let y1: i32 = args.next().and_then(|s| s.parse().ok()).unwrap_or(104);
    let wz: i32 = args.next().and_then(|s| s.parse().ok()).unwrap_or(4);

    let gen = ChunkGenerator::new(seed);
    let st = &gen.state;

    println!("y     depth       ridges      erosion     surf_level");
    for y in y0..=y1 {
        let mut env = DensityEnv::new(wx, y, wz, st.noises.noises());
        let depth = compute(&st.router.depth, &mut env);
        let mut env2 = DensityEnv::new(wx, y, wz, st.noises.noises());
        let ridges = compute(&st.router.ridges, &mut env2);
        let mut env3 = DensityEnv::new(wx, y, wz, st.noises.noises());
        let erosion = compute(&st.router.erosion, &mut env3);
        let mut env4 = DensityEnv::new(wx, y, wz, st.noises.noises());
        let surf = compute(&st.router.preliminary_surface_level, &mut env4);
        println!(
            "{y:4}  {depth:10.6}  {ridges:10.6}  {erosion:10.6}  {surf:10.6}"
        );
    }
}
