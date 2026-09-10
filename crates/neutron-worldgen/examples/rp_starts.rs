// Dump vanilla saved StructureStarts for ruined portals around a window.
use neutron_world::nbt::ussr_nbt::owned::{List, Tag};
use neutron_world::nbt::read_nbt;
use neutron_world::Region;
use std::collections::HashMap;
use std::path::PathBuf;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let region_dir = args
        .get(1)
        .map(|s| s.as_str())
        .unwrap_or("tools/nbt-ref/vanilla-fresh-424242/world/dimensions/minecraft/overworld/region");
    let mut regions: HashMap<(i32, i32), Region> = HashMap::new();
    for cz in 1..=3i32 {
        for cx in 5..=9i32 {
            let (rx, rz) = (cx >> 5, cz >> 5);
            let key = (rx, rz);
            if !regions.contains_key(&key) {
                let path = PathBuf::from(format!("{region_dir}/r.{rx}.{rz}.mca"));
                let region = Region::open(&path).unwrap().with_coords(rx, rz);
                regions.insert(key, region);
            }
            let region = regions.get(&key).unwrap();
            let Some(data) = region.get_chunk(cx & 31, cz & 31).ok().flatten() else { continue };
            let nbt = read_nbt(&data).unwrap();
            let structures = get_compound(&nbt.compound.tags, "structures");
            let starts = structures.and_then(|c| get_compound(&c.tags, "starts"));
            for entry in starts.as_ref().map(|c| c.tags.clone()).unwrap_or_default().iter() {
                let (sid, start) = (&entry.0, &entry.1);
                if sid.to_string().contains("ruined") {
                    println!("chunk({cx},{cz}) start={sid}");
                    dump("  ", start);
                }
            }
        }
    }
}

fn get_compound<'a>(tags: &'a [(neutron_world::nbt::ussr_nbt::mutf8::MString, Tag)], name: &str) -> Option<&'a neutron_world::nbt::ussr_nbt::owned::Compound> {
    tags.iter()
        .find(|(k, _)| k.to_string() == name)
        .and_then(|(_, v)| match v {
            Tag::Compound(c) => Some(c),
            _ => None,
        })
}

fn dump(indent: &str, tag: &Tag) {
    if let Tag::Compound(c) = tag {
        for (k, v) in &c.tags {
            match v {
                Tag::Compound(inner) => {
                    println!("{indent}{}:", k);
                    dump(&format!("{indent}  "), &Tag::Compound(inner.clone()));
                }
                Tag::List(List::Compound(items)) => {
                    println!("{indent}{k}: [{} items]", items.len());
                    for it in items {
                        dump(&format!("{indent}  "), &Tag::Compound(it.clone()));
                    }
                }
                other => println!("{indent}{}: {}", k, tag_to_str(other)),
            }
        }
    }
}

fn tag_to_str(t: &Tag) -> String {
    match t {
        Tag::Byte(b) => format!("byte {b}"),
        Tag::Short(s) => format!("short {s}"),
        Tag::Int(i) => format!("int {i}"),
        Tag::Long(l) => format!("long {l}"),
        Tag::Float(f) => format!("float {f}"),
        Tag::Double(d) => format!("double {d}"),
        Tag::String(s) => s.to_string(),
        other => format!("<{:?}>", other),
    }
}
