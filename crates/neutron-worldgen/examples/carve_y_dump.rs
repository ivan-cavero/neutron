//! Seed 424242: carver start Ys and before/after air/water y-bands for
//! chunks (0,0) and (0,1). Changed-block Y histogram as ellipsoid-write proxy.
//!
//!   cargo run --release -p neutron-worldgen --example carve_y_dump

use neutron_worldgen::carvers::{
    apply_carvers_region, CARVE_CAN_REACH_FAIL, CARVE_EARLY_OUT, CARVE_ELLIPSOIDS,
    CARVE_ELLIPSOID_HIT, CARVE_EMPTY_RANGE, CARVE_ROOM_CALLS, CARVE_STARTS, CARVE_TARGET_WRITES,
    CARVE_TUNNEL_STEPS, CARVE_WRITES,
};
use neutron_worldgen::generator::{ChunkGenerator, WORLD_BOTTOM, WORLD_TOP};
use neutron_worldgen::legacy_rng::LegacyRandom;
use neutron_worldgen::region_buf::RegionBuf;
use neutron_worldgen::surface::{vanilla_name, BlockId};
use std::sync::atomic::Ordering;

const APPLY_RANGE: i32 = 8;
const SEED: i64 = 424242;

struct CaveCfg {
    name: &'static str,
    probability: f32,
    y_min: i32,
    y_max: i32,
}

fn sample_y(rng: &mut LegacyRandom, y_min: i32, y_max: i32) -> i32 {
    if y_max <= y_min {
        return y_min;
    }
    y_min + rng.next_int(y_max - y_min + 1)
}

fn band5(y: i32) -> usize {
    if (-32..-16).contains(&y) {
        0
    } else if (-16..0).contains(&y) {
        1
    } else if (0..16).contains(&y) {
        2
    } else if (16..32).contains(&y) {
        3
    } else {
        4
    }
}

const BAND5_LABELS: [&str; 5] = [
    "[-32,-16)",
    "[-16,0)",
    "[0,16)",
    "[16,32)",
    "elsewhere",
];

fn bin16(y: i32) -> i32 {
    y.div_euclid(16) * 16
}

fn dest_kind(b: BlockId) -> usize {
    match b {
        BlockId::Air => 0,
        BlockId::Water => 1,
        BlockId::Lava => 2,
        _ => 3,
    }
}

fn count_chunk_bands(region: &RegionBuf, cx: i32, cz: i32) -> ([u32; 5], [u32; 5], u32, u32, u32) {
    let mut air_b = [0u32; 5];
    let mut water_b = [0u32; 5];
    let mut air = 0u32;
    let mut water = 0u32;
    let mut lava = 0u32;
    for y in WORLD_BOTTOM..WORLD_TOP {
        let bi = band5(y);
        for z in 0..16 {
            for x in 0..16 {
                match region.get(cx * 16 + x, y, cz * 16 + z) {
                    BlockId::Air => {
                        air += 1;
                        air_b[bi] += 1;
                    }
                    BlockId::Water => {
                        water += 1;
                        water_b[bi] += 1;
                    }
                    BlockId::Lava => lava += 1,
                    _ => {}
                }
            }
        }
    }
    (air_b, water_b, air, water, lava)
}

fn print_chunk_bands(label: &str, cx: i32, cz: i32, region: &RegionBuf) {
    let (air_b, water_b, air, water, lava) = count_chunk_bands(region, cx, cz);
    println!("  {label} chunk ({cx},{cz}): air={air} water={water} lava={lava}");
    for i in 0..5 {
        println!(
            "    y{:<12} air={:<6} water={}",
            BAND5_LABELS[i], air_b[i], water_b[i]
        );
    }
    let air_n32_0 = air_b[0] + air_b[1];
    let water_n32_0 = water_b[0] + water_b[1];
    println!("    y[-32,0)      air={air_n32_0:<6} water={water_n32_0}");
}

