use neutron_worldgen::sculk::SCULK_ENABLED;
use neutron_worldgen::{
    biome_source::{biome_id, climate_at_block, find_biome},
    density::DensityEnv,
    feature_rng::FeatureRandom,
    generator::WORLD_BOTTOM,
    surface::BlockId,
    ChunkGenerator,
};
use std::collections::HashMap;

fn main() {
    assert!(SCULK_ENABLED);
    let g = ChunkGenerator::new(12345);
    let st = &g.state;
    let ch = g.generate_chunk(6, -2);
    let mut rng = FeatureRandom::new(12345);
    let ox = 6 * 16;
    let oz = -2 * 16;
    let dec = rng.set_decoration_seed(12345, ox, oz);
    rng.set_feature_seed(dec, 1, 7);
    let mut reasons: HashMap<String, u32> = HashMap::new();
    let mut biome_ok = 0u32;
    let mut would_spread = 0u32;
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
            *reasons.entry("not_biome".into()).or_default() += 1;
            continue;
        }
        biome_ok += 1;
        let b = ch.block_at(lx, y, lz);
        let open = matches!(
            b,
            BlockId::Air
                | BlockId::Water
                | BlockId::Sculk
                | BlockId::SculkVein
                | BlockId::SculkCatalyst
        );
        if !open {
            *reasons.entry(format!("solid:{b:?}")).or_default() += 1;
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
        let mut has = false;
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
            ) {
                has = true;
                break;
            }
        }
        if has {
            would_spread += 1;
            *reasons.entry("spread_ok".into()).or_default() += 1;
        } else {
            *reasons.entry("open_no_solid_n".into()).or_default() += 1;
        }
    }
    println!("biome_ok={biome_ok} would_spread={would_spread}");
    let mut v: Vec<_> = reasons.into_iter().collect();
    v.sort_by(|a, b| b.1.cmp(&a.1));
    for (k, c) in v {
        println!("  {c:4} {k}");
    }
}
