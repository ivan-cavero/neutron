// Copyright (c) 2026 Neutron Contributors -- MIT License
//
// Surface rules engine matching vanilla 26.2 `SurfaceSystem.buildSurface` +
// `SurfaceRules` evaluation over the datapack `surface_rule` JSON.

use std::collections::HashMap;
use std::sync::OnceLock;

use serde_json::Value;

use crate::biome_source::{self, biome_id};
use crate::density::DensityEnv;
use crate::generator::{WORLD_BOTTOM, WORLD_TOP};
use crate::noise::NormalNoise;
use crate::positional::PositionalRandomFactory;
use crate::surface::BlockId;
use crate::worldgen::WorldgenState;

/// Apply the overworld surface_rule tree to a filled stone/fluid chunk.
pub fn apply_surface_rules(
    blocks: &mut [u16],
    heightmap: &[i16],
    cx: i32,
    cz: i32,
    st: &WorldgenState,
) {
    let rule = overworld_rule();
    let chunk_min_x = cx * 16;
    let chunk_min_z = cz * 16;
    let main_rng = PositionalRandomFactory::new(st.main_lo, st.main_hi);

    for lx in 0..16usize {
        for lz in 0..16usize {
            let world_x = chunk_min_x + lx as i32;
            let world_z = chunk_min_z + lz as i32;
            let surface_y = heightmap[lz * 16 + lx] as i32;
            if surface_y < WORLD_BOTTOM {
                continue;
            }

            // surfaceDepth = (int)(surfaceNoise*2.75 + 3.0 + random.at(x,0,z).nextDouble()*0.25)
            let surface_noise = st
                .noises
                .noises()
                .get("surface")
                .map(|n| n.get_value(world_x as f64, 0.0, world_z as f64))
                .unwrap_or(0.0);
            let mut depth_rng = main_rng.at(world_x, 0, world_z);
            let surface_depth =
                (surface_noise * 2.75 + 3.0 + depth_rng.next_f64() * 0.25) as i32;

            let surface_secondary = st
                .noises
                .noises()
                .get("surface_secondary")
                .map(|n| n.get_value(world_x as f64, 0.0, world_z as f64))
                .unwrap_or(0.0);

            // preliminary surface level (density) for above_preliminary_surface
            let prelim = {
                let mut env = DensityEnv::new(world_x, 0, world_z, st.noises.noises());
                crate::density::compute(&st.router.preliminary_surface_level, &mut env) as i32
            };
            // minSurfaceLevel ≈ prelim + surfaceDepth - 8 (lerp cache simplified)
            let min_surface_level = prelim + surface_depth - 8;

            // Surface biome at top of column (cave biomes re-sampled only deep below).
            let surface_biome = sample_biome(st, world_x, surface_y.max(WORLD_BOTTOM), world_z);

            // Steep: neighbour height delta >= 4 (within chunk)
            let steep = is_steep(heightmap, lx, lz);

            let mut stone_above = 0i32;
            let mut water_height = i32::MIN;
            let mut next_ceiling_stone_y = i32::MAX;
            let end_y = WORLD_BOTTOM;
            let height = surface_y + 1;
            // Cache last cave-biome sample to avoid density eval per block.
            let mut cached_biome = surface_biome;
            let mut cached_biome_y = surface_y;

            for y in (end_y..=height.min(WORLD_TOP - 1)).rev() {
                let idx = block_index(lx, y, lz);
                let old = BlockId::from_u16(blocks[idx]).unwrap_or(BlockId::Air);

                if old.is_air() {
                    stone_above = 0;
                    water_height = i32::MIN;
                    continue;
                }
                if old.is_fluid() {
                    if water_height == i32::MIN {
                        water_height = y + 1;
                    }
                    continue;
                }

                // stone run — find how deep we are above the next non-stone below
                if next_ceiling_stone_y >= y {
                    next_ceiling_stone_y = WORLD_BOTTOM;
                    let mut look = y - 1;
                    while look >= end_y - 1 {
                        if look < WORLD_BOTTOM {
                            next_ceiling_stone_y = WORLD_BOTTOM;
                            break;
                        }
                        let b = BlockId::from_u16(blocks[block_index(lx, look, lz)])
                            .unwrap_or(BlockId::Air);
                        if !is_stone_like(b) {
                            next_ceiling_stone_y = look + 1;
                            break;
                        }
                        look -= 1;
                    }
                }
                let stone_below = y - next_ceiling_stone_y + 1;
                stone_above += 1;

                // Only replace default stone (and deepslate pre-surface for deepslate rule)
                if old != BlockId::Stone && old != BlockId::Deepslate {
                    continue;
                }

                // Re-sample biome every 8 blocks below surface for cave transitions.
                if y < surface_y - 8 && (cached_biome_y - y).abs() >= 8 {
                    cached_biome = sample_biome(st, world_x, y, world_z);
                    cached_biome_y = y;
                }
                let biome = if y >= min_surface_level - 16 {
                    surface_biome
                } else {
                    cached_biome
                };

                let mut ctx = RuleContext {
                    x: world_x,
                    y,
                    z: world_z,
                    stone_depth_above: stone_above,
                    stone_depth_below: stone_below,
                    water_height,
                    surface_depth,
                    surface_secondary,
                    min_surface_level,
                    biome,
                    steep,
                    hole: surface_depth <= 0,
                    noises: st.noises.noises(),
                    main_rng,
                    sea_level: st.sea_level,
                };

                if let Some(new_block) = rule.try_apply(&mut ctx) {
                    blocks[idx] = new_block.as_u16();
                }
            }
        }
    }
}

