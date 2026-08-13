// Quick verification harness for the 26.2 RNG + noise core against the Java probe.
use neutron_worldgen::Xoroshiro128;

fn main() {
    let seed: i64 = std::env::args()
        .nth(1)
        .unwrap_or("42".into())
        .parse()
        .unwrap();
    let mut rng = Xoroshiro128::new(seed);
    println!(
        "seed={} mainPosLo={} mainPosHi={}",
        seed,
        rng.fork_positional().0 as i64,
        rng.fork_positional().1 as i64
    );
    let mut rng = Xoroshiro128::new(seed);
    for i in 0..12 {
        println!("nextDouble[{}] = {:.17e}", i, rng.next_f64());
    }
}
