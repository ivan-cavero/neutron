//! lush_chain_dump — CLOSED DUMP: first divergent quantity in the lush-family
//! feature chain under ticket_sim origin ordering.
//!
//! Question (seed 424242, consumer windows (-1,-2) and (-2,-2)): with the
//! deterministic ticket_sim decoration order (deco_schedule.rs, commit a5021c8)
//! as the DEFAULT (NEUTRON_SCULK_ORIGIN_ORDER must be unset), where does the
//! lush/sculk chain (step-6 ore_clay/disk_clay → step-7 sculk_vein +
//! sculk_patch_deep_dark → step-9 lush moss/clay-pool/vine/litter features)
//! first diverge between the vanilla reference world and Neutron, and did the
//! order fix change the pre-ticket_sim failure mode ("divergence onset ==
//! first-ACCEPT index")?
//!
//! Method (reuses the ore_swap_dump / deco_stream_probe machinery):
//!   1. Census: full Neutron `generate_chunk` (ticket_sim default) vs the
//!      vanilla .mca refs → per-chunk lush-family mismatch rows (families:
//!      moss, clay, sculk, litter, water-paired).
//!   2. Chain replay A/B: two 5×5 RegionBufs — VAN (vanilla refs stripped of
//!      lush/sculk/vegetal feature output) and NEU (generated chunks stripped
//!      the same way) — run the SAME lush-chain features for the SAME origins
//!      in the SAME ticket_sim order (window_order of the 5×5 buffer, inner
//!      3×3). Every run records per-attempt (position, modifier-gate) logs and
//!      the buffer write-diff on each side.
//!   3. Root = first run where (draw counts | attempt logs | write sets)
//!      differ between sides. Root fine dump compares attempt logs: onset
//!      index vs the first accepted attempt index → gate-input flip (same
//!      draws, gate differs over upstream terrain), stream flip (positions or
//!      draws diverge), winner flip (cell claimed by ≥2 runs, different
//!      last-writer).
//!   4. Upstream attribution at the root: compare pre-run cell state both
//!      sides, then the pristine (pre-chain) state — surface material diff,
//!      carver-edge (air↔solid), or spill from an earlier chain run.
//!
//! Citations — Java (26.2 decompile, tools/mc-decompiler/output/26.2/src):
//!   OreFeature.place/doPlace  feature/OreFeature.java:23-181
//!   DiskFeature.place         feature/DiskFeature.java:23-63
//!   MultifaceGrowthFeature    feature/MultifaceGrowthFeature.java:23-70
//!   SculkPatchFeature         feature/SculkPatchFeature.java:24-66
//!   VegetationPatchFeature    feature/VegetationPatchFeature.java:30-150
//!   RandomPatchFeature        feature/RandomPatchFeature.java:22-52
//!   placement chain           chunk/ChunkGenerator.java:318-401
//!   EnvironmentScanPlacement  level/levelgen/placement/EnvironmentScanPlacement.java
//! Neutron counterparts (READ ONLY):
//!   features.rs        place_ore_blob_inner :1046, place_disk :709,
//!                      biome_gate_ok :626, should_skip_air_check :1296
//!   feature_dispatch/  place_placed_feature_step mod.rs:260 (modifier chain),
//!                      vegetation.rs:394 place_vegetation_patch (moss_patch),
//!                      predicates.rs:100 would_survive, sampling.rs
//!   sculk/             mod.rs:177 apply_sculk_origin (+ origin order :407),
//!                      place.rs:33 place_sculk_vein, probes.rs (gate replays)
//!   deco_schedule.rs   window_order :741 (ticket_sim default)
//!
//! Build (standalone, no Cargo.toml change):
//!   rustc --edition 2021 -O lush_chain_dump.rs \
//!     --extern neutron_worldgen=target/release/libneutron_worldgen.rlib \
//!     --extern neutron_world=target/release/deps/libneutron_world-<hash>.rlib \
//!     --extern serde_json=target/release/deps/libserde_json-<hash>.rlib \
//!     -L dependency=target/release/deps
//! Run: ./lush_chain_dump [seed=424242] [region_dir] [out_dir=/tmp/opencode]

use neutron_world::nbt::ussr_nbt::owned::{List, Tag};
use neutron_world::nbt::{compound_get, read_nbt};
use neutron_world::Region;
use neutron_worldgen::biome_manager::noise_biome_at_quart;
use neutron_worldgen::biome_source::{biome_id, biome_id_at_block};
use neutron_worldgen::deco_schedule;
use neutron_worldgen::feature_catalog;
use neutron_worldgen::feature_dispatch::{biome_id_to_name, place_placed_feature};
use neutron_worldgen::feature_rng::FeatureRandom;
use neutron_worldgen::generator::{ChunkGenerator, NoiseCache, WORLD_BOTTOM, WORLD_TOP};
use neutron_worldgen::multiface_spreader::FaceMap;
use neutron_worldgen::region_buf::RegionBuf;
use neutron_worldgen::sculk;
use neutron_worldgen::surface::{vanilla_name, BlockId};
use neutron_worldgen::worldgen::WorldgenState;
use serde_json::Value;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::io::Write;

const WINDOWS: [(i32, i32); 2] = [(-1, -2), (-2, -2)];
const BUF_RADIUS: i32 = 2;
const PI: f32 = 3.1415927;
const SIN_SCALE: f64 = 10430.378350470453; // carvers.rs:55 (Java Mth table)

/// Lush-family placed features replayed at step 9 (family filter over the
/// origin's biome-union feature list).
const STEP9_FAMILY: &[&str] = &[
    "lush",
    "sculk",
    "leaf_litter",
    "cave_vine",
    "vines_cave",
    "dripleaf",
    "azalea",
    "spore_blossom",
    "patch_tall_grass",
    "glow_lichen",
    "moss",
];

/// Lush-family census blocks (either side of a mismatch pulls the row in).
const LUSH_FAMILY_NAMES: &[&str] = &[
    "moss_block",
    "moss_carpet",
    "cave_vines",
    "cave_vines_plant",
    "azalea",
    "flowering_azalea",
    "azalea_leaves",
    "flowering_azalea_leaves",
    "big_dripleaf",
    "big_dripleaf_stem",
    "small_dripleaf",
    "rooted_dirt",
    "clay",
    "sculk",
    "sculk_vein",
    "sculk_catalyst",
    "sculk_sensor",
    "sculk_shrieker",
    "leaf_litter",
    "water",
];

fn family_of(name: &str) -> &'static str {
    match name {
        "moss_block" | "moss_carpet" => "moss",
        "cave_vines" | "cave_vines_plant" => "vines",
        "azalea" | "flowering_azalea" | "azalea_leaves" | "flowering_azalea_leaves"
        | "rooted_dirt" => "azalea",
        "big_dripleaf" | "big_dripleaf_stem" | "small_dripleaf" => "dripleaf",
        "clay" => "clay",
        "sculk" | "sculk_vein" | "sculk_catalyst" | "sculk_sensor" | "sculk_shrieker" => "sculk",
        "leaf_litter" => "litter",
        "water" => "water",
        _ => "other",
    }
}

/// Feature-output stripping table (applied identically to BOTH sides before
/// the chain replay so the replay sees "terrain at draw time" symmetrically).
/// GROUND cells (moss/clay/sculk grounds, rooted dirt) were solid before the
/// feature → revert to stone/deepslate. AIR cells (plants, carpets, vein
/// faces, litter, tree blocks) sat in air → revert to air.
const STRIP_TO_GROUND: &[&str] = &[
    "moss_block",
    "rooted_dirt",
    "clay",
    "sculk",
    "sculk_catalyst",
    "sculk_sensor",
    "sculk_shrieker",
];
const STRIP_TO_AIR: &[&str] = &[
    "moss_carpet",
    "cave_vines",
    "cave_vines_plant",
    "azalea",
    "flowering_azalea",
    "azalea_leaves",
    "flowering_azalea_leaves",
    "big_dripleaf",
    "big_dripleaf_stem",
    "small_dripleaf",
    "leaf_litter",
    "sculk_vein",
    "glow_lichen",
    "vine",
    "hanging_roots",
    "short_grass",
    "tall_grass",
    "fern",
    "large_fern",
    "spore_blossom",
    "oak_log",
    "oak_leaves",
    "dark_oak_log",
    "dark_oak_leaves",
    "birch_log",
    "birch_leaves",
    "pale_oak_log",
    "pale_oak_leaves",
    "pale_hanging_moss",
];

fn strip_block(b: BlockId, y: i32) -> BlockId {
    let n = vanilla_name(b);
    let n = n.trim_start_matches("minecraft:");
    if STRIP_TO_GROUND.contains(&n) {
        return if y < 0 { BlockId::Deepslate } else { BlockId::Stone };
    }
    if STRIP_TO_AIR.contains(&n) {
        return BlockId::Air;
    }
    b
}

