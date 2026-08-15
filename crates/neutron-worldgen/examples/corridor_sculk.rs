use neutron_worldgen::feature_rng::FeatureRandom;
use neutron_worldgen::multiface_spreader::FaceMap;
use neutron_worldgen::region_buf::RegionBuf;
use neutron_worldgen::surface::BlockId;
// We need to call run_patch - it's private. Simulate by applying public sculk on a custom region - can't.
// Instead inline minimal loop using public MultifaceSpreader + logic

use neutron_worldgen::multiface_spreader::MultifaceSpreader;
use neutron_worldgen::multiface_spreader::DIRS;

fn main() {
    let mut region = RegionBuf::new(0, 0, 2); // bigger
                                              // Corridor along x at y=10 air, y=9 deepslate
    for x in 0..48 {
        for z in 8..12 {
            region.set(x, 9, z, BlockId::Deepslate);
            region.set(x, 10, z, BlockId::Air);
            region.set(x, 11, z, BlockId::Air);
        }
    }
    let mut faces = FaceMap::new();
    let mut rng = FeatureRandom::new(1);
    // Seed at (8,10,10)
    MultifaceSpreader::same_space().spread_all(&mut region, &mut faces, 8, 10, 10);
    let mut placed = 0;
    // Simulate 64*10 charge roughly: 320 place attempts from expanding frontier
    let mut cursors = vec![(8i32, 10i32, 10i32, 32i32); 10];
    for _attempt in 0..64 {
        let mut next = vec![];
        for (x, y, z, mut charge) in cursors {
            if charge <= 0 {
                continue;
            }
            // same space
            MultifaceSpreader::same_space().spread_all(&mut region, &mut faces, x, y, z);
            // place sculk if vein or air with faces
            let mut did = false;
            if matches!(region.get(x, y, z), BlockId::SculkVein | BlockId::Air) {
                // ensure faces
                let mut mask = faces.get(&(x, y, z)).copied().unwrap_or(0);
                if mask == 0 {
                    for (i, &(dx, dy, dz)) in DIRS.iter().enumerate() {
                        if matches!(
                            region.get(x + dx, y + dy, z + dz),
                            BlockId::Deepslate | BlockId::Stone | BlockId::Tuff
                        ) {
                            mask |= 1 << i;
                        }
                    }
                    faces.insert((x, y, z), mask);
                    if region.get(x, y, z) == BlockId::Air {
                        region.set(x, y, z, BlockId::SculkVein);
                    }
                }
                for (i, &(dx, dy, dz)) in DIRS.iter().enumerate() {
                    if mask & (1 << i) == 0 {
                        continue;
                    }
                    let nx = x + dx;
                    let ny = y + dy;
                    let nz = z + dz;
                    if region.get(nx, ny, nz) == BlockId::Deepslate {
                        region.set(nx, ny, nz, BlockId::Sculk);
                        MultifaceSpreader::vein().spread_all(&mut region, &mut faces, nx, ny, nz);
                        placed += 1;
                        charge -= 1;
                        did = true;
                        break;
                    }
                }
            }
            // move to nearby vein
            let mut moved = (x, y, z);
            for (dx, dy, dz) in DIRS {
                let nx = x + dx;
                let ny = y + dy;
                let nz = z + dz;
                if region.get(nx, ny, nz) == BlockId::SculkVein {
                    moved = (nx, ny, nz);
                    break;
                }
            }
            // also search nearby air over deepslate
            if !did {
                for dx in -2i32..=2 {
                    for dz in -2i32..=2 {
                        let nx = x + dx;
                        let nz = z + dz;
                        let ny = 10;
                        if region.get(nx, ny, nz) == BlockId::Air
                            && region.get(nx, 9, nz) == BlockId::Deepslate
                        {
                            MultifaceSpreader::same_space().spread_all(
                                &mut region,
                                &mut faces,
                                nx,
                                ny,
                                nz,
                            );
                            moved = (nx, ny, nz);
                        }
                    }
                }
            }
            if charge > 0 {
                next.push((moved.0, moved.1, moved.2, charge));
            }
        }
        cursors = next;
        if cursors.is_empty() {
            break;
        }
    }
    let mut sculk = 0;
    for x in 0..48 {
        for z in 8..12 {
            if region.get(x, 9, z) == BlockId::Sculk {
                sculk += 1;
            }
        }
    }
    println!("corridor sculk={sculk}/{} placed_ops={placed}", 48 * 4);
}
