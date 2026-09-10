fn main() {
    let seq = neutron_worldgen::deco_schedule::simulate_canonical_pregen();
    let mut rank = std::collections::HashMap::new();
    for (i, &p) in seq.iter().enumerate() { rank.insert(p, i); }
    let mut v: Vec<((i64),(i32,i32))> = Vec::new();
    for cz in -16..=-12i32 {
        for cx in -16..=-12i32 {
            v.push((*rank.get(&(cx,cz)).unwrap_or(&9999) as i64, (cx,cz)));
        }
    }
    v.sort();
    for (r, p) in v { println!("{:4} ({},{})", r, p.0, p.1); }
}
