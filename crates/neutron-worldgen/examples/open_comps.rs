use neutron_world::nbt::ussr_nbt::owned::{List, Tag};
use neutron_world::nbt::{compound_get, read_nbt};
use neutron_world::Region;
use std::path::PathBuf;
fn is_open(n: &str) -> bool {
    let n = n.strip_prefix("minecraft:").unwrap_or(n);
    n == "air" || n == "cave_air" || n == "water" || n == "lava" || n.contains("sculk")
}
fn main() {
    let path = PathBuf::from(
        "tools/nbt-ref/vanilla1/world/dimensions/minecraft/overworld/region/r.0.-1.mca",
    );
    let region = Region::open(&path).unwrap().with_coords(0, -1);
    let data = region.get_chunk(6, 30).unwrap().unwrap();
    let nbt = read_nbt(&data).unwrap();
    let sections = match compound_get(&nbt.compound, "sections") {
        Some(Tag::List(List::Compound(l))) => l,
        _ => panic!(),
    };
    let mut open = vec![false; 98304];
    let wb = -64i32;
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
        for i in 0..4096u32 {
            let name = if nstates == 1 {
                names[0].clone()
            } else {
                let bits = ((nstates - 1).ilog2() + 1).max(4) as u32;
                let Tag::LongArray(data) = compound_get(bs, "data").unwrap() else {
                    panic!()
                };
                let longs: Vec<i64> = data.to_vec();
                let epl = 64 / bits;
                let mask = (1u64 << bits) - 1;
                let li = (i / epl) as usize;
                let bo = (i % epl) * bits;
                let idx = ((longs[li] as u64) >> bo) & mask;
                names[idx as usize].clone()
            };
            let ly = (i >> 8) as i32;
            let lz = ((i >> 4) & 15) as usize;
            let lx = (i & 15) as usize;
            let y = y_sec * 16 + ly;
            let idx = ((y - wb) as usize) * 256 + lz * 16 + lx;
            if idx < open.len() {
                open[idx] = is_open(&name);
            }
        }
    }
    // flood fill connected open components in deep Y
    let mut seen = vec![false; open.len()];
    let mut comps = Vec::new();
    for y in wb..64 {
        for z in 0..16usize {
            for x in 0..16usize {
                let idx = ((y - wb) as usize) * 256 + z * 16 + x;
                if !open[idx] || seen[idx] {
                    continue;
                }
                let mut stack = vec![(x as i32, y, z as i32)];
                seen[idx] = true;
                let mut size = 0u32;
                let mut miny = y;
                let mut maxy = y;
                while let Some((cx, cy, cz)) = stack.pop() {
                    size += 1;
                    miny = miny.min(cy);
                    maxy = maxy.max(cy);
                    for (dx, dy, dz) in [
                        (1, 0, 0),
                        (-1, 0, 0),
                        (0, 1, 0),
                        (0, -1, 0),
                        (0, 0, 1),
                        (0, 0, -1),
                    ] {
                        let nx = cx + dx;
                        let ny = cy + dy;
                        let nz = cz + dz;
                        if nx < 0 || nx > 15 || nz < 0 || nz > 15 || ny < wb || ny >= 320 {
                            continue;
                        }
                        let nidx = ((ny - wb) as usize) * 256 + (nz as usize) * 16 + (nx as usize);
                        if open[nidx] && !seen[nidx] {
                            seen[nidx] = true;
                            stack.push((nx, ny, nz));
                        }
                    }
                }
                if size >= 10 {
                    comps.push((size, miny, maxy));
                }
            }
        }
    }
    comps.sort_by(|a, b| b.0.cmp(&a.0));
    println!("open components size>=10 in Y[-64,64): {}", comps.len());
    for c in comps.iter().take(15) {
        println!("  size={} Y=[{}..{}]", c.0, c.1, c.2);
    }
}
