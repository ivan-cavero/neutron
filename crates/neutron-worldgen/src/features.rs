//! Underground ore, disk and magma features (generation step 6).
//!
//! Ports `OreFeature`, `DiskFeature`, `UnderwaterMagmaFeature` and the
//! placement chain (`count`, `rarity_filter`, `in_square`, `height_range`,
//! `OCEAN_FLOOR_WG`, water `matching_fluids`).
//!
//! FeatureSorter step-6 indices are explicit (`feature_index`). Biome filter
//! is sampled at the placement origin.
//!
//! Copyright (c) 2026 Neutron Contributors -- MIT License

use crate::biome_manager::biome_id_at_block;
use crate::biome_source::biome_id;
use crate::feature_rng::FeatureRandom;
use crate::generator::{WORLD_BOTTOM, WORLD_TOP};
use crate::region_buf::RegionBuf;
use crate::surface::BlockId;
use crate::worldgen::WorldgenState;

const STEP_UNDERGROUND_ORES: i32 = 6;
const PI: f32 = 3.1415927;

/// Apply underground ores for every chunk origin inside `region`.
///
/// Origin-major, center first (vanilla FEATURES order) with masking of the
/// not-yet-decorated origins — deterministic and matches a full WorldGenRegion
/// decoration pass over the area.
pub fn apply_underground_ores_region(region: &mut RegionBuf, level_seed: i64) {
    let order = crate::sculk::decoration_origin_order(region.chunks);
    for (pos, &(cxl, czl)) in order.iter().enumerate() {
        let origin_min_x = region.origin_x + cxl * 16;
        let origin_min_z = region.origin_z + czl * 16;
        apply_underground_ores_origin(
            region,
            level_seed,
            origin_min_x,
            origin_min_z,
            &order[pos + 1..],
        );
    }
}

/// Run the step-6 ore/disk pass for ONE chunk origin `(origin_min_x,
/// origin_min_z)`. `undecorated` are the origins after this one in the
/// decoration order: their feature output is masked to the terrain base for
/// the duration of the pass and restored afterwards (vanilla decorates each
/// origin while the not-yet-decorated neighbours are still at CARVERS).
pub(crate) fn apply_underground_ores_origin(
    region: &mut RegionBuf,
    level_seed: i64,
    origin_min_x: i32,
    origin_min_z: i32,
    undecorated: &[(i32, i32)],
) {
    let state = WorldgenState::overworld(level_seed);
    let saved = crate::sculk::mask_undecorated_output(region, undecorated, crate::sculk::FAMILY_ALL);
    let mut rng = FeatureRandom::new(level_seed);
    let decoration_seed = rng.set_decoration_seed(level_seed, origin_min_x, origin_min_z);
    for def in OVERWORLD_ORES {
        rng.set_feature_seed(decoration_seed, def.feature_index, STEP_UNDERGROUND_ORES);
        if matches!(def.gate, BiomeGate::Off) {
            continue;
        }
        place_feature(&mut rng, region, &state, origin_min_x, origin_min_z, def);
    }
    crate::sculk::restore_masked(region, saved);
}

// ---------------------------------------------------------------------------
// Feature definitions (from datapack placed_feature + configured_feature)
// ---------------------------------------------------------------------------

#[derive(Clone, Copy)]
enum CountSpec {
    Fixed(i32),
    Uniform { min: i32, max: i32 },
    Rarity(i32),
}

#[derive(Clone, Copy)]
enum HeightSpec {
    Uniform {
        min: HeightAnchor,
        max: HeightAnchor,
    },
    Trapezoid {
        min: HeightAnchor,
        max: HeightAnchor,
    },
}

#[derive(Clone, Copy)]
enum HeightAnchor {
    Absolute(i32),
    AboveBottom(i32),
    BelowTop(i32),
}

impl HeightAnchor {
    fn resolve(self) -> i32 {
        match self {
            HeightAnchor::Absolute(y) => y,
            HeightAnchor::AboveBottom(n) => WORLD_BOTTOM + n,
            HeightAnchor::BelowTop(n) => (WORLD_TOP - 1) - n,
        }
    }
}

#[derive(Clone, Copy)]
struct FeatureDef {
    /// FeatureSorter global index for step 6.
    feature_index: i32,
    gate: BiomeGate,
    count: CountSpec,
    y: YSpec,
    kind: FeatureKind,
}

