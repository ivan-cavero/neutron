// Extra dark oak: trunk bases + leaf clusters vs vanilla.
// cargo run -p neutron-worldgen --example tree_gap --release

use neutron_world::nbt::ussr_nbt::owned::{List, Tag};
use neutron_world::nbt::{compound_get, read_nbt};
use neutron_world::Region;
use neutron_worldgen::surface::BlockId;
use neutron_worldgen::{generator::WORLD_BOTTOM, ChunkGenerator};
use std::path::PathBuf;

fn load_vanilla(path: &str) -> Vec<String> {
    let path = PathBuf::from(path);
    let region = Region::open(&path).unwrap().with_coords(0, -1);
    let data = region.get_chunk(6, 30).unwrap().unwrap();
    let nbt = read_nbt(&data).unwrap();
    let sections = match compound_get(&nbt.compound, "sections") {
        Some(Tag::List(List::Compound(l))) => l,
        _ => panic!(),
    };
    let mut van = vec!["air".to_string(); 16 * 384 * 16];
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
                Some(Tag::String(s)) => s
                    .to_string()
                    .strip_prefix("minecraft:")
                    .unwrap_or(&s.to_string())
                    .to_string(),
                _ => "air".into(),
            })
            .collect();
        let nstates = names.len();
        if nstates == 1 {
            for i in 0..4096u32 {
                let ly = (i >> 8) as i32;
                let lz = ((i >> 4) & 15) as usize;
                let lx = (i & 15) as usize;
                let y = y_sec * 16 + ly;
                let idx = ((y - WORLD_BOTTOM) as usize) * 256 + lz * 16 + lx;
                van[idx] = names[0].clone();
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
            let lz = ((i >> 4) & 15) as usize;
            let lx = (i & 15) as usize;
            let y = y_sec * 16 + ly;
            let idx = ((y - WORLD_BOTTOM) as usize) * 256 + lz * 16 + lx;
            van[idx] = names.get(idxp as usize).cloned().unwrap_or_else(|| "air".into());
        }
    }
    van
}

fn idx(x: usize, y: i32, z: usize) -> usize {
    ((y - WORLD_BOTTOM) as usize) * 256 + z * 16 + x
}

fn trunk_bases(is_log: impl Fn(usize, i32, usize) -> bool) -> Vec<(usize, i32, usize)> {
    let mut out = Vec::new();
    for y in (WORLD_BOTTOM + 1)..320 {
        for z in 0..15usize {
            for x in 0..15usize {
                if is_log(x, y, z)
                    && is_log(x + 1, y, z)
                    && is_log(x, y, z + 1)
                    && is_log(x + 1, y, z + 1)
                    && !is_log(x, y - 1, z)
                {
                    out.push((x, y, z));
                }
            }
        }
    }
    out
}

fn main() {
    let path = std::env::args().nth(1).unwrap_or_else(|| {
        "tools/nbt-ref/vanilla1/world/dimensions/minecraft/overworld/region/r.0.-1.mca"
            .into()
    });
    let van = load_vanilla(&path);
    let chunk = ChunkGenerator::new(12345).generate_chunk(6, -2);

    let van_logs = trunk_bases(|x, y, z| van[idx(x, y, z)].contains("dark_oak_log"));
    let neu_logs = trunk_bases(|x, y, z| {
        chunk.block_at(x as u32, y, z as u32) == BlockId::DarkOakLog
    });

    let mut vleaf = 0u32;
    let mut nleaf = 0u32;
    let mut vlog = 0u32;
    let mut nlog = 0u32;
    for y in WORLD_BOTTOM..320 {
        for z in 0..16u32 {
            for x in 0..16u32 {
                let vn = van[idx(x as usize, y, z as usize)].as_str();
                let nb = chunk.block_at(x, y, z);
                if vn.contains("dark_oak_leaves") {
                    vleaf += 1;
                }
                if vn.contains("dark_oak_log") {
                    vlog += 1;
                }
                if nb == BlockId::DarkOakLeaves {
                    nleaf += 1;
                }
                if nb == BlockId::DarkOakLog {
                    nlog += 1;
                }
            }
        }
    }
    println!("vanilla  leaves={vleaf} logs={vlog} trunks_2x2={}", van_logs.len());
    println!("neutron  leaves={nleaf} logs={nlog} trunks_2x2={}", neu_logs.len());
    println!("van trunks: {van_logs:?}");
    println!("neu trunks: {neu_logs:?}");
    let van_set: std::collections::HashSet<_> = van_logs.iter().copied().collect();
    let extra: Vec<_> = neu_logs.iter().filter(|p| !van_set.contains(p)).copied().collect();
    let miss: Vec<_> = van_logs.iter().filter(|p| !neu_logs.contains(p)).copied().collect();
    println!("extra trunks: {extra:?}");
    println!("miss trunks:  {miss:?}");
}
