fn main() {
    let seq = neutron_worldgen::deco_schedule::simulate_canonical_pregen();
    let mut rank = std::collections::HashMap::new();
    for (i, &p) in seq.iter().enumerate() { rank.insert(p, i); }
    let origins = [(-13,-13),(-15,-15),(-15,-14),(-15,-13),(-14,-15),(-14,-14),(-14,-13),(-13,-15),(-13,-14),(-16,-13),(-14,-16),(-13,-12)];
    for o in origins {
        println!("({:3},{:3}) rank={}", o.0, o.1, rank.get(&o).map(|r| *r as i64).unwrap_or(-1));
    }
    println!("seq len {}", seq.len());
}
