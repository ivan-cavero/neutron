//! moss_terrain_stream — CLOSED DUMP: placement-stream reconstruction of the
//! TERRAIN(never feature-written) moss_block/clay pool in chunk (-1,-2),
//! seed 424242 (ticket_sim default ordering).
//!
//! Producers (jar 26.2, `data/minecraft/worldgen/biome/lush_caves.json`
//! step 9, byte-identical to `src/data/worldgen/biome/lush_caves.json`):
//!   lush_caves_vegetation          -> configured moss_patch          (count 125)
//!   lush_caves_clay                -> configured lush_caves_clay     (count 62)
//!   lush_caves_ceiling_vegetation  -> configured moss_patch_ceiling  (count 125)
//! Placement chain (all three): count -> in_square -> height_range(uniform
//! above_bottom 0 .. absolute 256) -> environment_scan(down,<=12, air->solid)
//! -> random_offset(xz 0, y 1) -> biome (BiomeFilter samples the biome AT the
//! final position; decompiled BiomeFilter.shouldPlace).
//!
//! Method:
//!   1. vanilla ground truth from the .mca: family cells + per-section biome
//!      palettes for the 3x3 around chunk (-1,-2);
//!   2. one real decorate_region_origin_major run with NEUTRON_TRACE_TREES=1,
//!      own stderr dup2-captured to /tmp/opencode/moss_stream_trace.log;
//!   3. per-draw stream parsed from OUR pipeline trace (no RNG replication);
//!   4. REJECT forensics on the pre-decoration region (TERRAIN cells are
//!      never feature-written, so pre-deco scene == scene at those cells);
//!   5. vanilla blob centre fit (xz_radius 5..8 grid) vs our ACCEPT centres;
//!   6. classification: (a) feature missing from origin list, (b) roll
//!      divergence, (c) gate never passes.
//!
//! Usage: rustc --edition 2021 -O moss_terrain_stream.rs --extern
//!   neutron_worldgen=target/release/libneutron_worldgen.rlib
//!   --extern neutron_world=target/release/deps/libneutron_world-c6bb0ce27214247b.rlib
//!   -L dependency=target/release/deps

use neutron_world::nbt::ussr_nbt::owned::{List, Tag};
use neutron_world::nbt::{compound_get, read_nbt};
use neutron_world::Region;
use neutron_worldgen::biome_manager::biome_id_at_block;
use neutron_worldgen::carvers;
use neutron_worldgen::feature_catalog::{self, step};
use neutron_worldgen::feature_dispatch::biome_id_to_name;
use neutron_worldgen::generator::{
    decorate_region_origin_major, ChunkGenerator, WORLD_BOTTOM, WORLD_TOP,
};
use neutron_worldgen::mineshaft;
use neutron_worldgen::region_buf::RegionBuf;
use neutron_worldgen::ruined_portal;
use neutron_worldgen::surface::BlockId;
use neutron_worldgen::worldgen::WorldgenState;
use std::collections::HashMap;
use std::os::unix::io::AsRawFd;
use std::os::unix::io::RawFd;

const SEED: i64 = 424242;
const CHUNK: (i32, i32) = (-1, -2);
const REGION_DIR: &str =
    "tools/nbt-ref/vanilla-fresh-424242/world/dimensions/minecraft/overworld/region";
const LUSH_FEATURES: [&str; 3] = [
    "minecraft:lush_caves_vegetation",
    "minecraft:lush_caves_clay",
    "minecraft:lush_caves_ceiling_vegetation",
];
const TRACE_PATH: &str = "/tmp/opencode/moss_stream_trace.log";
const REPORT_PATH: &str = "/tmp/opencode/moss_stream_report.txt";

type NoiseCache = HashMap<(i32, i32), (Vec<u16>, Vec<i16>, Vec<u8>)>;

extern "C" {
    fn dup2(oldfd: RawFd, newfd: RawFd) -> RawFd;
    fn dup(oldfd: RawFd) -> RawFd;
}

