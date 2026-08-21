//! Seed 424242: Neutron neighbourhood + SpringFeature.place conditions at
//! vanilla ref water cells, after doFill+carvers, then after step 8 springs,
//! then after full generate_chunk.
//!
//!   cargo run --release -p neutron-worldgen --example spring_neigh

use neutron_worldgen::carvers;
use neutron_worldgen::feature_catalog::step;
use neutron_worldgen::feature_dispatch;
use neutron_worldgen::generator::ChunkGenerator;
use neutron_worldgen::mineshaft;
use neutron_worldgen::region_buf::RegionBuf;
use neutron_worldgen::surface::{vanilla_name, BlockId};
use neutron_worldgen::NoiseCache;

const SEED: i64 = 424242;

const WATER: [(i32, i32, i32); 22] = [
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

fn is_valid(b: BlockId) -> bool {
    matches!(
        b,
        BlockId::Stone
            | BlockId::Granite
            | BlockId::Diorite
            | BlockId::Andesite
            | BlockId::Deepslate
            | BlockId::Tuff
            | BlockId::Calcite
            | BlockId::Dirt
            | BlockId::Snow
            | BlockId::PowderSnow
            | BlockId::PackedIce
    )
}

fn spring_shape(region: &RegionBuf, x: i32, y: i32, z: i32) -> (bool, i32, i32, BlockId) {
    let here = region.get(x, y, z);
    let above_ok = is_valid(region.get(x, y + 1, z));
    let below_ok = is_valid(region.get(x, y - 1, z));
    let origin_ok = matches!(here, BlockId::Air) || is_valid(here);
    let nb = [
        region.get(x - 1, y, z),
        region.get(x + 1, y, z),
        region.get(x, y, z - 1),
        region.get(x, y, z + 1),
        region.get(x, y - 1, z),
    ];
    let rock = nb.iter().filter(|b| is_valid(**b)).count() as i32;
    let holes = nb.iter().filter(|b| matches!(b, BlockId::Air)).count() as i32;
    let shaped = above_ok && below_ok && origin_ok && rock == 4 && holes == 1;
    (shaped, rock, holes, here)
}

fn count_band(region: &RegionBuf, cx: i32, cz: i32) -> (u32, u32) {
    let mut water = 0u32;
    let mut air = 0u32;
    for y in 0..16 {
        for z in 0..16 {
            for x in 0..16 {
                match region.get(cx * 16 + x, y, cz * 16 + z) {
                    BlockId::Water => water += 1,
                    BlockId::Air => air += 1,
                    _ => {}
                }
            }
        }
    }
    (water, air)
}

fn dump_cells(region: &RegionBuf, label: &str) {
    println!("=== {label} ===");
    let mut shaped_n = 0u32;
    for &(x, y, z) in &WATER {
        let (shaped, rock, holes, here) = spring_shape(region, x, y, z);
        if shaped {
            shaped_n += 1;
        }
        println!(
            "CELL ({x:2},{y:2},{z:2}) here={} above={} below={} W={} E={} N={} S={} rock={rock} hole={holes} SHAPED={shaped}",
            vanilla_name(here),
            vanilla_name(region.get(x, y + 1, z)),
            vanilla_name(region.get(x, y - 1, z)),
            vanilla_name(region.get(x - 1, y, z)),
            vanilla_name(region.get(x + 1, y, z)),
            vanilla_name(region.get(x, y, z - 1)),
            vanilla_name(region.get(x, y, z + 1)),
        );
    }
    println!("spring_shaped={shaped_n}/{}", WATER.len());
    for (cx, cz) in [(0, 0), (0, 1)] {
        let (w, a) = count_band(region, cx, cz);
        println!("  chunk ({cx},{cz}) y[0,16) water={w} air={a}");
    }
}

fn fill_carvers(gen: &ChunkGenerator) -> RegionBuf {
    let mut region = RegionBuf::new(0, 0, 2);
    let mut cache = NoiseCache::new();
    for dz in -2..=2 {
        for dx in -2..=2 {
            let col = cache.get_or_insert_with((dx, dz), || gen.generate_noise_and_surface(dx, dz));
            let (blocks, heightmap, _) = col.clone();
            region.put_chunk(dx, dz, &blocks, &heightmap);
        }
    }
    carvers::apply_carvers_region(&mut region, &gen.state);
    mineshaft::apply_mineshafts_region(&mut region, &gen.state);
    region
}

fn main() {
    println!("seed={SEED} SpringFeature.place / feature_dispatch::place_spring");
    let gen = ChunkGenerator::new(SEED);
    let mut region = fill_carvers(&gen);
    dump_cells(&region, "AFTER doFill+carvers+mineshafts (no features)");

    feature_dispatch::apply_step_region(&mut region, &gen.state, step::FLUID_SPRINGS, "plains");
    dump_cells(&region, "AFTER step 8 FLUID_SPRINGS only");

    let full00 = gen.generate_chunk(0, 0);
    let full01 = gen.generate_chunk(0, 1);
    println!("=== AFTER full generate_chunk ===");
    let mut hit = 0u32;
    for &(x, y, z) in &WATER {
        let b = if z >= 16 {
            full01.block_at(x.rem_euclid(16) as u32, y, (z - 16) as u32)
        } else {
            full00.block_at(x as u32, y, z as u32)
        };
        if b == BlockId::Water {
            hit += 1;
        }
        println!("CELL ({x:2},{y:2},{z:2}) {}", vanilla_name(b));
    }
    println!("full_generate water_at_probe_cells={hit}/{}", WATER.len());
    let mut w00 = 0u32;
    let mut a00 = 0u32;
    let mut w01 = 0u32;
    let mut a01 = 0u32;
    for y in 0..16 {
        for z in 0..16u32 {
            for x in 0..16u32 {
                match full00.block_at(x, y, z) {
                    BlockId::Water => w00 += 1,
                    BlockId::Air => a00 += 1,
                    _ => {}
                }
                match full01.block_at(x, y, z) {
                    BlockId::Water => w01 += 1,
                    BlockId::Air => a01 += 1,
                    _ => {}
                }
            }
        }
    }
    println!("  chunk (0,0) y[0,16) water={w00} air={a00}");
    println!("  chunk (0,1) y[0,16) water={w01} air={a01}");
    println!("FINDING: cite feature_dispatch.rs place_spring / SpringFeature.place; generate_chunk_cached steps 6-9");
}
