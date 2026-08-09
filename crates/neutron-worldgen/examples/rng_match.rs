use neutron_worldgen::legacy_rng::LegacyRandom;
fn main() {
    let mut rng = LegacyRandom::new(0);
    rng.set_large_feature_seed(12345, 6, -1);
    println!("rust (6,-1) nextFloat={:.8}", rng.next_f32());
    // full stream after start
    let mut rng = LegacyRandom::new(0);
    rng.set_large_feature_seed(12345, 6, -1);
    let f = rng.next_f32();
    let a = rng.next_int(15)+1;
    let b = rng.next_int(a)+1;
    let cc = rng.next_int(b);
    println!("rust start f={f:.8} a={a} b={b} caveCount={cc}");
}
