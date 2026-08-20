use neutron_worldgen::generator::WORLD_BOTTOM;
use neutron_worldgen::region_buf::RegionBuf;
use neutron_worldgen::surface::BlockId;
// We can't call private carve_ellipsoid. Use apply and check (0,0) which had 1841 writes.
use neutron_worldgen::carvers::{apply_carvers_region, CARVE_WRITES};
use std::sync::atomic::Ordering;

use neutron_worldgen::worldgen::WorldgenState;
use neutron_worldgen::ChunkGenerator;
fn test_target(tcx: i32, tcz: i32) -> u32 {
    let mut region = RegionBuf::new(tcx, tcz, 0);
    for y in WORLD_BOTTOM..320 {
        for z in 0..16 {
            for x in 0..16 {
                region.set(tcx * 16 + x, y, tcz * 16 + z, BlockId::Stone);
            }
        }
    }
    CARVE_WRITES.store(0, Ordering::Relaxed);
    let gen = ChunkGenerator::new(12345);
    let st: &WorldgenState = &gen.state;
    apply_carvers_region(&mut region, st);
    CARVE_WRITES.load(Ordering::Relaxed)
}

fn main() {
    for (cx, cz) in [(6, -2), (6, -5), (3, -2), (4, -1), (0, 0), (6, -3), (5, -2)] {
        let w = test_target(cx, cz);
        println!("target({cx},{cz}) writes={w}");
    }
}
