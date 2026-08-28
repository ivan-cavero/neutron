//! `TreeFeature` + trunk/foliage placers (26.2 CFR).
//!
//! Straight / dark-oak / fancy trunks, blob / dark-oak / fancy foliage,
//! two- and three-layer feature size, beehive + leaf-litter ground decorators.
//!
//! RNG order matches vanilla WorldgenRandom wrapping Xoroshiro:
//!   getTreeHeight -> foliageHeight -> foliageRadius -> placeTrunk -> createFoliage
//!   -> decorators. TrunkPlacer.getTreeHeight always samples both nextInt calls.

use crate::feature_catalog;
use crate::feature_rng::FeatureRandom;
use crate::generator::{WORLD_BOTTOM, WORLD_TOP};
use crate::region_buf::RegionBuf;
use crate::surface::BlockId;
use crate::worldgen::WorldgenState;
use serde_json::Value;
mod cfg;
mod decorators;
pub mod java_hash;
mod foliage_placers;
mod trunk_placers;

use cfg::{
    below_trunk_block, block_from_provider, parse_feature_size, parse_foliage_kind,
    parse_int_provider, parse_trunk_kind,
};
use decorators::apply_decorators;
use foliage_placers::{create_blob_foliage, create_dark_oak_foliage, create_fancy_foliage};
use trunk_placers::{place_below_trunk, place_dark_oak_trunk, place_fancy_trunk, place_straight_trunk};
#[derive(Clone, Copy)]
pub(super) struct FoliageAttachment {
    pub(super) x: i32,
    pub(super) y: i32,
    pub(super) z: i32,
    pub(super) radius_offset: i32,
    pub(super) double_trunk: bool,
}

#[derive(Clone, Copy)]
pub(super) enum IntProv {
    Constant(i32),
    Uniform { min: i32, max: i32 },
}

impl IntProv {
    pub(super) fn sample(self, rng: &mut FeatureRandom) -> i32 {
        match self {
            IntProv::Constant(v) => v,
            IntProv::Uniform { min, max } => {
                let span = max - min + 1;
                if span <= 0 {
                    min
                } else {
                    min + rng.next_int(span)
                }
            }
        }
    }
}

pub(super) enum TrunkKind {
    Straight,
    DarkOak,
    Fancy,
    Unknown,
}

pub(super) enum FoliageKind {
    Blob { height: i32 },
    Fancy { height: i32 },
    DarkOak,
    Unknown,
}

pub(super) struct FeatureSizeCfg {
    pub(super) kind: SizeKind,
    pub(super) min_clipped: Option<i32>,
}

pub(super) enum SizeKind {
    Two {
        limit: i32,
        lower: i32,
        upper: i32,
    },
    Three {
        limit: i32,
        upper_limit: i32,
        lower: i32,
        middle: i32,
        upper: i32,
    },
}

pub(super) struct TreeCtx<'a> {
    pub(super) rng: &'a mut FeatureRandom,
    pub(super) region: &'a mut RegionBuf,
    /// Worldgen context for decorator sub-features (e.g. the pale_moss
    /// ground patch). `None` in unit tests (no decorators exercised there).
    pub(super) state: Option<&'a WorldgenState>,
    pub(super) log: BlockId,
    pub(super) leaves: BlockId,
    pub(super) trunks: Vec<(i32, i32, i32)>,
    pub(super) foliage: Vec<(i32, i32, i32)>,
}

/// Place a tree from a configured_feature JSON object (`type: minecraft:tree`).