/// Vanilla `minecraft:biome` placement filter, sampled at the origin.
#[derive(Clone, Copy)]
enum BiomeGate {
    Any,
    /// Index reserved; no place (missing BlockId or colliding biome id).
    Off,
    Ids(&'static [u8]),
}

#[derive(Clone, Copy)]
enum YSpec {
    Range {
        height: HeightSpec,
        /// `surface_relative_threshold_filter` vs OCEAN_FLOOR_WG first-available.
        max_relative_to_ocean_floor_wg: Option<i32>,
    },
    /// `heightmap` OCEAN_FLOOR_WG (`getHeight` = first available = solid+1).
    OceanFloorWg {
        require_water: bool,
        require_mud: bool,
        y_offset: i32,
    },
}

#[derive(Clone, Copy)]
enum FeatureKind {
    Ore(OreDef),
    Disk(DiskDef),
    UnderwaterMagma {
        floor_search_range: i32,
        probability: f32,
        radius: i32,
    },
}

#[derive(Clone, Copy)]
struct OreDef {
    size: i32,
    discard_chance: f32,
    stone_block: BlockId,
    deepslate_block: Option<BlockId>,
    target: TargetKind,
}

#[derive(Clone, Copy)]
struct DiskDef {
    half_height: i32,
    radius_min: i32,
    radius_max: i32,
    target: DiskTarget,
    provider: DiskProvider,
}

#[derive(Clone, Copy)]
enum DiskTarget {
    DirtOrClay,
    DirtOrGrass,
    DirtOrMud,
}

#[derive(Clone, Copy)]
enum DiskProvider {
    Simple(BlockId),
    /// Sand; sandstone if the block below is air.
    SandOrSandstone,
    /// Grass if the block above is neither solid nor water; else dirt.
    GrassOrDirt,
}

#[derive(Clone, Copy)]
enum TargetKind {
    /// stone, granite, diorite, andesite
    StoneOre,
    /// stone + granite + diorite + andesite + tuff + deepslate
    BaseStone,
    /// deepslate + tuff only (unused for most; deepslate ores use dual target)
    DeepslateOre,
}

const fn range_y(height: HeightSpec) -> YSpec {
    YSpec::Range {
        height,
        max_relative_to_ocean_floor_wg: None,
    }
}

const fn ore_kind(
    size: i32,
    discard_chance: f32,
    stone_block: BlockId,
    deepslate_block: Option<BlockId>,
    target: TargetKind,
) -> FeatureKind {
    FeatureKind::Ore(OreDef {
        size,
        discard_chance,
        stone_block,
        deepslate_block,
        target,
    })
}

const OVERWORLD_ORES: &[FeatureDef] = &[
    // 0 ore_dirt
    FeatureDef {
        feature_index: 0,
        gate: BiomeGate::Any,
        count: CountSpec::Fixed(7),
        y: range_y(HeightSpec::Uniform {
            min: HeightAnchor::Absolute(0),
            max: HeightAnchor::Absolute(160),
        }),
        kind: ore_kind(33, 0.0, BlockId::Dirt, None, TargetKind::BaseStone),
    },
    // 1 ore_gravel
    FeatureDef {
        feature_index: 1,
        gate: BiomeGate::Any,
        count: CountSpec::Fixed(14),
        y: range_y(HeightSpec::Uniform {
            min: HeightAnchor::AboveBottom(0),
            max: HeightAnchor::BelowTop(0),
        }),
        kind: ore_kind(33, 0.0, BlockId::Gravel, None, TargetKind::BaseStone),
    },
    // 2 ore_granite_upper (rarity 6)
    FeatureDef {
        feature_index: 2,
        gate: BiomeGate::Any,
        count: CountSpec::Rarity(6),
        y: range_y(HeightSpec::Uniform {
            min: HeightAnchor::Absolute(64),
            max: HeightAnchor::Absolute(128),
        }),
        kind: ore_kind(64, 0.0, BlockId::Granite, None, TargetKind::BaseStone),
    },
    // 3 ore_granite_lower
    FeatureDef {
        feature_index: 3,
        gate: BiomeGate::Any,
        count: CountSpec::Fixed(2),
        y: range_y(HeightSpec::Uniform {
            min: HeightAnchor::Absolute(0),
            max: HeightAnchor::Absolute(60),
        }),
        kind: ore_kind(64, 0.0, BlockId::Granite, None, TargetKind::BaseStone),
    },
    // 4 ore_diorite_upper
    FeatureDef {
        feature_index: 4,
        gate: BiomeGate::Any,
        count: CountSpec::Rarity(6),
        y: range_y(HeightSpec::Uniform {
            min: HeightAnchor::Absolute(64),
            max: HeightAnchor::Absolute(128),
        }),
        kind: ore_kind(64, 0.0, BlockId::Diorite, None, TargetKind::BaseStone),
    },
    // 5 ore_diorite_lower
    FeatureDef {
        feature_index: 5,
        gate: BiomeGate::Any,
        count: CountSpec::Fixed(2),
        y: range_y(HeightSpec::Uniform {
            min: HeightAnchor::Absolute(0),
            max: HeightAnchor::Absolute(60),
        }),
        kind: ore_kind(64, 0.0, BlockId::Diorite, None, TargetKind::BaseStone),
    },
    // 6 ore_andesite_upper
    FeatureDef {
        feature_index: 6,
        gate: BiomeGate::Any,
        count: CountSpec::Rarity(6),
        y: range_y(HeightSpec::Uniform {
            min: HeightAnchor::Absolute(64),
            max: HeightAnchor::Absolute(128),
        }),
        kind: ore_kind(64, 0.0, BlockId::Andesite, None, TargetKind::BaseStone),
    },
    // 7 ore_andesite_lower
    FeatureDef {
        feature_index: 7,
        gate: BiomeGate::Any,
        count: CountSpec::Fixed(2),
        y: range_y(HeightSpec::Uniform {
            min: HeightAnchor::Absolute(0),
            max: HeightAnchor::Absolute(60),
        }),
        kind: ore_kind(64, 0.0, BlockId::Andesite, None, TargetKind::BaseStone),
    },
    // 8 ore_tuff
    FeatureDef {
        feature_index: 8,
        gate: BiomeGate::Any,
        count: CountSpec::Fixed(2),
        y: range_y(HeightSpec::Uniform {
            min: HeightAnchor::AboveBottom(0),
            max: HeightAnchor::Absolute(0),
        }),
        kind: ore_kind(64, 0.0, BlockId::Tuff, None, TargetKind::BaseStone),
    },
    // 9 ore_coal_upper
    FeatureDef {
        feature_index: 9,
        gate: BiomeGate::Any,
        count: CountSpec::Fixed(30),
        y: range_y(HeightSpec::Uniform {
            min: HeightAnchor::Absolute(136),
            max: HeightAnchor::BelowTop(0),
        }),
        kind: ore_kind(
            17,
            0.0,
            BlockId::CoalOre,
            Some(BlockId::DeepslateCoalOre),
            TargetKind::StoneOre,
        ),
    },
    // 10 ore_coal_lower (buried)
    FeatureDef {
        feature_index: 10,
        gate: BiomeGate::Any,
        count: CountSpec::Fixed(20),
        y: range_y(HeightSpec::Trapezoid {
            min: HeightAnchor::Absolute(0),
            max: HeightAnchor::Absolute(192),
        }),
        kind: ore_kind(
            17,
            0.5,
            BlockId::CoalOre,
            Some(BlockId::DeepslateCoalOre),
            TargetKind::StoneOre,
        ),
    },
    // 11 ore_iron_upper
    FeatureDef {
        feature_index: 11,
        gate: BiomeGate::Any,
        count: CountSpec::Fixed(90),
        y: range_y(HeightSpec::Trapezoid {
            min: HeightAnchor::Absolute(80),
            max: HeightAnchor::Absolute(384),
        }),
        kind: ore_kind(
            9,
            0.0,
            BlockId::IronOre,
            Some(BlockId::DeepslateIronOre),
            TargetKind::StoneOre,
        ),
    },
    // 12 ore_iron_middle
    FeatureDef {
        feature_index: 12,
        gate: BiomeGate::Any,
        count: CountSpec::Fixed(10),
        y: range_y(HeightSpec::Trapezoid {
            min: HeightAnchor::Absolute(-24),
            max: HeightAnchor::Absolute(56),
        }),
        kind: ore_kind(
            9,
            0.0,
            BlockId::IronOre,
            Some(BlockId::DeepslateIronOre),
            TargetKind::StoneOre,
        ),
    },
    // 13 ore_iron_small
    FeatureDef {
        feature_index: 13,
        gate: BiomeGate::Any,
        count: CountSpec::Fixed(10),
        y: range_y(HeightSpec::Uniform {
            min: HeightAnchor::AboveBottom(0),
            max: HeightAnchor::Absolute(72),
        }),
        kind: ore_kind(
            4,
            0.0,
            BlockId::IronOre,
            Some(BlockId::DeepslateIronOre),
            TargetKind::StoneOre,
        ),
    },
    // 14 ore_gold
    FeatureDef {
        feature_index: 14,
        gate: BiomeGate::Any,
        count: CountSpec::Fixed(4),
        y: range_y(HeightSpec::Trapezoid {
            min: HeightAnchor::Absolute(-64),
            max: HeightAnchor::Absolute(32),
        }),
        kind: ore_kind(
            9,
            0.5,
            BlockId::GoldOre,
            Some(BlockId::DeepslateGoldOre),
            TargetKind::StoneOre,
        ),
    },
    // 15 ore_gold_lower
    FeatureDef {
        feature_index: 15,
        gate: BiomeGate::Any,
        count: CountSpec::Uniform { min: 0, max: 1 },
        y: range_y(HeightSpec::Uniform {
            min: HeightAnchor::Absolute(-64),
            max: HeightAnchor::Absolute(-48),
        }),
        kind: ore_kind(
            9,
            0.5,
            BlockId::GoldOre,
            Some(BlockId::DeepslateGoldOre),
            TargetKind::StoneOre,
        ),
    },
    // 16 ore_redstone
    FeatureDef {
        feature_index: 16,
        gate: BiomeGate::Any,
        count: CountSpec::Fixed(4),
        y: range_y(HeightSpec::Uniform {
            min: HeightAnchor::AboveBottom(0),
            max: HeightAnchor::Absolute(15),
        }),
        kind: ore_kind(
            8,
            0.0,
            BlockId::RedstoneOre,
            Some(BlockId::DeepslateRedstoneOre),
            TargetKind::StoneOre,
        ),
    },
    // 17 ore_redstone_lower
    FeatureDef {
        feature_index: 17,
        gate: BiomeGate::Any,
        count: CountSpec::Fixed(8),
        y: range_y(HeightSpec::Trapezoid {
            min: HeightAnchor::AboveBottom(-32),
            max: HeightAnchor::AboveBottom(32),
        }),
        kind: ore_kind(
            8,
            0.0,
            BlockId::RedstoneOre,
            Some(BlockId::DeepslateRedstoneOre),
            TargetKind::StoneOre,
        ),
    },
    // 18 ore_diamond (small)
    FeatureDef {
        feature_index: 18,
        gate: BiomeGate::Any,
        count: CountSpec::Fixed(7),
        y: range_y(HeightSpec::Trapezoid {
            min: HeightAnchor::AboveBottom(-80),
            max: HeightAnchor::AboveBottom(80),
        }),
        kind: ore_kind(
            4,
            0.5,
            BlockId::DiamondOre,
            Some(BlockId::DeepslateDiamondOre),
            TargetKind::StoneOre,
        ),
    },
    // 19 ore_diamond_medium
    FeatureDef {
        feature_index: 19,
        gate: BiomeGate::Any,
        count: CountSpec::Fixed(2),
        y: range_y(HeightSpec::Uniform {
            min: HeightAnchor::Absolute(-64),
            max: HeightAnchor::Absolute(-4),
        }),
        kind: ore_kind(
            8,
            0.5,
            BlockId::DiamondOre,
            Some(BlockId::DeepslateDiamondOre),
            TargetKind::StoneOre,
        ),
    },
    // 20 ore_diamond_large (rarity 9)
    FeatureDef {
        feature_index: 20,
        gate: BiomeGate::Any,
        count: CountSpec::Rarity(9),
        y: range_y(HeightSpec::Trapezoid {
            min: HeightAnchor::AboveBottom(-80),
            max: HeightAnchor::AboveBottom(80),
        }),
        kind: ore_kind(
            12,
            0.7,
            BlockId::DiamondOre,
            Some(BlockId::DeepslateDiamondOre),
            TargetKind::StoneOre,
        ),
    },
    // 21 ore_diamond_buried
    FeatureDef {
        feature_index: 21,
        gate: BiomeGate::Any,
        count: CountSpec::Fixed(4),
        y: range_y(HeightSpec::Trapezoid {
            min: HeightAnchor::AboveBottom(-80),
            max: HeightAnchor::AboveBottom(80),
        }),
        kind: ore_kind(
            8,
            1.0,
            BlockId::DiamondOre,
            Some(BlockId::DeepslateDiamondOre),
            TargetKind::StoneOre,
        ),
    },
    // 22 ore_lapis
    FeatureDef {
        feature_index: 22,
        gate: BiomeGate::Any,
        count: CountSpec::Fixed(2),
        y: range_y(HeightSpec::Trapezoid {
            min: HeightAnchor::Absolute(-32),
            max: HeightAnchor::Absolute(32),
        }),
        kind: ore_kind(
            7,
            0.0,
            BlockId::LapisOre,
            Some(BlockId::DeepslateLapisOre),
            TargetKind::StoneOre,
        ),
    },
    // 23 ore_lapis_buried
    FeatureDef {
        feature_index: 23,
        gate: BiomeGate::Any,
        count: CountSpec::Fixed(4),
        y: range_y(HeightSpec::Uniform {
            min: HeightAnchor::AboveBottom(0),
            max: HeightAnchor::Absolute(64),
        }),
        kind: ore_kind(
            7,
            1.0,
            BlockId::LapisOre,
            Some(BlockId::DeepslateLapisOre),
            TargetKind::StoneOre,
        ),
    },
    // 24 ore_copper_large — dripstone_caves only
    FeatureDef {
        feature_index: 24,
        gate: BiomeGate::Ids(&[biome_id::DRIPSTONE_CAVES]),
        count: CountSpec::Fixed(16),
        y: range_y(HeightSpec::Trapezoid {
            min: HeightAnchor::Absolute(-16),
            max: HeightAnchor::Absolute(112),
        }),
        kind: ore_kind(
            20,
            0.0,
            BlockId::CopperOre,
            Some(BlockId::DeepslateCopperOre),
            TargetKind::StoneOre,
        ),
    },
    // 25 ore_copper
    FeatureDef {
        feature_index: 25,
        gate: BiomeGate::Any,
        count: CountSpec::Fixed(16),
        y: range_y(HeightSpec::Trapezoid {
            min: HeightAnchor::Absolute(-16),
            max: HeightAnchor::Absolute(112),
        }),
        kind: ore_kind(
            10,
            0.0,
            BlockId::CopperOre,
            Some(BlockId::DeepslateCopperOre),
            TargetKind::StoneOre,
        ),
    },
    // 26 underwater_magma — most overworld biomes (incl. deep_dark)
    FeatureDef {
        feature_index: 26,
        gate: BiomeGate::Any,
        count: CountSpec::Uniform { min: 44, max: 52 },
        y: YSpec::Range {
            height: HeightSpec::Uniform {
                min: HeightAnchor::AboveBottom(0),
                max: HeightAnchor::Absolute(256),
            },
            max_relative_to_ocean_floor_wg: Some(-2),
        },
        kind: FeatureKind::UnderwaterMagma {
            floor_search_range: 5,
            probability: 0.5,
            radius: 1,
        },
    },
    // 27 ore_clay — lush_caves only
    FeatureDef {
        feature_index: 27,
        gate: BiomeGate::Ids(&[biome_id::LUSH_CAVES]),
        count: CountSpec::Fixed(46),
        y: range_y(HeightSpec::Uniform {
            min: HeightAnchor::AboveBottom(0),
            max: HeightAnchor::Absolute(256),
        }),
        kind: ore_kind(33, 0.0, BlockId::Clay, None, TargetKind::BaseStone),
    },
    // 28 ore_gold_extra — badlands-like only (configured ore_gold, discard 0)
    FeatureDef {
        feature_index: 28,
        gate: BiomeGate::Ids(&[
            biome_id::BADLANDS,
            biome_id::ERODED_BADLANDS,
            biome_id::WOODED_BADLANDS,
        ]),
        count: CountSpec::Fixed(50),
        y: range_y(HeightSpec::Uniform {
            min: HeightAnchor::Absolute(32),
            max: HeightAnchor::Absolute(256),
        }),
        kind: ore_kind(
            9,
            0.0,
            BlockId::GoldOre,
            Some(BlockId::DeepslateGoldOre),
            TargetKind::StoneOre,
        ),
    },
    // 29 disk_grass — mangrove_swamp only
    FeatureDef {
        feature_index: 29,
        gate: BiomeGate::Ids(&[biome_id::MANGROVE_SWAMP]),
        count: CountSpec::Fixed(1),
        y: YSpec::OceanFloorWg {
            require_water: false,
            require_mud: true,
            y_offset: -1,
        },
        kind: FeatureKind::Disk(DiskDef {
            half_height: 2,
            radius_min: 2,
            radius_max: 6,
            target: DiskTarget::DirtOrMud,
            provider: DiskProvider::GrassOrDirt,
        }),
    },
    // 30 disk_sand — most biomes (incl. deep_dark); count 3
    FeatureDef {
        feature_index: 30,
        gate: BiomeGate::Any,
        count: CountSpec::Fixed(3),
        y: YSpec::OceanFloorWg {
            require_water: true,
            require_mud: false,
            y_offset: 0,
        },
        kind: FeatureKind::Disk(DiskDef {
            half_height: 2,
            radius_min: 2,
            radius_max: 6,
            target: DiskTarget::DirtOrGrass,
            provider: DiskProvider::SandOrSandstone,
        }),
    },
    // 31 disk_clay — most biomes (incl. deep_dark)
    FeatureDef {
        feature_index: 31,
        gate: BiomeGate::Any,
        count: CountSpec::Fixed(1),
        y: YSpec::OceanFloorWg {
            require_water: true,
            require_mud: false,
            y_offset: 0,
        },
        kind: FeatureKind::Disk(DiskDef {
            half_height: 1,
            radius_min: 2,
            radius_max: 3,
            target: DiskTarget::DirtOrClay,
            provider: DiskProvider::Simple(BlockId::Clay),
        }),
    },
    // 32 disk_gravel — most biomes (incl. deep_dark)
    FeatureDef {
        feature_index: 32,
        gate: BiomeGate::Any,
        count: CountSpec::Fixed(1),
        y: YSpec::OceanFloorWg {
            require_water: true,
            require_mud: false,
            y_offset: 0,
        },
        kind: FeatureKind::Disk(DiskDef {
            half_height: 2,
            radius_min: 2,
            radius_max: 5,
            target: DiskTarget::DirtOrGrass,
            provider: DiskProvider::Simple(BlockId::Gravel),
        }),
    },
    // 33 ore_emerald — peaks/grove/meadow/…; BlockId lacks emerald ore
    FeatureDef {
        feature_index: 33,
        gate: BiomeGate::Off,
        count: CountSpec::Fixed(100),
        y: range_y(HeightSpec::Trapezoid {
            min: HeightAnchor::Absolute(-16),
            max: HeightAnchor::Absolute(480),
        }),
        kind: ore_kind(3, 0.0, BlockId::CopperOre, None, TargetKind::StoneOre),
    },
];

// ---------------------------------------------------------------------------
// Placement + OreFeature
// ---------------------------------------------------------------------------

fn place_feature(
    rng: &mut FeatureRandom,
    region: &mut RegionBuf,
    state: &WorldgenState,
    origin_min_x: i32,
    origin_min_z: i32,
    def: &FeatureDef,
) {
    let attempts = match def.count {
        CountSpec::Fixed(n) => n,
        CountSpec::Uniform { min, max } => {
            if max < min {
                0
            } else {
                min + rng.next_int(max - min + 1)
            }
        }
        CountSpec::Rarity(chance) => {
            // 26.2 RarityFilter.shouldPlace: nextFloat() < 1.0f / chance
            // (not nextInt(chance) == 0 — that desyncs the rest of the chain).
            if chance > 0 && rng.next_f32() < 1.0 / chance as f32 {
                1
            } else {
                0
            }
        }
    };

    for _ in 0..attempts {
        let lx = rng.next_int(16);
        let lz = rng.next_int(16);
        let x = origin_min_x + lx;
        let z = origin_min_z + lz;
        let y = match def.y {
            YSpec::Range {
                height,
                max_relative_to_ocean_floor_wg,
            } => {
                let y = sample_height(rng, height);
                if y < WORLD_BOTTOM || y >= WORLD_TOP {
                    continue;
                }
                if let Some(max_rel) = max_relative_to_ocean_floor_wg {
                    let Some(surf) = ocean_floor_wg_first_available(region, x, z) else {
                        continue;
                    };
                    if y > surf + max_rel {
                        continue;
                    }
                }
                y
            }
            YSpec::OceanFloorWg {
                require_water,
                require_mud,
                y_offset,
            } => {
                let Some(base) = ocean_floor_wg_first_available(region, x, z) else {
                    continue;
                };
                let y = base + y_offset;
                if y < WORLD_BOTTOM || y >= WORLD_TOP {
                    continue;
                }
                if require_water && region.get(x, y, z) != BlockId::Water {
                    continue;
                }
                if require_mud && region.get(x, y, z) != BlockId::Mud {
                    continue;
                }
                y
            }
        };
        if !biome_gate_ok(state, def.gate, def.feature_index, x, y, z) {
            continue;
        }
        match def.kind {
            FeatureKind::Ore(ore) => place_ore_blob(rng, region, x, y, z, &ore),
            FeatureKind::Disk(disk) => place_disk(rng, region, x, y, z, &disk),
            FeatureKind::UnderwaterMagma {
                floor_search_range,
                probability,
                radius,
            } => place_underwater_magma(
                rng,
                region,
                x,
                y,
                z,
                floor_search_range,
                probability,
                radius,
            ),
        }
    }
}

/// `BiomeFilter.shouldPlace` + optional id allow-list.
fn biome_gate_ok(
    state: &WorldgenState,
    gate: BiomeGate,
    feature_index: i32,
    x: i32,
    y: i32,
    z: i32,
) -> bool {
    match gate {
        BiomeGate::Off => false,
        BiomeGate::Any | BiomeGate::Ids(_) => {
            if let BiomeGate::Ids(ids) = gate {
                if !ids.contains(&biome_id_at_block(state, x, y, z)) {
                    return false;
                }
            }
            let id = biome_id_at_block(state, x, y, z);
            let bname = crate::feature_dispatch::biome_id_to_name(id);
            let Some(placed) = crate::feature_catalog::features_per_step_at(STEP_UNDERGROUND_ORES)
                .get(feature_index as usize)
            else {
                return true;
            };
            crate::feature_catalog::features_at_step(bname, STEP_UNDERGROUND_ORES)
                .iter()
                .any(|f| {
                    f.strip_prefix("minecraft:").unwrap_or(f.as_str()) == placed.as_str()
                })
        }
    }
}

/// OCEAN_FLOOR_WG `Chunk.getHeight` = first available = stored solid Y + 1.
/// Heightmaps in `RegionBuf` are post-surface / pre-carver (WG usage).
fn ocean_floor_wg_first_available(region: &RegionBuf, x: i32, z: i32) -> Option<i32> {
    let lx = x - region.origin_x;
    let lz = z - region.origin_z;
    if lx < 0 || lz < 0 || lx >= region.side || lz >= region.side {
        return None;
    }
    let cxl = lx / 16;
    let czl = lz / 16;
    let hx = (lx % 16) as usize;
    let hz = (lz % 16) as usize;
    let hi = (czl * region.chunks + cxl) as usize;
    let hm = region.heightmaps.get(hi)?;
    let solid_y = hm[hz * 16 + hx] as i32;
    if solid_y <= WORLD_BOTTOM {
        return None;
    }
    Some(solid_y + 1)
}

/// `DiskFeature.place` + `placeColumn`. Radius via UniformInt.
fn place_disk(
    rng: &mut FeatureRandom,
    region: &mut RegionBuf,
    ox: i32,
    oy: i32,
    oz: i32,
    def: &DiskDef,
) {
    let span = def.radius_max - def.radius_min + 1;
    let r = if span <= 0 {
        def.radius_min
    } else {
        def.radius_min + rng.next_int(span)
    };
    let top = oy + def.half_height;
    let bottom = oy - def.half_height - 1;
    for z in (oz - r)..=(oz + r) {
        for x in (ox - r)..=(ox + r) {
            let xd = x - ox;
            let zd = z - oz;
            if xd * xd + zd * zd > r * r {
                continue;
            }
            place_disk_column(region, def, top, bottom, x, z);
        }
    }
}

/// Generic config-driven disk placement (`DiskFeature` at any step, e.g.
/// `ice_patch` at step 4). Reads `half_height`, `radius` (uniform), the
/// `state_provider` and the `target` block predicate from the configured-feature
/// JSON. RNG: one `radius` sample, then per-column state draws (simple
/// providers consume none) — matches `DiskFeature.place` + `placeColumn`.
pub(crate) fn place_disk_from_config(
    rng: &mut FeatureRandom,
    region: &mut RegionBuf,
    ox: i32,
    oy: i32,
    oz: i32,
    cfg: &serde_json::Value,
) {
    let c = &cfg["config"];
    let half_height = c["half_height"].as_i64().unwrap_or(1) as i32;
    let radius = c["radius"].as_i64().map(|n| (n as i32, n as i32)).unwrap_or_else(|| {
        let min = c["radius"]["min_inclusive"].as_i64().unwrap_or(2) as i32;
        let max = c["radius"]["max_inclusive"].as_i64().unwrap_or(3) as i32;
        (min, max)
    });
    let r = if radius.1 <= radius.0 {
        radius.0
    } else {
        radius.0 + rng.next_int(radius.1 - radius.0 + 1)
    };
    let Some(state) =
        crate::feature_dispatch::block_from_to_place(rng, &c["state_provider"])
    else {
        return;
    };
    let top = oy + half_height;
    let bottom = oy - half_height - 1;
    for z in (oz - r)..=(oz + r) {
        for x in (ox - r)..=(ox + r) {
            let xd = x - ox;
            let zd = z - oz;
            if xd * xd + zd * zd > r * r {
                continue;
            }
            for y in (bottom + 1..=top).rev() {
                if region.index(x, y, z).is_none() {
                    continue;
                }
                if !crate::feature_dispatch::eval_block_predicate(region, x, y, z, &c["target"]) {
                    continue;
                }
                region.set(x, y, z, state);
            }
        }
    }
}

fn place_disk_column(region: &mut RegionBuf, def: &DiskDef, top: i32, bottom: i32, x: i32, z: i32) {
    for y in (bottom + 1..=top).rev() {
        if y < WORLD_BOTTOM || y >= WORLD_TOP {
            continue;
        }
        if region.index(x, y, z).is_none() {
            continue;
        }
        if !disk_target_match(region.get(x, y, z), def.target) {
            continue;
        }
        let Some(state) = disk_state(region, def.provider, x, y, z) else {
            continue;
        };
        region.set(x, y, z, state);
    }
}

fn disk_target_match(existing: BlockId, target: DiskTarget) -> bool {
    match target {
        DiskTarget::DirtOrClay => matches!(existing, BlockId::Dirt | BlockId::Clay),
        DiskTarget::DirtOrGrass => matches!(existing, BlockId::Dirt | BlockId::GrassBlock),
        DiskTarget::DirtOrMud => matches!(existing, BlockId::Dirt | BlockId::Mud),
    }
}

fn disk_state(
    region: &RegionBuf,
    provider: DiskProvider,
    x: i32,
    y: i32,
    z: i32,
) -> Option<BlockId> {
    match provider {
        DiskProvider::Simple(b) => Some(b),
        DiskProvider::SandOrSandstone => {
            if region.get(x, y - 1, z) == BlockId::Air {
                Some(BlockId::Sandstone)
            } else {
                Some(BlockId::Sand)
            }
        }
        DiskProvider::GrassOrDirt => {
            let above = region.get(x, y + 1, z);
            if !is_solid_predicate(above) && above != BlockId::Water {
                Some(BlockId::GrassBlock)
            } else {
                Some(BlockId::Dirt)
            }
        }
    }
}

fn is_solid_predicate(b: BlockId) -> bool {
    !matches!(
        b,
        BlockId::Air
            | BlockId::Water
            | BlockId::Lava
            | BlockId::ShortGrass
            | BlockId::LeafLitter
            | BlockId::Snow
    )
}

/// `UnderwaterMagmaFeature.place`. Magma has no `BlockId` in surface.rs, so
/// valid positions are found and RNG is consumed, but the block is not written.
fn place_underwater_magma(
    rng: &mut FeatureRandom,
    region: &mut RegionBuf,
    ox: i32,
    oy: i32,
    oz: i32,
    floor_search_range: i32,
    probability: f32,
    radius: i32,
) {
    let Some(floor_y) = magma_floor_y(region, ox, oy, oz, floor_search_range) else {
        return;
    };
    for z in (oz - radius)..=(oz + radius) {
        for y in (floor_y - radius)..=(floor_y + radius) {
            for x in (ox - radius)..=(ox + radius) {
                if rng.next_f32() >= probability {
                    continue;
                }
                if !magma_valid_placement(region, x, y, z) {
                    continue;
                }
                // No Magma BlockId — leave the solid block in place.
            }
        }
    }
}

/// `Column.scan` with inside=water, edge=!water, then `Column.getFloor`.
fn magma_floor_y(region: &RegionBuf, x: i32, y: i32, z: i32, search_range: i32) -> Option<i32> {
    if region.get(x, y, z) != BlockId::Water {
        return None;
    }
    magma_scan_direction(region, x, y, z, search_range, -1)
}

fn magma_scan_direction(
    region: &RegionBuf,
    x: i32,
    start_y: i32,
    z: i32,
    search_range: i32,
    dy: i32,
) -> Option<i32> {
    let mut cy = start_y;
    let mut i = 1;
    while i < search_range && region.get(x, cy, z) == BlockId::Water {
        cy += dy;
        i += 1;
    }
    if region.get(x, cy, z) != BlockId::Water {
        Some(cy)
    } else {
        None
    }
}

fn magma_valid_placement(region: &RegionBuf, x: i32, y: i32, z: i32) -> bool {
    let here = region.get(x, y, z);
    if matches!(here, BlockId::Water | BlockId::Air) {
        return false;
    }
    if magma_visible_from_outside(region.get(x, y - 1, z)) {
        return false;
    }
    for (dx, dz) in [(0, -1), (0, 1), (-1, 0), (1, 0)] {
        if magma_visible_from_outside(region.get(x + dx, y, z + dz)) {
            return false;
        }
    }
    true
}

fn magma_visible_from_outside(b: BlockId) -> bool {
    matches!(
        b,
        BlockId::Air
            | BlockId::Water
            | BlockId::Lava
            | BlockId::ShortGrass
            | BlockId::LeafLitter
            | BlockId::OakLeaves
            | BlockId::DarkOakLeaves
            | BlockId::Snow
            | BlockId::SculkVein
    )
}

fn sample_height(rng: &mut FeatureRandom, h: HeightSpec) -> i32 {
    match h {
        HeightSpec::Uniform { min, max } => {
            let lo = min.resolve();
            let hi = max.resolve();
            if hi <= lo {
                return lo;
            }
            // Mth.randomBetweenInclusive(random, lo, hi)
            lo + rng.next_int(hi - lo + 1)
        }
        HeightSpec::Trapezoid { min, max } => {
            // Vanilla TrapezoidHeight with plateau=0:
            //   range = max - min
            //   bottom = range / 2; top = range - bottom
            //   return min + randomBetweenInclusive(0, top) + randomBetweenInclusive(0, bottom)
            let lo = min.resolve();
            let hi = max.resolve();
            if hi <= lo {
                return lo;
            }
            let range = hi - lo;
            let bottom = range / 2;
            let top = range - bottom;
            lo + rng.next_int(top + 1) + rng.next_int(bottom + 1)
        }
    }
}

/// `OreFeature.place` + `doPlace` into a multi-chunk region.
fn place_ore_blob(
    rng: &mut FeatureRandom,
    region: &mut RegionBuf,
    ox: i32,
    oy: i32,
    oz: i32,
    def: &OreDef,
) {
    let size = def.size;
    if size <= 0 {
        return;
    }
    let target_fn = |existing: BlockId| target_match(existing, def);
    place_ore_blob_inner(rng, region, ox, oy, oz, size, def.discard_chance, target_fn);
}

/// Generic config-driven ore placement (`OreFeature` at any step, e.g.
/// `ore_infested` at step 7). Reads `size`, `discard_chance_on_air_exposure`
/// and the ordered `targets` array (state + tag_match predicate) from the
/// configured-feature JSON, then runs the same blob algorithm as the batch.
pub(crate) fn place_ore_from_config(
    rng: &mut FeatureRandom,
    region: &mut RegionBuf,
    ox: i32,
    oy: i32,
    oz: i32,
    cfg: &serde_json::Value,
) {
    let c = &cfg["config"];
    let size = c["size"].as_i64().unwrap_or(0) as i32;
    let discard = c["discard_chance_on_air_exposure"].as_f64().unwrap_or(0.0) as f32;
    if size <= 0 {
        return;
    }
    // Ordered first-match targets (vanilla iterates targetStates in order).
    let mut targets: Vec<(BlockId, TargetKind)> = Vec::new();
    if let Some(arr) = c["targets"].as_array() {
        for t in arr {
            let state = t["state"]["Name"]
                .as_str()
                .and_then(crate::surface::BlockId::from_name);
            let Some(state) = state else { continue };
            let tag = t["target"]["tag"].as_str().unwrap_or("");
            let kind = match tag {
                "minecraft:deepslate_ore_replaceables" => TargetKind::DeepslateOre,
                "minecraft:base_stone_overworld" => TargetKind::BaseStone,
                _ => TargetKind::StoneOre,
            };
            targets.push((state, kind));
        }
    }
    if targets.is_empty() {
        return;
    }
    let target_fn = move |existing: BlockId| -> Option<BlockId> {
        for (state, kind) in &targets {
            let hits = match kind {
                TargetKind::StoneOre => is_stone_ore_replaceable(existing),
                TargetKind::BaseStone => is_base_stone(existing),
                TargetKind::DeepslateOre => is_deepslate_ore_replaceable(existing),
            };
            if hits {
                return Some(*state);
            }
        }
        None
    };
    place_ore_blob_inner(rng, region, ox, oy, oz, size, discard, target_fn);
}

/// Shared blob math: `OreFeature.place` + `doPlace` with a target resolver.
fn place_ore_blob_inner(
    rng: &mut FeatureRandom,
    region: &mut RegionBuf,
    ox: i32,
    oy: i32,
    oz: i32,
    size: i32,
    discard_chance: f32,
    target_fn: impl Fn(BlockId) -> Option<BlockId>,
) {
    if size <= 0 {
        return;
    }
    let angle = rng.next_f32() * PI;
    let f = size as f32 / 8.0;
    // Match Java: Math.sin((double)angle) after float angle
    let start_x = ox as f64 + (angle as f64).sin() * f as f64;
    let end_x = ox as f64 - (angle as f64).sin() * f as f64;
    let start_z = oz as f64 + (angle as f64).cos() * f as f64;
    let end_z = oz as f64 - (angle as f64).cos() * f as f64;
    let start_y = (oy + rng.next_int(3) - 2) as f64;
    let end_y = (oy + rng.next_int(3) - 2) as f64;

    // Bounding box is computed in `place` *before* doPlace. If no column in the
    // box has OCEAN_FLOOR_WG >= minY, vanilla returns false without consuming
    // the per-sphere nextDouble stream.
    let cell = ((size as f32 / 16.0 * 2.0 + 1.0) / 2.0).ceil() as i32;
    let f_ceil = f.ceil() as i32;
    let start_block_x = ox - f_ceil - cell;
    let start_block_y = oy - 2 - cell;
    let start_block_z = oz - f_ceil - cell;
    let size_xz = 2 * (f_ceil + cell);
    let size_y = 2 * (2 + cell);
    if !ocean_floor_wg_allows_ore(region, start_block_x, start_block_y, start_block_z, size_xz) {
        return;
    }

    // Sphere path samples (`doPlace`)
    let mut spheres = vec![0f64; (size as usize) * 4];
    for i in 0..size {
        // Vanilla: float t = (float)i / (float)size; then (double)t into lerp.
        let t = i as f32 / size as f32;
        let td = t as f64;
        let sx = lerp(td, start_x, end_x);
        let sy = lerp(td, start_y, end_y);
        let sz = lerp(td, start_z, end_z);
        let blip = rng.next_f64() * size as f64 / 16.0;
        // Java: ((Mth.sin((double)(PI * t)) + 1.0f) * blip + 1.0) / 2.0
        let sin_part = (crate::carvers::mth_sin_d((PI * t) as f64) + 1.0) as f64;
        let radius = (sin_part * blip + 1.0) / 2.0;
        let base = (i as usize) * 4;
        spheres[base] = sx;
        spheres[base + 1] = sy;
        spheres[base + 2] = sz;
        spheres[base + 3] = radius;
    }

    // Remove overlapping spheres (bytecode cull)
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
        let cx = spheres[bi];
        let cy = spheres[bi + 1];
        let cz = spheres[bi + 2];
        let min_x = floor(cx - radius).max(start_block_x);
        let min_y = floor(cy - radius).max(start_block_y);
        let min_z = floor(cz - radius).max(start_block_z);
        let max_x = floor(cx + radius).max(min_x);
        let max_y = floor(cy + radius).max(min_y);
        let max_z = floor(cz + radius).max(min_z);

        for x in min_x..=max_x {
            let dx = ((x as f64 + 0.5) - cx) / radius;
            if dx * dx >= 1.0 {
                continue;
            }
            for y in min_y..=max_y {
                if y < WORLD_BOTTOM || y >= WORLD_TOP {
                    continue;
                }
                let dy = ((y as f64 + 0.5) - cy) / radius;
                if dx * dx + dy * dy >= 1.0 {
                    continue;
                }
                for z in min_z..=max_z {
                    let dz = ((z as f64 + 0.5) - cz) / radius;
                    if dx * dx + dy * dy + dz * dz >= 1.0 {
                        continue;
                    }
                    let bit = ((x - start_block_x)
                        + (y - start_block_y) * size_xz
                        + (z - start_block_z) * size_xz * size_y)
                        as usize;
                    if bit >= bitset.len() || bitset[bit] {
                        continue;
                    }
                    bitset[bit] = true;

                    if region.index(x, y, z).is_none() {
                        continue;
                    }
                    let existing = region.get(x, y, z);
                    let Some(replacement) = target_fn(existing) else {
                        continue;
                    };
                    // OreFeature.canPlaceOre: shouldSkipAirCheck *then* isAdjacentToAir.
                    if !can_place_ore(region, x, y, z, discard_chance, rng) {
                        continue;
                    }
                    region.set(x, y, z, replacement);
                }
            }
        }
    }
}

