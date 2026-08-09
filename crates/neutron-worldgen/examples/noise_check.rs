// Compare Rust noise core against Java probe ground truth (tools/java-probe).
// Params extracted verbatim from the vanilla datapack JSONs.
use neutron_worldgen::noise::NormalNoise;
use neutron_worldgen::rng::Xoroshiro128;

fn main() {
    let seed: i64 = std::env::args().nth(1).unwrap_or("42".into()).parse().unwrap();

    let mut rng = Xoroshiro128::new(seed);
    let (lo, hi) = rng.fork_positional();
    println!("seed={} mainPosLo={} mainPosHi={}", seed, lo as i64, hi as i64);

    let params: &[(&str, i32, &[f64])] = &[
        ("clay_bands_offset", -8, &[1.0]),
        ("surface", -6, &[1.0, 1.0, 1.0]),
        ("surface_secondary", -6, &[1.0, 1.0, 0.0, 1.0]),
        ("badlands_pillar", -2, &[1.0, 1.0, 1.0, 1.0]),
        ("badlands_pillar_roof", -8, &[1.0]),
        ("badlands_surface", -6, &[1.0, 1.0, 1.0]),
        ("iceberg_pillar", -6, &[1.0, 1.0, 1.0, 1.0]),
        ("iceberg_pillar_roof", -3, &[1.0]),
        ("iceberg_surface", -6, &[1.0, 1.0, 1.0]),
        ("aquifer_barrier", -3, &[1.0]),
        ("aquifer_fluid_level_floodedness", -7, &[1.0]),
        ("aquifer_fluid_level_spread", -5, &[1.0]),
        ("aquifer_lava", -1, &[1.0]),
        ("offset", -3, &[1.0, 1.0, 1.0, 0.0]),
        ("temperature", -10, &[1.5, 0.0, 1.0, 0.0, 0.0, 0.0]),
        ("vegetation", -8, &[1.0, 1.0, 0.0, 0.0, 0.0, 0.0]),
        ("continentalness", -9, &[1.0, 1.0, 2.0, 2.0, 2.0, 1.0, 1.0, 1.0, 1.0]),
        ("erosion", -9, &[1.0, 1.0, 0.0, 1.0, 1.0]),
        ("ridge", -7, &[1.0, 2.0, 1.0, 0.0, 0.0, 0.0]),
        ("jagged", -16, &[1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0]),
        ("cave_entrance", -7, &[0.4, 0.5, 1.0]),
        ("spaghetti_roughness_modulator", -8, &[1.0]),
        ("spaghetti_roughness", -5, &[1.0]),
        ("spaghetti_3d_rarity", -11, &[1.0]),
        ("spaghetti_3d_1", -7, &[1.0]),
        ("spaghetti_3d_2", -7, &[1.0]),
        ("spaghetti_3d_thickness", -8, &[1.0]),
        ("cave_layer", -8, &[1.0]),
        ("cave_cheese", -8, &[0.5, 1.0, 2.0, 1.0, 2.0, 1.0, 0.0, 2.0, 0.0]),
        ("spaghetti_2d_modulator", -11, &[1.0]),
        ("spaghetti_2d", -7, &[1.0]),
        ("spaghetti_2d_thickness", -11, &[1.0]),
        ("spaghetti_2d_elevation", -8, &[1.0]),
        ("pillar", -7, &[1.0, 1.0]),
        ("pillar_rareness", -8, &[1.0]),
        ("pillar_thickness", -8, &[1.0]),
        ("noodle", -8, &[1.0]),
        ("noodle_thickness", -8, &[1.0]),
        ("noodle_ridge_a", -7, &[1.0]),
        ("noodle_ridge_b", -7, &[1.0]),
        ("ore_veininess", -8, &[1.0]),
        ("ore_vein_a", -7, &[1.0]),
        ("ore_vein_b", -7, &[1.0]),
        ("ore_gap", -5, &[1.0]),
    ];

    let noise_rng = Xoroshiro128::from_raw(lo, hi);
    for (key, first_octave, amps) in params {
        let s = noise_rng.from_hash_of(&format!("minecraft:{key}")).seed();
        let nn = NormalNoise::create(s.0, s.1, *first_octave, amps);
        let coords = [(0.0, 0.0, 0.0), (100.5, 40.0, 200.5), (-57.0, 63.0, 31.0)];
        let samples: Vec<String> = coords
            .iter()
            .map(|(x, y, z)| format!("{:.17e}", nn.get_value(*x, *y, *z)))
            .collect();
        println!("{} {}", key, samples.join(" "));
    }
}