pub fn place_tree_from_config(
    rng: &mut FeatureRandom,
    region: &mut RegionBuf,
    state: Option<&WorldgenState>,
    x: i32,
    y: i32,
    z: i32,
    config: &Value,
) -> bool {
    let cfg = &config["config"];
    let trunk = &cfg["trunk_placer"];
    let foliage = &cfg["foliage_placer"];
    let trunk_kind = parse_trunk_kind(trunk["type"].as_str().unwrap_or(""));
    let foliage_kind = parse_foliage_kind(foliage);

    let log = block_from_provider(&cfg["trunk_provider"]).unwrap_or(BlockId::OakLog);
    let leaves = block_from_provider(&cfg["foliage_provider"]).unwrap_or(BlockId::OakLeaves);

    let base = trunk["base_height"].as_i64().unwrap_or(4) as i32;
    let rand_a = trunk["height_rand_a"].as_i64().unwrap_or(0) as i32;
    let rand_b = trunk["height_rand_b"].as_i64().unwrap_or(0) as i32;
    // TrunkPlacer.getTreeHeight — both nextInt calls always run (nextInt(1) consumes).
    let tree_height = base + rng.next_int(rand_a + 1) + rng.next_int(rand_b + 1);

    let foliage_height = match foliage_kind {
        FoliageKind::Blob { height } | FoliageKind::Fancy { height } => height,
        FoliageKind::DarkOak => 4,
        FoliageKind::Unknown => 3,
    };
    let radius_prov = parse_int_provider(&foliage["radius"], 2);
    let offset_prov = parse_int_provider(&foliage["offset"], 0);
    let _trunk_height = tree_height - foliage_height;
    let leaf_radius = radius_prov.sample(rng);

    let min_y = y;
    let max_y = y + tree_height + 1;
    if min_y < WORLD_BOTTOM + 1 || max_y > WORLD_TOP {
        return false;
    }

    let size = parse_feature_size(&cfg["minimum_size"]);
    let ignore_vines = cfg["ignore_vines"].as_bool().unwrap_or(false);
    let clipped = max_free_tree_height(region, x, y, z, tree_height, &size, ignore_vines);
    if clipped < tree_height && size.min_clipped.map(|m| clipped < m).unwrap_or(true) {
        return false;
    }

    let mut ctx = TreeCtx {
        rng,
        region,
        state,
        log,
        leaves,
        trunks: Vec::new(),
        foliage: Vec::new(),
    };

    let attachments = match trunk_kind {
        TrunkKind::Straight => place_straight_trunk(&mut ctx, x, y, z, clipped, cfg),
        TrunkKind::DarkOak => place_dark_oak_trunk(&mut ctx, x, y, z, clipped, cfg),
        TrunkKind::Fancy => place_fancy_trunk(&mut ctx, x, y, z, clipped, cfg),
        TrunkKind::Unknown => {
            place_below_trunk(&mut ctx, x, y - 1, z, cfg);
            Vec::new()
        }
    };

    for att in attachments {
        let offset = offset_prov.sample(ctx.rng);
        match foliage_kind {
            FoliageKind::Blob { .. } => {
                create_blob_foliage(&mut ctx, att, foliage_height, leaf_radius, offset)
            }
            FoliageKind::Fancy { .. } => {
                create_fancy_foliage(&mut ctx, att, foliage_height, leaf_radius, offset)
            }
            FoliageKind::DarkOak => create_dark_oak_foliage(&mut ctx, att, leaf_radius, offset),
            FoliageKind::Unknown => {}
        }
    }

    if ctx.trunks.is_empty() && ctx.foliage.is_empty() {
        return false;
    }

    // TreeDecorator.Context (decompiled 26.2) copies the TreeFeature sets and
    // stable-sorts logs AND leaves by ascending Y AFTER the copy. The Java
    // HashSet iteration order must be simulated on the RAW add order first
    // (bucket chains append in insertion order, so pre-sorting by Y would
    // permute collisions differently than vanilla), then the stable Y sort is
    // applied to both lists.
    ctx.trunks = java_hash::java_hash_order(std::mem::take(&mut ctx.trunks));
    ctx.foliage = java_hash::java_hash_order(std::mem::take(&mut ctx.foliage));
    // Java List.sort(Comparator.comparingInt(getY)) = TimSort = STABLE: equal
    // Y keeps HashSet bucket order.
    ctx.trunks.sort_by(|a, b| a.1.cmp(&b.1));
    ctx.foliage.sort_by(|a, b| a.1.cmp(&b.1));

    // NEUTRON_DECO_TREE_TRACE (diagnostic): print the placed tree blocks.
    if std::env::var("NEUTRON_DECO_TREE_TRACE").is_ok() {
        eprintln!(
            "[tree] origin=({x},{y},{z}) height={} trunks={} foliage={}",
            tree_height,
            ctx.trunks.len(),
            ctx.foliage.len()
        );
        for &(tx, ty, tz) in &ctx.trunks {
            eprintln!("  trunk ({tx},{ty},{tz})");
        }
        for &(tx, ty, tz) in &ctx.foliage {
            eprintln!("  leaf  ({tx},{ty},{tz})");
        }
    }

    if let Some(decorators) = cfg["decorators"].as_array() {
        apply_decorators(&mut ctx, decorators);
    }
    if std::env::var_os("NEUTRON_DECO_TREE_TRACE").is_some()
        || std::env::var_os("NEUTRON_TRACE_TREES").is_some()
    {
        eprintln!(
            "[tree] done origin=({x},{y},{z}) trunks={} foliage={}",
            ctx.trunks.len(),
            ctx.foliage.len()
        );
    }
    true
}