/// Unique isStartChunk hits whose source is in APPLY_RANGE of (0,0) or (0,1).
/// Samples start Y the same way as `carve_from_chunk` / `canyon_from_chunk`.
fn dump_starts(level_seed: i64) {
    let cave_cfgs = [
        CaveCfg {
            name: "cave",
            probability: 0.15,
            y_min: WORLD_BOTTOM + 8,
            y_max: 180,
        },
        CaveCfg {
            name: "cave_extra",
            probability: 0.07,
            y_min: WORLD_BOTTOM + 8,
            y_max: 47,
        },
    ];

    println!("=== unique starts that can reach target (0,0) or (0,1) ===");
    println!("APPLY_RANGE={APPLY_RANGE}  source union cx=-8..=8 cz=-8..=9");
    println!(
        "cave y=[{},{}] p=0.15; cave_extra y=[{},{}] p=0.07; canyon y=[10,67] p=0.01",
        WORLD_BOTTOM + 8,
        180,
        WORLD_BOTTOM + 8,
        47
    );

    let mut start_hits = [0u32; 3];
    let mut instance_ys: Vec<(usize, i32, i32, i32, i32, i32)> = Vec::new();
    let mut start_band = [0u32; 5];
    let mut start_bin: std::collections::BTreeMap<i32, u32> = std::collections::BTreeMap::new();
    let mut cave_count_zero = [0u32; 2];

    // Union of sources for targets (0,0) and (0,1).
    for source_cx in -8..=8 {
        for source_cz in -8..=9 {
            for (index, cfg) in cave_cfgs.iter().enumerate() {
                let mut rng = LegacyRandom::new(0);
                rng.set_large_feature_seed(
                    level_seed.wrapping_add(index as i64),
                    source_cx,
                    source_cz,
                );
                if rng.next_f32() > cfg.probability {
                    continue;
                }
                start_hits[index] += 1;
                let a = rng.next_int(15) + 1;
                let b = rng.next_int(a) + 1;
                let cave_count = rng.next_int(b);
                if cave_count == 0 {
                    cave_count_zero[index] += 1;
                    println!(
                        "  START {} source=({},{}) cave_count=0 (no Y)",
                        cfg.name, source_cx, source_cz
                    );
                    continue;
                }
                for _ in 0..cave_count {
                    let x = source_cx * 16 + rng.next_int(16);
                    let y = sample_y(&mut rng, cfg.y_min, cfg.y_max);
                    let z = source_cz * 16 + rng.next_int(16);
                    println!(
                        "  START {} source=({},{}) pos=({},{},{})",
                        cfg.name, source_cx, source_cz, x, y, z
                    );
                    instance_ys.push((index, source_cx, source_cz, x, y, z));
                    start_band[band5(y)] += 1;
                    *start_bin.entry(bin16(y)).or_insert(0) += 1;
                }
            }
            {
                let mut rng = LegacyRandom::new(0);
                rng.set_large_feature_seed(level_seed.wrapping_add(2), source_cx, source_cz);
                if rng.next_f32() <= 0.01 {
                    start_hits[2] += 1;
                    let x = source_cx * 16 + rng.next_int(16);
                    let y = 10 + rng.next_int(67 - 10 + 1);
                    let z = source_cz * 16 + rng.next_int(16);
                    println!(
                        "  START canyon source=({},{}) pos=({},{},{})",
                        source_cx, source_cz, x, y, z
                    );
                    instance_ys.push((2, source_cx, source_cz, x, y, z));
                    start_band[band5(y)] += 1;
                    *start_bin.entry(bin16(y)).or_insert(0) += 1;
                }
            }
        }
    }

    println!();
    println!(
        "isStartChunk hits: cave={} cave_extra={} canyon={} (total={})",
        start_hits[0],
        start_hits[1],
        start_hits[2],
        start_hits[0] + start_hits[1] + start_hits[2]
    );
    println!(
        "isStart with cave_count=0: cave={} cave_extra={}",
        cave_count_zero[0], cave_count_zero[1]
    );
    println!(
        "sampled start instances (caves with Y + canyons): {}",
        instance_ys.len()
    );
    println!("start Y histogram (sampled instances):");
    for i in 0..5 {
        println!("  y{:<12} starts={}", BAND5_LABELS[i], start_band[i]);
    }
    let in_n32_0 = start_band[0] + start_band[1];
    println!("  y[-32,0)      starts={in_n32_0}");
    println!("start Y 16-high bins:");
    for (lo, n) in &start_bin {
        println!("  y[{},{}) n={}", lo, lo + 16, n);
    }
    if let (Some(min_y), Some(max_y)) = (
        instance_ys.iter().map(|t| t.4).min(),
        instance_ys.iter().map(|t| t.4).max(),
    ) {
        println!("start Y min={min_y} max={max_y}");
    }

    let n32: Vec<_> = instance_ys
        .iter()
        .filter(|t| t.4 >= -32 && t.4 < 0)
        .collect();
    println!("starts with Y in [-32,0): {}", n32.len());
    for (kind, scx, scz, x, y, z) in &n32 {
        let name = ["cave", "cave_extra", "canyon"][*kind];
        println!("    {name} source=({scx},{scz}) pos=({x},{y},{z})");
    }
}