// ---------------------------------------------------------------------------
// mirrors of crate-internal helpers (surface.rs / predicates.rs / sampling.rs)
// ---------------------------------------------------------------------------

fn blocks_motion_m(b: BlockId) -> bool {
    !matches!(
        b,
        BlockId::Air
            | BlockId::CaveAir
            | BlockId::Water
            | BlockId::Lava
            | BlockId::ShortGrass
            | BlockId::TallGrass
            | BlockId::LeafLitter
            | BlockId::Snow
            | BlockId::PowderSnow
            | BlockId::PaleMossCarpet
            | BlockId::PaleMossCarpetTopper
            | BlockId::MossCarpet
            | BlockId::CaveVines
            | BlockId::CaveVinesPlant
            | BlockId::PaleHangingMoss
            | BlockId::HangingRoots
            | BlockId::Azalea
            | BlockId::FloweringAzalea
            | BlockId::SmallDripleaf
            | BlockId::BigDripleaf
            | BlockId::BigDripleafStem
            | BlockId::Vine
            | BlockId::GlowLichen
            | BlockId::SculkVein
    )
}

fn is_air_m(b: BlockId) -> bool {
    b.is_air()
}

fn supports_vegetation_m(b: BlockId) -> bool {
    matches!(
        b,
        BlockId::Dirt
            | BlockId::CoarseDirt
            | BlockId::RootedDirt
            | BlockId::GrassBlock
            | BlockId::Podzol
            | BlockId::Mycelium
            | BlockId::Mud
            | BlockId::MossBlock
            | BlockId::PaleMossBlock
    )
}

fn sample_int_provider_m(rng: &mut FeatureRandom, v: &Value) -> i32 {
    if let Some(n) = v.as_i64() {
        return n as i32;
    }
    let Some(obj) = v.as_object() else { return 0 };
    match obj.get("type").and_then(|t| t.as_str()) {
        Some("minecraft:uniform") => {
            let min = obj
                .get("min_inclusive")
                .or_else(|| obj.get("value").and_then(|v| v.get("min_inclusive")))
                .and_then(|x| x.as_i64())
                .unwrap_or(0) as i32;
            let max = obj
                .get("max_inclusive")
                .or_else(|| obj.get("value").and_then(|v| v.get("max_inclusive")))
                .and_then(|x| x.as_i64())
                .unwrap_or(min as i64) as i32;
            min + rng.next_int((max - min + 1).max(1))
        }
        Some("minecraft:trapezoid") => {
            let min = obj["min"].as_i64().unwrap_or(0) as i32;
            let max = obj["max"].as_i64().unwrap_or(0) as i32;
            let a = min + rng.next_int((max - min + 1).max(1));
            let b = min + rng.next_int((max - min + 1).max(1));
            (a + b) / 2
        }
        Some("minecraft:constant") => obj["value"].as_i64().unwrap_or(0) as i32,
        _ => 0,
    }
}

fn resolve_anchor_m(v: &Value) -> i32 {
    if let Some(n) = v.get("absolute").and_then(|a| a.as_i64()) {
        return n as i32;
    }
    if let Some(n) = v.get("above_bottom").and_then(|a| a.as_i64()) {
        return WORLD_BOTTOM + n as i32;
    }
    if let Some(n) = v.get("below_top").and_then(|a| a.as_i64()) {
        return (WORLD_TOP - 1) - n as i32;
    }
    0
}

fn sample_height_m(rng: &mut FeatureRandom, height: &Value) -> i32 {
    let ty = height["type"].as_str().unwrap_or("minecraft:uniform");
    if ty.contains("uniform") {
        let min = resolve_anchor_m(&height["min_inclusive"]);
        let max = resolve_anchor_m(&height["max_inclusive"]);
        min + rng.next_int((max - min + 1).max(1))
    } else if ty.contains("trapezoid") {
        let min = resolve_anchor_m(&height["min_inclusive"]);
        let max = resolve_anchor_m(&height["max_inclusive"]);
        if min > max {
            return min;
        }
        let plateau = height["plateau"].as_i64().unwrap_or(0) as i32;
        let range = max - min;
        if plateau >= range {
            return min + rng.next_int(range + 1);
        }
        let plateau_start = (range - plateau) / 2;
        let plateau_end = range - plateau_start;
        min + rng.next_int(plateau_end + 1) + rng.next_int(plateau_start + 1)
    } else {
        64
    }
}

#[derive(Clone, Copy, PartialEq)]
enum HmKind {
    WorldSurface,
    OceanFloor,
}

fn heightmap_top_m(region: &RegionBuf, x: i32, z: i32, kind: HmKind) -> Option<i32> {
    for y in (WORLD_BOTTOM..WORLD_TOP).rev() {
        let b = region.get(x, y, z);
        let opaque = match kind {
            HmKind::WorldSurface => !b.is_air(),
            HmKind::OceanFloor => blocks_motion_m(b),
        };
        if opaque {
            return Some(y);
        }
    }
    None
}

/// Placement-count mirror (sampling.rs placement_count): literal / uniform
/// count providers, product over all count modifiers.
fn placement_count_m(rng: &mut FeatureRandom, placed: &Value) -> i32 {
    let Some(mods) = placed["placement"].as_array() else {
        return 1;
    };
    let mut product = 1i32;
    let mut saw = false;
    for m in mods {
        let ty = m["type"].as_str().unwrap_or("");
        if ty == "minecraft:count" {
            let c = &m["count"];
            let n = if let Some(n) = c.as_i64() {
                n as i32
            } else if c["type"].as_str() == Some("minecraft:uniform") {
                let min = c["min_inclusive"].as_i64().unwrap_or(0) as i32;
                let max = c["max_inclusive"].as_i64().unwrap_or(0) as i32;
                min + rng.next_int((max - min + 1).max(1))
            } else {
                sample_int_provider_m(rng, c)
            };
            product *= n.max(1);
            saw = true;
        } else if ty == "minecraft:noise_threshold_count" {
            let below = m["below_noise"].as_i64().unwrap_or(5) as i32;
            let above = m["above_noise"].as_i64().unwrap_or(10) as i32;
            let n = rng.next_f64() * 2.0 - 1.0;
            let level = m["noise_level"].as_f64().unwrap_or(-0.8);
            product *= if n < level { below } else { above };
            saw = true;
        }
    }
    if saw {
        product.min(512)
    } else {
        1
    }
}

// ---------------------------------------------------------------------------
// vanilla ref loader (same decoder as ore_swap_dump.rs; names unprefixed)
// ---------------------------------------------------------------------------

fn load_vanilla(region_dir: &str, cx: i32, cz: i32) -> Option<HashMap<(u8, i32, u8), String>> {
    let (rx, rz) = (cx >> 5, cz >> 5);
    let path = std::path::PathBuf::from(format!("{region_dir}/r.{rx}.{rz}.mca"));
    let region = Region::open(&path).ok()?.with_coords(rx, rz);
    let data = region.get_chunk(cx & 31, cz & 31).ok()??;
    let nbt = read_nbt(&data).ok()?;
    if let Some(Tag::String(s)) = compound_get(&nbt.compound, "Status") {
        if !s.to_string().ends_with("full") {
            return None;
        }
    } else {
        return None;
    }
    let sections = match compound_get(&nbt.compound, "sections") {
        Some(Tag::List(List::Compound(l))) => l,
        _ => return None,
    };
    let mut map = HashMap::new();
    for sec in sections {
        let y_sec = match compound_get(sec, "Y") {
            Some(Tag::Byte(y)) => *y as i8 as i32,
            Some(Tag::Int(y)) => *y,
            _ => continue,
        };
        let Some(Tag::Compound(bs)) = compound_get(sec, "block_states") else { continue };
        let Some(Tag::List(List::Compound(palette))) = compound_get(bs, "palette") else { continue };
        let names: Vec<String> = palette
            .iter()
            .map(|pc| match compound_get(pc, "Name") {
                Some(Tag::String(s)) => s.to_string().trim_start_matches("minecraft:").to_string(),
                _ => "air".into(),
            })
            .collect();
        if names.len() == 1 {
            for i in 0..4096u32 {
                map.insert(
                    ((i & 15) as u8, y_sec * 16 + (i >> 8) as i32, ((i >> 4) & 15) as u8),
                    names[0].clone(),
                );
            }
            continue;
        }
        let bits = ((names.len() - 1).ilog2() + 1).max(4) as u32;
        let Some(Tag::LongArray(d)) = compound_get(bs, "data") else { continue };
        let longs: Vec<i64> = d.to_vec();
        let epl = 64 / bits;
        let mask = (1u64 << bits) - 1;
        for i in 0..4096u32 {
            let li = (i / epl) as usize;
            let bo = (i % epl) * bits;
            let vidx = ((longs[li] as u64) >> bo) & mask;
            map.insert(
                ((i & 15) as u8, y_sec * 16 + (i >> 8) as i32, ((i >> 4) & 15) as u8),
                names.get(vidx as usize).cloned().unwrap_or_default(),
            );
        }
    }
    Some(map)
}