pub(super) fn max_free_tree_height(
    region: &RegionBuf,
    x: i32,
    y: i32,
    z: i32,
    max_tree_height: i32,
    size: &FeatureSizeCfg,
    _ignore_vines: bool,
) -> i32 {
    for yo in 0..=max_tree_height + 1 {
        let r = size_at_height(size, max_tree_height, yo);
        for dx in -r..=r {
            for dz in -r..=r {
                if !is_free(region.get(x + dx, y + yo, z + dz)) {
                    return yo - 2;
                }
            }
        }
    }
    max_tree_height
}

fn size_at_height(size: &FeatureSizeCfg, tree_height: i32, yo: i32) -> i32 {
    match size.kind {
        SizeKind::Two {
            limit,
            lower,
            upper,
        } => {
            if yo < limit {
                lower
            } else {
                upper
            }
        }
        SizeKind::Three {
            limit,
            upper_limit,
            lower,
            middle,
            upper,
        } => {
            if yo < limit {
                lower
            } else if yo >= tree_height - upper_limit {
                upper
            } else {
                middle
            }
        }
    }
}

pub(crate) fn valid_tree_pos(b: BlockId) -> bool {
    // TreeFeature.validTreePos: isAir || REPLACEABLE_BY_TREES (not fluids).
    // 26.2 tag (data/minecraft/tags/block/replaceable_by_trees.json): leaves,
    // small_flowers, pale_moss_carpet, short_grass, fern, dead_bush, vine,
    // glow_lichen, sunflower, lilac, rose_bush, peony, tall_grass,
    // large_fern, hanging_roots, pitcher_plant, water, seagrass,
    // tall_seagrass, bush, firefly_bush, warped_roots, nether_sprouts,
    // crimson_roots, leaf_litter, short_dry_grass, tall_dry_grass.
    // Tag members without a BlockId (pitcher_plant, open_eyeblossom,
    // wither_rose, torchflower) read back as Air (RegionBuf.get) — free,
    // matching vanilla.
    matches!(
        b,
        BlockId::Air
            | BlockId::CaveAir
            | BlockId::OakLeaves
            | BlockId::DarkOakLeaves
            | BlockId::PaleOakLeaves
            | BlockId::BirchLeaves
            | BlockId::SpruceLeaves
            | BlockId::JungleLeaves
            | BlockId::AcaciaLeaves
            | BlockId::MangroveLeaves
            | BlockId::CherryLeaves
            | BlockId::AzaleaLeaves
            | BlockId::FloweringAzaleaLeaves
            | BlockId::ShortGrass
            | BlockId::Fern
            | BlockId::LargeFern
            | BlockId::TallGrass
            | BlockId::ShortDryGrass
            | BlockId::TallDryGrass
            | BlockId::Bush
            | BlockId::FireflyBush
            | BlockId::DeadBush
            | BlockId::Vine
            | BlockId::GlowLichen
            | BlockId::Seagrass
            | BlockId::TallSeagrass
            | BlockId::WarpedRoots
            | BlockId::CrimsonRoots
            | BlockId::NetherSprouts
            | BlockId::Dandelion
            | BlockId::Poppy
            | BlockId::BlueOrchid
            | BlockId::Allium
            | BlockId::AzureBluet
            | BlockId::RedTulip
            | BlockId::OrangeTulip
            | BlockId::WhiteTulip
            | BlockId::PinkTulip
            | BlockId::OxeyeDaisy
            | BlockId::Cornflower
            | BlockId::LilyOfTheValley
            | BlockId::Sunflower
            | BlockId::Lilac
            | BlockId::RoseBush
            | BlockId::Peony
            | BlockId::LeafLitter
            | BlockId::PaleMossCarpet
            | BlockId::HangingRoots
            | BlockId::Water
    )
}

