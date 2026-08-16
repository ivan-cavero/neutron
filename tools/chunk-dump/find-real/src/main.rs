use neutron_world::{Region, nbt::{read_nbt, compound_get}};
use neutron_world::nbt::ussr_nbt::owned::{Tag as T, List as L, Compound as C};
fn main() {
    let mut found = 0;
    for (rx, rz) in [(-1,-1), (-1,0), (0,-1), (0,0)] {
        let path = format!("../vanilla1/world/dimensions/minecraft/overworld/region/r.{rx}.{rz}.mca");
        let region = match Region::open(std::path::Path::new(&path)) { Ok(r) => r.with_coords(rx, rz), Err(e) => { eprintln!("open err {path}: {e}"); continue } };
        for cz in 0..32 { for cx in 0..32 {
            match region.get_chunk(cx, cz) {
                Ok(Some(data)) => {
                if let Ok(nbt) = read_nbt(&data) {
                    let status = compound_get(&nbt.compound, "Status").map(|t| format!("{:?}", t)).unwrap_or_default();
                    let sections = compound_get(&nbt.compound, "sections").cloned();
                    let mut non_air = false;
                    if let Some(T::List(L::Compound(secs))) = sections {
                        for s in secs {
                            if let Some(T::Compound(bs)) = compound_get(&s, "block_states") {
                                if let Some(T::List(L::Compound(pal))) = compound_get(bs, "palette") {
                                    for p in pal {
                                        if let Some(T::String(n)) = compound_get(p, "Name") {
                                            if n.to_string() != "minecraft:air" { non_air = true; }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    if non_air {
                        println!("chunk ({},{}) status={:?}", rx*32+cx, rz*32+cz, status);
                        found += 1;
                    } else {
                        let st = status;
                        println!("empty chunk ({},{}) status={:?}", rx*32+cx, rz*32+cz, st);
                    }
                } else { eprintln!("read_nbt err"); }
                }
                Ok(None) => {}
                Err(e) => { eprintln!("get_chunk err {e}"); }
            }
        }}
    }
    println!("total non-air chunks: {found}");
}