fn is_steep(heightmap: &[i16], lx: usize, lz: usize) -> bool {
    let h = heightmap[lz * 16 + lx] as i32;
    let z0 = lz.saturating_sub(1);
    let z1 = (lz + 1).min(15);
    let x0 = lx.saturating_sub(1);
    let x1 = (lx + 1).min(15);
    let hn = heightmap[z0 * 16 + lx] as i32;
    let hs = heightmap[z1 * 16 + lx] as i32;
    if hs >= hn + 4 || hn >= hs + 4 {
        return true;
    }
    let hw = heightmap[lz * 16 + x0] as i32;
    let he = heightmap[lz * 16 + x1] as i32;
    he >= hw + 4 || hw >= he + 4 || (h - hn).abs() >= 4 || (h - hs).abs() >= 4
}

fn is_stone_like(b: BlockId) -> bool {
    !b.is_air() && !b.is_fluid()
}

fn sample_biome(st: &WorldgenState, x: i32, y: i32, z: i32) -> u8 {
    let mut env = DensityEnv::new(x, y, z, st.noises.noises());
    let c = biome_source::climate_at_block(
        &mut env,
        &st.router.temperature,
        &st.router.vegetation,
        &st.router.continents,
        &st.router.erosion,
        &st.router.depth,
        &st.router.ridges,
    );
    biome_source::find_biome(&c)
}

#[inline]
fn block_index(lx: usize, y: i32, lz: usize) -> usize {
    ((y - WORLD_BOTTOM) as usize) * 256 + lz * 16 + lx
}

// ---------------------------------------------------------------------------
// Rule tree
// ---------------------------------------------------------------------------

struct RuleContext<'a> {
    x: i32,
    y: i32,
    z: i32,
    stone_depth_above: i32,
    stone_depth_below: i32,
    water_height: i32,
    surface_depth: i32,
    surface_secondary: f64,
    min_surface_level: i32,
    biome: u8,
    steep: bool,
    hole: bool,
    noises: &'a HashMap<String, NormalNoise>,
    main_rng: PositionalRandomFactory,
    sea_level: i32,
}

enum Rule {
    Sequence(Vec<Rule>),
    Condition { cond: Condition, then: Box<Rule> },
    Block(BlockId),
    Bandlands,
}

enum Condition {
    VerticalGradient {
        name: String,
        true_at: VerticalAnchor,
        false_at: VerticalAnchor,
    },
    AbovePreliminarySurface,
    StoneDepth {
        offset: i32,
        add_surface_depth: bool,
        secondary_depth_range: i32,
        ceiling: bool,
    },
    Biome(Vec<u8>),
    YAbove {
        anchor: VerticalAnchor,
        surface_depth_multiplier: i32,
        add_stone_depth: bool,
    },
    Water {
        offset: i32,
        surface_depth_multiplier: i32,
        add_stone_depth: bool,
    },
    NoiseThreshold {
        noise: String,
        min: f64,
        max: f64,
    },
    Not(Box<Condition>),
    Hole,
    Steep,
    Temperature,
}

#[derive(Clone, Copy)]
enum VerticalAnchor {
    Absolute(i32),
    AboveBottom(i32),
    BelowTop(i32),
}

