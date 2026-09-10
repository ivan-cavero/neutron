//! Print neutron's per-step global FeatureSorter index table (as
//! `feature_catalog::build_features_per_step` computes it from
//! `OVERWORLD_BIOME_ORDER` + embedded biome JSONs) and live RNG draws for
//! decorationSeed = `set_decoration_seed(424242, -16, -32)` [origin chunk
//! (-1,-2)]. Line format matches `tools/worldgen-probe/src/ProbeIndexTable.java`
//! so the two dumps can be diffed mechanically.
//!
//!   cargo run --release -p neutron-worldgen --example index_table

use neutron_worldgen::feature_catalog::features_per_step_at;
use neutron_worldgen::feature_rng::FeatureRandom;

fn main() {
    println!("=== featuresPerStep ===");
    for step in 0..11 {
        let feats = features_per_step_at(step);
        println!("STEP {step} {}", feats.len());
        for (i, f) in feats.iter().enumerate() {
            println!("IDX {step} {i} minecraft:{f}");
        }
    }

    let mut rng = FeatureRandom::new(0);
    let decoration_seed = rng.set_decoration_seed(424242, -16, -32);
    println!("DEC {decoration_seed}");

    draw_for(decoration_seed, 9, 20, 35);
    draw_for(decoration_seed, 6, 20, 30);
}

fn draw_for(decoration_seed: i64, step: i32, lo: i32, hi: i32) {
    let feats = features_per_step_at(step);
    for i in lo..=hi {
        if i as usize >= feats.len() {
            break;
        }
        let mut rng = FeatureRandom::new(0);
        rng.set_feature_seed(decoration_seed, i, step);
        let d1 = rng.next_int(16);
        let d2 = rng.next_int(16);
        let d3 = rng.next_int(256);
        let d4 = rng.next_int(16);
        let d5 = rng.next_int(16);
        let d6 = rng.next_int(16);
        let fb = rng.next_f32();
        println!(
            "DRAW {step} {i} minecraft:{} {d1} {d2} {d3} {d4} {d5} {d6} {:08x}",
            feats[i as usize],
            fb.to_bits()
        );
    }
}
