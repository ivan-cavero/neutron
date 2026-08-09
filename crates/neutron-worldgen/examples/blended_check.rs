use neutron_worldgen::noise::BlendedNoise;
fn main() {
    let bn = BlendedNoise::create_unseeded(0.25, 0.125, 80.0, 160.0, 8.0);
    let coords = [(0,0,0), (100,40,200), (-57,63,31), (1234,-64,5678), (16,320,16)];
    for (x,y,z) in coords {
        println!("blended({},{},{}) = {:.17e}", x, y, z, bn.compute(x, y, z));
    }
    println!("maxValue = {:.17e}", bn.max_value());
}