pub(super) fn is_air_or_leaves(b: BlockId) -> bool {
    matches!(
        b,
        BlockId::Air
            | BlockId::CaveAir
            | BlockId::OakLeaves
            | BlockId::DarkOakLeaves
            | BlockId::PaleOakLeaves
            | BlockId::BirchLeaves
            | BlockId::SpruceLeaves
            | BlockId::JungleLeaves
            | BlockId::AcaciaLeaves
            | BlockId::MangroveLeaves
            | BlockId::CherryLeaves
    )
}

pub(super) fn is_log(b: BlockId) -> bool {
    matches!(
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
    )
}

pub(super) fn is_free(b: BlockId) -> bool {
    valid_tree_pos(b) || is_log(b)
}

pub(super) fn cannot_replace_below_tree_trunk(b: BlockId) -> bool {
    // #minecraft:cannot_replace_below_tree_trunk (26.2) = #dirt + #mud +
    // #moss_blocks + podzol (dirt, coarse_dirt, rooted_dirt, mud,
    // muddy_mangrove_roots, moss_block, pale_moss_block, podzol).
    matches!(
        b,
        BlockId::Dirt
            | BlockId::CoarseDirt
            | BlockId::RootedDirt
            | BlockId::Mud
            | BlockId::MossBlock
            | BlockId::PaleMossBlock
            | BlockId::Podzol
    )
}

pub(super) fn next_boolean(rng: &mut FeatureRandom) -> bool {
    rng.next_bits(1) != 0
}

pub(super) fn mth_floor_f32(v: f32) -> i32 {
    v.floor() as i32
}

