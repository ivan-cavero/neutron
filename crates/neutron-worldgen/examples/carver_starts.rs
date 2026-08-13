use neutron_worldgen::legacy_rng::LegacyRandom;
fn main() {
    let mut starts = Vec::new();
    for dz in -8..=8i32 {
        for dx in -8..=8i32 {
            for index in 0..2i64 {
                let mut rng = LegacyRandom::new(0);
                rng.set_large_feature_seed(12345i64.wrapping_add(index), 6 + dx, -2 + dz);
                let f = rng.next_f32();
                let p = if index == 0 { 0.15f32 } else { 0.07 };
                if f <= p {
                    starts.push((6 + dx, -2 + dz, index, f));
                }
            }
        }
    }
    println!("total starts={}", starts.len());
    starts.sort_by_key(|(x, z, _, _)| (x - 6).abs() + (z - (-2)).abs());
    for s in starts.iter().take(25) {
        let dist = (s.0 - 6).abs() + (s.1 - (-2)).abs();
        println!(
            "  source=({},{}) idx={} f={:.4} dist={}",
            s.0, s.1, s.2, s.3, dist
        );
    }
}