fn target_match(existing: BlockId, def: &OreDef) -> Option<BlockId> {
    match def.target {
        TargetKind::StoneOre => {
            if is_stone_ore_replaceable(existing) {
                Some(def.stone_block)
            } else if is_deepslate_ore_replaceable(existing) {
                Some(def.deepslate_block.unwrap_or(def.stone_block))
            } else {
                None
            }
        }
        TargetKind::BaseStone => {
            if is_base_stone(existing) {
                Some(def.stone_block)
            } else {
                None
            }
        }
        TargetKind::DeepslateOre => {
            if is_deepslate_ore_replaceable(existing) {
                Some(def.deepslate_block.unwrap_or(def.stone_block))
            } else {
                None
            }
        }
    }
}

fn is_stone_ore_replaceable(b: BlockId) -> bool {
    matches!(
        b,
        BlockId::Stone | BlockId::Granite | BlockId::Diorite | BlockId::Andesite
    )
}

fn is_deepslate_ore_replaceable(b: BlockId) -> bool {
    matches!(b, BlockId::Deepslate | BlockId::Tuff)
}

fn is_base_stone(b: BlockId) -> bool {
    matches!(
        b,
        BlockId::Stone
            | BlockId::Granite
            | BlockId::Diorite
            | BlockId::Andesite
            | BlockId::Tuff
            | BlockId::Deepslate
    )
}

