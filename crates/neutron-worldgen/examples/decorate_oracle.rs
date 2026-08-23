//! Vanilla decoration oracle bridge.
//!
//! 1. Exports the 5×5 pre-decoration snapshot (NDEC1 format) for
//!    tools/worldgen-probe/src/ProbeDecorate.java.
//! 2. Runs neutron's full decoration and dumps the final center chunk.
//! 3. With --compare <vanillaWritesLog>, diffs vanilla's recorded writes
//!    (last-writer-wins) against our final blocks cell-by-cell.
//!
//! Usage:
//!   cargo run -r -p neutron-worldgen --example decorate_oracle -- \
//!     <seed> <ccx> <ccz> <outPrefix> [--compare <vanillaLog>]
use neutron_worldgen::{ChunkGenerator, NoiseCache};
use std::io::Write;

fn main() {
    let mut args = std::env::args().skip(1);
    let seed: i64 = args.next().unwrap().parse().unwrap();
    let ccx: i32 = args.next().unwrap().parse().unwrap();
    let ccz: i32 = args.next().unwrap().parse().unwrap();
    let prefix = args.next().unwrap();
    let mut compare: Option<String> = None;
    while let Some(a) = args.next() {
        if a == "--compare" {
            compare = Some(args.next().expect("log path"));
        }
    }

    let gen = ChunkGenerator::new(seed);
    let (chunk_pals, chunk_idxs, bios, global_pal) =
        gen.export_predecorate(ccx, ccz);

    // biome name table by neutron biome id (sparse ids -> dense indices)
    let mut dense: Vec<String> = Vec::new();
    let mut remap: std::collections::HashMap<u8, u8> = std::collections::HashMap::new();
    for id in 0u8..=64 {
        let n = neutron_worldgen::feature_dispatch::biome_id_to_name(id).to_string();
        let next = match dense.iter().position(|x| x == &n) {
            Some(p) => p as u8,
            None => {
                dense.push(n);
                (dense.len() - 1) as u8
            }
        };
        remap.insert(id, next);
    }
    // rewrite grids through remap
    let bios: Vec<[u8; 1536]> = bios
        .iter()
        .map(|g| {
            let mut o = [0u8; 1536];
            for (i, v) in g.iter().enumerate() {
                o[i] = remap.get(v).copied().unwrap_or(0);
            }
            o
        })
        .collect();

    // ---- NDEC1 ----
    let dump_path = format!("{prefix}.ndec");
    {
        let mut f = std::fs::File::create(&dump_path).unwrap();
        f.write_all(b"NDEC1").unwrap();
        f.write_all(&seed.to_le_bytes()).unwrap();
        f.write_all(&ccx.to_le_bytes()).unwrap();
        f.write_all(&ccz.to_le_bytes()).unwrap();
        f.write_all(&(dense.len() as u16).to_le_bytes()).unwrap();
        for n in &dense {
            f.write_all(&(n.len() as u16).to_le_bytes()).unwrap();
            f.write_all(n.as_bytes()).unwrap();
        }
        for ci in 0..25 {
            // every chunk references the FINAL global palette (indices are global)
            let pal = &global_pal;
            f.write_all(&(pal.len() as u16).to_le_bytes()).unwrap();
            for n in pal {
                f.write_all(&(n.len() as u16).to_le_bytes()).unwrap();
                f.write_all(n.as_bytes()).unwrap();
            }
            for v in &chunk_idxs[ci] {
                f.write_all(&v.to_le_bytes()).unwrap();
            }
            f.write_all(bios[ci].as_slice()).unwrap();
        }
    }
    println!("wrote {}", dump_path);

    // ---- neutron final center chunk ----
    let mut cache = NoiseCache::new();
    let chunk = gen.generate_chunk_cached(ccx, ccz, &mut cache);
    let fin_path = format!("{prefix}.neufinal");
    {
        let mut f = std::fs::File::create(&fin_path).unwrap();
        for y in -64..320 {
            for lz in 0..16u32 {
                for lx in 0..16u32 {
                    let b = chunk.block_at(lx, y, lz);
                    f.write_all(&(b as u16).to_le_bytes()).unwrap();
                }
            }
        }
    }
    println!("wrote {}", fin_path);

    // ---- compare ----
    let Some(van_log) = compare else { return };
    let content = std::fs::read_to_string(&van_log).unwrap();
    let mut last: std::collections::HashMap<(i32, i32, i32), String> =
        std::collections::HashMap::new();
    for line in content.lines() {
        let p: Vec<&str> = line.split('|').collect();
        if p.len() >= 4 {
            last.insert(
                (p[0].parse().unwrap(), p[1].parse().unwrap(), p[2].parse().unwrap()),
                p[3].to_string(),
            );
        }
    }
    println!("vanilla written cells (unique): {}", last.len());
    // track which ORIGIN wrote each cell last
    let mut last_origin: std::collections::HashMap<(i32, i32, i32), (i32, i32)> =
        std::collections::HashMap::new();
    for line in content.lines() {
        let p: Vec<&str> = line.split('|').collect();
        if p.len() >= 6 {
            if let (Ok(x), Ok(y), Ok(z), Ok(ox), Ok(oz)) = (
                p[0].parse::<i32>(),
                p[1].parse::<i32>(),
                p[2].parse::<i32>(),
                p[4].parse::<i32>(),
                p[5].parse::<i32>(),
            ) {
                last_origin.insert((x, y, z), (ox, oz));
            }
        }
    }
    let mut mismatch = 0usize;
    let mut checked = 0usize;
    let mut by_origin: std::collections::HashMap<(i32, i32), u32> =
        std::collections::HashMap::new();
    for ((x, y, z), want) in &last {
        let lx = (x - ccx * 16) as u32;
        let lz = (z - ccz * 16) as u32;
        if !(0..16).contains(&lx) || !(0..16).contains(&lz) || !(-64..320).contains(y) {
            continue; // outside compared chunk
        }
        checked += 1;
        let b = chunk.block_at(lx, *y, lz);
        let got = b.block_name();
        let want_s = if want.starts_with("minecraft:") {
            want.clone()
        } else {
            format!("minecraft:{want}")
        };
        if got != want_s {
            mismatch += 1;
            let k = (*x, *y, *z);
            *by_origin.entry(*last_origin.get(&k).unwrap_or(&(i32::MIN, i32::MIN)))
                .or_insert(0) += 1;
            if mismatch <= 40 {
                println!("DIFF ({},{},{}) vanilla={} neutron={}", x, y, z, want_s, got);
            }
        }
    }
    println!(
        "compared {} vanilla-written cells inside center chunk: {} mismatches",
        checked, mismatch
    );
    let mut rows: Vec<_> = by_origin.iter().collect();
    rows.sort_by(|a, b| b.1.cmp(a.1));
    for ((ox, oz), n) in &rows {
        println!("  mismatches from origin ({},{}): {}", ox, oz, n);
    }
}
