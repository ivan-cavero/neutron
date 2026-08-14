// Dump vanilla mineshaft structure start pieces from a chunk.
use neutron_world::nbt::ussr_nbt::owned::{List, Tag};
use neutron_world::nbt::{compound_get, read_nbt};
use neutron_world::Region;
use std::path::PathBuf;

fn main() {
    let cx: i32 = 4;
    let cz: i32 = -1;
    let path = PathBuf::from(
        "tools/nbt-ref/vanilla1/world/dimensions/minecraft/overworld/region/r.0.-1.mca",
    );
    let region = Region::open(&path).unwrap().with_coords(0, -1);
    let data = region
        .get_chunk(cx.rem_euclid(32), cz.rem_euclid(32))
        .unwrap()
        .unwrap();
    let nbt = read_nbt(&data).unwrap();
    let Some(Tag::Compound(structs)) = compound_get(&nbt.compound, "structures") else {
        println!("no structures");
        return;
    };
    if let Some(Tag::Compound(starts)) = compound_get(structs, "starts") {
        println!("starts={}", starts.tags.len());
        for (k, v) in &starts.tags {
            println!("== {k} ==");
            show(v, 0);
        }
    }
    if let Some(Tag::Compound(refs)) = compound_get(structs, "References") {
        for (k, v) in &refs.tags {
            println!("ref {k}: {v:?}");
        }
    }
}

fn show(tag: &Tag, indent: usize) {
    let pad = "  ".repeat(indent);
    match tag {
        Tag::Compound(c) => {
            for (k, v) in &c.tags {
                match v {
                    Tag::Compound(_) | Tag::List(_) => {
                        println!("{pad}{k}:");
                        show(v, indent + 1);
                    }
                    _ => println!("{pad}{k}={v:?}"),
                }
            }
        }
        Tag::List(List::Compound(cs)) => {
            println!("{pad}list[{}]", cs.len());
            for (i, c) in cs.iter().enumerate() {
                if let Some(Tag::IntArray(bb)) = compound_get(c, "BB") {
                    let v = bb.to_vec();
                    let id = match compound_get(c, "id") {
                        Some(Tag::String(s)) => s.to_string(),
                        _ => "?".into(),
                    };
                    println!("V[{i}] {id} {} {} {} {} {} {}", v[0], v[1], v[2], v[3], v[4], v[5]);
                }
            }
        }
        other => println!("{pad}{other:?}"),
    }
}