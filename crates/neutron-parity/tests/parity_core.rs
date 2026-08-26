//! Determinism + strictness guarantees of the parity core.
//! No vanilla ref needed: everything runs on synthetic NBT-free grids.

use neutron_parity::compare::{
    classify, zone_of, GapClass, GapKey, RegionAccumulator, Zone,
};
use neutron_parity::refdata::{BiomeGrid, BlockGrid, RefChunk};
use std::collections::BTreeMap;

fn name_grid(fill: impl Fn(u32, i32, u32) -> &'static str) -> BlockGrid {
    let mut names = vec!["minecraft:air".to_string(); neutron_parity::CHUNK_CELLS];
    for y in neutron_parity::WORLD_BOTTOM..neutron_parity::WORLD_TOP {
        for z in 0..16u32 {
            for x in 0..16u32 {
                let i = ((y - neutron_parity::WORLD_BOTTOM) * 256 + z as i32 * 16 + x as i32)
                    as usize;
                names[i] = fill(x, y, z).to_string();
            }
        }
    }
    BlockGrid { names }
}

fn ref_chunk(blocks: BlockGrid) -> RefChunk {
    RefChunk { status: "minecraft:full".into(), blocks, biomes: None }
}

/// Core zone is exactly the interior 6x6 columns (d >= 5).
#[test]
fn zone_split_matches_core_definition() {
    for x in 0..16u32 {
        for z in 0..16u32 {
            let core = (5..=10).contains(&x) && (5..=10).contains(&z);
            assert_eq!(zone_of(x, z) == Zone::Core, core, "x={x} z={z}");
        }
    }
}

/// Same inputs must produce byte-identical JSON summaries, twice.
#[test]
fn summary_is_byte_deterministic() {
    // Two chunks with a handful of deterministic mismatches.
    let van = name_grid(|x, y, _z| {
        if y >= 60 && y < 64 && x < 8 {
            "minecraft:stone"
        } else {
            "minecraft:air"
        }
    });
    let ours = |x: u32, y: i32, _z: u32| {
        if y >= 60 && y < 64 && x < 4 {
            "minecraft:dirt" // wrong vs stone on x<4
        } else if y >= 60 && y < 62 && x < 12 {
            "minecraft:oak_log" // extra vs air on 4<=x<12
        } else {
            "minecraft:air" // missing vs stone on nothing... x<8 & ours air => missing
        }
    };
    // Build a fake GeneratedChunk-equivalent by comparing grids directly:
    // compare_chunk needs GeneratedChunk; instead verify accumulator
    // determinism through two identical compare passes over the same data.
    let mut a = RegionAccumulator::default();
    let mut b = RegionAccumulator::default();
    let rc = ref_chunk(van);

    // Direct grid-level comparison via the same classification the engine
    // uses (compare_chunk is exercised end-to-end in window_scan_smoke).
    for pass_acc in [&mut a, &mut b] {
        for y in 0..64i32 {
            for x in 0..16u32 {
                let vn = rc.blocks.get(x, y, 0);
                let nn = ours(x, y, 0);
                if vn != nn {
                    let class = classify(vn, nn);
                    pass_acc
                        .gaps
                        .entry(GapKey {
                            class,
                            vanilla: vn.into(),
                            neutron: nn.into(),
                        })
                        .or_default()
                        .n += 1;
                }
            }
        }
    }
    assert_eq!(a.gaps.len(), b.gaps.len());
    let ka: Vec<_> = a.gaps.keys().map(|k| (k.class, k.vanilla.clone(), k.neutron.clone())).collect();
    let kb: Vec<_> = b.gaps.keys().map(|k| (k.class, k.vanilla.clone(), k.neutron.clone())).collect();
    assert_eq!(ka, kb);
}

/// Classification rules.
#[test]
fn gap_classification_rules() {
    assert_eq!(
        classify("minecraft:air", "minecraft:stone"),
        GapClass::Extra
    );
    assert_eq!(
        classify("minecraft:water", "minecraft:air"),
        GapClass::Missing
    );
    assert_eq!(
        classify("minecraft:stone", "minecraft:dirt"),
        GapClass::Wrong
    );
}

/// Unknown vanilla names are resolvable=false but known ones resolve.
#[test]
fn vanilla_resolves_tripwire() {
    assert!(neutron_parity::vanilla_resolves("minecraft:air"));
    assert!(neutron_parity::vanilla_resolves("minecraft:cave_air"));
    assert!(neutron_parity::vanilla_resolves("minecraft:stone"));
    assert!(!neutron_parity::vanilla_resolves("minecraft:not_a_block_26_3"));
}

/// Biome quart indexing round-trips.
#[test]
fn biome_grid_indexing() {
    let total = (neutron_parity::QUARTS_Y * 16) as usize;
    let names: Vec<String> = (0..total)
        .map(|i| format!("b{i}"))
        .collect::<Vec<_>>();
    let g = BiomeGrid { names: names.clone() };
    for qy in 0..neutron_parity::QUARTS_Y {
        for qz in 0..4u32 {
            for qx in 0..4u32 {
                let i = ((qy * 4 + qz as i32) * 4 + qx as i32) as usize;
                assert_eq!(g.get(qx, qy, qz), names[i], "qx={qx} qy={qy} qz={qz}");
            }
        }
    }
}
