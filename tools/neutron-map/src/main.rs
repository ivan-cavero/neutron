//! neutron-map — see worldgen without launching the game.
//!
//! Subcommands:
//! - `map <seed> <x0,z0> <x1,z1> --out PREFIX [--ref DIR] [--diff]`
//!     Render top-down surface maps: neutron vs vanilla reference (+ diff view).
//! - `biomes`                    List embedded biome data.
//! - `tree <biome>`              Per-generation-step feature list of a biome.
//! - `feature <placed_id>`       Dump embedded placed/configured JSON.
//!
//! Copyright (c) 2026 Neutron Contributors -- MIT License

use anyhow::{bail, Context, Result};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(|s| s.as_str()) {
        Some("map") => cmd_map(&args[1..]),
        Some("biomes") => cmd_biomes(),
        Some("tree") => cmd_tree(args.get(1).map(|s| s.as_str())),
        Some("feature") => cmd_feature(args.get(1).map(|s| s.as_str())),
        _ => bail!(
            "usage:\n  \
             neutron-map map <seed> <x0,z0> <x1,z1> --out PREFIX [--ref DIR] [--diff]\n  \
             neutron-map biomes\n  \
             neutron-map tree <biome>\n  \
             neutron-map feature <placed_id>"
        ),
    }
}

// ---------------------------------------------------------------------------
// map
// ---------------------------------------------------------------------------

struct Args {
    seed: i64,
    x0: i32,
    z0: i32,
    x1: i32,
    z1: i32,
    out: String,
    ref_dir: Option<String>,
    diff: bool,
}

fn parse_args(args: &[String]) -> Result<Args> {
    let mut it = args.iter();
    let seed: i64 = it.next().context("missing seed")?.parse().context("bad seed")?;
    let parse_pair = |s: &str| -> Result<(i32, i32)> {
        let (a, b) = s
            .split_once(',')
            .with_context(|| format!("expected x,z pair, got {s:?}"))?;
        Ok((a.parse()?, b.parse()?))
    };
    let (x0, z0) = parse_pair(it.next().context("missing x0,z0")?)?;
    let (x1, z1) = parse_pair(it.next().context("missing x1,z1")?)?;
    let mut out = String::from("map");
    let mut ref_dir = None;
    let mut diff = false;
    while let Some(a) = it.next() {
        match a.as_str() {
            "--out" => out = it.next().context("--out needs value")?.clone(),
            "--ref" => ref_dir = Some(it.next().context("--ref needs value")?.clone()),
            "--diff" => diff = true,
            other => bail!("unknown flag {other}"),
        }
    }
    if diff && ref_dir.is_none() {
        bail!("--diff needs --ref");
    }
    Ok(Args { seed, x0, z0, x1, z1, out, ref_dir, diff })
}

type Surface = (u8, [u8; 3]); // status(1=found), color

