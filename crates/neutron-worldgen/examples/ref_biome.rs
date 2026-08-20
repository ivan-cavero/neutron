//! run-058 T1: print the biome at chunk centers in the vanilla ref vs Neutron.
use neutron_world::nbt::ussr_nbt::owned::{List, Tag};
use neutron_world::nbt::{compound_get, read_nbt};
use neutron_world::Region;
use neutron_worldgen::biome_source;
use neutron_worldgen::ChunkGenerator;
use std::path::PathBuf;

fn main() {
    let seed: i64 = std::env::args().nth(1).and_then(|s| s.parse().ok()).unwrap_or(424242);
    let region_dir = std::env::args().nth(2).unwrap_or_else(|| "tools/nbt-ref/vanilla-fresh-424242/world/dimensions/minecraft/overworld/region".to_string());
    let gen = ChunkGenerator::new(seed);
    for (cx, cz) in [(0i32,0i32),(11,11),(5,3),(8,9)] {
        let (rx, rz) = (cx >> 5, cz >> 5);
        let path = PathBuf::from(format!("{region_dir}/r.{rx}.{rz}.mca"));
        let Ok(region) = Region::open(&path) else { println!("chunk ({cx},{cz}) no region file"); continue; };
        let region = region.with_coords(rx, rz);
        let Ok(Some(data)) = region.get_chunk(cx & 31, cz & 31) else { println!("chunk ({cx},{cz}) not present"); continue; };
        let Ok(nbt) = read_nbt(&data) else { continue; };
        let sections = match compound_get(&nbt.compound, "sections") {
            Some(Tag::List(List::Compound(l))) => l,
            _ => continue,
        };
        // find surface biome: biome at (8, y, 8) for y=100
        let mut biome = String::new();
        for sec in sections {
            let y_sec = match compound_get(sec, "Y") { Some(Tag::Byte(b)) => *b as i8 as i32, _ => continue };
            if y_sec != 6 { continue; } // section y=6 covers 96..111
            let Some(Tag::Compound(biomes)) = compound_get(sec, "biomes") else { continue };
            let Some(Tag::List(List::Compound(palette))) = compound_get(biomes, "palette") else { continue };
            let names: Vec<String> = palette.iter().map(|pc| match compound_get(pc, "Name") { Some(Tag::String(s)) => s.to_string(), _ => "?".into() }).collect();
            let arr = match compound_get(biomes, "data") { Some(Tag::LongArray(d)) => d.to_vec(), _ => vec![] };
            // block (8,100,8): section-local (8, 100-96=4, 8) -> quart (2, 1, 2)
            let idx = (1*4 + 2)*4 + 2;
            biome = if arr.is_empty() { names[0].clone() } else { names[arr[idx] as usize].clone() };
        }
        let nb = biome_source::biome_id_at_block(&gen.state, cx*16+8, 100, cz*16+8);
        println!("chunk ({cx},{cz}) ref_biome={biome} neutron_id={nb} ({})",
            match nb { x if x == biome_source::biome_id::PALE_GARDEN => "pale_garden", x if x == biome_source::biome_id::LUSH_CAVES => "lush_caves", x if x == biome_source::biome_id::PLAINS => "plains", x if x == biome_source::biome_id::FOREST => "forest", x if x == biome_source::biome_id::OCEAN => "ocean", x if x == biome_source::biome_id::RIVER => "river", _ => "?" });
    }
}
