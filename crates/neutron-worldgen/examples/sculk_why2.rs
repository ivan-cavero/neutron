use neutron_worldgen::{
    biome_source::{biome_id, climate_at_block, find_biome},
    density::DensityEnv,
    feature_rng::FeatureRandom,
    generator::WORLD_BOTTOM,
    surface::BlockId,
    ChunkGenerator,
};
fn main() {
    let g = ChunkGenerator::new(12345);
    let st = &g.state;
    let ch = g.generate_chunk(6, -2);
    let mut rng = FeatureRandom::new(12345);
    let ox = 96;
    let oz = -32;
    let dec = rng.set_decoration_seed(12345, ox, oz);
    rng.set_feature_seed(dec, 1, 7);
    let mut biome_ok = 0u32;
    let mut spread = 0u32;
    let mut on_vein = 0u32;
    let mut on_air = 0u32;
    for _ in 0..256 {
        let x = ox + rng.next_int(16);
        let z = oz + rng.next_int(16);
        let y = WORLD_BOTTOM + rng.next_int(256 - WORLD_BOTTOM + 1);
        let lx = (x - ox) as u32;
        let lz = (z - oz) as u32;
        let mut env = DensityEnv::new(x, y, z, st.noises.noises());
        let climate = climate_at_block(
            &mut env,
            &st.router.temperature,
            &st.router.vegetation,
            &st.router.continents,
            &st.router.erosion,
            &st.router.depth,
            &st.router.ridges,
        );
        if find_biome(&climate) != biome_id::DEEP_DARK {
            continue;
        }
        biome_ok += 1;
        let b = ch.block_at(lx, y, lz);
        let sculk_beh = matches!(
            b,
            BlockId::Sculk | BlockId::SculkVein | BlockId::SculkCatalyst
        );
        let open = matches!(b, BlockId::Air | BlockId::Water);
        if sculk_beh {
            on_vein += 1;
            spread += 1;
            continue;
        }
        if !open {
            continue;
        }
        on_air += 1;
        let dirs = [
            (0i32, -1, 0),
            (0, 1, 0),
            (0, 0, -1),
            (0, 0, 1),
            (-1, 0, 0),
            (1, 0, 0),
        ];
        for (dx, dy, dz) in dirs {
            let nx = lx as i32 + dx;
            let nz = lz as i32 + dz;
            let ny = y + dy;
            if nx < 0 || nx >= 16 || nz < 0 || nz >= 16 || ny < WORLD_BOTTOM || ny >= 320 {
                continue;
            }
            let nb = ch.block_at(nx as u32, ny, nz as u32);
            if !matches!(
                nb,
                BlockId::Air
                    | BlockId::Water
                    | BlockId::Lava
                    | BlockId::Sculk
                    | BlockId::SculkVein
                    | BlockId::SculkCatalyst
                    | BlockId::OakLeaves
                    | BlockId::Snow
                    | BlockId::PowderSnow
                    | BlockId::ShortGrass
            ) {
                spread += 1;
                break;
            }
        }
    }
    // count open deep dark
    let mut open_dd = 0u32;
    let mut vein_dd = 0u32;
    let mut sculk_dd = 0u32;
    for y in WORLD_BOTTOM..320 {
        for z in 0..16u32 {
            for x in 0..16u32 {
                let wx = ox + x as i32;
                let wz = oz + z as i32;
                let mut env = DensityEnv::new(wx, y, wz as i32, st.noises.noises());
                let climate = climate_at_block(
                    &mut env,
                    &st.router.temperature,
                    &st.router.vegetation,
                    &st.router.continents,
                    &st.router.erosion,
                    &st.router.depth,
                    &st.router.ridges,
                );
                if find_biome(&climate) != biome_id::DEEP_DARK {
                    continue;
                }
                match ch.block_at(x, y, z) {
                    BlockId::Air | BlockId::Water => open_dd += 1,
                    BlockId::SculkVein => vein_dd += 1,
                    BlockId::Sculk | BlockId::SculkCatalyst => sculk_dd += 1,
                    _ => {}
                }
            }
        }
    }
    println!(
        "patch samples: biome_ok={biome_ok} spread={spread} on_vein={on_vein} on_air_open={on_air}"
    );
    println!("chunk deep_dark: open={open_dd} vein={vein_dd} sculk={sculk_dd}");
}
