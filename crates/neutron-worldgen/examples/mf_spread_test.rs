use neutron_worldgen::feature_rng::FeatureRandom;
use neutron_worldgen::multiface_spreader::{FaceMap, MultifaceSpreader, DIRS};
use neutron_worldgen::region_buf::RegionBuf;
use neutron_worldgen::surface::BlockId;

fn main() {
    // Flat cave: stone floor y=5, air y=6..8, stone wall
    let mut region = RegionBuf::new(0, 0, 1);
    for x in 0..16 {
        for z in 0..16 {
            region.set(x, 5, z, BlockId::Deepslate);
            for y in 6..10 {
                region.set(x, y, z, BlockId::Air);
            }
        }
    }
    let mut faces = FaceMap::new();
    // same space at (8,6,8) over deepslate
    let n = MultifaceSpreader::same_space().spread_all(&mut region, &mut faces, 8, 6, 8);
    println!(
        "same_space n={n} block={:?} mask={:?}",
        region.get(8, 6, 8),
        faces.get(&(8, 6, 8))
    );

    // convert down to sculk manually like attemptPlaceSculk
    region.set(8, 5, 8, BlockId::Sculk);
    let n2 = MultifaceSpreader::vein().spread_all(&mut region, &mut faces, 8, 5, 8);
    println!("after convert spread_all from sculk n={n2}");
    let mut vein_count = 0;
    for x in 0..16 {
        for z in 0..16 {
            for y in 5..10 {
                if region.get(x, y, z) == BlockId::SculkVein {
                    vein_count += 1;
                }
            }
        }
    }
    println!("vein cells={vein_count}");
    // list vein positions
    for x in 6..11 {
        for z in 6..11 {
            for y in 5..9 {
                if region.get(x, y, z) == BlockId::SculkVein {
                    println!("  vein ({x},{y},{z}) mask={:?}", faces.get(&(x, y, z)));
                }
            }
        }
    }
}
