// Positional sculk parity: dump vanilla vs neutron sculk* coords for one chunk.
// Usage: cargo run -p neutron-worldgen --example sculk_pos -- [seed] [cx] [cz] [region_dir]

use neutron_world::nbt::ussr_nbt::owned::{List, Tag};
use neutron_world::nbt::{compound_get, read_nbt};
use neutron_world::Region;
use neutron_worldgen::surface::BlockId;
use neutron_worldgen::{generator::WORLD_BOTTOM, ChunkGenerator};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::PathBuf;

fn main() {
    let seed: i64 = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(12345);
    let cx: i32 = std::env::args()
        .nth(2)
        .and_then(|s| s.parse().ok())
        .unwrap_or(6);
    let cz: i32 = std::env::args()
        .nth(3)
        .and_then(|s| s.parse().ok())
        .unwrap_or(-2);

    let rx = cx.div_euclid(32);
    let rz = cz.div_euclid(32);
    let lcx = cx.rem_euclid(32);
    let lcz = cz.rem_euclid(32);
    let region_dir = std::env::args().nth(4).unwrap_or_else(|| {
        "tools/nbt-ref/vanilla-fresh-12345/world/dimensions/minecraft/overworld/region".to_string()
    });
    let path = PathBuf::from(format!("{region_dir}/r.{rx}.{rz}.mca"));
    let region = Region::open(&path)
        .expect("open region")
        .with_coords(rx, rz);
    let data = region
        .get_chunk(lcx, lcz)
        .expect("get")
        .expect("chunk present");
    let nbt = read_nbt(&data).expect("nbt");

    let sections = match compound_get(&nbt.compound, "sections") {
        Some(Tag::List(List::Compound(list))) => list,
        _ => panic!("no sections"),
    };

    // (x,y,z) -> "sculk" | "sculk_vein" | "sculk_catalyst" | ...
    let mut vanilla: HashMap<(i32, i32, i32), String> = HashMap::new();
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
            if !names[0].contains("sculk") {
                continue;
            }
            for i in 0..4096u32 {
                let ly = (i >> 8) as i32;
                let lz = ((i >> 4) & 15) as i32;
                let lx = (i & 15) as i32;
                let y = y_sec * 16 + ly;
                vanilla.insert((lx, y, lz), names[0].clone());
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
            let name = names
                .get(idx as usize)
                .cloned()
                .unwrap_or_else(|| "minecraft:air".into());
            if !name.contains("sculk") {
                continue;
            }
            let ly = (i >> 8) as i32;
            let lz = ((i >> 4) & 15) as i32;
            let lx = (i & 15) as i32;
            let y = y_sec * 16 + ly;
            vanilla.insert((lx, y, lz), name);
        }
    }

    let gen = ChunkGenerator::new(seed);
    let chunk = gen.generate_chunk(cx, cz);
    let mut neutron: HashMap<(i32, i32, i32), String> = HashMap::new();
    for y in WORLD_BOTTOM..320 {
        for z in 0..16i32 {
            for x in 0..16i32 {
                let b = chunk.block_at(x as u32, y, z as u32);
                let n = neutron_worldgen::surface::vanilla_name(b);
                if n.contains("sculk") {
                    neutron.insert((x, y, z), n.to_string());
                }
            }
        }
    }

    println!("seed={seed} chunk=({cx},{cz})  vanilla={} neutron={}", vanilla.len(), neutron.len());

    // Per-Y counts
    let mut vy: BTreeMap<i32, (u32, u32, u32)> = BTreeMap::new(); // (sculk, vein, other)
    let mut ny: BTreeMap<i32, (u32, u32, u32)> = BTreeMap::new();
    for (p, n) in &vanilla {
        let e = vy.entry(p.1).or_default();
        if n.ends_with(":sculk") { e.0 += 1 } else if n.ends_with("sculk_vein") { e.1 += 1 } else { e.2 += 1 }
    }
    for (p, n) in &neutron {
        let e = ny.entry(p.1).or_default();
        if n.ends_with(":sculk") { e.0 += 1 } else if n.ends_with("sculk_vein") { e.1 += 1 } else { e.2 += 1 }
    }
    println!("\nPer-Y (sculk | vein | other):");
    let ys: HashSet<i32> = vy.keys().copied().chain(ny.keys().copied()).collect();
    let mut ys: Vec<i32> = ys.into_iter().collect();
    ys.sort();
    for y in ys {
        let v = vy.get(&y).copied().unwrap_or((0, 0, 0));
        let n = ny.get(&y).copied().unwrap_or((0, 0, 0));
        println!("  y={y:>4}  vanilla sc={:>3} vn={:>3} ot={:>3}   neutron sc={:>3} vn={:>3} ot={:>3}",
            v.0, v.1, v.2, n.0, n.1, n.2);
    }

    // Only-vanilla / only-neutron positions
    let only_v: Vec<_> = vanilla
        .keys()
        .filter(|p| !neutron.contains_key(*p))
        .collect();
    let only_n: Vec<_> = neutron
        .keys()
        .filter(|p| !vanilla.contains_key(*p))
        .collect();
    println!("\nonly-vanilla sculk* = {}", only_v.len());
    println!("only-neutron sculk* = {}", only_n.len());

    // Type-disagree (both have sculk* but different block)
    let mut type_diff = 0u32;
    for (p, n) in &neutron {
        if let Some(v) = vanilla.get(p) {
            if v != n {
                type_diff += 1;
            }
        }
    }
    println!("type-disagree = {type_diff}");

    // Group only_v by Y to see if it is a layer shift
    let mut ov_by_y: BTreeMap<i32, u32> = BTreeMap::new();
    for p in &only_v {
        *ov_by_y.entry(p.1).or_default() += 1;
    }
    let mut on_by_y: BTreeMap<i32, u32> = BTreeMap::new();
    for p in &only_n {
        *on_by_y.entry(p.1).or_default() += 1;
    }
    println!("\nonly-vanilla by Y: {ov_by_y:?}");
    println!("only-neutron by Y: {on_by_y:?}");

    // Connected components (6-neighborhood) per set
    let patches_v = patches(&vanilla);
    let patches_n = patches(&neutron);
    println!("\nvanilla patches: {}", patches_v.len());
    for (i, p) in patches_v.iter().enumerate() {
        let (b0, b1) = bbox(p);
        let kinds = count_kinds(p, &vanilla);
        println!("  V{i}: n={} bbox=({},{},{})..({},{},{}) {kinds:?}", p.len(),
            b0.0, b0.1, b0.2, b1.0, b1.1, b1.2);
    }
    println!("neutron patches: {}", patches_n.len());
    for (i, p) in patches_n.iter().enumerate() {
        let (b0, b1) = bbox(p);
        let kinds = count_kinds(p, &neutron);
        println!("  N{i}: n={} bbox=({},{},{})..({},{},{}) {kinds:?}", p.len(),
            b0.0, b0.1, b0.2, b1.0, b1.1, b1.2);
    }

    // Full coordinate dumps (local coords; world = (cx*16+x, y, cz*16+z))
    if std::env::var_os("SCULK_POS_DUMP").is_some() {
        println!("\nvanilla sculk coords (local):");
        let mut vs: Vec<_> = vanilla.iter().collect();
        vs.sort();
        for (p, n) in vs {
            println!("  ({},{},{}) {}  world=({},{},{})", p.0, p.1, p.2, n,
                cx * 16 + p.0, p.1, cz * 16 + p.2);
        }
        println!("neutron sculk coords (local):");
        let mut ns: Vec<_> = neutron.iter().collect();
        ns.sort();
        for (p, n) in ns {
            println!("  ({},{},{}) {}  world=({},{},{})", p.0, p.1, p.2, n,
                cx * 16 + p.0, p.1, cz * 16 + p.2);
        }
    }
}