fn dump_changes(before: &RegionBuf, after: &RegionBuf) {
    println!("=== CHANGED blocks after apply_carvers_region (ellipsoid-write proxy) ===");

    #[derive(Clone)]
    struct Acc {
        n: u32,
        band_dest: [[u32; 4]; 5],
        bin: std::collections::BTreeMap<i32, [u32; 4]>,
        dest: [u32; 4],
        from: std::collections::BTreeMap<String, u32>,
    }
    impl Acc {
        fn new() -> Self {
            Self {
                n: 0,
                band_dest: [[0; 4]; 5],
                bin: std::collections::BTreeMap::new(),
                dest: [0; 4],
                from: std::collections::BTreeMap::new(),
            }
        }
        fn add(&mut self, y: i32, old: BlockId, new: BlockId) {
            self.n += 1;
            let di = dest_kind(new);
            self.dest[di] += 1;
            self.band_dest[band5(y)][di] += 1;
            self.bin.entry(bin16(y)).or_insert([0; 4])[di] += 1;
            *self.from.entry(vanilla_name(old).to_string()).or_insert(0) += 1;
        }
    }

    let mut by_chunk: std::collections::BTreeMap<(i32, i32), Acc> =
        std::collections::BTreeMap::new();
    let mut region_acc = Acc::new();
    let mut y_m32_0_lines = Vec::new();

    for y in WORLD_BOTTOM..WORLD_TOP {
        for z in before.origin_z..before.origin_z + before.side {
            for x in before.origin_x..before.origin_x + before.side {
                let old = before.get(x, y, z);
                let new = after.get(x, y, z);
                if old == new {
                    continue;
                }
                let cx = x.div_euclid(16);
                let cz = z.div_euclid(16);
                region_acc.add(y, old, new);
                by_chunk.entry((cx, cz)).or_insert_with(Acc::new).add(y, old, new);
                if (cx == 0 && (cz == 0 || cz == 1)) && (-32..0).contains(&y) {
                    y_m32_0_lines.push(format!(
                        "    ({x},{y},{z}) {} -> {}",
                        vanilla_name(old),
                        vanilla_name(new)
                    ));
                }
            }
        }
    }

    fn print_acc(label: &str, a: &Acc) {
        println!("  {label}: changed={}", a.n);
        println!(
            "    dest air={} water={} lava={} other={}",
            a.dest[0], a.dest[1], a.dest[2], a.dest[3]
        );
        for i in 0..5 {
            let row = a.band_dest[i];
            let tot: u32 = row.iter().sum();
            println!(
                "    y{:<12} writes={} air={} water={} lava={} other={}",
                BAND5_LABELS[i], tot, row[0], row[1], row[2], row[3]
            );
        }
        let n32: u32 = a.band_dest[0].iter().sum::<u32>() + a.band_dest[1].iter().sum::<u32>();
        println!("    y[-32,0)      writes={n32}");
        println!("    16-high bins (dest air/water/lava/other):");
        for (lo, row) in &a.bin {
            let tot: u32 = row.iter().sum();
            println!(
                "      y[{},{}) n={} air={} water={} lava={} other={}",
                lo,
                lo + 16,
                tot,
                row[0],
                row[1],
                row[2],
                row[3]
            );
        }
        if !a.from.is_empty() {
            print!("    from:");
            for (name, n) in &a.from {
                print!(" {name}={n}");
            }
            println!();
        }
    }

    print_acc("whole 3x3 region", &region_acc);
    for (cx, cz) in [(0, 0), (0, 1)] {
        let a = by_chunk.get(&(cx, cz)).cloned();
        match a {
            Some(acc) => print_acc(&format!("chunk ({cx},{cz})"), &acc),
            None => {
                println!("  chunk ({cx},{cz}): changed=0");
                println!("    y[-32,-16) writes=0");
                println!("    y[-16,0)   writes=0");
                println!("    y[0,16)    writes=0");
                println!("    y[16,32)   writes=0");
                println!("    elsewhere  writes=0");
                println!("    y[-32,0)   writes=0");
            }
        }
    }

    println!(
        "changed blocks in chunks (0,0)/(0,1) with y in [-32,0): {}",
        y_m32_0_lines.len()
    );
    for line in &y_m32_0_lines {
        println!("{line}");
    }
}

