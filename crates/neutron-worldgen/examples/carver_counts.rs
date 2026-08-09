use neutron_worldgen::carvers::{self, CARVE_STARTS, CARVE_WRITES};
use neutron_worldgen::ChunkGenerator;
use std::sync::atomic::Ordering;
fn main() {
    CARVE_STARTS.store(0, Ordering::Relaxed);
    CARVE_WRITES.store(0, Ordering::Relaxed);
    let gen = ChunkGenerator::new(12345);
    let _ = gen.generate_chunk(6, -2);
    println!("starts={} writes={}", CARVE_STARTS.load(Ordering::Relaxed), CARVE_WRITES.load(Ordering::Relaxed));
}
