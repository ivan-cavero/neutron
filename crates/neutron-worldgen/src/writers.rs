//! Writer attribution registry: WHO wrote a block cell.
//!
//! When `NEUTRON_WRITERS=1` is set at process start, every [`RegionBuf`]
//! carries a parallel plane stamping the last writer id per cell. The parity
//! ledger aggregates mismatches by writer so the report says which vanilla
//! feature family owns each gap — no more archaeology.
//!
//! One table, three columns:
//! - stable numeric id (gaps between ids are deliberate: insert without renumbering)
//! - short name for reports
//! - vanilla 26.2 class path RELATIVE to the decompile root; the java-map
//!   tripwire asserts every path exists, so a Mojang rename fails loudly
//!   instead of rotting silently.
//!
//! Copyright (c) 2026 Neutron Contributors -- MIT License

/// Cells only ever touched by terrain generation (doFill / surface rules).
pub const TERRAIN: u16 = 0;
/// Mask/restore machinery around undecorated-origin sculk gating. Ledger
/// consumers usually filter this out.
pub const MASK: u16 = 1;
/// Driver-stamped stage ids (aliases over the WRITERS table).
pub const SCULK_PATCH: u16 = 60;
pub const CARVER: u16 = 70;
pub const MINESHAFT: u16 = 71;
pub const ORE: u16 = 30;
pub const DISK: u16 = 31;
pub const UNDERWATER_MAGMA: u16 = 45;

/// (id, short name, java path relative to decompile src root).
/// TERRAIN/MASK have empty java paths (internal mechanics).
pub const WRITERS: &[(u16, &str, &str)] = &[
    (TERRAIN, "terrain", "net/minecraft/world/level/levelgen/NoiseBasedChunkGenerator.java"),
    (MASK, "mask", ""),
    // dispatched configured features (stamped in dispatch_configured)
    (10, "tree", "net/minecraft/world/level/levelgen/feature/TreeFeature.java"),
    (
        12,
        "huge_mushroom",
        "net/minecraft/world/level/levelgen/feature/AbstractHugeMushroomFeature.java",
    ),
    (
        13,
        "simple_block",
        "net/minecraft/world/level/levelgen/feature/SimpleBlockFeature.java",
    ),
    (
        14,
        "vegetation_patch",
        "net/minecraft/world/level/levelgen/feature/VegetationPatchFeature.java",
    ),
    (
        15,
        "root_system",
        "net/minecraft/world/level/levelgen/feature/RootSystemFeature.java",
    ),
    (
        16,
        "multiface_growth",
        "net/minecraft/world/level/levelgen/feature/MultifaceGrowthFeature.java",
    ),
    (17, "vines", "net/minecraft/world/level/levelgen/feature/VinesFeature.java"),
    (
        18,
        "sea_pickle",
        "net/minecraft/world/level/levelgen/feature/SeaPickleFeature.java",
    ),
    (19, "seagrass", "net/minecraft/world/level/levelgen/feature/SeagrassFeature.java"),
    (20, "kelp", "net/minecraft/world/level/levelgen/feature/KelpFeature.java"),
    (
        21,
        "block_blob",
        "net/minecraft/world/level/levelgen/feature/BlockBlobFeature.java",
    ),
    (
        22,
        "blue_ice",
        "net/minecraft/world/level/levelgen/feature/BlueIceFeature.java",
    ),
    (30, "ore", "net/minecraft/world/level/levelgen/feature/OreFeature.java"),
    (31, "disk", "net/minecraft/world/level/levelgen/feature/DiskFeature.java"),
    (
        45,
        "underwater_magma",
        "net/minecraft/world/level/levelgen/feature/UnderwaterMagmaFeature.java",
    ),
    (
        32,
        "desert_well",
        "net/minecraft/world/level/levelgen/feature/DesertWellFeature.java",
    ),
    (
        33,
        "freeze_top_layer",
        "net/minecraft/world/level/levelgen/feature/SnowAndFreezeFeature.java",
    ),
    (34, "spike", "net/minecraft/world/level/levelgen/feature/SpikeFeature.java"),
    (35, "bamboo", "net/minecraft/world/level/levelgen/feature/BambooFeature.java"),
    (
        36,
        "monster_room",
        "net/minecraft/world/level/levelgen/feature/MonsterRoomFeature.java",
    ),
    (37, "lake", "net/minecraft/world/level/levelgen/feature/LakeFeature.java"),
    (
        38,
        "speleothem_cluster",
        "net/minecraft/world/level/levelgen/feature/SpeleothemClusterFeature.java",
    ),
    (
        39,
        "large_dripstone",
        "net/minecraft/world/level/levelgen/feature/LargeDripstoneFeature.java",
    ),
    (40, "iceberg", "net/minecraft/world/level/levelgen/feature/IcebergFeature.java"),
    (41, "fossil", "net/minecraft/world/level/levelgen/feature/FossilFeature.java"),
    (42, "geode", "net/minecraft/world/level/levelgen/feature/GeodeFeature.java"),
    (
        43,
        "spring",
        "net/minecraft/world/level/levelgen/feature/SpringFeature.java",
    ),
    (
        44,
        "block_column",
        "net/minecraft/world/level/levelgen/feature/BlockColumnFeature.java",
    ),
    // driver-stamped stages (outside dispatch_configured)
    (
        60,
        "sculk_patch",
        "net/minecraft/world/level/levelgen/feature/SculkPatchFeature.java",
    ),
    (
        70,
        "carver",
        "net/minecraft/world/level/levelgen/carver/CaveWorldCarver.java",
    ),
    (
        71,
        "mineshaft",
        "net/minecraft/world/level/levelgen/structure/structures/MineshaftPieces.java",
    ),
];

