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
    for (cx, cz) in [(0i32,0i32),(11,11),(5,3),(8,9),(2,2)] {
        let (rx, rz) = (cx >> 5, cz >> 5);
        let path = PathBuf::from(format!("{region_dir}/r.{rx}.{rz}.mca"));
        let Ok(region) = Region::open(&path) else { println!("chunk ({cx},{cz}) no region"); continue; };
        let region = region.with_coords(rx, rz);
        let Ok(Some(data)) = region.get_chunk(cx & 31, cz & 31) else { println!("chunk ({cx},{cz}) not present"); continue; };
        let Ok(nbt) = read_nbt(&data) else { continue; };
        let sections = match compound_get(&nbt.compound, "sections") {
            Some(Tag::List(List::Compound(l))) => l,
            _ => continue,
        };
        let mut biome = String::new();
        for sec in sections {
            let y_sec = match compound_get(sec, "Y") { Some(Tag::Byte(b)) => *b as i8 as i32, _ => continue };
            if y_sec != 6 { continue; } // section y=6 covers 96..111
            let Some(Tag::Compound(biomes)) = compound_get(sec, "biomes") else { continue };
            let Some(Tag::List(List::Compound(palette))) = compound_get(biomes, "palette") else { continue };
            let names: Vec<String> = palette.iter().map(|pc| match compound_get(pc, "Name") { Some(Tag::String(s)) => s.to_string(), _ => "?".into() }).collect();
            let Some(Tag::LongArray(data)) = compound_get(biomes, "data") else { biome = names[0].clone(); continue; };
            let longs: Vec<i64> = data.to_vec();
            // biome grid: 4x4x4 per section. block (8,100,8) -> local (8,4,8) -> quart (2,1,2) -> idx (1*4+2)*4+2 = 26
            let bits = ((names.len()-1).ilog2()+1).max(2) as u32;
            let epl = 64/bits; let mask = (1u64<<bits)-1;
            let idx = 26u32;
            let li = (idx/epl) as usize; let bo = (idx%epl)*bits;
            let v = ((longs[li] as u64)>>bo)&mask;
            biome = names.get(v as usize).cloned().unwrap_or("?".into());
            break;
        }
        let nb = biome_source::biome_id_at_block(&gen.state, cx*16+8, 100, cz*16+8);
        let nname = match nb { x if x == biome_source::biome_id::PALE_GARDEN => "pale_garden", x if x == biome_source::biome_id::LUSH_CAVES => "lush_caves", x if x == biome_source::biome_id::PLAINS => "plains", x if x == biome_source::biome_id::FOREST => "forest", x if x == biome_source::biome_id::OCEAN => "ocean", x if x == biome_source::biome_id::RIVER => "river", x if x == biome_source::biome_id::DEEP_OCEAN => "deep_ocean", _ => "?" };
        let match_s = if biome == format!("minecraft:{nname}") { "MATCH" } else { "DIFF" };
        println!("chunk ({cx},{cz}) ref={biome} neutron={nname} {match_s}");
    }
}
