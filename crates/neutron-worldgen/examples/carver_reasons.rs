use neutron_worldgen::carvers::*;
use neutron_worldgen::region_buf::RegionBuf;
use neutron_worldgen::surface::BlockId;
use neutron_worldgen::generator::WORLD_BOTTOM;
use std::sync::atomic::Ordering;
fn main() {
    let tcx=6i32; let tcz=-2i32;
    let mut region = RegionBuf::new(tcx, tcz, 0);
    for y in WORLD_BOTTOM..320 {
        for z in 0..16 {
            for x in 0..16 {
                region.set(tcx*16+x,y,tcz*16+z, BlockId::Stone);
            }
        }
    }
    for a in [&CARVE_STARTS,&CARVE_WRITES,&CARVE_ELLIPSOIDS,&CARVE_ELLIPSOID_HIT,&CARVE_EARLY_OUT,&CARVE_EMPTY_RANGE,&CARVE_CAN_REACH_FAIL,&CARVE_ROOM_CALLS,&CARVE_TUNNEL_STEPS] {
        a.store(0, Ordering::Relaxed);
    }
    apply_carvers_region(&mut region, 12345);
    println!("starts={} rooms={} steps={} ellipsoids={} early_out={} empty_range={} hits={} writes={} reach_fail={}",
        CARVE_STARTS.load(Ordering::Relaxed),
        CARVE_ROOM_CALLS.load(Ordering::Relaxed),
        CARVE_TUNNEL_STEPS.load(Ordering::Relaxed),
        CARVE_ELLIPSOIDS.load(Ordering::Relaxed),
        CARVE_EARLY_OUT.load(Ordering::Relaxed),
        CARVE_EMPTY_RANGE.load(Ordering::Relaxed),
        CARVE_ELLIPSOID_HIT.load(Ordering::Relaxed),
        CARVE_WRITES.load(Ordering::Relaxed),
        CARVE_CAN_REACH_FAIL.load(Ordering::Relaxed));
}
