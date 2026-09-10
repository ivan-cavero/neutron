fn main() {
    let seed: i64 = std::env::args().nth(1).unwrap().parse().unwrap();
    let mut hits = Vec::new();
    for cz in -30..10i32 {
        for cx in -30..10i32 {
            if neutron_worldgen::mineshaft::is_mineshaft_chunk(seed, cx, cz) {
                hits.push((cx, cz));
            }
        }
    }
    println!("mineshaft starts near target: {hits:?}");
}
