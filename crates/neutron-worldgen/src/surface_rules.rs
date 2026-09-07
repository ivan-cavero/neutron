//! Surface rules: `SurfaceSystem.buildSurface` + datapack `surface_rule` JSON.
//!
//! Copyright (c) 2026 Neutron Contributors -- MIT License

use std::collections::HashMap;
use std::sync::OnceLock;

use serde_json::Value;

use crate::biome_source::biome_id;
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

            // Vanilla `height` in buildSurface is WORLD_SURFACE_WG + 1, which
            // INCLUDES fluids (SurfaceSystem.java:112,119). The caller's
            // heightmap is fluid-exclusive, so recompute the fluid-inclusive
            // top here: without it the y-loop starts below the water column,
            // water_height stays MIN at deep floors, the Water(-6) condition
            // returns true (vanilla treats MIN as exposed), and sediment
            // (dirt/sand) overwrites the stone floor vanilla leaves intact
            // (1.2M `stone->dirt` ledger cells across the 30-seed gate).
            let mut surface_y_fluid = surface_y;
            for y in (surface_y..WORLD_TOP).rev() {
                let b = BlockId::from_u16(
                    blocks[block_index(lx, y, lz)],
                )
                .unwrap_or(BlockId::Air);
                if !b.is_air() {
                    surface_y_fluid = y;
                    break;
                }
            }
            let surface_y = surface_y_fluid;
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
            let surface_depth = (surface_noise * 2.75 + 3.0 + depth_rng.next_f64() * 0.25) as i32;

            let surface_secondary = st
                .noises
                .noises()
                .get("surface_secondary")
                .map(|n| n.get_value(world_x as f64, 0.0, world_z as f64))
                .unwrap_or(0.0);

            // SurfaceRules.Context.getMinSurfaceLevel: bilinear lerp of the
            // 16x16 surface-cell corner preliminary surface levels. Each corner
            // is the preliminary_surface_level density function evaluated at
            // the quart-quantized corner block (NoiseChunk.preliminarySurfaceLevel:
            // QuartPos.toBlock(fromBlock(x)) = x & !3), floored and cached.
            // lerp alphas are f32 ((x & 15) / 16.0F); result floored, then
            // + surfaceDepth - 8.
            let corner_prelim = |cx: i32, cz: i32| -> i32 {
                let mut env = DensityEnv::new(cx & !3, 0, cz & !3, st.noises.noises());
                let v = crate::density::compute(&st.router.preliminary_surface_level, &mut env);
                v.floor() as i32
            };
            let cell_x = world_x >> 4;
            let cell_z = world_z >> 4;
            let tx = (world_x & 15) as f32 / 16.0f32;
            let tz = (world_z & 15) as f32 / 16.0f32;
            let p00 = corner_prelim(cell_x << 4, cell_z << 4) as f64;
            let p10 = corner_prelim((cell_x + 1) << 4, cell_z << 4) as f64;
            let p01 = corner_prelim(cell_x << 4, (cell_z + 1) << 4) as f64;
            let p11 = corner_prelim((cell_x + 1) << 4, (cell_z + 1) << 4) as f64;
            let lerp = |a: f64, b: f64, c: f64| b + a * (c - b);
            let prelim = lerp(
                tz as f64,
                lerp(tx as f64, p00, p10),
                lerp(tx as f64, p01, p11),
            );
            let min_surface_level = (prelim.floor() as i32) + surface_depth - 8;


            // Steep: neighbour height delta >= 4 (within chunk)
            let steep = is_steep(heightmap, lx, lz);

            let mut stone_above = 0i32;
            let mut water_height = i32::MIN;
            let mut next_ceiling_stone_y = i32::MAX;
            let end_y = WORLD_BOTTOM;
            let height = surface_y + 1;
            // Cache last cave-biome sample to avoid density eval per block.

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

                // Vanilla SurfaceSystem evaluates context.biome (BiomeManager.getBiome)
                // PER BLOCK — no surface shortcut. (The previous 8-block cache and the
                // min_surface_level-16 surface shortcut both desynced biome-gated rules:
                // on seed 777 sulfur_caves bands at y 27-45 sat above min_surf-16, got the
                // surface biome, and the sulfur/cinnabar rule never fired — a 190k-cell
                // sulfur-family gap.)
                let biome = sample_biome(st, world_x, y, world_z);

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
    // SurfaceRules.Context.SteepMaterialCondition: WORLD_SURFACE_WG heightmap,
    // ONE-directional — south >= north + 4, else west >= east + 4. Chunk-edge
    // neighbors clamp to the block's own row (equal heights -> false).
    let hn = heightmap[lz.saturating_sub(1) * 16 + lx] as i32;
    let hs = heightmap[(lz + 1).min(15) * 16 + lx] as i32;
    if hs >= hn + 4 {
        return true;
    }
    let hw = heightmap[lz * 16 + lx.saturating_sub(1)] as i32;
    let he = heightmap[lz * 16 + (lx + 1).min(15)] as i32;
    hw >= he + 4
}