/// `OreFeature.place` heightmap gate: first column with
/// `minY <= getHeight(OCEAN_FLOOR_WG, x, z)` → allow `doPlace`.
fn ocean_floor_wg_allows_ore(
    region: &RegionBuf,
    start_x: i32,
    start_y: i32,
    start_z: i32,
    size_xz: i32,
) -> bool {
    for x in start_x..=start_x + size_xz {
        for z in start_z..=start_z + size_xz {
            if let Some(h) = ocean_floor_wg_first_available(region, x, z) {
                if start_y <= h {
                    return true;
                }
            }
        }
    }
    false
}

/// `OreFeature.canPlaceOre` after the RuleTest already matched.
///
/// `shouldSkipAirCheck` runs (and may consume `nextFloat`) *before*
/// `isAdjacentToAir`. Fluids are not air (`BlockState.isAir()`).
fn can_place_ore(
    region: &RegionBuf,
    x: i32,
    y: i32,
    z: i32,
    chance: f32,
    rng: &mut FeatureRandom,
) -> bool {
    if should_skip_air_check(rng, chance) {
        return true;
    }
    !is_adjacent_to_air(region, x, y, z)
}

/// `OreFeature.shouldSkipAirCheck`.
fn should_skip_air_check(rng: &mut FeatureRandom, chance: f32) -> bool {
    if chance <= 0.0 {
        return true;
    }
    if chance >= 1.0 {
        return false;
    }
    rng.next_f32() >= chance
}