fn main() {
    std::env::set_var("NEUTRON_TRACE_TREES", "1");
    std::env::set_var("NEUTRON_WRITERS", "1");
    let t0 = std::time::Instant::now();
    let mut out = String::new();
    let (cx, cz) = CHUNK;
    println!("=== moss_terrain_stream seed={SEED} chunk=({cx},{cz})");

    // ---- vanilla ground truth (blocks + biome palettes), 3x3 ----
    let (van, van_biome) = load_vanilla_full(REGION_DIR, cx, cz);
    out.push_str(&format!("vanilla chunks loaded: {}/9\n", van.len()));

    // ---- TERRAIN pool from the mask_zone_cells.csv artifact ----
    let csv = std::fs::read_to_string("/tmp/opencode/mask_zone_cells.csv").unwrap();
    let mut pool: Vec<(i32, i32, i32, String)> = Vec::new();
    for line in csv.lines().skip(1) {
        let f: Vec<&str> = line.split(',').collect();
        if f.len() < 11 || f[6] != "TERRAIN(never feature-written)" {
            continue;
        }
        let (wx, y, wz) = (
            f[0].parse().unwrap(),
            f[1].parse().unwrap(),
            f[2].parse().unwrap(),
        );
        let (ccx, ccz): (i32, i32) = (f[3].parse().unwrap(), f[4].parse().unwrap());
        if ccx == cx && ccz == cz {
            pool.push((wx, y, wz, f[8].to_string()));
        }
    }
    println!("TERRAIN pool in chunk ({cx},{cz}): {} cells", pool.len());
    let mut by_block: HashMap<&str, usize> = HashMap::new();
    for (_, _, _, b) in &pool {
        *by_block.entry(b.as_str()).or_insert(0) += 1;
    }
    out.push_str(&format!(
        "TERRAIN pool ({cx},{cz}): {} cells by vanilla block: {by_block:?}\n",
        pool.len()
    ));

    // ---- biome ground truth at the pool cells ----
    let gen = ChunkGenerator::new(SEED);
    let state = &gen.state;
    let mut van_bio_tally: HashMap<String, usize> = HashMap::new();
    let mut our_bio_tally: HashMap<String, usize> = HashMap::new();
    let mut bio_diverge = 0usize;
    let snap = |v: i32| (v >> 2) << 2;
    for (wx, y, wz, _) in &pool {
        let vb = van_biome
            .get(&(snap(*wx), snap(*y), snap(*wz)))
            .cloned()
            .unwrap_or_else(|| "<none>".into());
        let ob = biome_id_to_name(biome_id_at_block(state, *wx, *y, *wz)).to_string();
        if vb != ob {
            bio_diverge += 1;
        }
        *van_bio_tally.entry(vb).or_insert(0) += 1;
        *our_bio_tally.entry(ob).or_insert(0) += 1;
    }
    out.push_str(&format!(
        "biome at pool cells: vanilla {van_bio_tally:?}\n                     ours   {our_bio_tally:?}\n                     divergent cells: {bio_diverge}/{}\n",
        pool.len()
    ));

    // ---- per-origin biome union (vanilla .mca sections vs our stored) ----
    let mut noise: NoiseCache = HashMap::new();
    let (mut region, inner, plans) = build_region(&gen, cx, cz, &mut noise);
    let world_of = |&(a, b): &(i32, i32)| (cx - 2 + a, cz - 2 + b);
    let inner_world: Vec<(i32, i32)> = inner.iter().map(world_of).collect();
    out.push_str("\n-- origin biome union (3x3 of origin), vanilla .mca vs our stored noise biomes\n");
    for (k, &(ocx, ocz)) in inner_world.iter().enumerate() {
        let vb = vanilla_union(&van_biome, ocx, ocz);
        let ob = our_union(&region, state, ocx, ocz);
        let v_has = vb.iter().any(|b| b == "lush_caves");
        let o_has = ob.iter().any(|b| b == "lush_caves");
        out.push_str(&format!(
            "  pass {k} chunk ({ocx},{ocz}): vanilla_union={vb:?} lush={v_has} | our_union={ob:?} lush={o_has}\n"
        ));
    }

    // ---- pre-decoration snapshot (scene forensics base, 3x3 chunks) ----
    let mut pre: HashMap<(i32, i32, i32), BlockId> = HashMap::new();
    for &(ccx, ccz) in inner_world.iter() {
        for y in WORLD_BOTTOM..WORLD_TOP {
            for lz in 0..16 {
                for lx in 0..16 {
                    let (wx, wz) = (ccx * 16 + lx, ccz * 16 + lz);
                    pre.insert((wx, y, wz), region.get(wx, y, wz));
                }
            }
        }
    }

    // ---- real pipeline run, own stderr captured ----
    let saved_err = unsafe { dup(2) };
    let trace_file = std::fs::File::create(TRACE_PATH).unwrap();
    unsafe {
        dup2(trace_file.as_raw_fd(), 2);
    }
    decorate_region_origin_major(&mut region, state, &inner, (cx, cz), &plans);
    unsafe {
        dup2(saved_err, 2);
    }
    let final_default: Vec<Vec<u16>> = inner_world
        .iter()
        .map(|&(ccx, ccz)| region.take_chunk(ccx, ccz).0)
        .collect();
    println!(
        "decorate done ({}ms), trace captured",
        t0.elapsed().as_millis()
    );

    // ---- our family cells after the full run (chunk (-1,-2)) ----
    let center_idx = inner_world
        .iter()
        .position(|&(a, b)| a == cx && b == cz)
        .expect("center chunk not in inner");
    let center_blocks = &final_default[center_idx];
    let mut our_family: HashMap<(i32, i32, i32), String> = HashMap::new();
    for y in WORLD_BOTTOM..WORLD_TOP {
        for lz in 0..16i32 {
            for lx in 0..16i32 {
                let (wx, wz) = (cx * 16 + lx, cz * 16 + lz);
                let n = name_of(center_blocks[idx(lx, y, lz)]);
                if n == "moss_block" || n == "clay" {
                    our_family.insert((wx, y, wz), n);
                }
            }
        }
    }
    out.push_str(&format!(
        "\nour family cells in chunk ({cx},{cz}) after full run: {} (moss {} | clay {})\n",
        our_family.len(),
        our_family
            .values()
            .filter(|b| b.as_str() == "moss_block")
            .count(),
        our_family.values().filter(|b| b.as_str() == "clay").count()
    ));
    let overlap = pool
        .iter()
        .filter(|(wx, y, wz, _)| our_family.contains_key(&(*wx, *y, *wz)))
        .count();
    out.push_str(&format!(
        "pool cells also family-painted by us: {overlap}/{}\n",
        pool.len()
    ));

    // ---- parse the trace: per-origin lush feature streams ----
    let trace = std::fs::read_to_string(TRACE_PATH).unwrap_or_default();
    let mut draws: Vec<TraceDraw> = Vec::new();
    let mut cur: Option<(i32, i32, String)> = None;
    for line in trace.lines() {
        if let Some(rest) = line.strip_prefix("[trace] chunk=(") {
            let mut it = rest.split(") placed=");
            let coord = it.next().unwrap_or("");
            let placed_full = it.next().unwrap_or("");
            let placed = placed_full.split_whitespace().next().unwrap_or("").to_string();
            let mut cc = coord.split(',');
            let (a, b): (i32, i32) = (
                cc.next().unwrap_or("0").parse().unwrap_or(0),
                cc.next().unwrap_or("0").parse().unwrap_or(0),
            );
            cur = Some((a, b, placed));
        } else if let Some(rest) = line.strip_prefix("[trace]   draw ") {
            let Some((ox, oz, placed)) = cur.clone() else {
                continue;
            };
            let (acc, coord) = if rest.contains("ACCEPT") {
                (true, rest.split("ACCEPT ").nth(1).unwrap_or(""))
            } else if rest.contains("REJECT") {
                (false, rest.split("REJECT ").nth(1).unwrap_or(""))
            } else {
                continue;
            };
            let coord = coord.trim_matches(|c| c == '(' || c == ')');
            let mut x = 0i32;
            let mut y = 0i32;
            let mut z = 0i32;
            // ACCEPT: "x=.. z=.. y=.."; REJECT: "x=..,z=..,y=.."
            for part in coord.split(|c: char| c == ' ' || c == ',') {
                let mut kv = part.splitn(2, '=');
                let key = kv.next().unwrap_or("");
                let val = kv.next().unwrap_or("");
                match key {
                    "x" => x = val.parse().unwrap_or(0),
                    "z" => z = val.parse().unwrap_or(0),
                    "y" => y = val.parse().unwrap_or(0),
                    _ => {}
                }
            }
            if LUSH_FEATURES.contains(&placed.as_str()) {
                draws.push(TraceDraw {
                    origin: (ox, oz),
                    placed,
                    accepted: acc,
                    x,
                    y,
                    z,
                });
            }
        }
    }
    out.push_str(&format!(
        "\n-- our lush-feature draws (parsed from {TRACE_PATH}): {} total\n",
        draws.len()
    ));

    // ---- group per origin + feature ----
    let mut per_origin: HashMap<(i32, i32), HashMap<String, (usize, usize)>> = HashMap::new();
    for d in &draws {
        let e = per_origin
            .entry(d.origin)
            .or_default()
            .entry(d.placed.clone())
            .or_insert((0, 0));
        e.0 += 1;
        if d.accepted {
            e.1 += 1;
        }
    }
    let mut any_accept = false;
    for (k, &(ocx, ocz)) in inner_world.iter().enumerate() {
        // trace logs WORLD min-corner; inner_world holds chunk coords
        let empty: HashMap<String, (usize, usize)> = HashMap::new();
        let m = per_origin
            .get(&(ocx * 16, ocz * 16))
            .unwrap_or(&empty);
        for f in LUSH_FEATURES {
            let (n, a) = m.get(f).copied().unwrap_or((0, 0));
            any_accept |= a > 0;
            let gi = feature_catalog::global_feature_index(step::VEGETAL_DECORATION, f)
                .map(|v| v.to_string())
                .unwrap_or("NONE".into());
            out.push_str(&format!(
                "  pass {k} origin ({ocx},{ocz}) {f:<42} global_idx={gi:<3} draws={n:<4} accepts={a}\n"
            ));
        }
    }
    if !any_accept {
        out.push_str("  => NO lush feature ever ACCEPTed at any of the 9 origins\n");
    }

    // ---- producer forensics: floor-patch draws in patch-y band near the blob ----
    // vanilla painted the blob from a vegetation_patch with origin y0 in
    // [floor_top-5 .. floor_top+5] = [6..19]; any disk column scan starts at
    // the SAME y0, so the whole blob (floor tops y11..14) needs y0 in [6..19].
    out.push_str("\n-- PRODUCER candidates: lush floor features, y0 in [6,19], (x,z) within 17 of blob bbox centre (-6,-29)\n");
    for d in draws.iter().filter(|d| {
        !d.placed.contains("ceiling") && d.y >= 6 && d.y <= 19
            && (d.x + 6).abs() <= 17 && (d.z + 29).abs() <= 17
    }) {
        let pb = pre
            .get(&(d.x, d.y, d.z))
            .map(|b| b.block_name().to_string())
            .unwrap_or_else(|| "<out-of-3x3>".into());
        let vstart = van
            .get(&(d.x >> 4, d.z >> 4))
            .and_then(|m| m.get(&(d.x, d.y, d.z)))
            .cloned()
            .unwrap_or_else(|| "<none>".into());
        let scan_ok = scan_down_ok(&pre, d.x, d.y, d.z);
        let mut fy = d.y;
        let mut first_solid: Option<i32> = None;
        while fy > WORLD_BOTTOM && d.y - fy <= 12 {
            fy -= 1;
            let b = pre.get(&(d.x, fy, d.z)).copied().unwrap_or(BlockId::Air);
            if !b.is_air() {
                first_solid = Some(fy);
                break;
            }
        }
        let floor = match first_solid {
            Some(fy) => format!(
                "solid at y{fy} ({})",
                name_of_neu(pre.get(&(d.x, fy, d.z)).copied())
            ),
            None => "none<=12".to_string(),
        };
        out.push_str(&format!(
            "  origin {:?} {} ({},{},{}) our_pre={pb} vanilla_start={vstart} scan_ok={scan_ok} floor_below: {floor}\n",
            d.origin, d.placed, d.x, d.y, d.z
        ));
    }

    // ---- two-sided gate verdict: every lush floor draw, our scene vs vanilla scene ----
    // vanilla scene proxy = final .mca blocks (cave geometry is unchanged by
    // features; the moss/clay patch itself paints INTO the floor).
    // vanilla EnvironmentScanPlacement(down,<=12): start must pass
    // `allowed` (#air); descend while allowed; target = solid at first fail.
    let van_get = |x: i32, y: i32, z: i32| -> Option<&String> {
        van.get(&(x >> 4, z >> 4)).and_then(|m| m.get(&(x, y, z)))
    };
    let van_scan_ok = |x: i32, y: i32, z: i32| -> bool {
        let Some(s) = van_get(x, y, z) else {
            return false;
        };
        if s != "air" && s != "cave_air" {
            return false;
        }
        let mut py = y;
        for _ in 0..12 {
            py -= 1;
            if py < WORLD_BOTTOM {
                return false;
            }
            let Some(b) = van_get(x, py, z) else {
                return false;
            };
            if b == "air" || b == "cave_air" {
                continue;
            }
            if b == "water" || b == "lava" {
                return false;
            }
            return true;
        }
        false
    };
    let mut tally2: HashMap<String, usize> = HashMap::new();
    let mut near_mismatch: Vec<String> = Vec::new();
    for d in draws.iter().filter(|d| !d.placed.contains("ceiling")) {
        let ours = scan_down_ok(&pre, d.x, d.y, d.z);
        let theirs = van_scan_ok(d.x, d.y, d.z);
        let key = format!("ours={ours} vanilla={theirs}");
        *tally2.entry(key).or_insert(0) += 1;
        if ours != theirs {
            let pb = pre
                .get(&(d.x, d.y, d.z))
                .map(|b| b.block_name().to_string())
                .unwrap_or("?".into());
            let vb = van_get(d.x, d.y, d.z).cloned().unwrap_or("?".into());
            // first solid below in both scenes (<=12)
            let mut our_floor = "none".to_string();
            let mut fy = d.y;
            while fy > WORLD_BOTTOM && d.y - fy <= 12 {
                fy -= 1;
                let b = pre.get(&(d.x, fy, d.z)).copied().unwrap_or(BlockId::Air);
                if !b.is_air() {
                    our_floor = format!("y{fy}:{}", b.block_name());
                    break;
                }
            }
            let mut van_floor = "none".to_string();
            let mut fy = d.y;
            while fy > WORLD_BOTTOM && d.y - fy <= 12 {
                fy -= 1;
                if let Some(b) = van_get(d.x, fy, d.z) {
                    if b != "air" && b != "cave_air" {
                        van_floor = format!("y{fy}:{b}");
                        break;
                    }
                }
            }
            near_mismatch.push(format!(
                "  origin {:?} {} ({},{},{}) ours={ours} vanilla={theirs} | start ours={pb} vanilla={vb} | floor ours={our_floor} vanilla={van_floor}",
                d.origin, d.placed, d.x, d.y, d.z
            ));
        }
    }
    out.push_str("\n-- two-sided gate verdict (lush floor draws, our pre-deco scene vs vanilla final blocks)\n");
    for (k, v) in &tally2 {
        out.push_str(&format!("  {k}: {v}\n"));
    }
    out.push_str(&format!(
        "\n  ALL gate mismatches ({}):\n",
        near_mismatch.len()
    ));
    for m in &near_mismatch {
        out.push_str(&m);
        out.push('\n');
    }

    // ---- two-sided gate verdict: ceiling draws, UP-scan, both scenes ----
    let van_scan_ok_up = |x: i32, y: i32, z: i32| -> bool {
        let Some(s) = van_get(x, y, z) else {
            return false;
        };
        if s != "air" && s != "cave_air" {
            return false;
        }
        let mut py = y;
        for _ in 0..12 {
            py += 1;
            if py >= WORLD_TOP {
                return false;
            }
            let Some(b) = van_get(x, py, z) else {
                return false;
            };
            if b == "air" || b == "cave_air" {
                continue;
            }
            if b == "water" || b == "lava" {
                return false;
            }
            return true;
        }
        false
    };
    let scan_up_ok = |pre: &HashMap<(i32, i32, i32), BlockId>, x: i32, y: i32, z: i32| -> bool {
        let get = |p: (i32, i32, i32)| -> BlockId { pre.get(&p).copied().unwrap_or(BlockId::Air) };
        if !get((x, y, z)).is_air() {
            return false;
        }
        let mut py = y;
        for _ in 0..12 {
            py += 1;
            if py >= WORLD_TOP {
                return false;
            }
            let b = get((x, py, z));
            if b.is_air() {
                continue;
            }
            if matches!(b, BlockId::Water | BlockId::Lava) {
                return false;
            }
            return true;
        }
        false
    };
    let mut ceil_mismatch = Vec::new();
    for d in draws.iter().filter(|d| d.placed.contains("ceiling")) {
        let ours = scan_up_ok(&pre, d.x, d.y, d.z);
        let theirs = van_scan_ok_up(d.x, d.y, d.z);
        if ours != theirs
            && d.y >= 0
            && d.y <= 40
            && (d.x + 6).abs() <= 24
            && (d.z + 29).abs() <= 24
        {
            let pb = pre
                .get(&(d.x, d.y, d.z))
                .map(|b| b.block_name().to_string())
                .unwrap_or("?".into());
            let vb = van_get(d.x, d.y, d.z).cloned().unwrap_or("?".into());
            ceil_mismatch.push(format!(
                "  origin {:?} ceiling ({},{},{}) ours={ours} vanilla={theirs} start ours={pb} vanilla={vb}",
                d.origin, d.x, d.y, d.z
            ));
        }
    }
    out.push_str(&format!(
        "\n-- ceiling (moss_patch_ceiling) gate mismatches in-chunk, y 0..40: {}\n",
        ceil_mismatch.len()
    ));
    for m in &ceil_mismatch {
        out.push_str(&m);
        out.push('\n');
    }

    // ---- REJECT forensics: killer modifier per lush REJECT near the pool ----
    out.push_str("\n-- REJECT forensics (lush draws within chunk +-8 blocks)\n");
    let mut killer_tally: HashMap<String, usize> = HashMap::new();
    for d in draws.iter().filter(|d| !d.accepted) {
        let scan_ok = scan_down_ok(&pre, d.x, d.y, d.z);
        let ob = biome_id_to_name(biome_id_at_block(state, d.x, d.y, d.z));
        let vb = van_biome
            .get(&((d.x >> 2) << 2, (d.y >> 2) << 2, (d.z >> 2) << 2))
            .cloned()
            .unwrap_or_else(|| "<none>".into());
        let killer = if !scan_ok {
            "environment_scan"
        } else if ob != "lush_caves" {
            "biome_filter(our biome not lush)"
        } else if vb != "lush_caves" {
            "biome_filter(ours lush, vanilla not)"
        } else {
            "other"
        };
        *killer_tally.entry(killer.to_string()).or_insert(0) += 1;
        if d.x >= cx * 16 - 8
            && d.x < cx * 16 + 24
            && d.z >= cz * 16 - 8
            && d.z < cz * 16 + 24
            && killer != "other"
        {
            let pb = pre
                .get(&(d.x, d.y, d.z))
                .map(|b| b.block_name())
                .unwrap_or("?");
            out.push_str(&format!(
                "  origin {:?} {} REJECT ({},{},{}) pre_block={pb} scan_ok={scan_ok} our_biome={ob} vanilla_biome={vb} -> {killer}\n",
                d.origin, d.placed, d.x, d.y, d.z
            ));
        }
    }
    out.push_str(&format!("REJECT killer tally (all 9 origins): {killer_tally:?}\n"));

    // ---- vanilla blob centre fit vs our ACCEPT centres ----
    out.push_str("\n-- vanilla blob fit (vegetation_patch disk, xz_radius 5..8)\n");
    let moss_cells: Vec<(i32, i32, i32)> = pool
        .iter()
        .filter(|(_, _, _, b)| b == "moss_block")
        .map(|(wx, y, wz, _)| (*wx, *y, *wz))
        .collect();
    let clay_cells: Vec<(i32, i32, i32)> = pool
        .iter()
        .filter(|(_, _, _, b)| b == "clay")
        .map(|(wx, y, wz, _)| (*wx, *y, *wz))
        .collect();
    for (label, cells) in [("moss_block", &moss_cells), ("clay", &clay_cells)] {
        let (fit, cover) = fit_blob(cells, cx * 16, cz * 16);
        out.push_str(&format!(
            "  {label}: {} cells; best fit centre=(x{},z{}) r={} -> {cover} cells inside\n",
            cells.len(),
            fit.0,
            fit.1,
            fit.2
        ));
    }

    // ---- decisive draws: lush, (x,z) on a pool column, y within scan reach ----
    let footprint: std::collections::HashSet<(i32, i32)> =
        pool.iter().map(|(wx, _, wz, _)| (*wx, *wz)).collect();
    out.push_str("\n-- DECISIVE draws: lush, (x,z) on a pool column, y in [8,30];\n   our pre-deco block at start cell + vanilla final blocks of the scan window\n");
    for d in draws.iter().filter(|d| {
        footprint.contains(&(d.x, d.z)) && d.y >= 8 && d.y <= 30
    }) {
        let pb = pre
            .get(&(d.x, d.y, d.z))
            .map(|b| b.block_name().to_string())
            .unwrap_or_else(|| "?".into());
        let scan_ok = scan_down_ok(&pre, d.x, d.y, d.z);
        let vcol: Vec<String> = (d.y..(d.y + 13).min(WORLD_TOP))
            .map(|yy| {
                let vb = van
                    .get(&(d.x >> 4, d.z >> 4))
                    .and_then(|m| m.get(&(d.x, yy, d.z)))
                    .cloned()
                    .unwrap_or_else(|| "?".into());
                format!("{yy}:{vb}")
            })
            .collect();
        out.push_str(&format!(
            "  origin {:?} {} ({},{},{}) our_pre={pb} scan_ok={scan_ok}\n    vanilla column: {}\n",
            d.origin,
            d.placed,
            d.x,
            d.y,
            d.z,
            vcol.join(" ")
        ));
    }

    // ---- columns y 0..26 at pool centroid ----
    out.push_str("\n-- columns y 0..26 at pool centroid (our pre-deco | vanilla final)\n");
    for (px, pz) in [(-6, -30), (-7, -28), (-5, -31), (-9, -32), (-4, -27), (-9, -25), (-11, -28)] {
        let mut line = String::new();
        for y in 0..27 {
            let ours = pre
                .get(&(px, y, pz))
                .map(|b| b.block_name().to_string())
                .unwrap_or("?".into());
            let vanb = van
                .get(&(px >> 4, pz >> 4))
                .and_then(|m| m.get(&(px, y, pz)))
                .cloned()
                .unwrap_or("?".into());
            line.push_str(&format!("y{y}:{}/{} ", ours, vanb));
        }
        out.push_str(&format!("  ({px},{pz}) {line}\n"));
    }
    out.push_str("\n-- our ACCEPT draws for lush features (all origins, chunk +-24)\n");
    for d in draws.iter().filter(|d| d.accepted) {
        if d.x >= cx * 16 - 24 && d.x < cx * 16 + 40 && d.z >= cz * 16 - 24 && d.z < cz * 16 + 40 {
            out.push_str(&format!(
                "  origin {:?} {} ACCEPT ({},{},{})\n",
                d.origin, d.placed, d.x, d.y, d.z
            ));
        }
    }

    // ---- classification summary ----
    out.push_str("\n== CLASSIFICATION\n");
    out.push_str("  (a) feature missing: lush_caves biome JSON identical jar<->crate (features[9] checked); union lush presence per origin printed above.\n");
    out.push_str(&format!("  (b)/(c) evidence: REJECT killer tally {killer_tally:?}\n"));
    out.push_str(&format!(
        "  pool coverage by our family pass: {overlap}/{}; biome divergences at pool cells: {bio_diverge}\n",
        pool.len()
    ));

    // ---- 5 rows ----
    let union_c = our_union(&region, state, cx, cz);
    out.push_str("\n== 5 ROWS (x,y,z | vanilla | neutron | biome@pool-cell ours/vanilla | feature | class)\n");
    for (wx, y, wz, vb) in pool.iter().take(5) {
        let nv = our_family
            .get(&(*wx, *y, *wz))
            .cloned()
            .unwrap_or_else(|| name_of(center_blocks[idx(wx & 15, *y, wz & 15)]));
        let ob = biome_id_to_name(biome_id_at_block(state, *wx, *y, *wz));
        let vbb = van_biome
            .get(&((wx >> 2) << 2, (y >> 2) << 2, (wz >> 2) << 2))
            .cloned()
            .unwrap_or_else(|| "<none>".into());
        let feat = if vb == "moss_block" {
            "minecraft:lush_caves_vegetation(moss_patch)"
        } else {
            "minecraft:lush_caves_clay(clay_with_dripleaves)"
        };
        out.push_str(&format!(
            "  ({wx},{y},{wz}) | {vb} | {nv} | ours={ob} vanilla={vbb} | {feat} | class?\n"
        ));
    }
    out.push_str(&format!(
        "\norigin (-1,-2) biome union (ours): {union_c:?}\n"
    ));

    std::fs::write(REPORT_PATH, &out).unwrap();
    println!("report: {REPORT_PATH}");
    println!("trace:  {TRACE_PATH}");
    println!("done in {}ms", t0.elapsed().as_millis());
}