fn cmd_map(args: &[String]) -> Result<()> {
    let a = parse_args(args)?;
    let w = (a.x1 - a.x0 + 1).max(1) as u32;
    let h = (a.z1 - a.z0 + 1).max(1) as u32;
    println!("generating {w}x{h} chunks for seed {}...", a.seed);

    // chunk order must match pixel order: row-major by z then x
    let coords: Vec<(i32, i32)> = (a.z0..=a.z1)
        .flat_map(|z| (a.x0..=a.x1).map(move |x| (x, z)))
        .collect();

    // --- neutron -----------------------------------------------------------
    let gen = neutron_worldgen::ChunkGenerator::new(a.seed);
    let neu: Vec<Surface> = if coords.len() > 64 {
        std::thread::scope(|s| -> Vec<Surface> {
            let gen = &gen;
            let parts: Vec<&[(i32, i32)]> =
                coords.chunks(coords.len().div_ceil(num_threads())).collect();
            let handles: Vec<_> = parts
                .iter()
                .map(|part| {
                    let part: Vec<(i32, i32)> = (*part).to_vec();
                    s.spawn(move || -> Vec<Surface> {
                        let mut cache = neutron_worldgen::NoiseCache::with_cap(part.len());
                        part.iter()
                            .map(|&(cx, cz)| {
                                let ch = gen.generate_chunk_cached(cx, cz, &mut cache);
                                surface_of_neutron(&ch)
                            })
                            .collect()
                    })
                })
                .collect();
            handles
                .into_iter()
                .flat_map(|h| h.join().unwrap())
                .collect()
        })
    } else {
        let mut cache = neutron_worldgen::NoiseCache::with_cap(coords.len());
        coords
            .iter()
            .map(|&(cx, cz)| {
                let ch = gen.generate_chunk_cached(cx, cz, &mut cache);
                surface_of_neutron(&ch)
            })
            .collect()
    };

    let mut px: Vec<u8> = Vec::with_capacity((w * h * 3) as usize);
    for idx in 0..coords.len() {
        px.extend_from_slice(&neu[idx].1);
    }
    write_png(&format!("{}-neutron.png", a.out), w, h, &px)?;
    println!("wrote {}-neutron.png", a.out);

    // --- vanilla + diff ----------------------------------------------------
    if let Some(dir) = &a.ref_dir {
        let mut regions: HashMap<(i32, i32), Option<neutron_world::Region>> = HashMap::new();
        fn region_for<'a>(
            regions: &'a mut HashMap<(i32, i32), Option<neutron_world::Region>>,
            dir: &str,
            key: (i32, i32),
        ) -> &'a mut Option<neutron_world::Region> {
            regions.entry(key).or_insert_with(|| {
                neutron_world::Region::open(Path::new(&format!(
                    "{dir}/r.{}.{}.mca",
                    key.0, key.1
                )))
                .ok()
                .map(|r| r.with_coords(key.0, key.1))
            })
        }
        let mut van: Vec<Option<Surface>> = Vec::with_capacity(coords.len());
        for &(cx, cz) in &coords {
            van.push(match region_for(&mut regions, dir, (cx >> 5, cz >> 5)) {
                Some(r) => match r.get_chunk(cx & 31, cz & 31) {
                    Ok(Some(data)) => read_nbt_surface(&data),
                    _ => None,
                },
                None => None,
            });
        }

        let mut px: Vec<u8> = Vec::with_capacity((w * h * 3) as usize);
        for idx in 0..coords.len() {
            let color = van[idx].map(|(_, c)| c).unwrap_or([18, 18, 22]);
            px.extend_from_slice(&color);
        }
        write_png(&format!("{}-vanilla.png", a.out), w, h, &px)?;

        let (mut m, mut d, mut miss) = (0u32, 0u32, 0u32);
        let mut dx: Vec<u8> = Vec::with_capacity((w * h * 3) as usize);
        for idx in 0..coords.len() {
            let color = match van[idx] {
                None => {
                    miss += 1;
                    [16, 16, 20]
                }
                Some(v) => {
                    if v.0 == neu[idx].0 {
                        m += 1;
                        [36, 110, 36]
                    } else {
                        d += 1;
                        [205, 45, 45]
                    }
                }
            };
            dx.extend_from_slice(&color);
        }
        write_png(&format!("{}-diff.png", a.out), w, h, &dx)?;
        println!(
            "surface chunks: {m} match / {d} differ / {miss} missing\nwrote {}-vanilla.png + {}-diff.png",
            a.out, a.out
        );
    }
    Ok(())
}

fn num_threads() -> usize {
    std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4)
}
const _: () = ();

/// Top-down surface sample at column center (8,8): first non-air from top.
fn surface_of_neutron(ch: &neutron_worldgen::GeneratedChunk) -> Surface {
    use neutron_worldgen::surface::BlockId;
    for y in (neutron_worldgen::generator::WORLD_BOTTOM
        ..neutron_worldgen::generator::WORLD_TOP)
        .rev()
    {
        let b = ch.block_at(8, y, 8);
        if !b.is_air() && b != BlockId::CaveAir {
            return (1, block_color(b, y));
        }
    }
    (0, [12, 12, 16])
}