// ---------------------------------------------------------------------------
// run records + buffer diffing
// ---------------------------------------------------------------------------

#[derive(Clone, PartialEq)]
struct Attempt {
    x: i32,
    y: i32,
    z: i32,
    gate: bool,
}

#[derive(Clone)]
struct RunRec {
    seq: usize,
    opos: usize,
    origin: (i32, i32),
    step: i32,
    fidx: i32,
    name: String,
    /// feature-rng draws consumed (delta); None when an internal rng drives
    /// the feature (vein/patch probes).
    draws: Option<u32>,
    attempts: Vec<Attempt>,
    writes: Vec<(i32, i32, i32, u16)>,
}

fn diff_region(before: &[u16], region: &RegionBuf) -> Vec<(i32, i32, i32, u16)> {
    let mut out = Vec::new();
    for (i, (&before_b, &after)) in before.iter().zip(region.blocks.iter()).enumerate() {
        if before_b != after {
            let y = (i / (region.side as usize * region.side as usize)) as i32 + WORLD_BOTTOM;
            let rem = i % (region.side as usize * region.side as usize);
            let lz = (rem / region.side as usize) as i32;
            let lx = (rem % region.side as usize) as i32;
            out.push((
                region.origin_x + lx,
                y,
                region.origin_z + lz,
                region.blocks[i],
            ));
        }
    }
    out
}

/// Ocean-floor first-available from the buffer's STORED pre-carver
/// heightmaps (features.rs:689 semantics).
fn ocean_floor_wg_first_available_m(region: &RegionBuf, x: i32, z: i32) -> Option<i32> {
    let lx = x - region.origin_x;
    let lz = z - region.origin_z;
    if lx < 0 || lz < 0 || lx >= region.side || lz >= region.side {
        return None;
    }
    let cxl = lx / 16;
    let czl = lz / 16;
    let hi = (czl * region.chunks + cxl) as usize;
    let hm = region.heightmaps.get(hi)?;
    let solid_y = hm[((lz % 16) * 16 + (lx % 16)) as usize] as i32;
    if solid_y <= WORLD_BOTTOM {
        return None;
    }
    Some(solid_y + 1)
}

fn ocean_floor_wg_allows_ore_m(
    region: &RegionBuf,
    start_x: i32,
    start_y: i32,
    start_z: i32,
    size_xz: i32,
) -> bool {
    for x in start_x..=start_x + size_xz {
        for z in start_z..=start_z + size_xz {
            if let Some(h) = ocean_floor_wg_first_available_m(region, x, z) {
                if start_y <= h {
                    return true;
                }
            }
        }
    }
    false
}

#[inline]
fn lerp_m(t: f64, a: f64, b: f64) -> f64 {
    a + t * (b - a)
}

#[inline]
fn floor_m(v: f64) -> i32 {
    v.floor() as i32
}

fn sin_tbl_m(v: f64) -> f32 {
    let idx = ((v * SIN_SCALE) as i64 as u64 & 0xFFFF) as usize;
    static TABLE: std::sync::OnceLock<Vec<f32>> = std::sync::OnceLock::new();
    TABLE
        .get_or_init(|| (0..65536usize).map(|i| (i as f64 / SIN_SCALE).sin() as f32).collect())[idx]
}

// ---------------------------------------------------------------------------
// step-6 runners (ore_clay, disk_clay) — mirrors of features.rs place_feature
// ---------------------------------------------------------------------------

fn is_base_stone_overworld(b: BlockId) -> bool {
    matches!(
        b,
        BlockId::Stone | BlockId::Deepslate | BlockId::Granite | BlockId::Diorite | BlockId::Andesite | BlockId::Tuff
    )
}

/// place_ore_blob_inner (features.rs:1046) with the ore_clay target
/// (tag base_stone_overworld → clay, discard 0.0 → no air dice).
fn place_clay_blob(
    rng: &mut FeatureRandom,
    region: &mut RegionBuf,
    ox: i32,
    oy: i32,
    oz: i32,
    out: &mut Vec<(i32, i32, i32, u16)>,
) {
    let size = 33i32;
    let angle = rng.next_f32() * PI;
    let f = size as f32 / 8.0;
    let start_x = ox as f64 + (angle as f64).sin() * f as f64;
    let end_x = ox as f64 - (angle as f64).sin() * f as f64;
    let start_z = oz as f64 + (angle as f64).cos() * f as f64;
    let end_z = oz as f64 - (angle as f64).cos() * f as f64;
    let start_y = (oy + rng.next_int(3) - 2) as f64;
    let end_y = (oy + rng.next_int(3) - 2) as f64;

    let cellw = ((size as f32 / 16.0 * 2.0 + 1.0) / 2.0).ceil() as i32;
    let f_ceil = f.ceil() as i32;
    let sbx = ox - f_ceil - cellw;
    let sby = oy - 2 - cellw;
    let sbz = oz - f_ceil - cellw;
    let size_xz = 2 * (f_ceil + cellw);
    let size_y = 2 * (2 + cellw);
    if !ocean_floor_wg_allows_ore_m(region, sbx, sby, sbz, size_xz) {
        return;
    }

    let mut spheres = vec![0f64; (size as usize) * 4];
    for i in 0..size {
        let t = i as f32 / size as f32;
        let td = t as f64;
        let blip = rng.next_f64() * size as f64 / 16.0;
        let sin_part = (sin_tbl_m((PI * t) as f64) + 1.0f32) as f64;
        let radius = (sin_part * blip + 1.0) / 2.0;
        let base = (i as usize) * 4;
        spheres[base] = ox as f64 + lerp_m(td, start_x - ox as f64, end_x - ox as f64);
        spheres[base + 1] = lerp_m(td, start_y, end_y);
        spheres[base + 2] = oz as f64 + lerp_m(td, start_z - oz as f64, end_z - oz as f64);
        spheres[base + 3] = radius;
    }
    for i in 0..size - 1 {
        let bi = (i as usize) * 4;
        if spheres[bi + 3] <= 0.0 {
            continue;
        }
        for j in (i + 1)..size {
            let bj = (j as usize) * 4;
            if spheres[bj + 3] <= 0.0 {
                continue;
            }
            let dx = spheres[bi] - spheres[bj];
            let dy = spheres[bi + 1] - spheres[bj + 1];
            let dz = spheres[bi + 2] - spheres[bj + 2];
            let dr = spheres[bi + 3] - spheres[bj + 3];
            if dr * dr > dx * dx + dy * dy + dz * dz {
                if dr > 0.0 {
                    spheres[bj + 3] = -1.0;
                } else {
                    spheres[bi + 3] = -1.0;
                }
            }
        }
    }
    let mut bitset = vec![false; (size_xz * size_y * size_xz).max(1) as usize];
    for i in 0..size {
        let bi = (i as usize) * 4;
        let radius = spheres[bi + 3];
        if radius < 0.0 {
            continue;
        }
        let scx = spheres[bi];
        let scy = spheres[bi + 1];
        let scz = spheres[bi + 2];
        let min_x = floor_m(scx - radius).max(sbx);
        let min_y = floor_m(scy - radius).max(sby);
        let min_z = floor_m(scz - radius).max(sbz);
        let max_x = floor_m(scx + radius).max(min_x);
        let max_y = floor_m(scy + radius).max(min_y);
        let max_z = floor_m(scz + radius).max(min_z);
        for x in min_x..=max_x {
            let dx = ((x as f64 + 0.5) - scx) / radius;
            if dx * dx >= 1.0 {
                continue;
            }
            for y in min_y..=max_y {
                if y < WORLD_BOTTOM || y >= WORLD_TOP {
                    continue;
                }
                let dy = ((y as f64 + 0.5) - scy) / radius;
                if dx * dx + dy * dy >= 1.0 {
                    continue;
                }
                for z in min_z..=max_z {
                    let dz = ((z as f64 + 0.5) - scz) / radius;
                    if dx * dx + dy * dy + dz * dz >= 1.0 {
                        continue;
                    }
                    let bit = ((x - sbx) + (y - sby) * size_xz + (z - sbz) * size_xz * size_y)
                        as usize;
                    if bit >= bitset.len() || bitset[bit] {
                        continue;
                    }
                    bitset[bit] = true;
                    if region.index(x, y, z).is_none() {
                        continue;
                    }
                    let existing = region.get(x, y, z);
                    if is_base_stone_overworld(existing) {
                        region.set(x, y, z, BlockId::Clay);
                        out.push((x, y, z, BlockId::Clay.as_u16()));
                    }
                }
            }
        }
    }
}

