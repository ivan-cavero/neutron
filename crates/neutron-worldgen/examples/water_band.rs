//! Seed 424242: water/clay at vanilla ref water cells after full generate_chunk.
//!
//!   cargo run --release -p neutron-worldgen --example water_band

use neutron_worldgen::surface::{vanilla_name, BlockId};
use neutron_worldgen::ChunkGenerator;

const PTS: [(i32, i32, i32); 22] = [
    (12, 1, 15),
    (10, 2, 15),
    (8, 3, 14),
    (2, 5, 14),
    (5, 5, 14),
    (1, 5, 15),
    (8, 3, 15),
    (2, 5, 15),
    (5, 5, 15),
    (1, 6, 21),
    (3, 6, 23),
    (0, 5, 17),
    (1, 5, 17),
    (0, 5, 18),
    (1, 6, 19),
    (0, 6, 20),
    (2, 6, 22),
    (0, 5, 24),
    (1, 5, 25),
    (1, 4, 28),
    (2, 4, 28),
    (3, 4, 28),
];

fn main() {
    let gen = ChunkGenerator::new(424242);
    let c00 = gen.generate_chunk(0, 0);
    let c01 = gen.generate_chunk(0, 1);
    let mut water_n = 0u32;
    for &(x, y, z) in &PTS {
        let b = if z >= 16 {
            c01.block_at(x.rem_euclid(16) as u32, y, (z - 16) as u32)
        } else {
            c00.block_at(x as u32, y, z as u32)
        };
        if b == BlockId::Water {
            water_n += 1;
        }
        println!("CELL ({x:2},{y:2},{z:2}) {}", vanilla_name(b));
    }
    println!("probe_water={water_n}/{}", PTS.len());
    for (label, ch) in [("0,0", &c00), ("0,1", &c01)] {
        let mut w = 0u32;
        let mut a = 0u32;
        for y in 0..16 {
            for z in 0..16u32 {
                for x in 0..16u32 {
                    match ch.block_at(x, y, z) {
                        BlockId::Water => w += 1,
                        BlockId::Air => a += 1,
                        _ => {}
                    }
                }
            }
        }
        println!("chunk ({label}) y[0,16) water={w} air={a}");
    }
}