struct TraceDraw {
    origin: (i32, i32),
    placed: String,
    accepted: bool,
    x: i32,
    y: i32,
    z: i32,
}

/// EnvironmentScanPlacement(down, <=12, allowed=#minecraft:air, target=solid)
/// replica over the pre-decoration scene (cave-scene subset of blocks_motion:
/// air/cave_air/water/lava are the only non-solids reachable pre-decoration).
fn scan_down_ok(pre: &HashMap<(i32, i32, i32), BlockId>, x: i32, y: i32, z: i32) -> bool {
    let get = |p: (i32, i32, i32)| -> BlockId { pre.get(&p).copied().unwrap_or(BlockId::Air) };
    let air = |b: BlockId| b.is_air();
    let solid = |b: BlockId| {
        !matches!(
            b,
            BlockId::Air | BlockId::CaveAir | BlockId::Water | BlockId::Lava
        )
    };
    if !air(get((x, y, z))) {
        return false;
    }
    let mut py = y;
    for _ in 0..12 {
        if solid(get((x, py, z))) {
            return true;
        }
        py -= 1;
        if py < WORLD_BOTTOM {
            return false;
        }
        if !air(get((x, py, z))) {
            break;
        }
    }
    solid(get((x, py, z)))
}

/// Best disk fit: centre over the chunk +-8, radius 5..8; maximise covered
/// cells (|dx|<=r && |dz|<=r, corners excluded — edges are per-column chance
/// and do not bound the fit).
fn fit_blob(cells: &[(i32, i32, i32)], base_x: i32, base_z: i32) -> ((i32, i32, i32), usize) {
    let mut best = ((0, 0, 0), 0usize);
    for x0 in (base_x - 8)..(base_x + 24) {
        for z0 in (base_z - 8)..(base_z + 24) {
            for r in 5..=8i32 {
                let inside = cells
                    .iter()
                    .filter(|(wx, _, wz)| {
                        let (dx, dz) = (wx - x0, wz - z0);
                        dx.abs() <= r && dz.abs() <= r && !(dx.abs() == r && dz.abs() == r)
                    })
                    .count();
                if inside > best.1 {
                    best = ((x0, z0, r), inside);
                }
            }
        }
    }
    best
}

