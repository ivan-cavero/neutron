//! Biome gate + vanilla block-state predicates for sculk placement.
use super::*;
use crate::feature_catalog;
use crate::feature_rng::FeatureRandom;
use crate::region_buf::RegionBuf;
use crate::surface::BlockId;
use crate::worldgen::WorldgenState;

// ===================== helpers =====================

/// Diagnostic override for the deep_dark biome gate (parity experiments feed
/// the vanilla chunk's real 3D biomes here). `None` → neutron's biome source.
pub(super) static BIOME_GATE_OVERRIDE: std::sync::RwLock<
    Option<std::sync::Arc<dyn Fn(i32, i32, i32) -> bool + Send + Sync>>,
> = std::sync::RwLock::new(None);

/// Install a diagnostic deep_dark gate override (parity experiments only).
pub fn set_biome_gate_override(
    f: Option<std::sync::Arc<dyn Fn(i32, i32, i32) -> bool + Send + Sync>>,
) {
    *BIOME_GATE_OVERRIDE.write().unwrap() = f;
}

pub(super) fn is_deep_dark_at(state: &WorldgenState, x: i32, y: i32, z: i32) -> bool {
    if let Some(f) = &*BIOME_GATE_OVERRIDE.read().unwrap() {
        return f(x, y, z);
    }
    biome_id_at_block(state, x, y, z) == crate::biome_source::biome_id::DEEP_DARK
}

/// Only SculkBlock and SculkVeinBlock implement SculkBehaviour.
/// Catalyst / sensor / shrieker do not (javap 26.2).
pub(super) fn is_sculk_behaviour(b: BlockId) -> bool {
    matches!(b, BlockId::Sculk | BlockId::SculkVein)
}

/// Full collision cube (vanilla isCollisionShapeFullBlock). SCULK is solid;
/// veins/sensors/etc. are not.
pub(super) fn is_collision_full_block(b: BlockId) -> bool {
    matches!(
        b,
        BlockId::Sculk
            | BlockId::SculkCatalyst
            | BlockId::Stone
            | BlockId::Granite
            | BlockId::Diorite
            | BlockId::Andesite
            | BlockId::Dirt
            | BlockId::CoarseDirt
            | BlockId::GrassBlock
            | BlockId::Podzol
            | BlockId::Mycelium
            | BlockId::Gravel
            | BlockId::Sand
            | BlockId::RedSand
            | BlockId::Clay
            | BlockId::Calcite
            | BlockId::Tuff
            | BlockId::Deepslate
            | BlockId::Sandstone
            | BlockId::RedSandstone
            | BlockId::Terracotta
            | BlockId::WhiteTerracotta
            | BlockId::OrangeTerracotta
            | BlockId::BrownTerracotta
            | BlockId::BlackTerracotta
            | BlockId::YellowTerracotta
            | BlockId::RedTerracotta
            | BlockId::LightGrayTerracotta
            | BlockId::Mud
            | BlockId::Sulfur
            | BlockId::Cinnabar
            | BlockId::Bedrock
            | BlockId::Cobblestone
            | BlockId::CoalOre
            | BlockId::IronOre
            | BlockId::CopperOre
            | BlockId::GoldOre
            | BlockId::RedstoneOre
            | BlockId::LapisOre
            | BlockId::DiamondOre
            | BlockId::DeepslateCoalOre
            | BlockId::DeepslateIronOre
            | BlockId::DeepslateCopperOre
            | BlockId::DeepslateGoldOre
            | BlockId::DeepslateRedstoneOre
            | BlockId::DeepslateLapisOre
            | BlockId::DeepslateDiamondOre
            | BlockId::RawIronBlock
            | BlockId::RawCopperBlock
            | BlockId::OakLog
            | BlockId::DarkOakLog
            | BlockId::MossBlock
            | BlockId::PackedIce
            | BlockId::BlueIce
            | BlockId::Ice
    )
}

/// tags/block/sculk_replaceable — NOT world_gen. Used by hasSubstrateAccess.
/// Ores are not in this tag (vanilla).
pub(super) fn is_sculk_replaceable(b: BlockId) -> bool {
    matches!(
        b,
        BlockId::Stone
            | BlockId::Granite
            | BlockId::Diorite
            | BlockId::Andesite
            | BlockId::Dirt
            | BlockId::CoarseDirt
            | BlockId::GrassBlock
            | BlockId::Podzol
            | BlockId::Mycelium
            | BlockId::MossBlock
            | BlockId::Gravel
            | BlockId::Sand
            | BlockId::RedSand
            | BlockId::Clay
            | BlockId::Calcite
            | BlockId::Tuff
            | BlockId::Deepslate
            | BlockId::Sandstone
            | BlockId::RedSandstone
            | BlockId::Terracotta
            | BlockId::WhiteTerracotta
            | BlockId::OrangeTerracotta
            | BlockId::BrownTerracotta
            | BlockId::BlackTerracotta
            | BlockId::YellowTerracotta
            | BlockId::RedTerracotta
            | BlockId::LightGrayTerracotta
            | BlockId::Mud
            | BlockId::Sulfur
            | BlockId::Cinnabar
    )
}

/// tags/block/sculk_replaceable_world_gen = sculk_replaceable + deepslate bricks/tiles.
/// Those brick variants are not in BlockId; same set as the base tag here.
/// Used by worldgen SculkSpreader.replaceableBlocks() in attemptPlaceSculk.
pub(super) fn is_sculk_replaceable_world_gen(b: BlockId) -> bool {
    is_sculk_replaceable(b)
}

pub(super) fn is_air_or_water(b: BlockId) -> bool {
    matches!(b, BlockId::Air | BlockId::CaveAir | BlockId::Water)
}

pub(super) fn is_vein_placeable_on(b: BlockId) -> bool {
    matches!(
        b,
        BlockId::Stone
            | BlockId::Andesite
            | BlockId::Diorite
            | BlockId::Granite
            | BlockId::Calcite
            | BlockId::Tuff
            | BlockId::Deepslate
    )
}

pub(super) fn dir_index(dx: i32, dy: i32, dz: i32) -> Option<usize> {
    DIRS.iter().position(|&d| d == (dx, dy, dz))
}
