//! Multi-chunk base occupancy (noise+surface+carvers+ores, density-phase class)
//! for comparison with Java ProbeMultiBase open fractions.
use neutron_worldgen::surface::BlockId;
use neutron_worldgen::{generator::WORLD_BOTTOM, ChunkGenerator};

fn main() {
    let gen = ChunkGenerator::new(12345);
    let chunks = [
        (0, 0),
        (6, -2),
        (32, 0),
        (-32, 16),
        (0, 48),
        (64, -32),
        (-48, -48),
        (10, 10),
        (20, -5),
        (5, -3),
        (100, 0),
        (-100, 50),
    ];
    println!("cx,cz  open_frac  solid_frac");
    for &(cx, cz) in &chunks {
        let ch = gen.generate_chunk(cx, cz);
        let mut open = 0u32;
        let mut solid = 0u32;
        for y in WORLD_BOTTOM..320 {
            for z in (0..16).step_by(4) {
                for x in (0..16).step_by(4) {
                    let b = ch.block_at(x, y, z);
                    match b {
                        BlockId::Air
                        | BlockId::Water
                        | BlockId::Lava
                        | BlockId::Sculk
                        | BlockId::ShortGrass
                        | BlockId::OakLeaves => open += 1,
                        _ => solid += 1,
                    }
                }
            }
        }
        let total = (open + solid) as f64;
        println!(
            "{cx},{cz}  open={:.4} solid={:.4}",
            open as f64 / total,
            solid as f64 / total
        );
    }
}
