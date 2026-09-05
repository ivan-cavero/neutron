//! Prints neutron's climate parameters at block coordinates.
//! Usage: climate_at <seed> <x,y,z>...
use neutron_worldgen::biome::manager::climate_at;
use neutron_worldgen::ChunkGenerator;

fn main() {
    let mut a = std::env::args().skip(1);
    let seed: i64 = a.next().unwrap().parse().unwrap();
    let gen = ChunkGenerator::new(seed);
    let mut pts = Vec::new();
    while let (Some(x), Some(y), Some(z)) = (a.next(), a.next(), a.next()) {
        pts.push((x.parse::<i32>().unwrap(), y.parse::<i32>().unwrap(), z.parse::<i32>().unwrap()));
    }
    for (x, y, z) in pts {
        let c = climate_at(&gen.state, x, y, z);
        println!("CLIMATE {},{},{} temp={} humid={} cont={} erosion={} depth={} weird={}",
            x, y, z, c.temperature, c.humidity, c.continentalness,
            c.erosion, c.depth, c.weirdness);
    }
}
