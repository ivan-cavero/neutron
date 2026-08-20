//! run-058 T4 measurement: does the vanilla 424242 reference region contain
//! the blocks each ported feature places? And does neutron place them?
//! Usage: t4_measure [region_dir]
use neutron_world::nbt::ussr_nbt::owned::{List, Tag};
use neutron_world::nbt::{compound_get, read_nbt};
use neutron_world::Region;
use neutron_worldgen::surface::BlockId;
use neutron_worldgen::ChunkGenerator;
use std::collections::HashMap;
use std::path::PathBuf;

fn main() {
    let seed: i64 = 424242;
    let region_dir = std::env::args().nth(1).unwrap_or_else(|| {
        "F:/neutron/tools/nbt-ref/vanilla-fresh-424242/world/dimensions/minecraft/overworld/region".to_string()
    });
    // Blocks each ported feature places (by vanilla name).
    let targets: &[(&str, &str)] = &[
        ("desert_well", "sandstone_slab"),
        ("desert_well", "suspicious_sand"),
        ("desert_well", "sandstone"),
        ("ice_patch", "packed_ice"),
        ("iceberg", "blue_ice"),
        ("iceberg", "packed_ice"),
        ("ice_spike", "packed_ice"),
        ("ice_spike", "ice"),
        ("freeze_top_layer", "ice"),
        ("freeze_top_layer", "snow"),
        ("fossil", "bone_block"),
        ("fossil_coal", "coal_ore"),
        ("fossil_diamonds", "deepslate_diamond_ore"),
        ("monster_room", "chest"),
        ("monster_room", "spawner"),
        ("monster_room", "cobblestone"),
        ("monster_room", "mossy_cobblestone"),
        ("lake_lava", "lava"),
        ("sulfur_pool", "water"),
        ("sulfur_pool", "sulfur"),
        ("sulfur_pool", "potent_sulfur"),
        ("dripstone_cluster", "dripstone_block"),
        ("dripstone_cluster", "pointed_dripstone"),
        ("sulfur_spike_cluster", "sulfur_spike"),
        ("bamboo", "bamboo"),
        ("bamboo", "podzol"),
        ("amethyst_geode", "amethyst_block"),
        ("amethyst_geode", "budding_amethyst"),
        ("amethyst_geode", "calcite"),
        ("amethyst_geode", "smooth_basalt"),
        ("amethyst_geode", "small_amethyst_bud"),
        ("amethyst_geode", "medium_amethyst_bud"),
        ("amethyst_geode", "large_amethyst_bud"),
        ("amethyst_geode", "amethyst_cluster"),
    ];
    let mut want: Vec<(&str, &str, u32)> = targets.iter().map(|(f, b)| (*f, *b, 0)).collect();

    let mut regions: HashMap<(i32, i32), Region> = HashMap::new();
    for rx in -1..=0 {
        for rz in -1..=0 {
            let path = PathBuf::from(format!("{region_dir}/r.{rx}.{rz}.mca"));
            if let Ok(r) = Region::open(&path) {
                regions.insert((rx, rz), r.with_coords(rx, rz));
            }
        }
    }
    for ((rx, rz), reg) in &regions {
        for lz in 0..32i32 {
            for lx in 0..32i32 {
                if let Ok(Some(data)) = reg.get_chunk(lx, lz) {
                    if let Ok(nbt) = read_nbt(&data) {
                        let sections = match compound_get(&nbt.compound, "sections") {
                            Some(Tag::List(List::Compound(l))) => l,
                            _ => continue,
                        };
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
                            let nstates = names.len();
                            if nstates == 1 {
                                let n = names[0].clone();
                                for w in want.iter_mut() {
                                    if w.1 == n.trim_start_matches("minecraft:") {
                                        w.2 += 4096;
                                    }
                                }
                                continue;
                            }
                            let bits = ((nstates - 1).ilog2() + 1).max(4) as u32;
                            let Some(Tag::LongArray(data)) = compound_get(bs, "data") else { continue };
                            let longs: Vec<i64> = data.to_vec();
                            let epl = 64 / bits;
                            let mask = (1u64 << bits) - 1;
                            for i in 0..4096u32 {
                                let li = (i / epl) as usize;
                                let bo = (i % epl) * bits;
                                let idx = ((longs[li] as u64) >> bo) & mask;
                                let n = names.get(idx as usize).cloned().unwrap_or_default();
                                for w in want.iter_mut() {
                                    if w.1 == n.trim_start_matches("minecraft:") {
                                        w.2 += 1;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    println!("=== VANILLA REF 424242 (region r.-1..0.-1..0) block counts ===");
    for (f, b, c) in &want {
        println!("  {f:28} {b:28} {c}");
    }

    // Neutron generation over the same area.
    let gen = ChunkGenerator::new(seed);
    let mut neu: HashMap<&str, u64> = HashMap::new();
    for cx in -32..32 {
        for cz in -32..32 {
            let chunk = gen.generate_chunk(cx, cz);
            for y in -64..320 {
                for z in 0..16u8 {
                    for x in 0..16u8 {
                        let b = chunk.block_at(x as u32, y, z as u32);
                        let name = b.block_name().trim_start_matches("minecraft:").to_string();
                        for (_, bn, _) in &want {
                            if *bn == name {
                                *neu.entry(bn).or_insert(0) += 1;
                            }
                        }
                    }
                }
            }
        }
    }
    println!("=== NEUTRON GENERATED (same 64x64 chunk area) ===");
    for (f, b, c) in &want {
        let n = neu.get(b).copied().unwrap_or(0);
        let verdict = if *c == 0 && n == 0 {
            "0 bloques en region ref (port dormido)"
        } else if *c > 0 && n > 0 {
            "coincide (ambos colocan)"
        } else if *c > 0 {
            "REF TIENE, neutron no coloca (gap)"
        } else {
            "neutron coloca, ref no (posible over-place)"
        };
        println!("  {f:28} {b:28} ref={c:>8} neu={n:>8}  {verdict}");
    }
}
