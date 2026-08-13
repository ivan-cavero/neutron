use neutron_worldgen::generator::WORLD_BOTTOM;
use neutron_worldgen::sculk::{
    SCULK_BIOME_OK, SCULK_PLACED, SCULK_SPREAD_OK, SCULK_TRIES, SCULK_VEIN_PLACED,
};
use neutron_worldgen::{surface::BlockId, ChunkGenerator};
use std::sync::atomic::Ordering;
fn main() {
    for a in [
        &SCULK_TRIES,
        &SCULK_BIOME_OK,
        &SCULK_SPREAD_OK,
        &SCULK_PLACED,
        &SCULK_VEIN_PLACED,
    ] {
        a.store(0, Ordering::Relaxed);
    }
    let g = ChunkGenerator::new(12345);
    let ch = g.generate_chunk(6, -2);
    let mut sculk = 0u32;
    let mut vein = 0u32;
    let mut cat = 0u32;
    for y in WORLD_BOTTOM..320 {
        for z in 0..16u32 {
            for x in 0..16u32 {
                match ch.block_at(x, y, z) {
                    BlockId::Sculk => sculk += 1,
                    BlockId::SculkVein => vein += 1,
                    BlockId::SculkCatalyst => cat += 1,
                    _ => {}
                }
            }
        }
    }
    println!(
        "tries={} biome_ok={} spread_ok={} placed_ops={} vein_ops={} final_sculk={} vein={} catalyst={}",
        SCULK_TRIES.load(Ordering::Relaxed),
        SCULK_BIOME_OK.load(Ordering::Relaxed),
        SCULK_SPREAD_OK.load(Ordering::Relaxed),
        SCULK_PLACED.load(Ordering::Relaxed),
        SCULK_VEIN_PLACED.load(Ordering::Relaxed),
        sculk,
        vein,
        cat
    );
}