fn main() {
    println!("seed={SEED}  RegionBuf::new(0,0,1) fill dx,dz in -1..=1 then apply_carvers_region");
    println!(
        "WORLD_BOTTOM={WORLD_BOTTOM} cave y_min=WORLD_BOTTOM+8={} lava_y={}",
        WORLD_BOTTOM + 8,
        WORLD_BOTTOM + 8
    );

    dump_starts(SEED);

    let gen = ChunkGenerator::new(SEED);
    let mut region = RegionBuf::new(0, 0, 1);
    for dz in -1..=1 {
        for dx in -1..=1 {
            let (blocks, heightmap, _) = gen.generate_noise_and_surface(dx, dz);
            region.put_chunk(dx, dz, &blocks, &heightmap);
        }
    }

    println!();
    println!("=== BEFORE doFill+surface (no carvers) ===");
    print_chunk_bands("BEFORE", 0, 0, &region);
    print_chunk_bands("BEFORE", 0, 1, &region);

    let before = region.clone();

    CARVE_WRITES.store(0, Ordering::Relaxed);
    CARVE_STARTS.store(0, Ordering::Relaxed);
    CARVE_TARGET_WRITES.store(0, Ordering::Relaxed);
    CARVE_ELLIPSOIDS.store(0, Ordering::Relaxed);
    CARVE_ELLIPSOID_HIT.store(0, Ordering::Relaxed);
    CARVE_CAN_REACH_FAIL.store(0, Ordering::Relaxed);
    CARVE_ROOM_CALLS.store(0, Ordering::Relaxed);
    CARVE_TUNNEL_STEPS.store(0, Ordering::Relaxed);
    CARVE_EARLY_OUT.store(0, Ordering::Relaxed);
    CARVE_EMPTY_RANGE.store(0, Ordering::Relaxed);

    apply_carvers_region(&mut region, &gen.state);

    println!();
    println!("=== AFTER apply_carvers_region ===");
    print_chunk_bands("AFTER", 0, 0, &region);
    print_chunk_bands("AFTER", 0, 1, &region);

    println!();
    println!("=== BEFORE vs AFTER delta air/water ===");
    for (cx, cz) in [(0, 0), (0, 1)] {
        let (ba, bw, bair, bwater, _) = count_chunk_bands(&before, cx, cz);
        let (aa, aw, aair, awater, _) = count_chunk_bands(&region, cx, cz);
        println!(
            "  chunk ({cx},{cz}): air {bair}->{aair} (d={}), water {bwater}->{awater} (d={})",
            aair as i32 - bair as i32,
            awater as i32 - bwater as i32
        );
        for i in 0..5 {
            let da = aa[i] as i32 - ba[i] as i32;
            let dw = aw[i] as i32 - bw[i] as i32;
            println!(
                "    y{:<12} dair={da:<6} dwater={dw}  after_air={} after_water={}",
                BAND5_LABELS[i], aa[i], aw[i]
            );
        }
        let dair_n32 = (aa[0] + aa[1]) as i32 - (ba[0] + ba[1]) as i32;
        println!("    y[-32,0)      dair={dair_n32}");
    }

    println!();
    dump_changes(&before, &region);

    println!();
    println!("=== CARVE_* atomics (9 targets in 3x3 region; STARTS counted per target) ===");
    println!("CARVE_STARTS={}", CARVE_STARTS.load(Ordering::Relaxed));
    println!("CARVE_WRITES={}", CARVE_WRITES.load(Ordering::Relaxed));
    println!(
        "CARVE_TARGET_WRITES={}",
        CARVE_TARGET_WRITES.load(Ordering::Relaxed)
    );
    println!("CARVE_ELLIPSOIDS={}", CARVE_ELLIPSOIDS.load(Ordering::Relaxed));
    println!(
        "CARVE_ELLIPSOID_HIT={}",
        CARVE_ELLIPSOID_HIT.load(Ordering::Relaxed)
    );
    println!(
        "CARVE_CAN_REACH_FAIL={}",
        CARVE_CAN_REACH_FAIL.load(Ordering::Relaxed)
    );
    println!("CARVE_ROOM_CALLS={}", CARVE_ROOM_CALLS.load(Ordering::Relaxed));
    println!(
        "CARVE_TUNNEL_STEPS={}",
        CARVE_TUNNEL_STEPS.load(Ordering::Relaxed)
    );
    println!("CARVE_EARLY_OUT={}", CARVE_EARLY_OUT.load(Ordering::Relaxed));
    println!(
        "CARVE_EMPTY_RANGE={}",
        CARVE_EMPTY_RANGE.load(Ordering::Relaxed)
    );
}
