//! THROWAWAY diagnostic (not committed): generate neutron chunks around a
//! center and dump oak-family trunk clusters as JSON (mirror of
//! /tmp/opencode/tree_roots.py for the ref side).
//! Usage: neu_tree_dump <seed> <cx> <cz> <radius> <out.json>
use neutron_worldgen::surface::BlockId;
use neutron_worldgen::{ChunkGenerator, NoiseCache};
use std::collections::HashSet;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let seed: i64 = a[1].parse().unwrap();
    let cx0: i32 = a[2].parse().unwrap();
    let cz0: i32 = a[3].parse().unwrap();
    let rad: i32 = a[4].parse().unwrap();
    let out = &a[5];

    let gen = ChunkGenerator::new(seed);
    let mut logs = Vec::new();
    for dx in -rad..=rad {
        for dz in -rad..=rad {
            let mut cache = NoiseCache::new();
            let chunk = gen.generate_chunk_cached(cx0 + dx, cz0 + dz, &mut cache);
            let wb = neutron_worldgen::generator::WORLD_BOTTOM;
            for y in wb..neutron_worldgen::generator::WORLD_TOP {
                for z in 0..16u32 {
                    for x in 0..16u32 {
                        let b = chunk.block_at(x, y, z);
                        if matches!(
                            b,
                            BlockId::OakLog
                                | BlockId::DarkOakLog
                                | BlockId::PaleOakLog
                                | BlockId::BirchLog
                                | BlockId::SpruceLog
                                | BlockId::JungleLog
                                | BlockId::AcaciaLog
                                | BlockId::MangroveLog
                                | BlockId::CherryLog
                        ) {
                            logs.push((
                                (cx0 + dx) * 16 + x as i32,
                                y,
                                (cz0 + dz) * 16 + z as i32,
                                b,
                            ));
                        }
                    }
                }
            }
        }
    }

    // cluster 6-connected same-species
    let by_pos: std::collections::HashMap<(i32, i32, i32), BlockId> =
        logs.iter().map(|&(x, y, z, b)| ((x, y, z), b)).collect();
    let mut seen: HashSet<(i32, i32, i32)> = HashSet::new();
    let mut clusters = Vec::new();
    let name = |b: BlockId| -> &'static str {
        match b {
            BlockId::OakLog => "oak_log",
            BlockId::DarkOakLog => "dark_oak_log",
            BlockId::PaleOakLog => "pale_oak_log",
            BlockId::BirchLog => "birch_log",
            BlockId::SpruceLog => "spruce_log",
            _ => "other_log",
        }
    };
    for &(px, py, pz, pb) in &logs {
        if seen.contains(&(px, py, pz)) {
            continue;
        }
        let mut stack = vec![(px, py, pz)];
        seen.insert((px, py, pz));
        let mut comp = Vec::new();
        while let Some((x, y, z)) = stack.pop() {
            comp.push((x, y, z));
            for p in [
                (x + 1, y, z),
                (x - 1, y, z),
                (x, y + 1, z),
                (x, y - 1, z),
                (x, y, z + 1),
                (x, y, z - 1),
            ] {
                if !seen.contains(&p) && by_pos.get(&p) == Some(&pb) {
                    seen.insert(p);
                    stack.push(p);
                }
            }
        }
        let ys: Vec<i32> = comp.iter().map(|c| c.1).collect();
        clusters.push(serde_json::json!({
            "species": name(pb),
            "n": comp.len(),
            "y0": ys.iter().min().unwrap(),
            "y1": ys.iter().max().unwrap(),
            "x": comp.iter().map(|c| c.0).min().unwrap(),
            "z": comp.iter().map(|c| c.2).min().unwrap(),
        }));
    }
    serde_json::to_writer_pretty(
        std::fs::File::create(out).unwrap(),
        &serde_json::json!({ "count": clusters.len(), "clusters": clusters }),
    )
    .unwrap();
    println!("clusters: {}", clusters.len());
}
