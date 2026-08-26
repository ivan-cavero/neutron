//! VALIDATION dump for `WorldgenState::region_random` — prints the first
//! draws of the vanilla `WorldGenRegion.random` stream so they can be
//! diffed against tools/worldgen-probe ProbeRegionRandom.
//!
//! Usage: region_random_dump <seed> <originX> <originZ>
use neutron_worldgen::worldgen::WorldgenState;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let seed: i64 = a[1].parse().unwrap();
    let ox: i32 = a[2].parse().unwrap();
    let oz: i32 = a[3].parse().unwrap();
    let state = WorldgenState::overworld(seed);
    let mut r = state.region_random(ox, oz);
    let bools: Vec<bool> = (0..16).map(|_| r.next_boolean()).collect();
    println!(
        "bools={}",
        bools
            .iter()
            .map(|&b| if b { 'T' } else { 'F' })
            .collect::<String>()
    );
    let ints: Vec<String> = (0..8).map(|_| r.next_int(5).to_string()).collect();
    println!("ints5={}", ints.join(","));
}
