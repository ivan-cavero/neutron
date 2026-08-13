use neutron_worldgen::carvers;
use neutron_worldgen::generator::WORLD_BOTTOM;
use neutron_worldgen::region_buf::RegionBuf;
use neutron_worldgen::{surface::BlockId, ChunkGenerator};

fn main() {
    let gen = ChunkGenerator::new(12345);
    // Build 3x3 like generator
    let cx = 6i32;
    let cz = -2i32;
    let mut region = RegionBuf::new(cx, cz, 1);
    for dz in -1..=1 {
        for dx in -1..=1 {
            // use public generate via full chunk without carvers - hack: generate and put
            let ch = {
                // only noise+surface: generate_chunk currently includes carvers+ores
                // call generate_chunk and put - recursive carvers. Instead just count starts.
                ()
            };
            let _ = (dx, dz, ch);
        }
    }
    // Count isStartChunk hits
    use neutron_worldgen::legacy_rng::LegacyRandom;
    let mut starts = 0u32;
    let mut tries = 0u32;
    for dz in -8..=8i32 {
        for dx in -8..=8i32 {
            for index in 0..2i64 {
                tries += 1;
                let mut rng = LegacyRandom::new(0);
                rng.set_large_feature_seed(12345i64.wrapping_add(index), cx + dx, cz + dz);
                let f = rng.next_f32();
                let prob = if index == 0 { 0.15f32 } else { 0.07 };
                if f <= prob {
                    starts += 1;
                }
            }
        }
    }
    println!("carver start tries={tries} starts={starts}");

    // Full generate and count air in chunk
    let ch = gen.generate_chunk(cx, cz);
    let mut air = 0u32;
    let mut solid = 0u32;
    for y in WORLD_BOTTOM..320 {
        for z in 0..16u32 {
            for x in 0..16u32 {
                let b = ch.block_at(x, y, z);
                if matches!(b, BlockId::Air) {
                    air += 1;
                } else if !matches!(b, BlockId::Water | BlockId::Lava) {
                    solid += 1;
                }
            }
        }
    }
    println!("chunk air={air} solid={solid}");
}
