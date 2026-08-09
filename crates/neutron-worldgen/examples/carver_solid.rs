use neutron_worldgen::carvers::{self, CARVE_STARTS, CARVE_WRITES, CARVE_ROOM_CALLS, CARVE_ELLIPSOIDS, CARVE_ELLIPSOID_HIT};
use neutron_worldgen::region_buf::RegionBuf;
use neutron_worldgen::surface::BlockId;
use neutron_worldgen::generator::WORLD_BOTTOM;
use std::sync::atomic::Ordering;

fn main() {
    let cx=6i32; let cz=-2i32;
    let mut region = RegionBuf::new(cx, cz, 0);
    println!("origin=({},{}) side={} chunks={}", region.origin_x, region.origin_z, region.side, region.chunks);
    for y in WORLD_BOTTOM..320 {
        for z in 0..16 {
            for x in 0..16 {
                region.set(cx*16+x, y, cz*16+z, BlockId::Stone);
            }
        }
    }
    // verify set worked
    println!("sample stone={:?}", region.get(cx*16+8, 0, cz*16+8));
    CARVE_STARTS.store(0, Ordering::Relaxed);
    CARVE_WRITES.store(0, Ordering::Relaxed);
    CARVE_ROOM_CALLS.store(0, Ordering::Relaxed);
    CARVE_ELLIPSOIDS.store(0, Ordering::Relaxed);
    CARVE_ELLIPSOID_HIT.store(0, Ordering::Relaxed);
    carvers::apply_carvers_region(&mut region, 12345);
    println!("starts={} rooms={} ellipsoids={} hits={} writes={}",
        CARVE_STARTS.load(Ordering::Relaxed),
        CARVE_ROOM_CALLS.load(Ordering::Relaxed),
        CARVE_ELLIPSOIDS.load(Ordering::Relaxed),
        CARVE_ELLIPSOID_HIT.load(Ordering::Relaxed),
        CARVE_WRITES.load(Ordering::Relaxed));
    println!("center after={:?}", region.get(cx*16+8, 20, cz*16+8));
    // count air
    let mut air=0u32;
    for y in WORLD_BOTTOM..320 {
        for z in 0..16 {
            for x in 0..16 {
                let b = region.get(cx*16+x,y,cz*16+z);
                if matches!(b, BlockId::Air|BlockId::Lava) { air+=1; }
            }
        }
    }
    println!("air={air}");
}