fn is_stone_like(b: BlockId) -> bool {
    !b.is_air() && !b.is_fluid()
}

fn sample_biome(st: &WorldgenState, x: i32, y: i32, z: i32) -> u8 {
    // SurfaceSystem.buildSurface uses `biomeManager::getBiome` (4-block voronoi).
    crate::biome_manager::biome_id_at_block(st, x, y, z)
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
                // SurfaceRules.NoiseThresholdCondition.test: >= min && <= max
                v >= *min && v <= *max
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
            let name = v["result_state"]["Name"]
                .as_str()
                .unwrap_or("minecraft:stone");
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
        "deep_frozen_ocean" => biome_id::DEEP_FROZEN_OCEAN,
        "deep_cold_ocean" => biome_id::DEEP_COLD_OCEAN,
        "cold_ocean" => biome_id::COLD_OCEAN,
        "deep_lukewarm_ocean" => biome_id::DEEP_LUKEWARM_OCEAN,
        "lukewarm_ocean" => biome_id::LUKEWARM_OCEAN,
        "warm_ocean" => biome_id::WARM_OCEAN,
        "desert" => biome_id::DESERT,
        "plains" => biome_id::PLAINS,
        "sunflower_plains" => biome_id::SUNFLOWER_PLAINS,
        "forest" => biome_id::FOREST,
        "flower_forest" => biome_id::FLOWER_FOREST,
        "taiga" => biome_id::TAIGA,
        "snowy_taiga" => biome_id::SNOWY_TAIGA,
        "swamp" => biome_id::SWAMP,
        "mangrove_swamp" => biome_id::MANGROVE_SWAMP,
        "river" => biome_id::RIVER,
        "frozen_river" => biome_id::FROZEN_RIVER,
        "beach" => biome_id::BEACH,
        "snowy_beach" => biome_id::SNOWY_BEACH,
        "stony_shore" => biome_id::STONY_SHORE,
        "savanna" => biome_id::SAVANNA,
        "savanna_plateau" => biome_id::SAVANNA_PLATEAU,
        "windswept_savanna" => biome_id::WINDSWEPT_SAVANNA,
        "jungle" => biome_id::JUNGLE,
        "sparse_jungle" => biome_id::SPARSE_JUNGLE,
        "bamboo_jungle" => biome_id::BAMBOO_JUNGLE,
        "snowy_plains" => biome_id::SNOWY_PLAINS,
        "snowy_slopes" => biome_id::SNOWY_SLOPES,
        "jagged_peaks" => biome_id::JAGGED_PEAKS,
        "frozen_peaks" => biome_id::FROZEN_PEAKS,
        "stony_peaks" => biome_id::STONY_PEAKS,
        "grove" => biome_id::GROVE,
        "windswept_hills" => biome_id::WINDSWEPT_HILLS,
        "windswept_gravelly_hills" => biome_id::WINDSWEPT_GRAVELLY_HILLS,
        "windswept_forest" => biome_id::WINDSWEPT_FOREST,
        "dark_forest" => biome_id::DARK_FOREST,
        "meadow" => biome_id::MEADOW,
        "ice_spikes" => biome_id::ICE_SPIKES,
        "old_growth_pine_taiga" | "old_growth_pine_forest" => biome_id::OLD_GROWTH_PINE_FOREST,
        "old_growth_spruce_taiga" => biome_id::OLD_GROWTH_SPRUCE_TAIGA,
        "old_growth_birch_forest" => biome_id::OLD_GROWTH_BIRCH_FOREST,
        "birch_forest" => biome_id::BIRCH_FOREST,
        "cherry_grove" => biome_id::CHERRY_GROVE,
        "pale_garden" => biome_id::PALE_GARDEN,
        "badlands" => biome_id::BADLANDS,
        "eroded_badlands" => biome_id::ERODED_BADLANDS,
        "wooded_badlands" => biome_id::WOODED_BADLANDS,
        "dripstone_caves" => biome_id::DRIPSTONE_CAVES,
        "lush_caves" => biome_id::LUSH_CAVES,
        "deep_dark" => biome_id::DEEP_DARK,
        "mushroom_fields" => biome_id::MUSHROOM_FIELDS,
        "sulfur_caves" => biome_id::SULFUR_CAVES,
        _ => biome_id::PLAINS,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 777 sulfur_caves chunk (-5,-6): vanilla paints sulfur/cinnabar bands
    /// via the sulfur_cave_gradient noise (SurfaceRuleData sulfurCaveBands).
    /// This check pins the noise registration + biome gate used by the
    /// surface rule: params must match the vanilla JSON, and the biome
    /// gate `biome_name_to_id("sulfur_caves")` must equal the biome-source
    /// id (36).
    #[test]
    #[ignore = "diagnostic: generates chunk (-5,-6) seed 777 through noise+surface and prints blocks at mismatch cells"]
    fn sulfur_pipeline_dump() {
        let gen = crate::ChunkGenerator::new(777);
        // (chunk -> cells) so every dump line maps to its own chunk
        let mut by_chunk: std::collections::BTreeMap<(i32, i32), Vec<String>> = Default::default();
        for line in std::fs::read_to_string("/tmp/sulfur_cells.txt").unwrap().lines() {
            let mut it = line.split_whitespace();
            let wx: i32 = it.next().unwrap().parse().unwrap();
            let _y: i32 = it.next().unwrap().parse().unwrap();
            let wz: i32 = it.next().unwrap().parse().unwrap();
            by_chunk.entry((wx.div_euclid(16), wz.div_euclid(16))).or_default().push(line.to_string());
        }
        for ((cx, cz), lines) in by_chunk {
            let (blocks, _, _) = gen.generate_noise_and_surface(cx, cz);
            for line in lines.iter().take(12) {
                let mut it = line.split_whitespace();
                let wx: i32 = it.next().unwrap().parse().unwrap();
                let y: i32 = it.next().unwrap().parse().unwrap();
                let wz: i32 = it.next().unwrap().parse().unwrap();
                let lx = (wx - cx * 16) as usize;
                let lz = (wz - cz * 16) as usize;
                let rel = y - crate::generator::WORLD_BOTTOM;
                let idx = (rel as usize) * 256 + lz * 16 + lx;
                let b = crate::surface::BlockId::from_u16(blocks[idx]);
                let pip_biome = crate::biome::manager::biome_id_at_block(&gen.state, wx, y, wz);
                println!("PIPE {wx} {y} {wz} -> {:?} biome_here={pip_biome}", b);
            }
        }
    }

    #[test]
    #[ignore = "diagnostic: walks the parsed surface rule at vanilla-mismatch cells and prints the verdict"]
    fn sulfur_rule_walk() {
        let rule = overworld_rule();
        let reg = crate::density::DensityRegistry::build();
        let ns = crate::worldgen::NoiseSet::for_seed(777, &reg);
        for line in std::fs::read_to_string("/tmp/sulfur_cells.txt").unwrap().lines().take(20) {
            let mut it = line.split_whitespace();
            let x: i32 = it.next().unwrap().parse().unwrap();
            let y: i32 = it.next().unwrap().parse().unwrap();
            let z: i32 = it.next().unwrap().parse().unwrap();
            let grad = ns.noises().get("sulfur_cave_gradient").unwrap()
                .get_value(x as f64, y as f64, z as f64);
            let ctx = RuleContext {
                x, y, z,
                stone_depth_above: 1,
                stone_depth_below: 1,
                water_height: i32::MIN,
                surface_depth: 3,
                surface_secondary: 0.0,
                min_surface_level: 60,
                biome: biome_id::SULFUR_CAVES,
                steep: false,
                hole: false,
                noises: ns.noises(),
                main_rng: crate::positional::PositionalRandomFactory::new(0, 0),
                sea_level: 63,
            };
            let mut ctx = ctx;
            let out = rule.try_apply(&mut ctx);
            println!("WALK {x} {y} {z} grad={grad:.6} -> {:?}", out);
        }
    }

    #[test]
    #[ignore = "diagnostic: for each vanilla/neutron classifier mismatch, prints the climate target and the fitness of BOTH answers under neutron's metric — equal fitness = tie-break divergence"]
    fn sulfur_biome_dump() {
        let gen = crate::ChunkGenerator::new(456);
        let mut by_chunk: std::collections::BTreeMap<(i32, i32), Vec<String>> = Default::default();
        for line in std::fs::read_to_string("/tmp/clay456.txt").unwrap().lines() {
            let mut it = line.split_whitespace();
            let wx: i32 = it.next().unwrap().parse().unwrap();
            let _y: i32 = it.next().unwrap().parse().unwrap();
            let wz: i32 = it.next().unwrap().parse().unwrap();
            by_chunk.entry((wx.div_euclid(16), wz.div_euclid(16))).or_default().push(line.to_string());
        }
        for ((cx, cz), lines) in by_chunk.iter(){
            let (blocks, _, _) = gen.generate_noise_and_surface(*cx, *cz);
            for line in lines.iter().take(10) {
                let mut it = line.split_whitespace();
                let wx: i32 = it.next().unwrap().parse().unwrap();
                let y: i32 = it.next().unwrap().parse().unwrap();
                let wz: i32 = it.next().unwrap().parse().unwrap();
                let lx = (wx - cx * 16) as usize;
                let lz = (wz - cz * 16) as usize;
                let rel = y - crate::generator::WORLD_BOTTOM;
                let idx = (rel as usize) * 256 + lz * 16 + lx;
                let b = crate::surface::BlockId::from_u16(blocks[idx]);
                let pip = crate::biome::manager::biome_id_at_block(&gen.state, wx, y, wz);
                let g = crate::biome::manager::climate_at(&gen.state, wx, y, wz);
                let grad = crate::worldgen::NoiseSet::for_seed(777, &gen.state.reg);
                let gv = grad.noises().get("sulfur_cave_gradient").unwrap()
                    .get_value(wx as f64, y as f64, wz as f64);
                let _ = (pip, g);
                println!("CLAY {wx} {y} {wz} -> {:?} biome={pip}", b);
            }
        }
    }

    /// Column (-88,-56) seed 777 — drove the per-block biome fix (55b0f5f) and the
    /// shortcut removal (0d3093d). Vanilla voronoi (ProbeBiomeAt): birch_forest at
    /// y -30..-25, sulfur_caves at -24,-23, birch at -22..-20, sulfur at -19..-16.
    /// Neutron must match block-for-block now.
    #[test]
    #[ignore = "diagnostic: compares seed-777 column (-88,-56) biome ids against vanilla voronoi"]
    fn sulfur_column_biomes() {
        let gen = crate::ChunkGenerator::new(777);
        // (y, expected vanilla biome id)
        let want: [(i32, u8); 15] = [
            (-30, 33), (-29, 33), (-28, 33), (-27, 33), (-26, 33), (-25, 33),
            (-24, 36), (-23, 36), (-22, 33), (-21, 33), (-20, 33),
            (-19, 36), (-18, 36), (-17, 36), (-16, 36),
        ];
        for (y, exp) in want {
            let b = crate::biome::manager::biome_id_at_block(&gen.state, -88, y, -56);
            assert_eq!(b, exp, "column (-88,{y},-56)");
        }
    }

    /// Seed 777 chunk (-6,-4): the sulfur-band cells (-88,-24,-56) etc. must now be
    /// painted sulfur/cinnabar by the surface rule (previously deepslate — the
    /// 8-block cache bug).
    #[test]
    #[ignore = "diagnostic: verifies surface rule paints sulfur at the previously-broken cells"]
    fn sulfur_pipeline_fixed() {
        let gen = crate::ChunkGenerator::new(777);
        let (blocks, _, _) = gen.generate_noise_and_surface(-6, -4);
        let cells: [(i32, i32, i32); 4] = [
            (-88, -24, -56), (-85, -23, -61), (-84, -22, -50), (-83, -21, -54),
        ];
        for (wx, y, wz) in cells {
            let idx = ((y - crate::generator::WORLD_BOTTOM) as usize) * 256
                + ((wz + 64) as usize) * 16
                + ((wx + 96) as usize);
            let b = crate::surface::BlockId::from_u16(blocks[idx]).unwrap();
            assert!(
                matches!(b, crate::surface::BlockId::Sulfur | crate::surface::BlockId::Cinnabar),
                "({wx},{y},{wz}) = {b:?}, want sulfur/cinnabar"
            );
        }
    }

    #[test]
    fn sulfur_cave_gradient_noise_and_gate() {
        assert_eq!(biome_name_to_id("sulfur_caves"), biome_id::SULFUR_CAVES);
        let reg = crate::density::DensityRegistry::build();
        let (o, a) = reg.noise_params("sulfur_cave_gradient");
        assert_eq!(*o, -5);
        assert_eq!(a.as_slice(), &[1.0, 0.0, 1.0]);
    }
}
