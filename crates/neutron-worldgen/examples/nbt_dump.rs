use neutron_world::{Region, nbt::{read_nbt, compound_get}, nbt::ussr_nbt::owned::{Tag as T, List as L, Compound as C};
use std::path::Path;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let path = &args[1];
    let cx: i32 = args[2].parse().unwrap();
    let cz: i32 = args[3].parse().unwrap();
    let region = Region::open(Path::new(path)).unwrap();
    let (rx, rz) = neutron_world::parse_region_filename(Path::new(path).file_name().unwrap().to_str().unwrap()).unwrap();
    let data = region.get_chunk(cx, cz).unwrap().unwrap();
    let nbt = read_nbt(&data).unwrap();
    // Count non-air sections
    if let Some(T::List(L::Compound(sections))) = compound_get(&nbt.compound, "sections") {
        let mut count = 0;
        for s in sections {
            if let Some(T::Compound(bs)) = compound_get(s, "block_states") {
                if let Some(T::List(L::Compound(pal))) = compound_get(bs, "palette") {
                    if pal.iter().any(|p| {
                        matches!(compound_get(p, "Name").map(|n| matches!(n, T::String(s) if s.to_string() != "minecraft:air")), Some(true))
                    }) {
                        count += 1;
                    }
                }
            }
        }
        println!("Chunk ({},{}) in region ({},{}): {} non-air sections out of {}",
            rx*32+cx, rz*32+cz, rx, rz, count, sections.len());
        // Show first non-air section details
        for s in sections {
            if let Some(T::Compound(bs)) = compound_get(s, "block_states") {
                if let Some(T::List(L::Compound(pal))) = compound_get(bs, "palette") {
                    let has_non_air = pal.iter().any(|p| {
                        matches!(compound_get(p, "Name").map(|n| matches!(n, T::String(s) if s.to_string() != "minecraft:air")), Some(true))
                    });
                    if has_non_air {
                        let y = compound_get(s, "Y");
                        let has_data = compound_get(bs, "data").is_some();
                        let palette_size = pal.len();
                        println!("  Section Y={:?}: {} palette entries, has_data={}", y, palette_size, has_data);
                        // Show palette
                        for (i, p) in pal.iter().enumerate() {
                            let name = compound_get(p, "Name").map(|n| format!("{:?}", n)).unwrap_or_default();
                            let props = compound_get(p, "Properties");
                            println!("    [{}] {} props={:?}", i, name, props.is_some());
                        }
                        break;
                    }
                }
            }
        }
    }
}