/// Same 4x4x24 quart loop as feature_dispatch::origin_biome_union (pub fns
/// only): stored noise biomes, fallback noise_biome_at_quart.
fn our_union(
    region: &RegionBuf,
    state: &WorldgenState,
    ox0: i32,
    oz0: i32,
) -> Vec<String> {
    let cxl = (ox0 - region.origin_x) / 16;
    let czl = (oz0 - region.origin_z) / 16;
    let mut names: Vec<String> = Vec::new();
    for dz in -1..=1i32 {
        for dx in -1..=1i32 {
            let ncx = cxl + dx;
            let ncz = czl + dz;
            if ncx < 0 || ncz < 0 || ncx >= region.chunks || ncz >= region.chunks {
                continue;
            }
            let cx0 = region.origin_x + ncx * 16;
            let cz0 = region.origin_z + ncz * 16;
            for section in 0..24i32 {
                let base_y_q = (WORLD_BOTTOM + section * 16) >> 2;
                for sy4 in 0..4i32 {
                    for bz4 in 0..4i32 {
                        for bx4 in 0..4i32 {
                            let (qx, qy, qz) = (cx0 / 4 + bx4, base_y_q + sy4, cz0 / 4 + bz4);
                            let id = region.stored_noise_biome(qx, qy, qz).unwrap_or_else(|| {
                                neutron_worldgen::biome_manager::noise_biome_at_quart(
                                    state, qx, qy, qz,
                                )
                            });
                            let n = biome_id_to_name(id).to_string();
                            if !names.contains(&n) {
                                names.push(n);
                            }
                        }
                    }
                }
            }
        }
    }
    names
}