impl VerticalAnchor {
    fn resolve(self) -> i32 {
        match self {
            VerticalAnchor::Absolute(y) => y,
            VerticalAnchor::AboveBottom(n) => WORLD_BOTTOM + n,
            VerticalAnchor::BelowTop(n) => WORLD_TOP - 1 - n,
        }
    }
}

impl Rule {
    fn try_apply(&self, ctx: &mut RuleContext<'_>) -> Option<BlockId> {
        match self {
            Rule::Sequence(rules) => {
                for r in rules {
                    if let Some(b) = r.try_apply(ctx) {
                        return Some(b);
                    }
                }
                None
            }
            Rule::Condition { cond, then } => {
                if cond.test(ctx) {
                    then.try_apply(ctx)
                } else {
                    None
                }
            }
            Rule::Block(b) => Some(*b),
            Rule::Bandlands => Some(bandlands_block(ctx)),
        }
    }
}

impl Condition {
    fn test(&self, ctx: &mut RuleContext<'_>) -> bool {
        match self {
            Condition::VerticalGradient {
                name,
                true_at,
                false_at,
            } => {
                let t = true_at.resolve();
                let f = false_at.resolve();
                let y = ctx.y;
                if y <= t {
                    return true;
                }
                if y >= f {
                    return false;
                }
                // map(y, t, f, 1.0, 0.0)
                let threshold = map(y as f64, t as f64, f as f64, 1.0, 0.0);
                let factory = ctx.main_rng.from_hash_of_positional(name);
                let mut rng = factory.at(ctx.x, y, ctx.z);
                (rng.next_f32() as f64) < threshold
            }
            Condition::AbovePreliminarySurface => ctx.y >= ctx.min_surface_level,
            Condition::StoneDepth {
                offset,
                add_surface_depth,
                secondary_depth_range,
                ceiling,
            } => {
                let depth = if *ceiling {
                    ctx.stone_depth_below
                } else {
                    ctx.stone_depth_above
                };
                let surface_addon = if *add_surface_depth {
                    ctx.surface_depth
                } else {
                    0
                };
                let secondary = if *secondary_depth_range == 0 {
                    0
                } else {
                    map(
                        ctx.surface_secondary,
                        -1.0,
                        1.0,
                        0.0,
                        *secondary_depth_range as f64,
                    ) as i32
                };
                depth <= 1 + *offset + surface_addon + secondary
            }
            Condition::Biome(list) => list.contains(&ctx.biome),
            Condition::YAbove {
                anchor,
                surface_depth_multiplier,
                add_stone_depth,
            } => {
                let mut threshold = anchor.resolve();
                threshold += ctx.surface_depth * *surface_depth_multiplier;
                if *add_stone_depth {
                    threshold += ctx.stone_depth_above;
                }
                ctx.y >= threshold
            }
            Condition::Water {
                offset,
                surface_depth_multiplier,
                add_stone_depth,
            } => {
                if ctx.water_height == i32::MIN {
                    return true;
                }
                let stone_addon = if *add_stone_depth {
                    ctx.stone_depth_above
                } else {
                    0
                };
                ctx.y + stone_addon
                    >= ctx.water_height + *offset + ctx.surface_depth * *surface_depth_multiplier
            }
            Condition::NoiseThreshold { noise, min, max } => {
                // Most surface noises are 2D (y=0); a few are 3D (calcite, gravel, …).
                let y_sample = match noise.as_str() {
                    "surface" | "surface_secondary" | "surface_swamp" | "badlands_surface" => 0.0,
                    _ => ctx.y as f64,
                };
                let v = ctx
                    .noises
                    .get(noise.as_str())
                    .map(|n| n.get_value(ctx.x as f64, y_sample, ctx.z as f64))
                    .unwrap_or(0.0);
                v >= *min && v < *max
            }
            Condition::Not(inner) => !inner.test(ctx),
            Condition::Hole => ctx.hole,
            Condition::Steep => ctx.steep,
            Condition::Temperature => {
                // Frozen ocean ice vs water: approx cold if biome frozen
                matches!(
                    ctx.biome,
                    biome_id::FROZEN_OCEAN
                        | biome_id::SNOWY_PLAINS
                        | biome_id::ICE_SPIKES
                        | biome_id::FROZEN_PEAKS
                        | biome_id::SNOWY_SLOPES
                        | biome_id::GROVE
                        | biome_id::JAGGED_PEAKS
                )
            }
        }
    }
}