fn run_ore_clay(
    seq: usize,
    opos: usize,
    region: &mut RegionBuf,
    state: &WorldgenState,
    seed: i64,
    ox0: i32,
    oz0: i32,
) -> RunRec {
    let before = region.blocks.clone();
    let idx = feature_catalog::global_feature_index(6, "ore_clay").unwrap_or(27);
    let mut rng = FeatureRandom::new(seed);
    let dec = rng.set_decoration_seed(seed, ox0, oz0);
    rng.set_feature_seed(dec, idx, 6);
    let mut attempts = Vec::new();
    let mut writes = Vec::new();
    for _ in 0..46 {
        let lx = rng.next_int(16);
        let lz = rng.next_int(16);
        let x = ox0 + lx;
        let z = oz0 + lz;
        let y = -64 + rng.next_int(321); // uniform above_bottom 0 .. absolute 256
        let bid = biome_id_at_block(state, x, y, z);
        let bn = biome_id_to_name(bid);
        let listed = feature_catalog::features_at_step(bn, 6)
            .iter()
            .any(|f| f.trim_start_matches("minecraft:") == "ore_clay");
        let gate = bid == biome_id::LUSH_CAVES && listed;
        attempts.push(Attempt { x, y, z, gate });
        if !gate {
            continue;
        }
        place_clay_blob(&mut rng, region, x, y, z, &mut writes);
    }
    let draws = rng.draw_count();
    let extra = diff_region(&before, region);
    for e in extra {
        writes.push(e);
    }
    RunRec {
        seq,
        opos,
        origin: (ox0, oz0),
        step: 6,
        fidx: idx,
        name: "ore_clay".into(),
        draws: Some(draws),
        attempts,
        writes,
    }
}

fn run_disk_clay(
    seq: usize,
    opos: usize,
    region: &mut RegionBuf,
    state: &WorldgenState,
    seed: i64,
    ox0: i32,
    oz0: i32,
) -> RunRec {
    let before = region.blocks.clone();
    let idx = feature_catalog::global_feature_index(6, "disk_clay").unwrap_or(31);
    let mut rng = FeatureRandom::new(seed);
    let dec = rng.set_decoration_seed(seed, ox0, oz0);
    rng.set_feature_seed(dec, idx, 6);
    let mut attempts = Vec::new();
    let mut writes = Vec::new();
    // count = 1 (no count modifier), heightmap OCEAN_FLOOR_WG + matching_fluids
    let lx = rng.next_int(16);
    let lz = rng.next_int(16);
    let x = ox0 + lx;
    let z = oz0 + lz;
    let mut gate = false;
    if let Some(base) = ocean_floor_wg_first_available_m(region, x, z) {
        let y = base;
        let water_ok = y >= WORLD_BOTTOM
            && y < WORLD_TOP
            && region.get(x, y, z) == BlockId::Water;
        let bid = biome_id_at_block(state, x, y, z);
        let bn = biome_id_to_name(bid);
        let listed = feature_catalog::features_at_step(bn, 6)
            .iter()
            .any(|f| f.trim_start_matches("minecraft:") == "disk_clay");
        if water_ok && listed {
            // DiskFeature.place: radius uniform 2..3, half_height 1,
            // target dirt|clay → clay.
            let r = 2 + rng.next_int(2);
            let top = y + 1;
            let bottom = y - 2;
            for zc in (z - r)..=(z + r) {
                for xx in (x - r)..=(x + r) {
                    let xd = xx - x;
                    let zd = zc - z;
                    if xd * xd + zd * zd > r * r {
                        continue;
                    }
                    for yy in (bottom + 1..=top).rev() {
                        if yy < WORLD_BOTTOM || yy >= WORLD_TOP {
                            continue;
                        }
                        if region.index(xx, yy, zc).is_none() {
                            continue;
                        }
                        let ex = region.get(xx, yy, zc);
                        if ex == BlockId::Dirt || ex == BlockId::Clay {
                            region.set(xx, yy, zc, BlockId::Clay);
                            writes.push((xx, yy, zc, BlockId::Clay.as_u16()));
                        }
                    }
                }
            }
            gate = true;
        }
        attempts.push(Attempt { x, y, z, gate });
    }
    let draws = rng.draw_count();
    let extra = diff_region(&before, region);
    for e in extra {
        writes.push(e);
    }
    RunRec {
        seq,
        opos,
        origin: (ox0, oz0),
        step: 6,
        fidx: idx,
        name: "disk_clay".into(),
        draws: Some(draws),
        attempts,
        writes,
    }
}

// ---------------------------------------------------------------------------
// step-7 runners (sculk_vein via probe trace, sculk patch via production fn)
// ---------------------------------------------------------------------------

fn run_sculk_vein(
    seq: usize,
    opos: usize,
    region: &mut RegionBuf,
    state: &WorldgenState,
    seed: i64,
    ox0: i32,
    oz0: i32,
) -> RunRec {
    let before = region.blocks.clone();
    let idx = feature_catalog::global_feature_index(7, "sculk_vein").unwrap_or(0);
    // Per-attempt position+biome gate (same draw order as the feature).
    let gate = sculk::probe_vein_gate_origin(ox0, oz0, seed, idx, state);
    // Full replay with per-attempt events (SOLID/PLACED/FAILED).
    let (_events, _faces) = sculk::probe_vein_origin_traced(region, ox0, oz0, seed, idx, &gate);
    let attempts: Vec<Attempt> = gate
        .iter()
        .map(|&(x, y, z, ok)| Attempt { x, y, z, gate: ok != 0 })
        .collect();
    let writes = diff_region(&before, region);
    RunRec {
        seq,
        opos,
        origin: (ox0, oz0),
        step: 7,
        fidx: idx,
        name: "sculk_vein".to_string(),
        draws: None,
        attempts,
        writes,
    }
}

fn run_sculk_patch(
    seq: usize,
    opos: usize,
    region: &mut RegionBuf,
    state: &WorldgenState,
    seed: i64,
    ox0: i32,
    oz0: i32,
    faces: &mut FaceMap,
) -> RunRec {
    let before = region.blocks.clone();
    let idx = feature_catalog::global_feature_index(7, "sculk_patch_deep_dark").unwrap_or(1);
    let attempts = sculk::probe_patch_gate_origin(ox0, oz0, seed, idx, state)
        .into_iter()
        .map(|(x, y, z, ok)| Attempt { x, y, z, gate: ok != 0 })
        .collect();
    let empty: Vec<(i32, i32)> = Vec::new();
    sculk::apply_sculk_patch_only(region, state, ox0, oz0, &empty, faces);
    let writes = diff_region(&before, region);
    RunRec {
        seq,
        opos,
        origin: (ox0, oz0),
        step: 7,
        fidx: idx,
        name: "sculk_patch_deep_dark".into(),
        draws: None,
        attempts,
        writes,
    }
}

// ---------------------------------------------------------------------------
// step-9 logging mirror of place_placed_feature_step (feature_dispatch/mod.rs
// :260-478) — READ-ONLY: consumes a shadow rng identical to the write pass.
// ---------------------------------------------------------------------------

fn eval_predicate_m(region: &RegionBuf, x: i32, y: i32, z: i32, pred: &Value) -> bool {
    match pred["type"].as_str().unwrap_or("") {
        "minecraft:matching_block_tag" => {
            let tag = pred["tag"].as_str().unwrap_or("");
            let b = region.get(x, y, z);
            match tag.trim_start_matches("minecraft:") {
                "air" => is_air_m(b),
                "dirt" => matches!(
                    b,
                    BlockId::Dirt | BlockId::CoarseDirt | BlockId::RootedDirt | BlockId::GrassBlock | BlockId::Podzol | BlockId::Mycelium | BlockId::Mud
                ),
                "base_stone_overworld" => is_base_stone_overworld(b),
                "moss_replaceable" | "lush_ground_replaceable" => matches!(
                    b,
                    BlockId::Stone
                        | BlockId::Deepslate
                        | BlockId::Granite
                        | BlockId::Diorite
                        | BlockId::Andesite
                        | BlockId::Tuff
                        | BlockId::Dirt
                        | BlockId::CoarseDirt
                        | BlockId::RootedDirt
                        | BlockId::GrassBlock
                        | BlockId::Podzol
                        | BlockId::Mycelium
                        | BlockId::Mud
                        | BlockId::Clay
                        | BlockId::MossBlock
                        | BlockId::Calcite
                ),
                _ => false,
            }
        }
        "minecraft:matching_blocks" => {
            let ox = pred["offset"].as_array().and_then(|a| a.first()).and_then(|v| v.as_i64()).unwrap_or(0) as i32;
            let oy = pred["offset"].as_array().and_then(|a| a.get(1)).and_then(|v| v.as_i64()).unwrap_or(0) as i32;
            let oz = pred["offset"].as_array().and_then(|a| a.get(2)).and_then(|v| v.as_i64()).unwrap_or(0) as i32;
            let b = region.get(x + ox, y + oy, z + oz);
            if let Some(name) = pred["blocks"].as_str() {
                BlockId::from_name(name) == Some(b)
            } else if let Some(arr) = pred["blocks"].as_array() {
                arr.iter().any(|n| n.as_str().and_then(BlockId::from_name).map(|id| id == b).unwrap_or(false))
            } else {
                true
            }
        }
        "minecraft:would_survive" => supports_vegetation_m(region.get(x, y - 1, z)),
        "minecraft:solid" => blocks_motion_m(region.get(x, y, z)),
        "minecraft:has_sturdy_face" => {
            // full cubes only in the port (is_face_sturdy); direction "down"
            let _ = pred["direction"].as_str();
            blocks_motion_m(region.get(x, y, z))
        }
        "minecraft:true" => true,
        "minecraft:all_of" => pred["predicates"]
            .as_array()
            .map(|a| a.iter().all(|p| eval_predicate_m(region, x, y, z, p)))
            .unwrap_or(true),
        "minecraft:any_of" => pred["predicates"]
            .as_array()
            .map(|a| a.iter().any(|p| eval_predicate_m(region, x, y, z, p)))
            .unwrap_or(true),
        "minecraft:not" => !eval_predicate_m(region, x, y, z, &pred["predicate"]),
        _ => true,
    }
}

