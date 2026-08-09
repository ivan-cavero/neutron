use neutron_worldgen::carvers::{
    self, CARVE_CAN_REACH_FAIL, CARVE_ELLIPSOIDS, CARVE_ELLIPSOID_HIT, CARVE_ROOM_CALLS,
    CARVE_STARTS, CARVE_TUNNEL_STEPS, CARVE_WRITES, DIAG_SKIP_CAN_REACH,
};
use neutron_worldgen::surface::BlockId;
use neutron_worldgen::{generator::WORLD_BOTTOM, ChunkGenerator};
use std::sync::atomic::Ordering;

fn reset() {
    CARVE_STARTS.store(0, Ordering::Relaxed);
    CARVE_WRITES.store(0, Ordering::Relaxed);
    CARVE_ELLIPSOIDS.store(0, Ordering::Relaxed);
    CARVE_ELLIPSOID_HIT.store(0, Ordering::Relaxed);
    CARVE_CAN_REACH_FAIL.store(0, Ordering::Relaxed);
    CARVE_ROOM_CALLS.store(0, Ordering::Relaxed);
    CARVE_TUNNEL_STEPS.store(0, Ordering::Relaxed);
}

fn dump(tag: &str) {
    println!(
        "{tag}: starts={} rooms={} tunnel_steps={} ellipsoids={} hits={} can_reach_fail={} writes={}",
        CARVE_STARTS.load(Ordering::Relaxed),
        CARVE_ROOM_CALLS.load(Ordering::Relaxed),
        CARVE_TUNNEL_STEPS.load(Ordering::Relaxed),
        CARVE_ELLIPSOIDS.load(Ordering::Relaxed),
        CARVE_ELLIPSOID_HIT.load(Ordering::Relaxed),
        CARVE_CAN_REACH_FAIL.load(Ordering::Relaxed),
        CARVE_WRITES.load(Ordering::Relaxed),
    );
}

fn main() {
    let gen = ChunkGenerator::new(12345);

    // Baseline (normal canReach)
    DIAG_SKIP_CAN_REACH.store(0, Ordering::Relaxed);
    reset();
    let with = gen.generate_chunk(6, -2);
    dump("NORMAL");
    let mut air = 0u32;
    for y in WORLD_BOTTOM..320 {
        for z in 0..16u32 {
            for x in 0..16u32 {
                if matches!(with.block_at(x, y, z), BlockId::Air) {
                    air += 1;
                }
            }
        }
    }
    println!("air={air}");
    for (x, y, z) in [(0i32, -47, 12), (0, -46, 9), (2, -44, 7), (4, -36, 8)] {
        println!("  ({x},{y},{z})={:?}", with.block_at(x as u32, y, z as u32));
    }

    // Diagnostic: skip canReach
    DIAG_SKIP_CAN_REACH.store(1, Ordering::Relaxed);
    reset();
    let skip = gen.generate_chunk(6, -2);
    dump("SKIP_CAN_REACH");
    let mut air2 = 0u32;
    for y in WORLD_BOTTOM..320 {
        for z in 0..16u32 {
            for x in 0..16u32 {
                if matches!(skip.block_at(x, y, z), BlockId::Air) {
                    air2 += 1;
                }
            }
        }
    }
    println!("air={air2}");
    for (x, y, z) in [(0i32, -47, 12), (0, -46, 9), (2, -44, 7), (4, -36, 8)] {
        println!("  ({x},{y},{z})={:?}", skip.block_at(x as u32, y, z as u32));
    }

    let _ = carvers::CARVERS_ENABLED;
}
