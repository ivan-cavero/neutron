use neutron_world::nbt::ussr_nbt::owned::{List, Tag};
use neutron_world::nbt::{compound_get, read_nbt};
use neutron_world::Region;
use std::path::PathBuf;

fn show(tag: &Tag, indent: usize, max_depth: usize) {
    if indent > max_depth {
        return;
    }
    let pad = "  ".repeat(indent);
    match tag {
        Tag::Compound(c) => {
            for (k, v) in c.iter() {
                match v {
                    Tag::Compound(_) | Tag::List(_) => {
                        println!("{}{}:", pad, k);
                        show(v, indent + 1, max_depth);
                    }
                    Tag::String(s) => println!("{}{} = \"{}\"", pad, k, s),
                    Tag::Int(i) => println!("{}{} = {}", pad, k, i),
                    Tag::Long(i) => println!("{}{} = {}", pad, k, i),
                    Tag::Byte(i) => println!("{}{} = {}", pad, k, i),
                    _ => println!("{}{} = {:?}", pad, k, v),
                }
            }
        }
        Tag::List(List::Compound(l)) => {
            println!("{}[{} compounds]", pad, l.len());
            for (i, c) in l.iter().take(5).enumerate() {
                println!("{}[{}]:", pad, i);
                show(&Tag::Compound(c.clone()), indent + 1, max_depth);
            }
        }
        Tag::List(List::String(l)) => println!("{}{:?}", pad, l),
        _ => println!("{}{:?}", pad, tag),
    }
}

fn main() {
    let path = PathBuf::from(
        "tools/nbt-ref/vanilla1/world/dimensions/minecraft/overworld/region/r.0.-1.mca",
    );
    let region = Region::open(&path).unwrap().with_coords(0, -1);
    let data = region.get_chunk(6, 30).unwrap().unwrap();
    let nbt = read_nbt(&data).unwrap();
    // top-level keys
    for (k, _) in nbt.compound.iter() {
        println!("key: {k}");
    }
    if let Some(s) = compound_get(&nbt.compound, "structures") {
        println!("=== structures ===");
        show(s, 0, 4);
    }
    if let Some(s) = compound_get(&nbt.compound, "structures") {
        // already
    }
    // also check below_zero etc
    for key in ["structures", "structure_starts", "Starts"] {
        if let Some(s) = compound_get(&nbt.compound, key) {
            println!("found {key}");
        }
    }
}