/// Biome union of the 3x3 around origin (ox0,oz0) from the vanilla .mca
/// section palettes, sampled at quart resolution (x,z,y in {0,4,8,12}).
fn vanilla_union(van_biome: &HashMap<(i32, i32, i32), String>, ox0: i32, oz0: i32) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    for dz in -1..=1i32 {
        for dx in -1..=1i32 {
            let ccx = (ox0 >> 4) + dx;
            let ccz = (oz0 >> 4) + dz;
            for y in (WORLD_BOTTOM..WORLD_TOP).step_by(4) {
                for lz in (0..16i32).step_by(4) {
                    for lx in (0..16i32).step_by(4) {
                        let (wx, wz) = (ccx * 16 + lx, ccz * 16 + lz);
                        if let Some(b) = van_biome.get(&(wx, y, wz)) {
                            if !names.contains(b) {
                                names.push(b.clone());
                            }
                        }
                    }
                }
            }
        }
    }
    names
}

fn name_of(v: u16) -> String {
    BlockId::from_u16(v)
        .unwrap_or(BlockId::Air)
        .block_name()
        .to_string()
        .trim_start_matches("minecraft:")
        .to_string()
}

fn name_of_neu(b: Option<BlockId>) -> String {
    b.map(|x| x.block_name().to_string())
        .unwrap_or_else(|| "?".into())
}

