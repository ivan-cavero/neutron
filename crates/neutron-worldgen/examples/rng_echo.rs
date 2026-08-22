//! Mirror of tools/worldgen-probe/src/ProbePaleDraws.java: 16 consecutive
//! in_square draws for a placed feature — pure RNG parity check vs the jar.
//! Usage: rng_echo <seed> <ox> <oz> <index>
use neutron_worldgen::feature_rng::FeatureRandom;

fn main() {
    let mut args = std::env::args().skip(1);
    let seed: i64 = args.next().unwrap().parse().unwrap();
    let ox: i32 = args.next().unwrap().parse().unwrap();
    let oz: i32 = args.next().unwrap().parse().unwrap();
    let index: i32 = args.next().unwrap().parse().unwrap();
    let mut rng = FeatureRandom::new(seed);
    let dec = rng.set_decoration_seed(seed, ox, oz);
    rng.set_feature_seed(dec, index, 9);
    println!("dec={dec} seed={seed} ox={ox} oz={oz} index={index}");
    for i in 0..16 {
        let x = ox + rng.next_int(16);
        let z = oz + rng.next_int(16);
        println!("draw {} ({},{}) abs=({x},{z})", i + 1, x - ox, z - oz);
    }
}
