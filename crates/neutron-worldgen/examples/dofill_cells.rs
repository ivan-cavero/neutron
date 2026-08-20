//! Seed 424242: Neutron doFill (noise+surface) vs after-carvers at known
//! vanilla water cells, plus water/air counts in y 0..16 for chunks (0,0)/(0,1).
//!
//!   cargo run --release -p neutron-worldgen --example dofill_cells

use neutron_worldgen::carvers;
use neutron_worldgen::generator::{ChunkGenerator, WORLD_BOTTOM, WORLD_TOP};
use neutron_worldgen::region_buf::RegionBuf;
use neutron_worldgen::surface::{vanilla_name, BlockId};

fn count_chunk(region: &RegionBuf, cx: i32, cz: i32) {
    let mut water = 0u32;
    let mut air = 0u32;
    let bands = [-32, -16, 0, 16, 32];
    let mut wband = [0u32; 4];
    let mut aband = [0u32; 4];
    for y in WORLD_BOTTOM..WORLD_TOP {
        for z in 0..16 {
            for x in 0..16 {
                let b = region.get(cx * 16 + x, y, cz * 16 + z);
                let bi = bands.windows(2).position(|w| y >= w[0] && y < w[1]);
                match b {
                    BlockId::Water => {
                        water += 1;
                        if let Some(i) = bi {
                            wband[i] += 1;
                        }
                    }
                    BlockId::Air => {
                        air += 1;
                        if let Some(i) = bi {
                            aband[i] += 1;
                        }
                    }
                    _ => {}
                }
            }
        }
    }
    println!("  chunk ({cx},{cz}): water={water} air={air}");
    for i in 0..4 {
        println!(
            "    y{}..{} water={} air={}",
            bands[i],
            bands[i + 1],
            wband[i],
            aband[i]
        );
    }
}

fn cells(region: &RegionBuf, label: &str, pts: &[(i32, i32, i32)]) {
    println!("  {label}");
    for &(x, y, z) in pts {
        let b = region.get(x, y, z);
        println!("    ({x:2},{y:2},{z:2}) {}", vanilla_name(b));
    }
}

fn main() {
    let gen = ChunkGenerator::new(424242);
    let mut region = RegionBuf::new(0, 0, 1);
    for dz in -1..=1 {
        for dx in -1..=1 {
            let (blocks, heightmap, _) = gen.generate_noise_and_surface(dx, dz);
            region.put_chunk(dx, dz, &blocks, &heightmap);
        }
    }

    // Vanilla water / hairline cells from runs 053–059 (world coords).
    let pts = [
        (12, 1, 15),
        (10, 2, 15),
        (8, 3, 14),
        (2, 5, 14),
        (5, 5, 14),
        (1, 5, 15),
        (1, 5, 15),
        (1, 6, 21),
        (3, 6, 23),
        (0, 5, 17),
        (0, 5, 18),
    ];

    println!("AFTER doFill+surface (no carvers)");
    count_chunk(&region, 0, 0);
    count_chunk(&region, 0, 1);
    cells(&region, "probe cells", &pts);

    carvers::apply_carvers_region(&mut region, &gen.state);

    println!("AFTER carvers");
    count_chunk(&region, 0, 0);
    count_chunk(&region, 0, 1);
    cells(&region, "probe cells", &pts);
}
