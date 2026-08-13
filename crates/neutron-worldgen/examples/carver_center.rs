//! Apply carvers only for the center target; report write Y histogram and hits.
use neutron_worldgen::carvers::{
    self, CARVE_CAN_REACH_FAIL, CARVE_ELLIPSOIDS, CARVE_ELLIPSOID_HIT, CARVE_ROOM_CALLS,
    CARVE_STARTS, CARVE_TUNNEL_STEPS, CARVE_WRITES,
};
use neutron_worldgen::region_buf::RegionBuf;
use neutron_worldgen::surface::BlockId;
use neutron_worldgen::{generator::WORLD_BOTTOM, ChunkGenerator};
use std::sync::atomic::Ordering;

fn main() {
    let gen = ChunkGenerator::new(12345);
    let cx = 6i32;
    let cz = -2i32;

    // Build 1-chunk region (only center) via full gen noise path: generate neighbors
    // but apply carvers only on center by hand.
    let mut region = RegionBuf::new(cx, cz, 0); // single chunk
                                                // Use generate_chunk internals: generate noise+surface for center only via full chunk
                                                // hack: generate_chunk includes carvers; instead put pre-carve by...
                                                // Easiest: generate with CARVERS off temporarily — can't.
                                                // Use generate_chunk of neighbors? Region radius 0 = 1 chunk.
                                                // We'll call generate_noise by generating chunk then undoing? No.
                                                //
                                                // Approach: generate full chunk (with carvers), but separately count by
                                                // re-running apply on a fresh noise-only buffer.
                                                //
                                                // Public API lacks noise-only. Reconstruct: generate_chunk with carvers,
                                                // then compare air before/after by regenerating region manually.
                                                //
                                                // Actually: use FeatureRadius region like generator, fill via generate_chunk
                                                // of each cell WITHOUT double carvers by extracting from a side channel.
                                                //
                                                // Simplest path: temporarily rely on generate_chunk's 3x3 and just print
                                                // Y histogram of air cells that are "likely carved" — deep air surrounded
                                                // by solid.
                                                //
                                                // Better: export apply for one target. For now, put noise chunks by
                                                // generating each chunk and stripping — can't strip carves.
                                                //
                                                // HACK: set CARVERS_ENABLED is const true. Use region radius 0 by
                                                // calling the same public apply after filling with a generator helper.
                                                //
                                                // Fill region by taking blocks from generate_chunk of surrounding — those
                                                // already include carvers. Not clean.
                                                //
                                                // Direct: duplicate generate path from example using ChunkGenerator fields.
                                                // ChunkGenerator::generate_chunk is all we have.
                                                //
                                                // Measure: air count in deep Y bands after full generate.
    let ch = gen.generate_chunk(cx, cz);
    let mut y_hist = [0u32; 24]; // 16-high bands from -64
    let mut deep_air = 0u32;
    for y in WORLD_BOTTOM..320 {
        for z in 0..16u32 {
            for x in 0..16u32 {
                if matches!(ch.block_at(x, y, z), BlockId::Air) {
                    let band = ((y - WORLD_BOTTOM) / 16) as usize;
                    if band < 24 {
                        y_hist[band] += 1;
                    }
                    if y < 0 {
                        deep_air += 1;
                    }
                }
            }
        }
    }
    println!("deep_air(y<0)={deep_air}");
    for (i, c) in y_hist.iter().enumerate() {
        if *c > 0 {
            let y0 = WORLD_BOTTOM + (i as i32) * 16;
            println!("  Y[{y0}..{}] air={c}", y0 + 16);
        }
    }

    // Re-run with fresh counters on full generate
    CARVE_STARTS.store(0, Ordering::Relaxed);
    CARVE_WRITES.store(0, Ordering::Relaxed);
    CARVE_ELLIPSOIDS.store(0, Ordering::Relaxed);
    CARVE_ELLIPSOID_HIT.store(0, Ordering::Relaxed);
    CARVE_CAN_REACH_FAIL.store(0, Ordering::Relaxed);
    CARVE_ROOM_CALLS.store(0, Ordering::Relaxed);
    CARVE_TUNNEL_STEPS.store(0, Ordering::Relaxed);
    let _ = gen.generate_chunk(cx, cz);
    println!(
        "starts={} rooms={} steps={} ellipsoids={} hits={} fails={} writes={}",
        CARVE_STARTS.load(Ordering::Relaxed),
        CARVE_ROOM_CALLS.load(Ordering::Relaxed),
        CARVE_TUNNEL_STEPS.load(Ordering::Relaxed),
        CARVE_ELLIPSOIDS.load(Ordering::Relaxed),
        CARVE_ELLIPSOID_HIT.load(Ordering::Relaxed),
        CARVE_CAN_REACH_FAIL.load(Ordering::Relaxed),
        CARVE_WRITES.load(Ordering::Relaxed),
    );

    // Force a big room at the sculk gap center to see if writes land
    let mut region = RegionBuf::new(cx, cz, 0);
    // fill solid
    for y in WORLD_BOTTOM..320 {
        for z in 0..16 {
            for x in 0..16 {
                region.set(cx * 16 + x, y, cz * 16 + z, BlockId::Deepslate);
            }
        }
    }
    CARVE_WRITES.store(0, Ordering::Relaxed);
    // manual ellipsoid at gap
    // use internal via create by applying carvers on solid - only carves if replaceable
    carvers::apply_carvers_region(&mut region, 12345);
    let w = CARVE_WRITES.load(Ordering::Relaxed);
    let mut air = 0u32;
    for y in WORLD_BOTTOM..320 {
        for z in 0..16 {
            for x in 0..16 {
                if matches!(
                    region.get(cx * 16 + x, y, cz * 16 + z),
                    BlockId::Air | BlockId::Lava
                ) {
                    air += 1;
                }
            }
        }
    }
    println!("solid-only region carves: writes={w} air/lava={air}");
    // check samples
    for (x, y, z) in [(0i32, -47, 12), (0, -46, 9), (8, 20, 8)] {
        println!(
            "  ({x},{y},{z})={:?}",
            region.get(cx * 16 + x, y, cz * 16 + z)
        );
    }
}