/// One draw of the modifier chain; returns (x, y, z, accepted).
#[allow(clippy::too_many_arguments)]
fn logging_draw_m(
    rng: &mut FeatureRandom,
    region: &RegionBuf,
    state: &WorldgenState,
    placed: &Value,
    placed_id: &str,
    ox0: i32,
    oz0: i32,
    gen_step: i32,
) -> (i32, i32, i32, bool) {
    let mut x = ox0;
    let mut z = oz0;
    let mut y = 0i32;
    let mut ok = true;
    let mut has_xz = false;
    if let Some(mods) = placed["placement"].as_array() {
        for m in mods {
            let ty = m["type"].as_str().unwrap_or("");
            match ty {
                "minecraft:count" | "minecraft:count_on_every_layer" => {}
                "minecraft:noise_threshold_count" => {}
                "minecraft:in_square" => {
                    x = ox0 + rng.next_int(16);
                    z = oz0 + rng.next_int(16);
                    has_xz = true;
                }
                "minecraft:height_range" => {
                    y = sample_height_m(rng, &m["height"]);
                }
                "minecraft:heightmap" => {
                    if !has_xz {
                        x = ox0 + rng.next_int(16);
                        z = oz0 + rng.next_int(16);
                        has_xz = true;
                    }
                    let kind = if m["heightmap"].as_str().unwrap_or("").contains("OCEAN_FLOOR") {
                        HmKind::OceanFloor
                    } else {
                        HmKind::WorldSurface
                    };
                    match heightmap_top_m(region, x, z, kind) {
                        Some(sy) => y = sy + 1,
                        None => ok = false,
                    }
                }
                "minecraft:random_offset" => {
                    let ox = sample_int_provider_m(rng, &m["xz_spread"]);
                    let oy = sample_int_provider_m(rng, &m["y_spread"]);
                    let oz = sample_int_provider_m(rng, &m["xz_spread"]);
                    x += ox;
                    y += oy;
                    z += oz;
                }
                "minecraft:environment_scan" => {
                    let dir = m["direction_of_search"].as_str().unwrap_or("down");
                    let max_steps = m["max_steps"].as_i64().unwrap_or(12) as i32;
                    let true_pred = serde_json::json!({"type": "minecraft:true"});
                    let allowed = m.get("allowed_search_condition").unwrap_or(&true_pred);
                    let target = &m["target_condition"];
                    let mut py = y;
                    let mut found = None;
                    let mut out_of_world = false;
                    if !eval_predicate_m(region, x, py, z, allowed) {
                        ok = false;
                        break;
                    }
                    for _ in 0..max_steps {
                        if eval_predicate_m(region, x, py, z, target) {
                            found = Some(py);
                            break;
                        }
                        py += if dir == "down" { -1 } else { 1 };
                        if py < WORLD_BOTTOM || py >= WORLD_TOP {
                            out_of_world = true;
                            break;
                        }
                        if !eval_predicate_m(region, x, py, z, allowed) {
                            break;
                        }
                    }
                    if !out_of_world
                        && found.is_none()
                        && eval_predicate_m(region, x, py, z, target)
                    {
                        found = Some(py);
                    }
                    match found {
                        Some(fy) => y = fy,
                        None => ok = false,
                    }
                }
                "minecraft:biome" => {
                    let bname = biome_id_to_name(biome_id_at_block(state, x, y, z));
                    let step_list = feature_catalog::features_at_step(&bname, gen_step);
                    let id = placed_id.trim_start_matches("minecraft:");
                    if !step_list.iter().any(|f| f.trim_start_matches("minecraft:") == id) {
                        ok = false;
                    }
                }
                "minecraft:block_predicate_filter" => {
                    if !eval_predicate_m(region, x, y, z, &m["predicate"]) {
                        ok = false;
                    }
                }
                "minecraft:rarity_filter" => {
                    let chance = m["chance"].as_i64().unwrap_or(1) as i32;
                    if chance <= 0 || rng.next_f32() >= 1.0 / chance as f32 {
                        ok = false;
                    }
                }
                "minecraft:surface_relative_threshold_filter" => {
                    let kind = if m["heightmap"].as_str().unwrap_or("").contains("OCEAN_FLOOR") {
                        HmKind::OceanFloor
                    } else {
                        HmKind::WorldSurface
                    };
                    let min_inc = m["min_inclusive"].as_i64().unwrap_or(i32::MIN as i64);
                    let max_inc = m["max_inclusive"].as_i64().unwrap_or(i32::MAX as i64);
                    let surface = heightmap_top_m(region, x, z, kind)
                        .map(|s| s as i64 + 1)
                        .unwrap_or(i64::MIN / 4);
                    let yy = y as i64;
                    if !(surface + min_inc <= yy && yy <= surface + max_inc) {
                        ok = false;
                    }
                }
                _ => {}
            }
        }
    }
    if !has_xz {
        x = ox0 + rng.next_int(16);
        z = oz0 + rng.next_int(16);
    }
    (x, y, z, ok)
}

fn run_step9(
    seq: usize,
    opos: usize,
    region: &mut RegionBuf,
    state: &WorldgenState,
    seed: i64,
    ox0: i32,
    oz0: i32,
    placed_id: &str,
    fidx: i32,
) -> RunRec {
    let before = region.blocks.clone();
    let placed = feature_catalog::load_placed_feature(placed_id).cloned();
    let mut attempts = Vec::new();
    if let Some(placed) = &placed {
        // logging pass: shadow rng, same seed path as the write pass
        let mut rng2 = FeatureRandom::new(seed);
        let dec = rng2.set_decoration_seed(seed, ox0, oz0);
        rng2.set_feature_seed(dec, fidx, 9);
        let count = placement_count_m(&mut rng2, placed);
        for _ in 0..count {
            let (x, y, z, ok) =
                logging_draw_m(&mut rng2, region, state, placed, placed_id, ox0, oz0, 9);
            attempts.push(Attempt { x, y, z, gate: ok });
        }
    }
    // write pass: production dispatcher (identical modifier draws)
    let mut rng = FeatureRandom::new(seed);
    let dec = rng.set_decoration_seed(seed, ox0, oz0);
    rng.set_feature_seed(dec, fidx, 9);
    place_placed_feature(&mut rng, region, state, ox0, oz0, placed_id);
    let draws = rng.draw_count();
    let writes = diff_region(&before, region);
    RunRec {
        seq,
        opos,
        origin: (ox0, oz0),
        step: 9,
        fidx,
        name: placed_id.trim_start_matches("minecraft:").to_string(),
        draws: Some(draws),
        attempts,
        writes,
    }
}

// ---------------------------------------------------------------------------
// chain replay driver (per side)
// ---------------------------------------------------------------------------

