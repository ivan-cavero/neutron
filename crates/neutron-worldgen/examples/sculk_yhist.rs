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
    let ch = g.generate_chunk(6, -2);
    let mut by_y = std::collections::BTreeMap::new();
    let mut dd = 0u32;
    let mut open_s = 0u32;
    for y in WORLD_BOTTOM..256 {
        for z in 0..16i32 {
            for x in 0..16i32 {
                let wx = 96 + x;
                let wz = -32 + z;
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
                dd += 1;
                let b = ch.block_at(x as u32, y, z as u32);
                if !matches!(b, BlockId::Air | BlockId::Water) {
                    continue;
                }
                let dirs = [
                    (0i32, -1, 0),
                    (0, 1, 0),
                    (0, 0, -1),
                    (0, 0, 1),
                    (-1, 0, 0),
                    (1, 0, 0),
                ];
                let mut solid = false;
                for (dx, dy, dz) in dirs {
                    let nx = x + dx;
                    let nz = z + dz;
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
                            | BlockId::ShortGrass
                            | BlockId::LeafLitter
                    ) {
                        solid = true;
                        break;
                    }
                }
                if solid {
                    open_s += 1;
                    *by_y.entry(y).or_insert(0u32) += 1;
                }
            }
        }
    }
    println!("deep_dark_cells={dd} open_spreadable={open_s}");
    println!("y histogram (non-zero):");
    for (y, c) in by_y {
        if c > 0 {
            println!("  y={y:4} count={c}");
        }
    }
}
