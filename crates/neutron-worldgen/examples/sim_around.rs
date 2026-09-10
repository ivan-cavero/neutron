fn main() {
    let seq = neutron_worldgen::deco_schedule::simulate_canonical_pregen();
    for (i, &p) in seq.iter().enumerate() {
        if i >= 480 && i < 595 {
            println!("{i:4} ({:3},{:3})", p.0, p.1);
        }
    }
}