#[inline]
fn map(v: f64, from_min: f64, from_max: f64, to_min: f64, to_max: f64) -> f64 {
    if (from_max - from_min).abs() < 1e-12 {
        return to_min;
    }
    let t = (v - from_min) / (from_max - from_min);
    to_min + t * (to_max - to_min)
}

/// Simplified badlands terracotta banding by Y.
fn bandlands_block(ctx: &RuleContext<'_>) -> BlockId {
    // Vanilla uses clay_bands array; approximate with Y-mod bands.
    match ctx.y.rem_euclid(16) {
        0..=1 => BlockId::YellowTerracotta,
        2..=3 => BlockId::BrownTerracotta,
        4..=5 => BlockId::OrangeTerracotta,
        6..=7 => BlockId::RedTerracotta,
        8..=9 => BlockId::WhiteTerracotta,
        10..=11 => BlockId::LightGrayTerracotta,
        12..=13 => BlockId::Terracotta,
        _ => BlockId::OrangeTerracotta,
    }
}

// ---------------------------------------------------------------------------
// JSON parsing
// ---------------------------------------------------------------------------

fn overworld_rule() -> &'static Rule {
    static RULE: OnceLock<Rule> = OnceLock::new();
    RULE.get_or_init(|| {
        let json = crate::datapack_data::datapack_json("noise_settings_overworld.json")
            .expect("noise_settings_overworld.json");
        let value: Value = serde_json::from_str(json).expect("parse noise_settings");
        parse_rule(&value["surface_rule"])
    })
}

fn parse_rule(v: &Value) -> Rule {
    let t = v
        .get("type")
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .strip_prefix("minecraft:")
        .unwrap_or_else(|| v.get("type").and_then(|x| x.as_str()).unwrap_or(""));
    match t {
        "sequence" => {
            let seq = v["sequence"]
                .as_array()
                .unwrap_or(&Vec::new())
                .iter()
                .map(parse_rule)
                .collect();
            Rule::Sequence(seq)
        }
        "condition" => Rule::Condition {
            cond: parse_condition(&v["if_true"]),
            then: Box::new(parse_rule(&v["then_run"])),
        },
        "block" => {
            let name = v["result_state"]["Name"].as_str().unwrap_or("minecraft:stone");
            Rule::Block(BlockId::from_name(name).unwrap_or(BlockId::Stone))
        }
        "bandlands" => Rule::Bandlands,
        _ => Rule::Sequence(vec![]),
    }
}

fn parse_condition(v: &Value) -> Condition {
    let t = v
        .get("type")
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .strip_prefix("minecraft:")
        .unwrap_or("");
    match t {
        "vertical_gradient" => Condition::VerticalGradient {
            name: v["random_name"]
                .as_str()
                .unwrap_or("minecraft:bedrock_floor")
                .to_string(),
            true_at: parse_anchor(&v["true_at_and_below"]),
            false_at: parse_anchor(&v["false_at_and_above"]),
        },
        "above_preliminary_surface" => Condition::AbovePreliminarySurface,
        "stone_depth" => Condition::StoneDepth {
            offset: v["offset"].as_i64().unwrap_or(0) as i32,
            add_surface_depth: v["add_surface_depth"].as_bool().unwrap_or(false),
            secondary_depth_range: v["secondary_depth_range"].as_i64().unwrap_or(0) as i32,
            ceiling: v["surface_type"].as_str() == Some("ceiling"),
        },
        "biome" => {
            let biomes = match &v["biome_is"] {
                Value::String(s) => vec![biome_name_to_id(s)],
                Value::Array(arr) => arr
                    .iter()
                    .filter_map(|x| x.as_str().map(biome_name_to_id))
                    .collect(),
                _ => vec![],
            };
            Condition::Biome(biomes)
        }
        "y_above" => Condition::YAbove {
            anchor: parse_anchor(&v["anchor"]),
            surface_depth_multiplier: v["surface_depth_multiplier"].as_i64().unwrap_or(0) as i32,
            add_stone_depth: v["add_stone_depth"].as_bool().unwrap_or(false),
        },
        "water" => Condition::Water {
            offset: v["offset"].as_i64().unwrap_or(0) as i32,
            surface_depth_multiplier: v["surface_depth_multiplier"].as_i64().unwrap_or(0) as i32,
            add_stone_depth: v["add_stone_depth"].as_bool().unwrap_or(false),
        },
        "noise_threshold" => {
            let noise = v["noise"]
                .as_str()
                .unwrap_or("minecraft:surface")
                .strip_prefix("minecraft:")
                .unwrap_or("surface")
                .to_string();
            Condition::NoiseThreshold {
                noise,
                min: v["min_threshold"].as_f64().unwrap_or(f64::NEG_INFINITY),
                max: v["max_threshold"].as_f64().unwrap_or(f64::INFINITY),
            }
        }
        "not" => Condition::Not(Box::new(parse_condition(&v["invert"]))),
        "hole" => Condition::Hole,
        "steep" => Condition::Steep,
        "temperature" => Condition::Temperature,
        _ => Condition::Hole, // never matches harmlessly? hole is surfaceDepth<=0
    }
}