fn patches(map: &HashMap<(i32, i32, i32), String>) -> Vec<Vec<(i32, i32, i32)>> {
    let set: HashSet<&(i32, i32, i32)> = map.keys().collect();
    let mut seen: HashSet<(i32, i32, i32)> = HashSet::new();
    let mut out = Vec::new();
    for p in map.keys() {
        if seen.contains(p) {
            continue;
        }
        let mut comp = Vec::new();
        let mut stack = vec![*p];
        seen.insert(*p);
        while let Some(c) = stack.pop() {
            comp.push(c);
            for d in [
                (1i32, 0, 0),
                (-1, 0, 0),
                (0, 1, 0),
                (0, -1, 0),
                (0, 0, 1),
                (0, 0, -1),
            ] {
                let n = (c.0 + d.0, c.1 + d.1, c.2 + d.2);
                if set.contains(&n) && !seen.contains(&n) {
                    seen.insert(n);
                    stack.push(n);
                }
            }
        }
        out.push(comp);
    }
    out.sort_by(|a, b| b.len().cmp(&a.len()));
    out
}

fn bbox(p: &[(i32, i32, i32)]) -> ((i32, i32, i32), (i32, i32, i32)) {
    let mut b0 = (i32::MAX, i32::MAX, i32::MAX);
    let mut b1 = (i32::MIN, i32::MIN, i32::MIN);
    for c in p {
        b0.0 = b0.0.min(c.0);
        b0.1 = b0.1.min(c.1);
        b0.2 = b0.2.min(c.2);
        b1.0 = b1.0.max(c.0);
        b1.1 = b1.1.max(c.1);
        b1.2 = b1.2.max(c.2);
    }
    (b0, b1)
}

fn count_kinds(p: &[(i32, i32, i32)], map: &HashMap<(i32, i32, i32), String>) -> (u32, u32, u32) {
    let mut s = (0, 0, 0);
    for c in p {
        let n = map.get(c).map(|s| s.as_str()).unwrap_or("");
        if n.ends_with(":sculk") { s.0 += 1 } else if n.ends_with("sculk_vein") { s.1 += 1 } else { s.2 += 1 }
    }
    s
}
