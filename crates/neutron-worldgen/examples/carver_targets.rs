use neutron_worldgen::carvers::{self, CARVE_STARTS, CARVE_WRITES};
use neutron_worldgen::region_buf::RegionBuf;
use neutron_worldgen::surface::BlockId;
use neutron_worldgen::{generator::WORLD_BOTTOM, ChunkGenerator};
use std::sync::atomic::Ordering;

fn main() {
    let gen = ChunkGenerator::new(12345);
    let cx = 6i32;
    let cz = -2i32;
    // Build 3x3 noise+surface without carvers by generating each chunk and...
    // generate_chunk includes carvers. Work around: generate, count air,
    // OR build region from generate_chunk of each cell then re-fill noise-only.
    //
    // Better approach: generate full chunk, then also generate solid and apply
    // only for target counting writes into each local cell of region by
    // snapshotting before/after apply on a noise-filled region.
    //
    // Use generate_chunk for each of 3x3 — includes carvers already. Instead:
    // manually build by calling the same path as generator.
    // We'll monkey: put_chunk from generate_chunk, then OVERWRITE by
    // re-running noise... can't.
    //
    // Simplest proof: for each target in ±0 only, solid fill, apply, count writes.
    // Also for each of 9 positions as if they were the target alone.
    for (tcx, tcz) in [
        (6, -2),
        (5, -2),
        (6, -1),
        (7, -2),
        (6, -3),
        (0, 0),
        (10, 10),
    ] {
        let mut region = RegionBuf::new(tcx, tcz, 0);
        for y in WORLD_BOTTOM..320 {
            for z in 0..16 {
                for x in 0..16 {
                    region.set(tcx * 16 + x, y, tcz * 16 + z, BlockId::Stone);
                }
            }
        }
        CARVE_STARTS.store(0, Ordering::Relaxed);
        CARVE_WRITES.store(0, Ordering::Relaxed);
        carvers::apply_carvers_region(&mut region, 12345);
        let mut air = 0u32;
        for y in WORLD_BOTTOM..320 {
            for z in 0..16 {
                for x in 0..16 {
                    if matches!(
                        region.get(tcx * 16 + x, y, tcz * 16 + z),
                        BlockId::Air | BlockId::Lava
                    ) {
                        air += 1;
                    }
                }
            }
        }
        println!(
            "target({tcx},{tcz}): starts={} writes={} air={}",
            CARVE_STARTS.load(Ordering::Relaxed),
            CARVE_WRITES.load(Ordering::Relaxed),
            air
        );
    }

    // Force local start: brute check isStart for many seeds
    use neutron_worldgen::legacy_rng::LegacyRandom;
    let mut local_starts = 0u32;
    for index in 0..3i64 {
        let mut rng = LegacyRandom::new(0);
        rng.set_large_feature_seed(12345i64.wrapping_add(index), 6, -2);
        let f = rng.next_f32();
        let p = [0.15f32, 0.07, 0.01][index as usize];
        println!(
            "local index={index} nextFloat={f:.6} prob={p} start={}",
            f <= p
        );
        if f <= p {
            local_starts += 1;
        }
    }
    println!("local_starts={local_starts}");

    // How many of ±2 sources start?
    let mut near = 0u32;
    for dz in -2..=2i32 {
        for dx in -2..=2i32 {
            for index in 0..2i64 {
                let mut rng = LegacyRandom::new(0);
                rng.set_large_feature_seed(12345i64.wrapping_add(index), 6 + dx, -2 + dz);
                let f = rng.next_f32();
                let p = if index == 0 { 0.15f32 } else { 0.07 };
                if f <= p {
                    near += 1;
                }
            }
        }
    }
    println!("near ±2 starts={near}");
}
