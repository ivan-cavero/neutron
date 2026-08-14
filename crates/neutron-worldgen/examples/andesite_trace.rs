// Trace ore_andesite_upper attempts in the 3×3 around (6,-2).
// cargo run -p neutron-worldgen --example andesite_trace --release

use neutron_worldgen::feature_rng::FeatureRandom;
use neutron_worldgen::surface::BlockId;
use neutron_worldgen::{generator::WORLD_BOTTOM, ChunkGenerator};

fn main() {
    let seed = 12345i64;
    let cx = 6;
    let cz = -2;
    // Rebuild 3×3 heightmaps the same way the generator does.
    let gen = ChunkGenerator::new(seed);
    let _ = gen.generate_chunk(cx, cz); // warms nothing we need; we re-query via generate

    // Use generated center heightmap via a dummy generate of neighbors
    let mut hms = std::collections::HashMap::new();
    for dz in -1..=1 {
        for dx in -1..=1 {
            let ch = gen.generate_chunk(cx + dx, cz + dz);
            hms.insert((cx + dx, cz + dz), ch.heightmap.clone());
        }
    }

    fn first_avail(
        hms: &std::collections::HashMap<(i32, i32), Vec<i16>>,
        x: i32,
        z: i32,
    ) -> Option<i32> {
        let ccx = x.div_euclid(16);
        let ccz = z.div_euclid(16);
        let lx = x.rem_euclid(16) as usize;
        let lz = z.rem_euclid(16) as usize;
        let hm = hms.get(&(ccx, ccz))?;
        let solid = hm[lz * 16 + lx] as i32;
        if solid <= WORLD_BOTTOM {
            return None;
        }
        Some(solid + 1)
    }

    let size = 64i32;
    let f = size as f32 / 8.0;
    let cell = ((size as f32 / 16.0 * 2.0 + 1.0) / 2.0).ceil() as i32;
    let f_ceil = f.ceil() as i32;
    let size_xz = 2 * (f_ceil + cell);

    println!("cell={cell} f_ceil={f_ceil} size_xz={size_xz} start_y=oy-{}", 2 + cell);

    for dz in -1..=1 {
        for dx in -1..=1 {
            let ocx = cx + dx;
            let ocz = cz + dz;
            let ox0 = ocx * 16;
            let oz0 = ocz * 16;
            let mut rng = FeatureRandom::new(seed);
            let dec = rng.set_decoration_seed(seed, ox0, oz0);
            rng.set_feature_seed(dec, 6, 6); // andesite_upper
            let pass = rng.next_int(6) == 0;
            if !pass {
                println!("origin ({ocx},{ocz}) rarity SKIP");
                continue;
            }
            let lx = rng.next_int(16);
            let lz = rng.next_int(16);
            let x = ox0 + lx;
            let z = oz0 + lz;
            // uniform 64..=128 inclusive
            let y = 64 + rng.next_int(128 - 64 + 1);
            let start_x = x - f_ceil - cell;
            let start_y = y - 2 - cell;
            let start_z = z - f_ceil - cell;
            let mut gate = false;
            let mut max_h = i32::MIN;
            let mut n = 0u32;
            for px in start_x..=start_x + size_xz {
                for pz in start_z..=start_z + size_xz {
                    if let Some(h) = first_avail(&hms, px, pz) {
                        n += 1;
                        max_h = max_h.max(h);
                        if start_y <= h {
                            gate = true;
                        }
                    }
                }
            }
            println!(
                "origin ({ocx},{ocz}) PLACE at ({x},{y},{z}) start_y={start_y} max_h={max_h} cols={n} gate={gate}"
            );
        }
    }

    // Also print heightmap at extra centroid world (6*16+9, -2*16+5)=(105,-27)
    if let Some(h) = first_avail(&hms, 105, -27) {
        println!("centroid (105,-27) first_avail={h} solid={}", h - 1);
    }
}
