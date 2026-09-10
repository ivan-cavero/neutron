//! Dump Neutron biome at given block positions.
//! Usage: biome_at <seed> <x,y,z>...
use neutron_worldgen::biome_source::biome_id_at_block;
use neutron_worldgen::ChunkGenerator;

fn name(id: u8) -> &'static str {
    match id {
        neutron_worldgen::biome_source::biome_id::PLAINS => "plains",
        neutron_worldgen::biome_source::biome_id::FOREST => "forest",
        neutron_worldgen::biome_source::biome_id::DARK_FOREST => "dark_forest",
        neutron_worldgen::biome_source::biome_id::BIRCH_FOREST => "birch_forest",
        neutron_worldgen::biome_source::biome_id::OLD_GROWTH_BIRCH_FOREST => "old_growth_birch_forest",
        neutron_worldgen::biome_source::biome_id::TAIGA => "taiga",
        neutron_worldgen::biome_source::biome_id::PALE_GARDEN => "pale_garden",
        neutron_worldgen::biome_source::biome_id::MEADOW => "meadow",
        _ => "other",
    }
}

fn main() {
    let mut args = std::env::args().skip(1);
    let seed: i64 = args.next().and_then(|s| s.parse().ok()).unwrap_or(424242);
    let gen = ChunkGenerator::new(seed);
    for tok in args {
        let mut it = tok.split(',');
        let (x, y, z): (i32, i32, i32) = (
            it.next().and_then(|s| s.parse().ok()).unwrap_or(0),
            it.next().and_then(|s| s.parse().ok()).unwrap_or(70),
            it.next().and_then(|s| s.parse().ok()).unwrap_or(0),
        );
        let id = biome_id_at_block(&gen.state, x, y, z);
        println!("({x},{y},{z}) id={id} name={}", name(id));
    }
}