fn parse_anchor(v: &Value) -> VerticalAnchor {
    if let Some(a) = v.get("absolute").and_then(|x| x.as_i64()) {
        return VerticalAnchor::Absolute(a as i32);
    }
    if let Some(a) = v.get("above_bottom").and_then(|x| x.as_i64()) {
        return VerticalAnchor::AboveBottom(a as i32);
    }
    if let Some(a) = v.get("below_top").and_then(|x| x.as_i64()) {
        return VerticalAnchor::BelowTop(a as i32);
    }
    VerticalAnchor::Absolute(0)
}

fn biome_name_to_id(name: &str) -> u8 {
    let n = name.strip_prefix("minecraft:").unwrap_or(name);
    match n {
        "ocean" => biome_id::OCEAN,
        "deep_ocean" => biome_id::DEEP_OCEAN,
        "frozen_ocean" => biome_id::FROZEN_OCEAN,
        "deep_frozen_ocean" => biome_id::FROZEN_OCEAN, // closest
        "lukewarm_ocean" | "deep_lukewarm_ocean" | "warm_ocean" => biome_id::OCEAN,
        "desert" => biome_id::DESERT,
        "plains" => biome_id::PLAINS,
        "forest" => biome_id::FOREST,
        "taiga" => biome_id::TAIGA,
        "swamp" => biome_id::SWAMP,
        "mangrove_swamp" => biome_id::MANGROVE_SWAMP,
        "river" => biome_id::RIVER,
        "frozen_river" => biome_id::FROZEN_RIVER,
        "beach" | "snowy_beach" => biome_id::BEACH,
        "stony_shore" => biome_id::STONY_SHORE,
        "savanna" | "windswept_savanna" => biome_id::SAVANNA,
        "jungle" => biome_id::JUNGLE,
        "snowy_plains" => biome_id::SNOWY_PLAINS,
        "snowy_slopes" => biome_id::SNOWY_SLOPES,
        "jagged_peaks" => biome_id::JAGGED_PEAKS,
        "frozen_peaks" => biome_id::FROZEN_PEAKS,
        "stony_peaks" => biome_id::STONY_PEAKS,
        "grove" => biome_id::GROVE,
        "windswept_hills" | "windswept_gravelly_hills" => biome_id::WINDSWEPT_HILLS,
        "dark_forest" => biome_id::DARK_FOREST,
        "meadow" => biome_id::MEADOW,
        "ice_spikes" => biome_id::ICE_SPIKES,
        "old_growth_pine_taiga" | "old_growth_pine_forest" => biome_id::OLD_GROWTH_PINE_FOREST,
        "old_growth_spruce_taiga" | "old_growth_birch_forest" => biome_id::OLD_GROWTH_BIRCH_FOREST,
        "birch_forest" => biome_id::BIRCH_FOREST,
        "cherry_grove" => biome_id::CHERRY_GROVE,
        "badlands" => biome_id::BADLANDS,
        "eroded_badlands" => biome_id::ERODED_BADLANDS,
        "wooded_badlands" => biome_id::WOODED_BADLANDS,
        "dripstone_caves" => biome_id::DRIPSTONE_CAVES,
        "deep_dark" => biome_id::DEEP_DARK,
        "mushroom_fields" => biome_id::MUSHROOM_FIELDS,
        "sulfur_caves" => biome_id::DRIPSTONE_CAVES, // fallback until dedicated id
        _ => biome_id::PLAINS,
    }
}