fn read_nbt_surface(data: &[u8]) -> Option<Surface> {
    use neutron_world::nbt::ussr_nbt::owned::{List, Tag};
    use neutron_world::nbt::{compound_get, read_nbt};
    let nbt = read_nbt(data).ok()?;
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
    let mut best: Option<(i32, String)> = None;
    for sec in sections {
        let y_sec = match compound_get(sec, "Y") {
            Some(Tag::Byte(y)) => *y as i8 as i32,
            _ => continue,
        };
        let Some(Tag::Compound(bs)) = compound_get(sec, "block_states") else { continue };
        let Some(Tag::List(List::Compound(palette))) = compound_get(bs, "palette") else { continue };
        let names: Vec<String> = palette
            .iter()
            .map(|pc| match compound_get(pc, "Name") {
                Some(Tag::String(s)) => s.to_string(),
                _ => "minecraft:air".into(),
            })
            .collect();
        if names.len() == 1 {
            if !names[0].ends_with(":air") && best.is_none() {
                // uniform solid section: its top block y_sec*16+15
                let y = y_sec * 16 + 15;
                best = Some((y, names[0].clone()));
            }
            continue;
        }
        let bits = ((names.len() - 1).ilog2() + 1).max(4) as usize;
        let Some(Tag::LongArray(d)) = compound_get(bs, "data") else { continue };
        let longs: Vec<i64> = d.to_vec();
        let epl = 64 / bits;
        let mask = (1u64 << bits) - 1;
        for ly in (0..16i32).rev() {
            let i = (ly * 256 + 8 * 16 + 8) as usize;
            let li = i / epl as usize;
            let bo = (i % epl) * bits;
            let Some(l) = longs.get(li) else { continue };
            let idx = ((*l as u64) >> bo) & mask;
            let name = names.get(idx as usize)?.clone();
            if !name.ends_with(":air") && !name.ends_with(":cave_air") {
                let y = y_sec * 16 + ly;
                if best.as_ref().map(|(by, _)| y > *by).unwrap_or(true) {
                    best = Some((y, name));
                }
                break;
            }
        }
    }
    best.map(|(y, name)| (1u8, vanilla_color(&name, y)))
}

fn block_color(b: neutron_worldgen::surface::BlockId, y: i32) -> [u8; 3] {
    use neutron_worldgen::surface::BlockId::*;
    let base: [u8; 3] = match b {
        Water => [48, 88, 190],
        Lava => [230, 100, 30],
        GrassBlock => [95, 159, 53],
        Dirt | CoarseDirt | RootedDirt => [134, 96, 67],
        Sand => [219, 207, 163],
        RedSand => [190, 102, 33],
        Gravel => [136, 126, 126],
        Stone => [125, 125, 125],
        Granite => [149, 103, 85],
        Diorite => [188, 188, 197],
        Andesite => [136, 136, 137],
        Deepslate => [80, 80, 86],
        Tuff => [108, 109, 102],
        Snow | PowderSnow => [240, 246, 246],
        Ice | PackedIce | BlueIce => [145, 183, 253],
        Clay => [160, 166, 179],
        Calcite => [223, 224, 220],
        SmoothBasalt => [72, 72, 78],
        AmethystBlock | BuddingAmethyst => [133, 97, 187],
        Bedrock => [60, 60, 60],
        MossBlock => [90, 128, 47],
        PaleOakLog | PaleMossBlock => [142, 143, 130],
        Sculk | SculkVein | SculkCatalyst | SculkSensor | SculkShrieker => [12, 29, 36],
        OakLog | DarkOakLog | SpruceLog | BirchLog | JungleLog | AcaciaLog | MangroveLog
        | CherryLog => [102, 81, 50],
        OakLeaves | DarkOakLeaves | BirchLeaves | SpruceLeaves | JungleLeaves | AcaciaLeaves
        | MangroveLeaves | CherryLeaves | PaleOakLeaves => [60, 120, 45],
        Terracotta | WhiteTerracotta | YellowTerracotta | RedTerracotta | OrangeTerracotta
        | BrownTerracotta | BlackTerracotta | LightGrayTerracotta => [152, 94, 67],
        Mud => [60, 57, 58],
        Chest | Spawner => [120, 80, 150],
        _ => [150, 150, 150],
    };
    shade(base, y)
}

fn vanilla_color(name: &str, y: i32) -> [u8; 3] {
    let short = name.trim_start_matches("minecraft:");
    let base: [u8; 3] = match short {
        "water" => [48, 88, 190],
        "lava" => [230, 100, 30],
        "grass_block" => [95, 159, 53],
        "dirt" | "coarse_dirt" | "rooted_dirt" => [134, 96, 67],
        "sand" => [219, 207, 163],
        "red_sand" => [190, 102, 33],
        "gravel" => [136, 126, 126],
        "stone" => [125, 125, 125],
        "granite" => [149, 103, 85],
        "diorite" => [188, 188, 197],
        "andesite" => [136, 136, 137],
        "deepslate" => [80, 80, 86],
        "tuff" => [108, 109, 102],
        "snow" | "snow_block" | "powder_snow" => [240, 246, 246],
        "ice" | "packed_ice" | "blue_ice" => [145, 183, 253],
        "clay" => [160, 166, 179],
        "calcite" => [223, 224, 220],
        "smooth_basalt" => [72, 72, 78],
        "amethyst_block" | "budding_amethyst" => [133, 97, 187],
        "bedrock" => [60, 60, 60],
        "moss_block" => [90, 128, 47],
        "pale_moss_block" => [142, 143, 130],
        "sculk" | "sculk_vein" => [12, 29, 36],
        n if n.ends_with("_leaves") => [60, 120, 45],
        n if n.contains("_planks") || n.ends_with("_log") || n.ends_with("_stem") => [102, 81, 50],
        n if n.contains("terracotta") => [152, 94, 67],
        n if n.contains("coral") => [220, 100, 140],
        n if n.contains("seagrass") || n.contains("kelp") => [40, 130, 90],
        _ => [150, 150, 150],
    };
    shade(base, y)
}

