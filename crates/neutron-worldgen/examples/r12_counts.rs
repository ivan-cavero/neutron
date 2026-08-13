use neutron_worldgen::carvers::{CARVE_STARTS, CARVE_WRITES};
use neutron_worldgen::generator::WORLD_BOTTOM;
use neutron_worldgen::{surface::BlockId, ChunkGenerator};
use std::sync::atomic::Ordering;
fn main() {
    CARVE_STARTS.store(0, Ordering::Relaxed);
    CARVE_WRITES.store(0, Ordering::Relaxed);
    let g = ChunkGenerator::new(12345);
    let ch = g.generate_chunk(6, -2);
    let mut grass = 0u32;
    let mut sculk = 0u32;
    let mut air = 0u32;
    for y in WORLD_BOTTOM..320 {
        for z in 0..16u32 {
            for x in 0..16u32 {
                match ch.block_at(x, y, z) {
                    BlockId::ShortGrass => grass += 1,
                    BlockId::Sculk | BlockId::SculkCatalyst => sculk += 1,
                    BlockId::Air => air += 1,
                    _ => {}
                }
            }
        }
    }
    println!(
        "starts={} writes={} short_grass={} sculk={} air={}",
        CARVE_STARTS.load(Ordering::Relaxed),
        CARVE_WRITES.load(Ordering::Relaxed),
        grass,
        sculk,
        air
    );
}
