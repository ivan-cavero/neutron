use neutron_worldgen::sculk::{SCULK_BIOME_OK, SCULK_PLACED, SCULK_SPREAD_OK, SCULK_TRIES};
use neutron_worldgen::{generator::WORLD_BOTTOM, surface::BlockId, ChunkGenerator};
use std::sync::atomic::Ordering;
fn main() {
    for a in [
        &SCULK_TRIES,
        &SCULK_BIOME_OK,
        &SCULK_SPREAD_OK,
        &SCULK_PLACED,
    ] {
        a.store(0, Ordering::Relaxed);
    }
    let g = ChunkGenerator::new(12345);
    let mut total = 0u32;
    let mut per = vec![];
    for cz in -3..=-1 {
        for cx in 5..=7 {
            let ch = g.generate_chunk(cx, cz);
            // NOTE: each generate rebuilds its own 3x3 - not shared. Count only center of each call.
            let mut sc = 0u32;
            for y in WORLD_BOTTOM..320 {
                for z in 0..16u32 {
                    for x in 0..16u32 {
                        if matches!(
                            ch.block_at(x, y, z),
                            BlockId::Sculk | BlockId::SculkCatalyst | BlockId::SculkVein
                        ) {
                            sc += 1;
                        }
                    }
                }
            }
            per.push((cx, cz, sc));
            total += sc;
        }
    }
    println!("per-chunk sculk (each independent gen): {:?}", per);
    println!("sum={}", total);
    println!(
        "last counters tries={} biome={} spread={} placed={}",
        SCULK_TRIES.load(Ordering::Relaxed),
        SCULK_BIOME_OK.load(Ordering::Relaxed),
        SCULK_SPREAD_OK.load(Ordering::Relaxed),
        SCULK_PLACED.load(Ordering::Relaxed)
    );
}