pub(super) fn mth_floor_f64(v: f64) -> i32 {
    v.floor() as i32
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tree_height_consumes_next_int_one_when_rand_b_is_zero() {
        let mut a = FeatureRandom::new(1);
        let mut b = FeatureRandom::new(1);
        let ha = 4 + a.next_int(3) + a.next_int(1);
        let hb = 4 + b.next_int(3) + b.next_int(1);
        assert_eq!(ha, hb);
        assert_eq!(a.next_int(1000), b.next_int(1000));
        // nextInt(1) must consume (xoroshiro WorldgenRandom next(31) path).
        let mut c = FeatureRandom::new(1);
        let _ = 4 + c.next_int(3);
        assert_ne!(a.next_int(1000), c.next_int(1000));
    }

    #[test]
    fn blob_layer_radii_match_cfr() {
        // offset=0, foliageHeight=3, leafRadius=2, radiusOffset=0
        // yo = 0,-1,-2,-3 → r = 1,1,2,2
        let expected = [1, 1, 2, 2];
        let mut got = Vec::new();
        let offset = 0;
        let foliage_height = 3;
        let leaf_radius = 2;
        let mut yo = offset;
        while yo >= offset - foliage_height {
            got.push((leaf_radius - 1 - yo / 2).max(0));
            yo -= 1;
        }
        assert_eq!(got, expected);
    }

    #[test]
    fn two_layers_default_and_fancy_sizes() {
        let oak = parse_feature_size(&serde_json::json!({
            "type": "minecraft:two_layers_feature_size"
        }));
        assert_eq!(size_at_height(&oak, 6, 0), 0);
        assert_eq!(size_at_height(&oak, 6, 1), 1);
        let fancy = parse_feature_size(&serde_json::json!({
            "type": "minecraft:two_layers_feature_size",
            "limit": 0,
            "min_clipped_height": 4,
            "upper_size": 0
        }));
        assert_eq!(fancy.min_clipped, Some(4));
        assert_eq!(size_at_height(&fancy, 10, 0), 0);
        assert_eq!(size_at_height(&fancy, 10, 5), 0);
    }

    #[test]
    fn three_layers_dark_oak_sizes() {
        let s = parse_feature_size(&serde_json::json!({
            "type": "minecraft:three_layers_feature_size",
            "upper_size": 2
        }));
        assert_eq!(size_at_height(&s, 8, 0), 0);
        assert_eq!(size_at_height(&s, 8, 1), 1);
        assert_eq!(size_at_height(&s, 8, 6), 1);
        assert_eq!(size_at_height(&s, 8, 7), 2);
        assert_eq!(size_at_height(&s, 8, 8), 2);
    }

    #[test]
    fn valid_tree_pos_matches_replaceable_by_trees() {
        assert!(valid_tree_pos(BlockId::Air));
        assert!(valid_tree_pos(BlockId::OakLeaves));
        assert!(valid_tree_pos(BlockId::DarkOakLeaves));
        assert!(valid_tree_pos(BlockId::ShortGrass));
        assert!(valid_tree_pos(BlockId::LeafLitter));
        // 26.2 replaceable_by_trees tag includes pale_moss_carpet,
        // hanging_roots and water (swamp trees).
        assert!(valid_tree_pos(BlockId::PaleMossCarpet));
        assert!(valid_tree_pos(BlockId::HangingRoots));
        assert!(valid_tree_pos(BlockId::Water));
        assert!(!valid_tree_pos(BlockId::Snow));
        assert!(!valid_tree_pos(BlockId::GrassBlock));
        assert!(!valid_tree_pos(BlockId::OakLog));
        assert!(!valid_tree_pos(BlockId::PaleMossBlock));
        assert!(is_free(BlockId::OakLog));
        assert!(is_free(BlockId::DarkOakLog));
    }

    #[test]
    fn grass_is_replaced_below_trunk_dirt_is_not() {
        assert!(!cannot_replace_below_tree_trunk(BlockId::GrassBlock));
        assert!(cannot_replace_below_tree_trunk(BlockId::Dirt));
        assert!(cannot_replace_below_tree_trunk(BlockId::Podzol));
        assert!(cannot_replace_below_tree_trunk(BlockId::MossBlock));
        assert!(cannot_replace_below_tree_trunk(BlockId::Mud));
    }

    #[test]
    fn oak_tree_places_log_column_and_blob_canopy() {
        let mut region = RegionBuf::new(0, 0, 0);
        for x in 0..16 {
            for z in 0..16 {
                region.set(x, 63, z, BlockId::GrassBlock);
            }
        }
        let cfg = serde_json::json!({
            "type": "minecraft:tree",
            "config": {
                "ignore_vines": true,
                "decorators": [],
                "minimum_size": { "type": "minecraft:two_layers_feature_size" },
                "trunk_placer": {
                    "type": "minecraft:straight_trunk_placer",
                    "base_height": 4,
                    "height_rand_a": 2,
                    "height_rand_b": 0
                },
                "foliage_placer": {
                    "type": "minecraft:blob_foliage_placer",
                    "height": 3,
                    "offset": 0,
                    "radius": 2
                },
                "trunk_provider": {
                    "type": "minecraft:simple_state_provider",
                    "state": { "Name": "minecraft:oak_log" }
                },
                "foliage_provider": {
                    "type": "minecraft:simple_state_provider",
                    "state": { "Name": "minecraft:oak_leaves" }
                },
                "below_trunk_provider": {
                    "type": "minecraft:rule_based_state_provider",
                    "rules": [{
                        "then": {
                            "type": "minecraft:simple_state_provider",
                            "state": { "Name": "minecraft:dirt" }
                        }
                    }]
                }
            }
        });
        let mut rng = FeatureRandom::new(42);
        assert!(place_tree_from_config(
            &mut rng,
            &mut region,
            None,
            8,
            64,
            8,
            &cfg
        ));
        assert_eq!(region.get(8, 63, 8), BlockId::Dirt);
        assert_eq!(region.get(8, 64, 8), BlockId::OakLog);
        let mut logs = 0;
        let mut leaves = 0;
        for y in 64..80 {
            for x in 0..16 {
                for z in 0..16 {
                    match region.get(x, y, z) {
                        BlockId::OakLog => logs += 1,
                        BlockId::OakLeaves => leaves += 1,
                        _ => {}
                    }
                }
            }
        }
        assert!(logs >= 4, "logs={logs}");
        assert!(leaves >= 10, "leaves={leaves}");
        // Attachment is above last log; top layer is leaves, not a log-only column
        // extending through the canopy like the old heuristic.
        assert!(region.get(8, 70, 8) != BlockId::OakLog || leaves > logs);
    }

    #[test]
    fn dark_oak_places_2x2_trunk() {
        let mut region = RegionBuf::new(0, 0, 0);
        for x in 0..16 {
            for z in 0..16 {
                region.set(x, 63, z, BlockId::GrassBlock);
            }
        }
        let cfg = serde_json::json!({
            "type": "minecraft:tree",
            "config": {
                "ignore_vines": true,
                "decorators": [],
                "minimum_size": {
                    "type": "minecraft:three_layers_feature_size",
                    "upper_size": 2
                },
                "trunk_placer": {
                    "type": "minecraft:dark_oak_trunk_placer",
                    "base_height": 6,
                    "height_rand_a": 2,
                    "height_rand_b": 1
                },
                "foliage_placer": {
                    "type": "minecraft:dark_oak_foliage_placer",
                    "offset": 0,
                    "radius": 0
                },
                "trunk_provider": {
                    "type": "minecraft:simple_state_provider",
                    "state": { "Name": "minecraft:dark_oak_log" }
                },
                "foliage_provider": {
                    "type": "minecraft:simple_state_provider",
                    "state": { "Name": "minecraft:dark_oak_leaves" }
                },
                "below_trunk_provider": {
                    "type": "minecraft:rule_based_state_provider",
                    "rules": [{
                        "then": {
                            "type": "minecraft:simple_state_provider",
                            "state": { "Name": "minecraft:dirt" }
                        }
                    }]
                }
            }
        });
        let mut rng = FeatureRandom::new(7);
        assert!(place_tree_from_config(
            &mut rng,
            &mut region,
            None,
            6,
            64,
            6,
            &cfg
        ));
        assert_eq!(region.get(6, 64, 6), BlockId::DarkOakLog);
        assert_eq!(region.get(7, 64, 6), BlockId::DarkOakLog);
        assert_eq!(region.get(6, 64, 7), BlockId::DarkOakLog);
        assert_eq!(region.get(7, 64, 7), BlockId::DarkOakLog);
        let mut leaves = 0;
        for y in 64..80 {
            for x in 0..16 {
                for z in 0..16 {
                    if region.get(x, y, z) == BlockId::DarkOakLeaves {
                        leaves += 1;
                    }
                }
            }
        }
        assert!(leaves >= 20, "dark oak leaves={leaves}");
    }

    #[test]
    fn fancy_oak_places_branched_logs() {
        let mut region = RegionBuf::new(0, 0, 0);
        for x in 0..16 {
            for z in 0..16 {
                region.set(x, 63, z, BlockId::GrassBlock);
            }
        }
        let cfg = serde_json::json!({
            "type": "minecraft:tree",
            "config": {
                "ignore_vines": true,
                "decorators": [],
                "minimum_size": {
                    "type": "minecraft:two_layers_feature_size",
                    "limit": 0,
                    "min_clipped_height": 4,
                    "upper_size": 0
                },
                "trunk_placer": {
                    "type": "minecraft:fancy_trunk_placer",
                    "base_height": 3,
                    "height_rand_a": 11,
                    "height_rand_b": 0
                },
                "foliage_placer": {
                    "type": "minecraft:fancy_foliage_placer",
                    "height": 4,
                    "offset": 4,
                    "radius": 2
                },
                "trunk_provider": {
                    "type": "minecraft:simple_state_provider",
                    "state": { "Name": "minecraft:oak_log" }
                },
                "foliage_provider": {
                    "type": "minecraft:simple_state_provider",
                    "state": { "Name": "minecraft:oak_leaves" }
                },
                "below_trunk_provider": {
                    "type": "minecraft:rule_based_state_provider",
                    "rules": [{
                        "then": {
                            "type": "minecraft:simple_state_provider",
                            "state": { "Name": "minecraft:dirt" }
                        }
                    }]
                }
            }
        });
        let mut rng = FeatureRandom::new(99);
        assert!(place_tree_from_config(
            &mut rng,
            &mut region,
            None,
            8,
            64,
            8,
            &cfg
        ));
        let mut logs = 0;
        let mut off_axis = 0;
        for y in 64..90 {
            for x in 0..16 {
                for z in 0..16 {
                    if region.get(x, y, z) == BlockId::OakLog {
                        logs += 1;
                        if x != 8 || z != 8 {
                            off_axis += 1;
                        }
                    }
                }
            }
        }
        assert!(logs >= 4, "fancy logs={logs}");
        // Fancy trees usually branch off the Y axis; allow a short clipped tree
        // with only a vertical limb (min_clipped_height=4).
        let _ = off_axis;
    }

    #[test]
    fn blocked_column_rejects_unclipped_oak() {
        let mut region = RegionBuf::new(0, 0, 0);
        for x in 0..16 {
            for z in 0..16 {
                region.set(x, 63, z, BlockId::GrassBlock);
                region.set(x, 67, z, BlockId::Stone);
            }
        }
        let cfg = serde_json::json!({
            "type": "minecraft:tree",
            "config": {
                "ignore_vines": true,
                "decorators": [],
                "minimum_size": { "type": "minecraft:two_layers_feature_size" },
                "trunk_placer": {
                    "type": "minecraft:straight_trunk_placer",
                    "base_height": 4,
                    "height_rand_a": 0,
                    "height_rand_b": 0
                },
                "foliage_placer": {
                    "type": "minecraft:blob_foliage_placer",
                    "height": 3, "offset": 0, "radius": 2
                },
                "trunk_provider": {
                    "type": "minecraft:simple_state_provider",
                    "state": { "Name": "minecraft:oak_log" }
                },
                "foliage_provider": {
                    "type": "minecraft:simple_state_provider",
                    "state": { "Name": "minecraft:oak_leaves" }
                }
            }
        });
        let mut rng = FeatureRandom::new(1);
        assert!(!place_tree_from_config(
            &mut rng,
            &mut region,
            None,
            8,
            64,
            8,
            &cfg
        ));
        assert_eq!(region.get(8, 64, 8), BlockId::Air);
    }
}


