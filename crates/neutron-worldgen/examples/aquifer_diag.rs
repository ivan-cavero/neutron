// Aquifer diagnostics: Y histogram of neutron-water-vs-vanilla-air mismatches.
// Usage: cargo run -p neutron-worldgen --example aquifer_diag -- [seed] [cx] [cz] [region_dir]

use neutron_world::nbt::ussr_nbt::owned::{List, Tag};
use neutron_world::nbt::{compound_get, read_nbt};
use neutron_world::Region;
use neutron_worldgen::surface::BlockId;
use neutron_worldgen::{generator::WORLD_BOTTOM, ChunkGenerator};
use std::collections::BTreeMap;
use std::path::PathBuf;

fn main() {
    let seed: i64 = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(424242);
    let cx: i32 = std::env::args()
        .nth(2)
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    let cz: i32 = std::env::args()
        .nth(3)
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    let region_dir = std::env::args().nth(4).unwrap_or_else(|| {
        "tools/nbt-ref/vanilla-fresh-424242/world/dimensions/minecraft/overworld/region".to_string()
    });

    let rx = cx.div_euclid(32);
    let rz = cz.div_euclid(32);
    let lcx = cx.rem_euclid(32);
    let lcz = cz.rem_euclid(32);
    let path = PathBuf::from(format!("{region_dir}/r.{rx}.{rz}.mca"));
    let region = Region::open(&path).expect("open region").with_coords(rx, rz);
    let data = region
        .get_chunk(lcx, lcz)
        .expect("get")
        .expect("chunk present");
    let nbt = read_nbt(&data).expect("nbt");
    let sections = match compound_get(&nbt.compound, "sections") {
        Some(Tag::List(List::Compound(list))) => list,
        _ => panic!("no sections"),
    };

    let mut vanilla: std::collections::HashMap<(u8, i32, u8), String> = std::collections::HashMap::new();
    for sec in sections {
        let y_sec = match compound_get(sec, "Y") {
            Some(Tag::Byte(y)) => *y as i8 as i32,
            Some(Tag::Int(y)) => *y,
            _ => continue,
        };
        let Some(Tag::Compound(bs)) = compound_get(sec, "block_states") else {
            continue;
        };
        let Some(Tag::List(List::Compound(palette))) = compound_get(bs, "palette") else {
            continue;
        };
        let names: Vec<String> = palette
            .iter()
            .map(|pc| match compound_get(pc, "Name") {
                Some(Tag::String(s)) => s.to_string(),
                _ => "minecraft:air".into(),
            })
            .collect();
        let nstates = names.len();
        if nstates == 1 {
            for i in 0..4096u32 {
                let ly = (i >> 8) as i32;
                let lz = ((i >> 4) & 15) as u8;
                let lx = (i & 15) as u8;
                vanilla.insert((lx, y_sec * 16 + ly, lz), names[0].clone());
            }
            continue;
        }
        let bits = ((nstates - 1).ilog2() + 1).max(4) as u32;
        let Some(Tag::LongArray(data)) = compound_get(bs, "data") else {
            continue;
        };
        let longs: Vec<i64> = data.to_vec();
        let epl = 64 / bits;
        let mask = (1u64 << bits) - 1;
        for i in 0..4096u32 {
            let li = (i / epl) as usize;
            let bo = (i % epl) * bits;
            let idx = ((longs[li] as u64) >> bo) & mask;
            let ly = (i >> 8) as i32;
            let lz = ((i >> 4) & 15) as u8;
            let lx = (i & 15) as u8;
            let name = names
                .get(idx as usize)
                .cloned()
                .unwrap_or_else(|| "minecraft:air".into());
            vanilla.insert((lx, y_sec * 16 + ly, lz), name);
        }
    }

    let gen = ChunkGenerator::new(seed);
    let chunk = gen.generate_chunk(cx, cz);

    // Y histogram of (vanilla=air, neutron=water).
    let mut hist: BTreeMap<i32, u32> = BTreeMap::new();
    let mut samples: Vec<(i32, u8, u8)> = Vec::new();
    let mut total = 0u32;
    for y in WORLD_BOTTOM..320 {
        for z in 0..16u8 {
            for x in 0..16u8 {
                let nb = chunk.block_at(x as u32, y, z as u32);
                let vname = vanilla
                    .get(&(x, y, z))
                    .map(|s| s.as_str())
                    .unwrap_or("minecraft:air");
                if nb == BlockId::Water && vname == "minecraft:air" {
                    *hist.entry(y).or_insert(0) += 1;
                    total += 1;
                    if samples.len() < 8 {
                        samples.push((y, x, z));
                    }
                }
            }
        }
    }

    println!("seed={seed} chunk=({cx},{cz}) air->water total={total}");
    println!("Y range: {:?}..{:?}", hist.keys().next(), hist.keys().next_back());
    println!("\nY histogram:");
    for (y, c) in &hist {
        println!("  y={y:>4}  {c:>5}  {}", "#".repeat((*c as usize / 8).max(1)));
    }
    println!("\nSample positions (y,x,z): {samples:?}");
    println!("\nsea_level=63; wrong skip_sampling_above_y with max_prelim=0 is 34");

    // Neutron preliminary_surface_level at the vanilla probe grid (-16..=25 step 4),
    // same quantization as NoiseChunk.preliminarySurfaceLevel.
    let noises = gen.state.noises.noises();
    let mut max_prelim = i32::MIN;
    print!("\nprelim(x,z): ");
    for z in (-16..=25).step_by(4) {
        for x in (-16..=25).step_by(4) {
            let qx = ((x >> 2) << 2) as i32;
            let qz = ((z >> 2) << 2) as i32;
            let mut env = neutron_worldgen::density::DensityEnv::new(qx, 0, qz, noises);
            let v = neutron_worldgen::density::compute(
                &gen.state.router.preliminary_surface_level,
                &mut env,
            )
            .floor() as i32;
            max_prelim = max_prelim.max(v);
        }
    }
    let max_adjusted = max_prelim + 8;
    let skip_grid_y = (max_adjusted + 12).div_euclid(12) + 1;
    let skip_y = skip_grid_y * 12 + 11 - 1;
    println!("maxPreliminarySurfaceLevel={max_prelim} skipSamplingAboveY={skip_y} (vanilla probe: max=96 skip=130)");
}