/// Step-9 feature list for one origin: union of the 3×3 neighbourhood biome
/// feature lists (feature_dispatch/mod.rs origin_biome_union), filtered to the
/// lush family, in global FeatureSorter order.
fn step9_family_ids(
    region: &RegionBuf,
    state: &WorldgenState,
    ox0: i32,
    oz0: i32,
) -> Vec<(i32, String)> {
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
                            let id = noise_biome_at_quart(
                                state,
                                cx0 / 4 + bx4,
                                base_y_q + sy4,
                                cz0 / 4 + bz4,
                            );
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
    let mut merged: Vec<(i32, String)> = Vec::new();
    for b in &names {
        for f in feature_catalog::features_at_step(b, 9) {
            let short = f.trim_start_matches("minecraft:").to_string();
            if !STEP9_FAMILY.iter().any(|k| short.contains(k)) {
                continue;
            }
            if let Some(idx) = feature_catalog::global_feature_index(9, &f) {
                if !merged.iter().any(|(_, s)| *s == short) {
                    merged.push((idx, short));
                }
            }
        }
    }
    merged.sort_by_key(|(i, _)| *i);
    merged
}

fn replay_side(
    side: &str,
    region: &mut RegionBuf,
    state: &WorldgenState,
    seed: i64,
    log: &mut impl Write,
) -> Vec<RunRec> {
    let chunks = region.chunks;
    let origin_x = region.origin_x;
    let origin_z = region.origin_z;
    // ticket_sim default: window_order (deco_schedule.rs:741). The env var is
    // unset (enforced in main) so this equals sculk::decoration_origin_order.
    let order = deco_schedule::window_order(chunks, origin_x, origin_z);
    let mid = chunks / 2;
    let inner: Vec<(usize, (i32, i32))> = order
        .iter()
        .enumerate()
        .filter(|(_, &(cxl, czl))| (cxl - mid).abs() <= 1 && (czl - mid).abs() <= 1)
        .map(|(i, &p)| (i, p))
        .collect();
    let mut seq = 0usize;
    let mut runs = Vec::new();
    let mut faces: FaceMap = HashMap::new();
    for &(opos, (cxl, czl)) in &inner {
        let ox0 = origin_x + cxl * 16;
        let oz0 = origin_z + czl * 16;
        // step 6
        let r = run_ore_clay(seq, opos, region, state, seed, ox0, oz0);
        seq += 1;
        runs.push(r);
        let r = run_disk_clay(seq, opos, region, state, seed, ox0, oz0);
        seq += 1;
        runs.push(r);
        // step 7 (vein first, then patch — mirrors decorate_region_origin_major)
        let r = run_sculk_vein(seq, opos, region, state, seed, ox0, oz0);
        seq += 1;
        runs.push(r);
        let r = run_sculk_patch(seq, opos, region, state, seed, ox0, oz0, &mut faces);
        seq += 1;
        runs.push(r);
        // step 9
        let ids = step9_family_ids(region, state, ox0, oz0);
        for (fidx, id) in ids {
            let placed_id = format!("minecraft:{id}");
            let r = run_step9(seq, opos, region, state, seed, ox0, oz0, &placed_id, fidx);
            seq += 1;
            runs.push(r);
        }
        writeln!(
            log,
            "[{side}] origin {opos} ({cxl},{czl}) w({ox0},{oz0}) done ({} runs)",
            runs.len()
        )
        .ok();
    }
    runs
}

// ---------------------------------------------------------------------------
// classification + reporting
// ---------------------------------------------------------------------------

struct WindowReport {
    census: Vec<CensusRow>,
    root: Option<String>,
    root_detail: Vec<String>,
    class_counts: BTreeMap<String, u64>,
    fam_counts: BTreeMap<String, u64>,
    divergent_runs: Vec<String>,
    fidelity_gaps: u64,
}

struct CensusRow {
    x: i32,
    y: i32,
    z: i32,
    van: String,
    neu: String,
}

fn classify_window(
    wx: i32,
    wz: i32,
    van: &HashMap<(u8, i32, u8), String>,
    raw_neu: &[u16],
    van_final_replay: &HashMap<(i32, i32, i32), BlockId>,
    neu_final_replay: &HashMap<(i32, i32, i32), BlockId>,
    van_runs: &[RunRec],
    neu_runs: &[RunRec],
    pristine_van: &[u16],
    pristine_neu: &[u16],
    buf_origin: (i32, i32),
    buf_side: i32,
    csv: &mut impl Write,
    rootlog: &mut impl Write,
) -> WindowReport {
    let mut rep = WindowReport {
        census: Vec::new(),
        root: None,
        root_detail: Vec::new(),
        class_counts: BTreeMap::new(),
        fam_counts: BTreeMap::new(),
        divergent_runs: Vec::new(),
        fidelity_gaps: 0,
    };

    // ---- census over the consumer chunk ----
    let x_lo = wx * 16;
    let z_lo = wz * 16;
    for lz in 0..16i32 {
        for lx in 0..16i32 {
            for y in WORLD_BOTTOM..WORLD_TOP {
                let vn = van
                    .get(&(lx as u8, y, lz as u8))
                    .map(|s| s.as_str())
                    .unwrap_or("air");
                let raw_i = (((y - WORLD_BOTTOM) * 256 + lz * 16 + lx)) as usize;
                let nb = raw_neu
                    .get(raw_i)
                    .and_then(|&v| BlockId::from_u16(v))
                    .unwrap_or(BlockId::Air);
                let nb_name_full = vanilla_name(nb);
                let nn = nb_name_full.trim_start_matches("minecraft:");
                if vn == nn {
                    continue;
                }
                let v_lush = LUSH_FAMILY_NAMES.contains(&vn);
                let n_lush = LUSH_FAMILY_NAMES.contains(&nn);
                if !v_lush && !n_lush {
                    continue;
                }
                rep.census.push(CensusRow {
                    x: x_lo + lx,
                    y,
                    z: z_lo + lz,
                    van: vn.to_string(),
                    neu: nn.to_string(),
                });
                let fam = if family_of(vn) != "other" { family_of(vn) } else { family_of(nn) };
                *rep.fam_counts.entry(fam.to_string()).or_insert(0) += 1;
            }
        }
    }

    // ---- run comparison ----
    assert_eq!(van_runs.len(), neu_runs.len(), "run lists out of sync");
    let mut root_seq: Option<usize> = None;
    let mut root_kind = "";
    for (rv, rn) in van_runs.iter().zip(neu_runs.iter()) {
        let draws_differ = match (rv.draws, rn.draws) {
            (Some(a), Some(b)) => a != b,
            _ => false,
        };
        let attempts_differ = rv.attempts != rn.attempts;
        let writes_differ = rv.writes != rn.writes;
        if draws_differ || attempts_differ || writes_differ {
            if root_seq.is_none() {
                root_seq = Some(rv.seq);
                root_kind = if draws_differ { "stream" } else { "gate" };
                rep.root = Some(format!(
                    "seq={} origin({},{}) step={} {} kind={root_kind}",
                    rv.seq, rv.origin.0, rv.origin.1, rv.step, rv.name
                ));
            }
            let kind = if draws_differ {
                "stream"
            } else if attempts_differ {
                "gate(attempts)"
            } else {
                "gate(writes)"
            };
            let n_cells = {
                let sv: HashSet<_> = rv.writes.iter().collect();
                let sn: HashSet<_> = rn.writes.iter().collect();
                sv.symmetric_difference(&sn).count()
            };
            rep.divergent_runs.push(format!(
                "seq={} origin({},{}) {} [{}] diff_cells={n_cells}",
                rv.seq, rv.origin.0, rv.origin.1, rv.name, kind
            ));
        }
    }

    // ---- root fine dump ----
    if let Some(rs) = root_seq {
        let rv = &van_runs[rs];
        let rn = &neu_runs[rs];
        writeln!(
            rootlog,
            "== window ({wx},{wz}) ROOT seq={} {} origin({},{}) step={} kind={root_kind}",
            rs, rv.name, rv.origin.0, rv.origin.1, rv.step
        )
        .ok();
        writeln!(
            rootlog,
            "   draws van={:?} neu={:?}  attempts van={} neu={}  writes van={} neu={}",
            rv.draws,
            rn.draws,
            rv.attempts.len(),
            rn.attempts.len(),
            rv.writes.len(),
            rn.writes.len()
        )
        .ok();
        let first_accept = rv.attempts.iter().position(|a| a.gate);
        let mut onset: Option<(usize, String)> = None;
        for (k, (a, b)) in rv.attempts.iter().zip(rn.attempts.iter()).enumerate() {
            if a != b {
                onset = Some((
                    k,
                    format!(
                        "attempt diverges: van=({},{},{},{}) neu=({},{},{},{})",
                        a.x, a.y, a.z, a.gate, b.x, b.y, b.z, b.gate
                    ),
                ));
                break;
            }
        }
        match (&onset, first_accept) {
            (None, Some(a)) => {
                writeln!(
                    rootlog,
                    "   attempt logs IDENTICAL; divergence is inside the feature placement of accepted draws."
                )
                .ok();
                writeln!(
                    rootlog,
                    "   VERDICT(onset==first-accept): YES (onset at/after first accepted attempt index {a}; first divergent write below)"
                )
                .ok();
            }
            (Some((k, d)), Some(a)) => {
                writeln!(rootlog, "   first divergent attempt idx={k}: {d}").ok();
                writeln!(
                    rootlog,
                    "   VERDICT(onset==first-accept): {} (first accept idx={a}, onset idx={k})",
                    if *k == a { "YES" } else { "NO" }
                )
                .ok();
            }
            (Some((k, d)), None) => {
                writeln!(
                    rootlog,
                    "   first divergent attempt idx={k}: {d}; NO accepted attempts in this run"
                )
                .ok();
                writeln!(rootlog, "   VERDICT(onset==first-accept): NO (no accepts at all)").ok();
            }
            (None, None) => {
                writeln!(rootlog, "   no attempts recorded / no accepts").ok();
            }
        }
        // first divergent write cells (symmetric difference)
        let sv: HashSet<_> = rv.writes.iter().collect();
        let sn: HashSet<_> = rn.writes.iter().collect();
        let mut diffs: Vec<_> = sv.symmetric_difference(&sn).collect();
        diffs.sort_by_key(|w| (w.0, w.1, w.2));
        writeln!(rootlog, "   first divergent write cells (up to 24):").ok();
        for w in diffs.iter().take(24) {
            let (x, y, z, b) = ***w;
            let pre_v = pre_state(pristine_van, buf_origin, buf_side, van_runs, rs, x, y, z);
            let pre_n = pre_state(pristine_neu, buf_origin, buf_side, neu_runs, rs, x, y, z);
            let pv0 = pristine_get(pristine_van, buf_origin, buf_side, x, y, z);
            let pn0 = pristine_get(pristine_neu, buf_origin, buf_side, x, y, z);
            let upstream = if pre_v == pre_n {
                "same-pre-run(non-terrain?)".to_string()
            } else if pv0 != pn0 {
                if pv0.is_air() != pn0.is_air() {
                    "terrain-microdiff:carver-edge".to_string()
                } else {
                    "terrain-microdiff:surface-material".to_string()
                }
            } else {
                "chain-spill:earlier-run".to_string()
            };
            writeln!(
                rootlog,
                "     cell ({x},{y},{z}) van_pre={} neu_pre={} new_block={} upstream={upstream}",
                vanilla_name(pre_v),
                vanilla_name(pre_n),
                vanilla_name(BlockId::from_u16(b).unwrap_or(BlockId::Air)),
            )
            .ok();
        }
        if let Some((k, d)) = onset {
            rep.root_detail.push(format!("onset attempt idx={k}: {d}"));
        }
        rep.root_detail
            .push(format!("root kind={root_kind} first_accept={first_accept:?}"));
    }

    // ---- per-cell classification ----
    let van_writes = writes_map(van_runs);
    let neu_writes = writes_map(neu_runs);
    let root = root_seq;
    let root_is_stream = root_kind == "stream";
    for row in &rep.census {
        let key = (row.x, row.y, row.z);
        let rv = van_writes.get(&key);
        let rn = neu_writes.get(&key);
        let cls;
        let producers;
        match (rv, rn) {
            (Some(v), Some(n)) => {
                let lv = v.last().unwrap();
                let ln = n.last().unwrap();
                if lv.0 == ln.0 {
                    cls = if Some(lv.0) == root && root_kind == "stream" {
                        "stream"
                    } else {
                        "gate-input"
                    };
                } else {
                    cls = "winner";
                }
                producers = format!(
                    "van=[{}] neu=[{}]",
                    v.iter().map(|(s, _)| run_tag(van_runs, *s)).collect::<Vec<_>>().join(","),
                    n.iter().map(|(s, _)| run_tag(neu_runs, *s)).collect::<Vec<_>>().join(",")
                );
            }
            (Some(v), None) | (None, Some(v)) => {
                let side_has = if rv.is_some() { "van" } else { "neu" };
                let last = v.last().unwrap().0;
                cls = if root.map_or(false, |r| root_is_stream && last > r) {
                    "stream"
                } else {
                    "gate-input"
                };
                producers = format!("{side_has}=[{}]", run_tag(if rv.is_some() { van_runs } else { neu_runs }, last));
            }
            (None, None) => {
                let vfin = van_final_replay.get(&key).copied().unwrap_or(BlockId::Air);
                let nfin = neu_final_replay.get(&key).copied().unwrap_or(BlockId::Air);
                if vfin == nfin {
                    cls = "outside-chain";
                } else {
                    cls = "replay-gap";
                }
                producers = "[]".to_string();
            }
        }
        *rep.class_counts.entry(cls.to_string()).or_insert(0) += 1;
        writeln!(
            csv,
            "{},{},{},{},{},{},{},{}",
            row.x, row.y, row.z, row.van, row.neu, cls, producers,
            if family_of(&row.van) != "other" { family_of(&row.van) } else { family_of(&row.neu) }
        )
        .ok();
    }
    rep
}

