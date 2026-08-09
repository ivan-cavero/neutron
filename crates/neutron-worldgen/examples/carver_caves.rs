use neutron_worldgen::legacy_rng::LegacyRandom;
fn main() {
    let tcx=6i32; let tcz=-2i32;
    let mut with_caves = Vec::new();
    for dz in -8..=8i32 {
        for dx in -8..=8i32 {
            for index in 0..2i64 {
                let mut rng = LegacyRandom::new(0);
                rng.set_large_feature_seed(12345i64.wrapping_add(index), tcx+dx, tcz+dz);
                let f = rng.next_f32();
                let p = if index==0 {0.15f32} else {0.07};
                if f > p { continue; }
                let a = rng.next_int(15)+1;
                let b = rng.next_int(a)+1;
                let cc = rng.next_int(b);
                let dist = dx.abs()+dz.abs();
                if cc > 0 {
                    with_caves.push((tcx+dx, tcz+dz, index, cc, dist));
                }
            }
        }
    }
    with_caves.sort_by_key(|s| s.4);
    println!("starts_with_caves={}", with_caves.len());
    for s in with_caves.iter().take(20) {
        println!("  source=({},{}) idx={} caveCount={} dist={}", s.0,s.1,s.2,s.3,s.4);
    }
}
