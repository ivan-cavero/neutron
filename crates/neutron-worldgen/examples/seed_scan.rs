//! run-058 T1: scan seeds whose biome layout matches the ref world.
//! Targets: (0,0)=pale_garden(54), (1,1)=pale_garden, (2,2)=pale_garden,
//! (11,11)=plains, (7,7)=plains, (3,9)=plains, (4,10)=plains.
use neutron_worldgen::biome_source::biome_id_at_block;
use neutron_worldgen::biome_source::biome_id;
use neutron_worldgen::ChunkGenerator;

fn main() {
    let start: i64 = std::env::args().nth(1).and_then(|s| s.parse().ok()).unwrap_or(0);
    let count: i64 = std::env::args().nth(2).and_then(|s| s.parse().ok()).unwrap_or(100000);
    // target chunks and expected biomes
    let targets: [(i32, i32, u8); 7] = [
        (0, 0, biome_id::PALE_GARDEN),
        (1, 1, biome_id::PALE_GARDEN),
        (2, 2, biome_id::PALE_GARDEN),
        (11, 11, biome_id::PLAINS),
        (7, 7, biome_id::PLAINS),
        (3, 9, biome_id::PLAINS),
        (4, 10, biome_id::PLAINS),
    ];
    let mut found = 0;
    for seed in start..start + count {
        let gen = ChunkGenerator::new(seed);
        let mut ok = 0;
        for (cx, cz, want) in &targets {
            let id = biome_id_at_block(&gen.state, cx * 16 + 8, 100, cz * 16 + 8);
            if id == *want {
                ok += 1;
            }
        }
        if ok >= 5 {
            println!("seed={seed} matches {ok}/{}", targets.len());
            found += 1;
            if found > 20 { break; }
        }
    }
    eprintln!("scan done, found {found}");
}