fn idx(lx: i32, y: i32, lz: i32) -> usize {
    ((y - WORLD_BOTTOM) as usize) * 256 + (lz as usize) * 16 + (lx as usize)
}

/// Replicates `generate_chunk_cached` up to (and including) the frozen
/// ruined-portal plans (copied from examples/mask_zone_probe.rs).
fn build_region(
    gen: &ChunkGenerator,
    cx: i32,
    cz: i32,
    noise: &mut NoiseCache,
) -> (RegionBuf, Vec<(i32, i32)>, ruined_portal::Plans) {
    let mut region = RegionBuf::new(cx, cz, 2);
    for dz in -2..=2 {
        for dx in -2..=2 {
            let key = (cx + dx, cz + dz);
            let entry = noise
                .entry(key)
                .or_insert_with(|| gen.generate_noise_and_surface(key.0, key.1));
            let (blocks, heightmap, biomes) = entry.clone();
            region.put_chunk(key.0, key.1, &blocks, &heightmap);
            region.put_chunk_biomes(key.0, key.1, &biomes);
        }
    }
    region.current_writer = neutron_worldgen::writers::CARVER;
    carvers::apply_carvers_region(&mut region, &gen.state);
    region.current_writer = neutron_worldgen::writers::MINESHAFT;
    mineshaft::apply_mineshafts_region(&mut region, &gen.state);
    region.current_writer = neutron_worldgen::writers::TERRAIN;
    let order = neutron_worldgen::deco_schedule::window_order(
        region.chunks,
        region.origin_x,
        region.origin_z,
    );
    let mid = region.chunks / 2;
    let inner: Vec<(i32, i32)> = order
        .into_iter()
        .filter(|&(a, b)| (a - mid).abs() <= 1 && (b - mid).abs() <= 1)
        .collect();
    let rp_owners: Vec<(i32, i32)> = inner
        .iter()
        .map(|&(a, b)| ((region.origin_x >> 4) + a, (region.origin_z >> 4) + b))
        .collect();
    let plans = ruined_portal::prepare_region_plans(&gen.state, &region, &rp_owners);
    (region, inner, plans)
}

