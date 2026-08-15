// Multi-chunk parity: neutron vs vanilla fresh reference across a chunk
// radius, with core/border split (border cells carry the vanilla thread-
// scheduling noise; core must be deterministic).
// Usage: region_parity [seed] [cx] [cz] [radius] [region_dir]
use neutron_world::nbt::ussr_nbt::owned::{List, Tag};
use neutron_world::nbt::{compound_get, read_nbt};
use neutron_world::Region;
use neutron_worldgen::surface::{is_vegetation_name, vanilla_name, BlockId};
use neutron_worldgen::ChunkGenerator;
use std::collections::HashMap;
use std::path::PathBuf;

fn load_vanilla_chunk(
    regions: &mut HashMap<(i32, i32), Region>,
    region_dir: &str,
    cx: i32,
    cz: i32,
) -> Option<HashMap<(u8, i32, u8), String>> {
    let (rx, rz) = (cx >> 5, cz >> 5);
    let key = (rx, rz);
    if !regions.contains_key(&key) {
        let path = PathBuf::from(format!("{region_dir}/r.{rx}.{rz}.mca"));
        let region = Region::open(&path).ok()?.with_coords(rx, rz);
        regions.insert(key, region);
    }
    let region = regions.get(&key)?;
    let data = region.get_chunk(cx & 31, cz & 31).ok()??;
    let nbt = read_nbt(&data).ok()?;
    let sections = match compound_get(&nbt.compound, "sections") {
        Some(Tag::List(List::Compound(l))) => l,
        _ => return None,
    };
    let mut map = HashMap::new();
    for sec in sections {
        let y_sec = match compound_get(sec, "Y") {
            Some(Tag::Byte(y)) => *y as i8 as i32,
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
                map.insert((lx, y_sec * 16 + ly, lz), names[0].clone());
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
            let idxp = ((longs[li] as u64) >> bo) & mask;
            let ly = (i >> 8) as i32;
            let lz = ((i >> 4) & 15) as u8;
            let lx = (i & 15) as u8;
            let name = names.get(idxp as usize).cloned().unwrap_or_default();
            map.insert((lx, y_sec * 16 + ly, lz), name);
        }
    }
    Some(map)
}

fn main() {
    let mut args = std::env::args().skip(1);
    let seed: i64 = args.next().and_then(|s| s.parse().ok()).unwrap_or(12345);
    let cx: i32 = args.next().and_then(|s| s.parse().ok()).unwrap_or(6);
    let cz: i32 = args.next().and_then(|s| s.parse().ok()).unwrap_or(-2);
    let radius: i32 = args.next().and_then(|s| s.parse().ok()).unwrap_or(1);
    let region_dir = args.next().unwrap_or_else(|| {
        "tools/nbt-ref/vanilla-fresh-12345/world/dimensions/minecraft/overworld/region".to_string()
    });

    let gen = ChunkGenerator::new(seed);
    let mut regions: HashMap<(i32, i32), Region> = HashMap::new();
    println!("seed={seed} center=({cx},{cz}) radius={radius}");
    println!("{:>10} {:>9} {:>9} {:>9} {:>9}", "chunk", "ALL", "BASE", "core", "border");
    let mut tot = [0u64; 8];
    let mut chunks = 0u64;
    for dz in -radius..=radius {
        for dx in -radius..=radius {
            let (ccx, ccz) = (cx + dx, cz + dz);
            let Some(van) = load_vanilla_chunk(&mut regions, &region_dir, ccx, ccz) else {
                println!("{ccx:>5},{ccz:>4}     missing");
                continue;
            };
            let chunk = gen.generate_chunk(ccx, ccz);
            let wb = neutron_worldgen::generator::WORLD_BOTTOM;
            let wt = neutron_worldgen::generator::WORLD_TOP;
            let mut all = [0u64; 2];
            let mut base = [0u64; 2];
            let mut core = [0u64; 2];
            let mut border = [0u64; 2];
            for y in wb..wt {
                for z in 0..16u32 {
                    for x in 0..16u32 {
                        let b = chunk.block_at(x, y, z);
                        let nn = vanilla_name(b);
                        let vn = van
                            .get(&(x as u8, y, z as u8))
                            .map(|s| s.as_str())
                            .unwrap_or("minecraft:air");
                        let m = (nn == vn) as u64;
                        all[m as usize] += 1;
                        if !is_vegetation_name(vn) {
                            base[m as usize] += 1;
                        }
                        let d = (x as i32)
                            .min(15 - x as i32)
                            .min(z as i32)
                            .min(15 - z as i32);
                        if d >= 5 {
                            core[m as usize] += 1;
                        } else {
                            border[m as usize] += 1;
                        }
                    }
                }
            }
            for i in 0..2 {
                tot[i] += all[i];
            }
            chunks += 1;
            let pct = |a: [u64; 2]| 100.0 * a[1] as f64 / (a[0] + a[1]) as f64;
            println!(
                "{ccx:>5},{ccz:>4} {:>8.2}% {:>8.2}% {:>8.2}% {:>8.2}%",
                pct(all),
                pct(base),
                pct(core),
                pct(border)
            );
        }
    }
    if tot[0] + tot[1] > 0 {
        println!(
            "REGION ALL: {:.2}% over {chunks} chunks",
            100.0 * tot[1] as f64 / (tot[0] + tot[1]) as f64
        );
    }
}