/// Map a configured_feature `type` to its writer id. Unknown types fall back
/// to TERRAIN (0) — extend the table when a new feature type is ported; the
/// dispatch-coverage test will surface new types anyway.
pub fn for_configured_type(ty: &str) -> u16 {
    match ty {
        "minecraft:tree" => 10,
        "minecraft:huge_red_mushroom" | "minecraft:huge_brown_mushroom" => 12,
        "minecraft:simple_block" | "minecraft:simple_random_selector" => 13,
        "minecraft:vegetation_patch" | "minecraft:waterlogged_vegetation_patch" => 14,
        "minecraft:root_system" => 15,
        "minecraft:multiface_growth" => 16,
        "minecraft:vines" => 17,
        "minecraft:sea_pickle" => 18,
        "minecraft:seagrass" => 19,
        "minecraft:kelp" => 20,
        "minecraft:block_blob" => 21,
        "minecraft:blue_ice" => 22,
        "minecraft:ore" | "minecraft:scattered_ore" => 30,
        "minecraft:disk" => 31,
        "minecraft:desert_well" => 32,
        "minecraft:freeze_top_layer" => 33,
        "minecraft:spike" => 34,
        "minecraft:bamboo" => 35,
        "minecraft:monster_room" => 36,
        "minecraft:lake" => 37,
        "minecraft:speleothem_cluster" => 38,
        "minecraft:large_dripstone" => 39,
        "minecraft:iceberg" => 40,
        "minecraft:fossil" => 41,
        "minecraft:geode" => 42,
        "minecraft:spring_feature" => 43,
        "minecraft:block_column" => 44,
        _ => TERRAIN,
    }
}

/// Report name for a writer id (`"writer#17"` for unknown ids — never panic).
pub fn name(id: u16) -> &'static str {
    WRITERS
        .iter()
        .find(|(i, _, _)| *i == id)
        .map(|(_, n, _)| *n)
        .unwrap_or("unknown")
}

/// Java path for a writer id (empty string = internal mechanic).
pub fn java_path(id: u16) -> &'static str {
    WRITERS
        .iter()
        .find(|(i, _, _)| *i == id)
        .map(|(_, _, p)| *p)
        .unwrap_or("")
}

/// Java path for a report name ("" when unknown / internal).
pub fn java_by_name(report_name: &str) -> &'static str {
    WRITERS
        .iter()
        .find(|(_, n, _)| *n == report_name)
        .map(|(_, _, p)| *p)
        .unwrap_or("")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn ids_and_names_unique() {
        let mut ids = BTreeSet::new();
        let mut names = BTreeSet::new();
        for &(id, name, _) in WRITERS {
            assert!(ids.insert(id), "duplicate writer id {id}");
            assert!(names.insert(name), "duplicate writer name {name}");
        }
    }

    #[test]
    fn lookup_never_panics() {
        assert_eq!(name(TERRAIN), "terrain");
        assert_eq!(name(u16::MAX - 1), "unknown");
        assert!(java_path(10).ends_with("TreeFeature.java"));
    }

    #[test]
    fn every_dispatch_arm_has_a_writer() {
        for ty in [
            "minecraft:tree",
            "minecraft:disk",
            "minecraft:geode",
            "minecraft:vegetation_patch",
            "minecraft:waterlogged_vegetation_patch",
        ] {
            assert_ne!(for_configured_type(ty), TERRAIN, "{ty} must map");
        }
    }
}