/// Vanilla chunk blocks + per-section biome palettes, full-status chunks only
/// (block decoder copied from examples/mask_zone_probe.rs).
fn load_vanilla_full(
    region_dir: &str,
    cx: i32,
    cz: i32,
) -> (
    HashMap<(i32, i32), HashMap<(i32, i32, i32), String>>,
    HashMap<(i32, i32, i32), String>,
) {
    let mut blocks: HashMap<(i32, i32), HashMap<(i32, i32, i32), String>> = HashMap::new();
    let mut biomes: HashMap<(i32, i32, i32), String> = HashMap::new();
    for dz in -1..=1i32 {
        for dx in -1..=1i32 {
            let (ccx, ccz) = (cx + dx, cz + dz);
            let (rx, rz) = (ccx >> 5, ccz >> 5);
            let path = std::path::PathBuf::from(format!("{region_dir}/r.{rx}.{rz}.mca"));
            let region = match Region::open(&path) {
                Ok(r) => r.with_coords(rx, rz),
                Err(_) => continue,
            };
            let Ok(Some(data)) = region.get_chunk(ccx & 31, ccz & 31) else {
                continue;
            };
            let nbt = match read_nbt(&data) {
                Ok(n) => n,
                Err(_) => continue,
            };
            if let Some(Tag::String(s)) = compound_get(&nbt.compound, "Status") {
                if !s.to_string().ends_with("full") {
                    continue;
                }
            } else {
                continue;
            }
            let sections = match compound_get(&nbt.compound, "sections") {
                Some(Tag::List(List::Compound(l))) => l,
                _ => continue,
            };
            let mut map = HashMap::new();
            for sec in sections {
                let y_sec = match compound_get(sec, "Y") {
                    Some(Tag::Byte(y)) => *y as i8 as i32,
                    Some(Tag::Int(y)) => *y,
                    _ => continue,
                };
                // ---- biome palette ----
                if let Some(Tag::Compound(bs)) = compound_get(sec, "biomes") {
                    let palette: Vec<String> = match compound_get(bs, "palette") {
                        Some(Tag::List(List::String(l))) => l
                            .iter()
                            .map(|s| s.to_string().trim_start_matches("minecraft:").to_string())
                            .collect(),
                        _ => Vec::new(),
                    };
                    if palette.len() == 1 {
                        for qy in 0..4i32 {
                            for qz in 0..4i32 {
                                for qx in 0..4i32 {
                                    let (wx, y, wz) = (
                                        ccx * 16 + qx * 4,
                                        y_sec * 16 + qy * 4,
                                        ccz * 16 + qz * 4,
                                    );
                                    biomes.insert((wx, y, wz), palette[0].clone());
                                }
                            }
                        }
                    } else if let Some(Tag::LongArray(d)) = compound_get(bs, "data") {
                        let longs: Vec<i64> = d.to_vec();
                        // biomes: NO 4-bit floor (64 entries, ceil_log2 bits;
                        // a 2-entry palette packs into a single long)
                        let bits = ((palette.len() - 1).ilog2() + 1).max(1) as u32;
                        let epl = 64 / bits;
                        let mask = (1u64 << bits) - 1;
                        for qy in 0..4i32 {
                            for qz in 0..4i32 {
                                for qx in 0..4i32 {
                                    let i: u32 = ((qy * 4 + qz) * 4 + qx) as u32;
                                    let li = (i / epl) as usize;
                                    let bo = (i % epl) * bits;
                                    let vidx = ((longs[li] as u64) >> bo) & mask;
                                    let (wx, y, wz) = (
                                        ccx * 16 + qx * 4,
                                        y_sec * 16 + qy * 4,
                                        ccz * 16 + qz * 4,
                                    );
                                    biomes.insert(
                                        (wx, y, wz),
                                        palette
                                            .get(vidx as usize)
                                            .cloned()
                                            .unwrap_or_default(),
                                    );
                                }
                            }
                        }
                    }
                }
                // ---- blocks ----
                let Some(Tag::Compound(bs)) = compound_get(sec, "block_states") else {
                    continue;
                };
                let Some(Tag::List(List::Compound(palette))) = compound_get(bs, "palette") else {
                    continue;
                };
                let names: Vec<String> = palette
                    .iter()
                    .map(|pc| match compound_get(pc, "Name") {
                        Some(Tag::String(s)) => {
                            s.to_string().trim_start_matches("minecraft:").to_string()
                        }
                        _ => "air".into(),
                    })
                    .collect();
                if names.len() == 1 {
                    for i in 0..4096u32 {
                        map.insert(
                            (
                                ccx * 16 + (i & 15) as i32,
                                y_sec * 16 + (i >> 8) as i32,
                                ccz * 16 + ((i >> 4) & 15) as i32,
                            ),
                            names[0].clone(),
                        );
                    }
                    continue;
                }
                let bits = ((names.len() - 1).ilog2() + 1).max(4) as u32;
                let Some(Tag::LongArray(d)) = compound_get(bs, "data") else {
                    continue;
                };
                let longs: Vec<i64> = d.to_vec();
                let epl = 64 / bits;
                let mask = (1u64 << bits) - 1;
                for i in 0..4096u32 {
                    let li = (i / epl) as usize;
                    let bo = (i % epl) * bits;
                    let vidx = ((longs[li] as u64) >> bo) & mask;
                    map.insert(
                        (
                            ccx * 16 + (i & 15) as i32,
                            y_sec * 16 + (i >> 8) as i32,
                            ccz * 16 + ((i >> 4) & 15) as i32,
                        ),
                        names.get(vidx as usize).cloned().unwrap_or_default(),
                    );
                }
            }
            blocks.insert((ccx, ccz), map);
        }
    }
    (blocks, biomes)
}
