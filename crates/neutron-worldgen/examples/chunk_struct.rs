use neutron_world::nbt::ussr_nbt::owned::Tag;
use neutron_world::nbt::{compound_get, read_nbt};
use neutron_world::Region;
use std::path::PathBuf;
fn main() {
    let path = PathBuf::from(
        "tools/nbt-ref/vanilla1/world/dimensions/minecraft/overworld/region/r.0.-1.mca",
    );
    let region = Region::open(&path).unwrap().with_coords(0, -1);
    let data = region.get_chunk(6, 30).unwrap().unwrap();
    let nbt = read_nbt(&data).unwrap();
    if let Some(Tag::Compound(structs)) = compound_get(&nbt.compound, "structures") {
        if let Some(Tag::Compound(starts)) = compound_get(structs, "starts") {
            println!("starts count={}", starts.tags.len());
            for (k, v) in &starts.tags {
                println!("start {k}: {v:?}");
            }
        }
        if let Some(Tag::Compound(refs)) = compound_get(structs, "References") {
            println!("refs count={}", refs.tags.len());
            for (k, v) in &refs.tags {
                println!("ref {k}: {v:?}");
            }
        }
    }
}
