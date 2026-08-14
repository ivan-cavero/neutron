// Dump FeatureSorter index + 16 in_square attempts for dark_forest_vegetation.
// cargo run -p neutron-worldgen --example veg_pos --release

use neutron_worldgen::feature_catalog::{self, step};
use neutron_worldgen::feature_rng::FeatureRandom;
use neutron_worldgen::biome_source::biome_id_at_block;
use neutron_worldgen::generator::ChunkGenerator;

fn main() {
    let idx = feature_catalog::global_feature_index(step::VEGETAL_DECORATION, "dark_forest_vegetation");
    println!("dark_forest_vegetation global index step9 = {idx:?}");
    let list = feature_catalog::features_at_step("dark_forest", step::VEGETAL_DECORATION);
    println!("dark_forest step9 ({}):", list.len());
    for (i, f) in list.iter().enumerate() {
        let g = feature_catalog::global_feature_index(step::VEGETAL_DECORATION, f);
        println!("  biome[{i}] {f} global={g:?}");
    }

    let gen = ChunkGenerator::new(12345);
    let Some(gi) = idx else { return };
    // Replay in_square for origin (6,-2) = (96,-32) and neighbors.
    for dz in -1..=1 {
        for dx in -1..=1 {
            let ox = (6 + dx) * 16;
            let oz = (-2 + dz) * 16;
            let cx = ox + 8;
            let cz = oz + 8;
            // biome at y=136-ish
            let bid = biome_id_at_block(&gen.state, cx, 136, cz);
            let mut rng = FeatureRandom::new(12345);
            let dec = rng.set_decoration_seed(12345, ox, oz);
            rng.set_feature_seed(dec, gi, step::VEGETAL_DECORATION);
            // count 16 consumes nothing extra (constant)
            print!("origin ({},{}) biome_id={bid}", ox, oz);
            let mut hit = false;
            let mut pts = Vec::new();
            for _ in 0..16 {
                let x = ox + rng.next_int(16);
                let z = oz + rng.next_int(16);
                pts.push((x, z));
                if x == 104 && z == -24 {
                    hit = true;
                }
            }
            if ox == 96 && oz == -32 {
                println!(" pts={pts:?} hit_104_-24={hit}");
            } else {
                println!(" hit_104_-24={hit}");
            }
        }
    }

    // Confirm vanilla trunk column block under (104,136,-24)
    let chunk = gen.generate_chunk(6, -2);
    println!(
        "neutron at local (8,136,8) = {:?}",
        chunk.block_at(8, 136, 8)
    );
    println!(
        "neutron at local (8,135,8) = {:?}",
        chunk.block_at(8, 135, 8)
    );
}
