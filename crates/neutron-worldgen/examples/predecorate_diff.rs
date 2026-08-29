//! Cell-level diff: vanilla REAL pre-decoration (ProbePreDecorate dump:
//! fillFromNoise + buildSurface + applyCarvers) vs Neutron export_predecorate.
//!
//! Usage: predecorate_diff <seed> <ccx> <ccz> <dump> [region_dir]
use neutron_worldgen::surface::{vanilla_name, BlockId};
use neutron_worldgen::generator::WORLD_BOTTOM;
use neutron_worldgen::ChunkGenerator;
use std::io::Read;

fn main() {
    let mut args = std::env::args().skip(1);
    let seed: i64 = args.next().and_then(|s| s.parse().ok()).unwrap_or(424242);
    let ccx: i32 = args.next().and_then(|s| s.parse().ok()).unwrap_or(2);
    let ccz: i32 = args.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    let dump_path = args.next().expect("dump path");
    let _ = seed;

    // ---- load vanilla dump ----
    let mut buf = Vec::new();
    std::fs::File::open(&dump_path)
        .expect("dump file")
        .read_to_end(&mut buf)
        .unwrap();
    struct Rd<'a> { buf: &'a [u8], pos: usize }
    impl<'a> Rd<'a> {
        fn u16(&mut self) -> u16 { let v = u16::from_be_bytes([self.buf[self.pos], self.buf[self.pos + 1]]); self.pos += 2; v }
        fn i32(&mut self) -> i32 { let v = i32::from_be_bytes([self.buf[self.pos], self.buf[self.pos + 1], self.buf[self.pos + 2], self.buf[self.pos + 3]]); self.pos += 4; v }
        fn i64(&mut self) -> i64 { let mut b = [0u8; 8]; b.copy_from_slice(&self.buf[self.pos..self.pos + 8]); self.pos += 8; i64::from_be_bytes(b) }
        fn bytes(&mut self, n: usize) -> Vec<u8> { let v = self.buf[self.pos..self.pos + n].to_vec(); self.pos += n; v }
    }
    let mut r = Rd { buf: &buf, pos: 6 };
    assert!(&buf[0..6] == b"PREDC1", "bad magic");
    let dseed = r.i64();
    let dccx = r.i32();
    let dccz = r.i32();
    assert_eq!((dseed, dccx, dccz), (seed, ccx, ccz), "dump/window mismatch");
    let mut van: Vec<(Vec<String>, Vec<u16>)> = Vec::new();
    for _ in 0..25 {
        let pc = r.u16() as usize;
        let mut pal = Vec::with_capacity(pc);
        for _ in 0..pc {
            let len = r.u16() as usize;
            pal.push(String::from_utf8(r.bytes(len)).unwrap());
        }
        let n = 16 * 384 * 16;
        let mut idxs = Vec::with_capacity(n);
        for _ in 0..n {
            idxs.push(r.u16());
        }
        van.push((pal, idxs));
    }

    // ---- neutron pre-decoration ----
    let gen = ChunkGenerator::new(seed);
    let (pals, blocks, _bio, _names) = gen.export_predecorate(ccx, ccz);

    // ---- diff ----
    let mut total_diff = 0u64;
    let mut total_cells = 0u64;
    let mut per_chunk: Vec<(i32, i32, u64, Vec<(i32, i32, i32, String, String)>)> = Vec::new();
    for czl in -1..=1 {
        for cxl in -1..=1 {
            let vidx = ((czl + 2) * 5 + (cxl + 2)) as usize;
            let (vpal, vidxs) = &van[vidx];
            let npal = &pals[vidx];
            let nidxs = &blocks[vidx];
            let mut diffs: Vec<(i32, i32, i32, String, String)> = Vec::new();
            for ly in 0..384usize {
                for lz in 0..16usize {
                    for lx in 0..16usize {
                        let idx = ly * 256 + lz * 16 + lx;
                        let vi = vidxs[idx] as usize;
                        let vn = vpal.get(vi).cloned().unwrap_or_default();
                        let ni = nidxs[idx] as usize;
                        let nn = npal.get(ni).cloned().unwrap_or_default();
                        if vn != nn {
                            let y = WORLD_BOTTOM + ly as i32;
                            let wx = (ccx + cxl) * 16 + lx as i32;
                            let wz = (ccz + czl) * 16 + lz as i32;
                            diffs.push((wx, y, wz, vn, nn));
                        }
                    }
                }
            }
            total_diff += diffs.len() as u64;
            total_cells += (16 * 384 * 16) as u64;
            per_chunk.push(((ccx + cxl), (ccz + czl), diffs.len() as u64, diffs));
        }
    }
    let total_cells = total_cells; // 9 chunks
    println!(
        "PREDECORATE DIFF (inner 3x3): {} cells differ / {} ({:.3}%)",
        total_diff,
        total_cells,
        100.0 * total_diff as f64 / total_cells as f64
    );
    // y-band + class-pair histograms
    let mut bands: std::collections::HashMap<i32, u64> = Default::default();
    let mut pairs: std::collections::HashMap<(String, String), u64> = Default::default();
    let mut pair_examples: std::collections::HashMap<(String, String), (i32, i32, i32)> = Default::default();
    for (_cx, _cz, _d, diffs) in &per_chunk {
        for (wx, y, wz, vn, nn) in diffs {
            *bands.entry(y / 16).or_insert(0) += 1;
            let key = (vn.trim_start_matches("minecraft:").to_string(), nn.trim_start_matches("minecraft:").to_string());
            pair_examples.entry(key.clone()).or_insert((*wx, *y, *wz));
            *pairs.entry(key).or_insert(0) += 1;
        }
    }
    let mut bv: Vec<_> = bands.iter().collect();
    bv.sort_by_key(|(k, _)| **k);
    print!("BANDS:");
    for (k, v) in bv { print!(" {}:{},", k, v); }
    println!();
    let mut pv: Vec<_> = pairs.iter().collect();
    pv.sort_by_key(|(_, v)| std::cmp::Reverse(**v));
    for ((vc, nc), n) in pv.iter().take(14) {
        let ex = pair_examples.get(&(vc.clone(), nc.clone())).unwrap();
        println!("  PAIR {:>7} van={} neu={} e.g.({},{},{})", n, vc, nc, ex.0, ex.1, ex.2);
    }
    per_chunk.sort_by_key(|(_, _, d, _)| std::cmp::Reverse(*d));
    for (cx, cz, d, diffs) in per_chunk.iter().take(9) {
        println!("  chunk ({},{}) diffs={} ({:.2}%)", cx, cz, d, 100.0 * *d as f64 / 98304.0);
        for (wx, y, wz, vn, nn) in diffs.iter().take(4) {
            println!(
                "    ({},{},{}) van={} neu={} | van_block={} neu_block={}",
                wx, y, wz,
                vn.trim_start_matches("minecraft:"),
                nn.trim_start_matches("minecraft:"),
                y_block_class(vn),
                y_block_class(nn)
            );
        }
    }
}

fn y_block_class(name: &str) -> &'static str {
    match name {
        "minecraft:air" | "minecraft:cave_air" => "air",
        "minecraft:stone" | "minecraft:granite" | "minecraft:diorite" | "minecraft:andesite"
        | "minecraft:tuff" | "minecraft:deepslate" => "stone",
        "minecraft:dirt" | "minecraft:grass_block" | "minecraft:coarse_dirt"
        | "minecraft:rooted_dirt" | "minecraft:podzol" | "minecraft:mycelium" => "soil",
        "minecraft:pale_moss_block" | "minecraft:moss_block" => "moss",
        n if n.ends_with("_ore") => "ore",
        n if n.contains("sculk") => "sculk",
        n if n.contains("water") => "water",
        n if n.contains("lava") => "lava",
        _ => "other",
    }
}