/// `Feature.isAdjacentToAir` — `BlockState.isAir()` on all 6 faces.
fn is_adjacent_to_air(region: &RegionBuf, x: i32, y: i32, z: i32) -> bool {
    neighbor_is_air(region, x + 1, y, z)
        || neighbor_is_air(region, x - 1, y, z)
        || neighbor_is_air(region, x, y + 1, z)
        || neighbor_is_air(region, x, y - 1, z)
        || neighbor_is_air(region, x, y, z + 1)
        || neighbor_is_air(region, x, y, z - 1)
}

fn neighbor_is_air(region: &RegionBuf, x: i32, y: i32, z: i32) -> bool {
    if y < WORLD_BOTTOM || y >= WORLD_TOP {
        return true;
    }
    if region.index(x, y, z).is_none() {
        return false;
    }
    region.get(x, y, z).is_air()
}

#[inline]
fn lerp(t: f64, a: f64, b: f64) -> f64 {
    a + t * (b - a)
}

#[inline]
fn floor(v: f64) -> i32 {
    v.floor() as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn step6_feature_indices_are_explicit_0_to_33() {
        let idxs: Vec<i32> = OVERWORLD_ORES.iter().map(|d| d.feature_index).collect();
        assert_eq!(idxs, (0..=33).collect::<Vec<i32>>());
        assert!(matches!(OVERWORLD_ORES[24].gate, BiomeGate::Ids(_)));
        assert!(matches!(OVERWORLD_ORES[25].gate, BiomeGate::Any));
        assert!(matches!(OVERWORLD_ORES[26].gate, BiomeGate::Any));
        assert!(matches!(OVERWORLD_ORES[27].gate, BiomeGate::Ids(_)));
        assert!(matches!(OVERWORLD_ORES[28].gate, BiomeGate::Ids(_)));
        assert!(matches!(OVERWORLD_ORES[29].gate, BiomeGate::Ids(_)));
        assert!(matches!(OVERWORLD_ORES[30].gate, BiomeGate::Any));
        assert!(matches!(OVERWORLD_ORES[31].gate, BiomeGate::Any));
        assert!(matches!(OVERWORLD_ORES[32].gate, BiomeGate::Any));
        assert!(matches!(OVERWORLD_ORES[33].gate, BiomeGate::Off));
        assert!(matches!(OVERWORLD_ORES[30].kind, FeatureKind::Disk(_)));
        assert!(matches!(OVERWORLD_ORES[31].kind, FeatureKind::Disk(_)));
        assert!(matches!(OVERWORLD_ORES[32].kind, FeatureKind::Disk(_)));
        assert!(matches!(
            OVERWORLD_ORES[26].kind,
            FeatureKind::UnderwaterMagma { .. }
        ));
    }

    #[test]
    fn should_skip_air_check_matches_vanilla_consumption() {
        // chance <= 0 → true, no draw; chance >= 1 → false, no draw.
        let mut rng = FeatureRandom::new(1);
        let before = rng.next_int(16);
        let mut rng = FeatureRandom::new(1);
        assert!(should_skip_air_check(&mut rng, 0.0));
        assert!(!should_skip_air_check(&mut rng, 1.0));
        assert_eq!(rng.next_int(16), before);

        // 0 < chance < 1 consumes nextFloat; skip iff nextFloat >= chance.
        let mut a = FeatureRandom::new(99);
        let mut b = FeatureRandom::new(99);
        let skip = should_skip_air_check(&mut a, 0.5);
        let drawn = b.next_f32();
        assert_eq!(skip, drawn >= 0.5);
        assert_eq!(a.next_int(8), b.next_int(8));
    }

    #[test]
    fn rarity_filter_uses_next_float_not_next_int() {
        // ProbeRarity vs 26.2: setFeatureSeed(dec(12345,96,-32), 6, 6)
        // nextFloat = 0.42050785 >= 1/6 → do not place.
        let mut rng = FeatureRandom::new(12345);
        let dec = rng.set_decoration_seed(12345, 96, -32);
        rng.set_feature_seed(dec, 6, 6);
        let f = rng.next_f32();
        assert!((f - 0.42050785).abs() < 1e-6, "got {f}");
        assert!(f >= 1.0 / 6.0);
    }
}