fn run_tag(runs: &[RunRec], seq: usize) -> String {
    match runs.get(seq) {
        Some(r) => format!("s{}:{}@({},{})", seq, r.name, r.origin.0 >> 4, r.origin.1 >> 4),
        None => format!("s{seq}?"),
    }
}

fn writes_map(runs: &[RunRec]) -> HashMap<(i32, i32, i32), Vec<(usize, u16)>> {
    let mut out: HashMap<(i32, i32, i32), Vec<(usize, u16)>> = HashMap::new();
    for r in runs {
        for (x, y, z, b) in &r.writes {
            out.entry((*x, *y, *z)).or_default().push((r.seq, *b));
        }
    }
    out
}

/// Buffer state at cell (x,y,z) just before run `seq`: last write by an
/// earlier run, else the pristine (pre-chain) value.
fn pre_state(
    pristine: &[u16],
    origin: (i32, i32),
    side: i32,
    runs: &[RunRec],
    seq: usize,
    x: i32,
    y: i32,
    z: i32,
) -> BlockId {
    for r in runs[..seq.min(runs.len())].iter().rev() {
        for &(wx, wy, wz, wb) in r.writes.iter().rev() {
            if (wx, wy, wz) == (x, y, z) {
                return BlockId::from_u16(wb).unwrap_or(BlockId::Air);
            }
        }
    }
    let lx = x - origin.0;
    let lz = z - origin.1;
    if y < WORLD_BOTTOM || y >= WORLD_TOP || lx < 0 || lz < 0 || lx >= side || lz >= side {
        return BlockId::Air;
    }
    let i = ((y - WORLD_BOTTOM) as usize) * (side as usize) * (side as usize)
        + (lz as usize) * (side as usize)
        + (lx as usize);
    BlockId::from_u16(pristine.get(i).copied().unwrap_or(0)).unwrap_or(BlockId::Air)
}

fn pristine_get(
    pristine: &[u16],
    origin: (i32, i32),
    side: i32,
    x: i32,
    y: i32,
    z: i32,
) -> BlockId {
    pre_state(pristine, origin, side, &[], 0, x, y, z)
}

// ---------------------------------------------------------------------------
// main
// ---------------------------------------------------------------------------