fn shade(mut c: [u8; 3], y: i32) -> [u8; 3] {
    let t = ((y as f64 + 64.0) / 384.0).clamp(0.0, 1.0);
    let f = 0.55 + 0.65 * t;
    for v in &mut c {
        *v = ((*v as f64) * f).clamp(0.0, 255.0) as u8;
    }
    c
}

fn write_png(path: &str, w: u32, h: u32, rgb: &[u8]) -> Result<()> {
    let file = std::fs::File::create(path)?;
    let mut enc = png::Encoder::new(std::io::BufWriter::new(file), w, h);
    enc.set_color(png::ColorType::Rgb);
    enc.set_depth(png::BitDepth::Eight);
    let mut writer = enc.write_header()?;
    writer.write_image_data(rgb)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// tree inspector
// ---------------------------------------------------------------------------

fn cmd_biomes() -> Result<()> {
    use neutron_worldgen::datapack_data::EMBEDDED_PATHS;
    let mut count = 0;
    for p in EMBEDDED_PATHS {
        if let Some(rest) = p.strip_prefix("biome/") {
            if let Some(name) = rest.strip_suffix(".json") {
                println!("{name}");
                count += 1;
            }
        }
    }
    println!("# {count} biomes");
    Ok(())
}

fn cmd_tree(biome: Option<&str>) -> Result<()> {
    let Some(biome) = biome else {
        bail!("usage: neutron-map tree <biome>");
    };
    const STEPS: [&str; 11] = [
        "raw_generation",
        "lakes",
        "local_modifications",
        "underground_structures",
        "surface_structures",
        "strongholds",
        "underground_ores",
        "underground_decoration",
        "fluid_springs",
        "vegetal_decoration",
        "top_layer_modification",
    ];
    println!("# {biome}");
    for (step, label) in STEPS.iter().enumerate() {
        let feats = neutron_worldgen::feature_catalog::features_at_step(biome, step as i32);
        if feats.is_empty() {
            continue;
        }
        println!("step {step} ({label}):");
        for f in &feats {
            let gi = neutron_worldgen::feature_catalog::global_feature_index(step as i32, f);
            println!("  [{:>2}] {f}", gi.unwrap_or(-1));
        }
    }
    Ok(())
}

fn cmd_feature(id: Option<&str>) -> Result<()> {
    let Some(id) = id else {
        bail!("usage: neutron-map feature <placed_id>   (e.g. minecraft:pale_oak_trees)");
    };
    let bare = id.trim_start_matches("minecraft:");
    let placed = neutron_worldgen::datapack_data::datapack_json(&format!(
        "placed_feature/{bare}.json"
    ))
    .with_context(|| format!("placed feature '{bare}' not found in embedded data"))?;
    println!("=== placed_feature/{bare} ===");
    println!("{}", pretty(placed));
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(placed) {
        if let Some(feat) = v["feature"].as_str() {
            let bare_cfg = feat.trim_start_matches("minecraft:");
            if let Some(cfg) = neutron_worldgen::datapack_data::datapack_json(&format!(
                "configured_feature/{bare_cfg}.json"
            )) {
                println!("=== configured_feature/{bare_cfg} ===");
                println!("{}", pretty(cfg));
            }
        }
    }
    Ok(())
}

fn pretty(s: &str) -> String {
    serde_json::from_str::<serde_json::Value>(s)
        .map(|v| serde_json::to_string_pretty(&v).unwrap_or_else(|_| s.to_string()))
        .unwrap_or_else(|_| s.to_string())
}
