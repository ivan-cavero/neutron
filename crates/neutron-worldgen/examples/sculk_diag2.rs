use neutron_worldgen::feature_rng::FeatureRandom;
use neutron_worldgen::multiface_spreader::FaceMap;
use neutron_worldgen::region_buf::RegionBuf;
use neutron_worldgen::sculk;
use neutron_worldgen::surface::BlockId;
use neutron_worldgen::{generator::WORLD_BOTTOM, ChunkGenerator};
use std::sync::atomic::Ordering;

fn main() {
    // Full generate chunk and track
    for a in [
        &sculk::SCULK_TRIES,
        &sculk::SCULK_BIOME_OK,
        &sculk::SCULK_SPREAD_OK,
        &sculk::SCULK_PLACED,
        &sculk::SCULK_VEIN_PLACED,
    ] {
        a.store(0, Ordering::Relaxed);
    }
    let g = ChunkGenerator::new(12345);
    let ch = g.generate_chunk(6, -2);
    let mut sculk_c = 0u32;
    let mut vein_c = 0u32;
    let mut open_dd_solid_n = 0u32;
    // count open with solid in chunk using generator biomes approx all deep dark lower
    for y in WORLD_BOTTOM..320 {
        for z in 0..16u32 {
            for x in 0..16u32 {
                match ch.block_at(x, y, z) {
                    BlockId::Sculk | BlockId::SculkCatalyst => sculk_c += 1,
                    BlockId::SculkVein => vein_c += 1,
                    _ => {}
                }
            }
        }
    }
    println!(
        "tries={} biome_ok={} spread_ok={} placed={} vein_ops={} final_sculk={} vein={}",
        sculk::SCULK_TRIES.load(Ordering::Relaxed),
        sculk::SCULK_BIOME_OK.load(Ordering::Relaxed),
        sculk::SCULK_SPREAD_OK.load(Ordering::Relaxed),
        sculk::SCULK_PLACED.load(Ordering::Relaxed),
        sculk::SCULK_VEIN_PLACED.load(Ordering::Relaxed),
        sculk_c,
        vein_c
    );

    // How many open cells with solid n and deep dark? use climate
    let st = &g.state;
    let mut open_spreadable = 0u32;
    let mut vein_spreadable = 0u32;
    for y in WORLD_BOTTOM..256 {
        for z in 0..16i32 {
            for x in 0..16i32 {
                let wx = 96 + x;
                let wz = -32 + z;
                let mut env =
                    neutron_worldgen::density::DensityEnv::new(wx, y, wz, st.noises.noises());
                let climate = neutron_worldgen::biome_source::climate_at_block(
                    &mut env,
                    &st.router.temperature,
                    &st.router.vegetation,
                    &st.router.continents,
                    &st.router.erosion,
                    &st.router.depth,
                    &st.router.ridges,
                );
                if neutron_worldgen::biome_source::find_biome(&climate)
                    != neutron_worldgen::biome_source::biome_id::DEEP_DARK
                {
                    continue;
                }
                let b = ch.block_at(x as u32, y, z as u32);
                if matches!(
                    b,
                    BlockId::SculkVein | BlockId::Sculk | BlockId::SculkCatalyst
                ) {
                    vein_spreadable += 1;
                    continue;
                }
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
                            | BlockId::Snow
                    ) {
                        open_spreadable += 1;
                        break;
                    }
                }
            }
        }
    }
    println!(
        "deep_dark open_with_solid_n={open_spreadable} sculk_behaviour_cells={vein_spreadable}"
    );
}