fn main() {
    let mut args = std::env::args().skip(1);
    let seed: i64 = args.next().and_then(|s| s.parse().ok()).unwrap_or(424242);
    let region_dir = args.next().unwrap_or_else(|| {
        "tools/nbt-ref/vanilla-fresh-424242/world/dimensions/minecraft/overworld/region".to_string()
    });
    let out_dir = args.next().unwrap_or_else(|| "/tmp/opencode".to_string());
    std::fs::create_dir_all(&out_dir).ok();

    // The generator MUST run with the ticket_sim default.
    if let Some(o) = std::env::var("NEUTRON_SCULK_ORIGIN_ORDER").ok() {
        panic!("NEUTRON_SCULK_ORIGIN_ORDER={o} is set — unset it (ticket_sim default required)");
    }

    println!("=== lush_chain_dump seed={seed} windows={WINDOWS:?}");
    let t0 = std::time::Instant::now();
    let gen = ChunkGenerator::new(seed);
    let state = WorldgenState::overworld(seed);

    // ---- chunk inventory (all 5×5 buffers for both windows) ----
    let (cx_min, cx_max) = (
        WINDOWS.iter().map(|w| w.0).min().unwrap() - BUF_RADIUS,
        WINDOWS.iter().map(|w| w.0).max().unwrap() + BUF_RADIUS,
    );
    let (cz_min, cz_max) = (
        WINDOWS.iter().map(|w| w.1).min().unwrap() - BUF_RADIUS,
        WINDOWS.iter().map(|w| w.1).max().unwrap() + BUF_RADIUS,
    );
    let mut coords: Vec<(i32, i32)> = Vec::new();
    for cz in cz_min..=cz_max {
        for cx in cx_min..=cx_max {
            coords.push((cx, cz));
        }
    }
    println!(
        "[inv] generating {} chunks ({}..{} x {}..{})",
        coords.len(),
        cx_min,
        cx_max,
        cz_min,
        cz_max
    );
    let workers = 4usize;
    let inventory: HashMap<(i32, i32), (Vec<u16>, Vec<i16>)> = std::thread::scope(|s| {
        let mut handles = Vec::new();
        for chunk_chunk in coords.chunks(coords.len().div_ceil(workers)) {
            let gen = &gen;
            handles.push(s.spawn(move || {
                let mut cache = NoiseCache::new();
                let mut out = Vec::new();
                for &(cx, cz) in chunk_chunk {
                    let g = gen.generate_chunk_cached(cx, cz, &mut cache);
                    // pre-carver heightmap (ore/disk gates read THIS)
                    let (_, hm_pre, _) = gen.generate_noise_and_surface(cx, cz);
                    out.push(((cx, cz), g.blocks, hm_pre));
                }
                out
            }));
        }
        handles
            .into_iter()
            .flat_map(|h| h.join().unwrap())
            .map(|(k, b, h)| (k, (b, h)))
            .collect::<HashMap<_, _>>()
    });
    println!("[inv] done in {:.0}s", t0.elapsed().as_secs_f64());

    // ---- vanilla maps ----
    let mut van_maps: HashMap<(i32, i32), HashMap<(u8, i32, u8), String>> = HashMap::new();
    for &(cx, cz) in &coords {
        if let Some(m) = load_vanilla(&region_dir, cx, cz) {
            van_maps.insert((cx, cz), m);
        } else {
            eprintln!("[van] missing/not-full chunk ({cx},{cz})");
        }
    }

    let mut global_class: BTreeMap<String, u64> = BTreeMap::new();
    let mut global_fam: BTreeMap<String, u64> = BTreeMap::new();
    let mut verdicts: Vec<String> = Vec::new();

    for &(wx, wz) in &WINDOWS {
        let tag = format!("{wx}_{wz}");
        let census_csv = format!("{out_dir}/lush_census_{tag}.csv");
        let runs_log = format!("{out_dir}/lush_runs_{tag}.log");
        let root_log = format!("{out_dir}/lush_root_{tag}.log");
        let mut csv = std::io::BufWriter::new(std::fs::File::create(&census_csv).expect("csv"));
        let mut log = std::fs::File::create(&runs_log).expect("log");
        let mut rootlog = std::fs::File::create(&root_log).expect("rootlog");
        writeln!(csv, "x,y,z,vanilla,neutron,class,producers,family").ok();

        // ---- build the two replay buffers ----
        let mut buf_van = RegionBuf::new(wx, wz, BUF_RADIUS);
        let mut buf_neu = RegionBuf::new(wx, wz, BUF_RADIUS);
        for cz in (wz - BUF_RADIUS)..=(wz + BUF_RADIUS) {
            for cx in (wx - BUF_RADIUS)..=(wx + BUF_RADIUS) {
                // VAN side: vanilla refs stripped of feature output
                if let Some(vm) = van_maps.get(&(cx, cz)) {
                    let mut blocks = vec![BlockId::Air.as_u16(); 16 * 384 * 16];
                    let mut hm = vec![WORLD_BOTTOM as i16; 256];
                    for lz in 0..16i32 {
                        for lx in 0..16i32 {
                            for y in WORLD_BOTTOM..WORLD_TOP {
                                if let Some(name) = vm.get(&(lx as u8, y, lz as u8)) {
                                    if let Some(b) = BlockId::from_name(name) {
                                        let sb = strip_block(b, y);
                                        let bi = (((y - WORLD_BOTTOM) * 256
                                            + lz * 16
                                            + lx)
                                            as usize);
                                        blocks[bi] = sb.as_u16();
                                        if blocks_motion_m(sb) {
                                            hm[(lz * 16 + lx) as usize] = y as i16;
                                        }
                                    }
                                }
                            }
                        }
                    }
                    buf_van.put_chunk(cx, cz, &blocks, &hm);
                }
                // NEU side: generated final chunks stripped the same way
                if let Some((blocks, hm_pre)) = inventory.get(&(cx, cz)) {
                    let mut sb = vec![BlockId::Air.as_u16(); 16 * 384 * 16];
                    for (i, &b) in blocks.iter().enumerate() {
                        let y = (i / 256) as i32 + WORLD_BOTTOM;
                        let bid = BlockId::from_u16(b).unwrap_or(BlockId::Air);
                        sb[i] = strip_block(bid, y).as_u16();
                    }
                    buf_neu.put_chunk(cx, cz, &sb, hm_pre);
                }
            }
        }
        let pristine_van = buf_van.blocks.clone();
        let pristine_neu = buf_neu.blocks.clone();

        // replay both sides over their own buffers
        let van_runs = replay_side("VAN", &mut buf_van, &state, seed, &mut log);
        let neu_runs = replay_side("NEU", &mut buf_neu, &state, seed, &mut log);

        // replay-final maps of the consumer chunk (for gap classification)
        let mut van_final_replay = HashMap::new();
        let mut neu_final_replay = HashMap::new();
        for lz in 0..16i32 {
            for lx in 0..16i32 {
                for y in WORLD_BOTTOM..WORLD_TOP {
                    let k = (wx * 16 + lx, y, wz * 16 + lz);
                    van_final_replay.insert(k, buf_van.get(k.0, y, k.2));
                    neu_final_replay.insert(k, buf_neu.get(k.0, y, k.2));
                }
            }
        }

        let raw_neu = inventory
            .get(&(wx, wz))
            .map(|(b, _)| b.clone())
            .unwrap_or_default();
        let rep = classify_window(
            wx,
            wz,
            van_maps.entry((wx, wz)).or_default(),
            &raw_neu,
            &van_final_replay,
            &neu_final_replay,
            &van_runs,
            &neu_runs,
            &pristine_van,
            &pristine_neu,
            (buf_van.origin_x, buf_van.origin_z),
            buf_van.side,
            &mut csv,
            &mut rootlog,
        );
        csv.flush().ok();
        log.flush().ok();
        rootlog.flush().ok();

        // summary
        println!("\n---- window ({wx},{wz}) ----");
        println!("census lush-family mismatch cells: {}", rep.census.len());
        let mut fv: Vec<_> = rep.fam_counts.iter().collect();
        fv.sort_by_key(|(_, c)| std::cmp::Reverse(**c));
        for (f, c) in &fv {
            println!("  {f:<9} {c:>6}  ({:.1}%)", **c as f64 / rep.census.len().max(1) as f64 * 100.0);
        }
        let n = rep.census.len() as f64;
        let mut cv: Vec<_> = rep.class_counts.clone().into_iter().collect();
        cv.sort_by_key(|(_, c)| std::cmp::Reverse(*c));
        for (c, k) in &cv {
            println!("  {c:<14} {k:>6} ({:.1}%)", *k as f64 / n.max(1.0) * 100.0);
        }
        println!("root: {}", rep.root.as_deref().unwrap_or("<none>"));
        for d in &rep.root_detail {
            println!("  {d}");
        }
        if let Some(r) = rep.divergent_runs.first() {
            println!("first divergent run: {r}");
        }
        println!("divergent runs: {}", rep.divergent_runs.len());
        verdicts.push(format!(
            "window ({wx},{wz}): cells={} root={} classes={cv:?} divergent_runs={}",
            rep.census.len(),
            rep.root.as_deref().unwrap_or("none"),
            rep.divergent_runs.len(),
        ));
        for (c, k) in &rep.class_counts {
            *global_class.entry(c.clone()).or_insert(0) += k;
        }
        for (f, k) in &rep.fam_counts {
            *global_fam.entry(f.clone()).or_insert(0) += 1;
        }
    }

    println!("\n==================== GLOBAL SUMMARY ====================");
    println!("windows: {WINDOWS:?}  seed={seed}  elapsed {:.0}s", t0.elapsed().as_secs_f64());
    let mut fv: Vec<_> = global_fam.into_iter().collect();
    fv.sort_by_key(|(_, c)| std::cmp::Reverse(*c));
    println!("family totals (both windows):");
    for (f, c) in &fv {
        println!("  {f:<9} {c}");
    }
    let mut gv: Vec<_> = global_class.into_iter().collect();
    gv.sort_by_key(|(_, c)| std::cmp::Reverse(*c));
    println!("3-way classification totals:");
    let tot: u64 = gv.iter().map(|(_, c)| c).sum();
    for (c, k) in &gv {
        println!("  {c:<14} {k:>6} ({:.1}%)", *k as f64 / tot.max(1) as f64 * 100.0);
    }
    println!("\nVERDICTS:");
    for v in &verdicts {
        println!("  {v}");
    }
    println!("\nartifacts in {out_dir}: census/runs/root per window");
}
