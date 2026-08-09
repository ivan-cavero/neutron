use neutron_worldgen::noise::BlendedNoise;
fn main() {
    let bn = BlendedNoise::create_unseeded(0.25, 0.125, 80.0, 160.0, 8.0);
    bn.dump_octaves();
    println!("maxValue = {:.17e}", bn.max_value());
}
