use neutron_worldgen::{
    biome_source::{biome_id, climate_at_block, find_biome},
    density::DensityEnv,
    generator::WORLD_BOTTOM,
    surface::BlockId,
    ChunkGenerator,
};
fn main() {
    let g = ChunkGenerator::new(12345);
    let st = &g.state;
    // count deep_dark air with solid neighbor BEFORE sculk by scanning noise+surface+carvers+ores via generate
    // Instead: generate and check open deep dark cells that could host patches
    let ch = g.generate_chunk(6, -2);
    let mut dd_air = 0u32;
    let mut dd_solid = 0u32;
    let mut dd_air_solid_n = 0u32;
    let mut sculk = 0u32;
    for y in WORLD_BOTTOM..320 {
        for z in 0..16i32 {
            for x in 0..16i32 {
                let wx = 6 * 16 + x;
                let wz = -2 * 16 + z;
                let mut env = DensityEnv::new(wx, y, wz, st.noises.noises());
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
                let b = ch.block_at(x as u32, y, z as u32);
                if matches!(b, BlockId::Sculk | BlockId::SculkCatalyst) {
                    sculk += 1;
                }
                let open = matches!(
                    b,
                    BlockId::Air
                        | BlockId::Water
                        | BlockId::Sculk
                        | BlockId::SculkVein
                        | BlockId::SculkCatalyst
                );
                if open {
                    dd_air += 1;
                    let dirs = [
                        (0, -1, 0),
                        (0, 1, 0),
                        (0, 0, -1),
                        (0, 0, 1),
                        (-1, 0, 0),
                        (1, 0, 0),
                    ];
                    let mut has = false;
                    for (dx, dy, dz) in dirs {
                        let nb = if x + dx >= 0 && x + dx < 16 && z + dz >= 0 && z + dz < 16 {
                            ch.block_at((x + dx) as u32, y + dy, (z + dz) as u32)
                        } else {
                            BlockId::Stone
                        };
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
                        ) {
                            has = true;
                            break;
                        }
                    }
                    if has {
                        dd_air_solid_n += 1;
                    }
                } else {
                    dd_solid += 1;
                }
            }
        }
    }
    println!("deep_dark: open={dd_air} solid={dd_solid} open_with_solid_n={dd_air_solid_n} sculk={sculk}");
}
