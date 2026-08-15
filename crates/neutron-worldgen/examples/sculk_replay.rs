// Replay the dumped pre-sculk cave (tools/java-probe/cave-98-43-23.txt) through
// neutron's ChargeCursor patch, matching ProbeSculkPatch tick-by-tick.
// cargo run -p neutron-worldgen --example sculk_replay --release [dump] [seed]

use neutron_worldgen::multiface_spreader::FaceMap;
use neutron_worldgen::region_buf::RegionBuf;
use neutron_worldgen::sculk;
use neutron_worldgen::surface::BlockId;

fn parse_name(n: &str) -> BlockId {
    match n {
        "air" => BlockId::Air,
        "stone" => BlockId::Stone,
        "granite" => BlockId::Granite,
        "diorite" => BlockId::Diorite,
        "andesite" => BlockId::Andesite,
        "dirt" => BlockId::Dirt,
        "gravel" => BlockId::Gravel,
        "deepslate" => BlockId::Deepslate,
        "tuff" => BlockId::Tuff,
        "calcite" => BlockId::Calcite,
        "water" => BlockId::Water,
        "lava" => BlockId::Lava,
        "clay" => BlockId::Clay,
        "bedrock" => BlockId::Bedrock,
        "coal_ore" => BlockId::CoalOre,
        "iron_ore" => BlockId::IronOre,
        "copper_ore" => BlockId::CopperOre,
        "gold_ore" => BlockId::GoldOre,
        "redstone_ore" => BlockId::RedstoneOre,
        "lapis_ore" => BlockId::LapisOre,
        "diamond_ore" => BlockId::DiamondOre,
        "deepslate_coal_ore" => BlockId::DeepslateCoalOre,
        "deepslate_iron_ore" => BlockId::DeepslateIronOre,
        "deepslate_copper_ore" => BlockId::DeepslateCopperOre,
        "deepslate_gold_ore" => BlockId::DeepslateGoldOre,
        "deepslate_redstone_ore" => BlockId::DeepslateRedstoneOre,
        "deepslate_lapis_ore" => BlockId::DeepslateLapisOre,
        "deepslate_diamond_ore" => BlockId::DeepslateDiamondOre,
        "raw_iron_block" => BlockId::RawIronBlock,
        other => {
            eprintln!("unmapped block name {other:?} -> deepslate");
            BlockId::Deepslate
        }
    }
}

fn main() {
    let dump = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "tools/java-probe/cave-98-43-23.txt".to_string());
    let seed: i64 = std::env::args().nth(2).and_then(|s| s.parse().ok()).unwrap_or(1);

    // Dump covers chunks 5..7 x -3..-1 → region center (6,-2) radius 1.
    let mut region = RegionBuf::new(6, -2, 1);
    let mut origin = (98i32, -43i32, -23i32);
    let content = std::fs::read_to_string(&dump).expect("read dump");
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let p: Vec<&str> = line.split_whitespace().collect();
        if p[0] == "origin" {
            origin = (p[1].parse().unwrap(), p[2].parse().unwrap(), p[3].parse().unwrap());
            continue;
        }
        let x: i32 = p[0].parse().unwrap();
        let y: i32 = p[1].parse().unwrap();
        let z: i32 = p[2].parse().unwrap();
        let b = parse_name(p[3]);
        if b == BlockId::Air {
            continue; // air = absent in the Java map
        }
        region.set(x, y, z, b);
    }
    // Any cell NOT in the dump is air in the Java map: clear everything first.
    // RegionBuf starts filled with Air (see region_buf.rs), so absent == air already.

    let faces: FaceMap = FaceMap::new();
    let mut r2 = RegionBuf::new(6, -2, 1);
    r2.blocks.copy_from_slice(&region.blocks);
    let (sculk_n, vein_n, growth_n, roll, draws) = sculk::probe_run_patch(&mut r2, origin, seed);
    println!(
        "seed={seed} origin=({},{},{}) sculk={sculk_n} vein={vein_n} growth={growth_n} roll={roll:.6} draws={draws}",
        origin.0, origin.1, origin.2
    );

    // Tick-by-tick re-run with NEUTRON_SCULK_STEPS so sculk.rs prints after1/after3 dumps.
    std::env::set_var("NEUTRON_SCULK_STEPS", "1");
    let mut r3 = RegionBuf::new(6, -2, 1);
    r3.blocks.copy_from_slice(&region.blocks);
    let _ = sculk::probe_run_patch(&mut r3, origin, seed);
}
