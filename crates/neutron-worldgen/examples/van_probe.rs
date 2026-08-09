use neutron_world::nbt::ussr_nbt::owned::{List, Tag};
use neutron_world::nbt::{compound_get, read_nbt};
use neutron_world::Region;
use std::path::PathBuf;
fn main() {
  for (cx,cz) in [(0i32,0i32),(6,-2),(4,0),(-5,3),(1,-1),(5,-2)] {
    let rx=cx.div_euclid(32); let rz=cz.div_euclid(32);
    let lx=cx.rem_euclid(32); let lz=cz.rem_euclid(32);
    let path=PathBuf::from(format!("tools/nbt-ref/vanilla1/world/dimensions/minecraft/overworld/region/r.{rx}.{rz}.mca"));
    let region=Region::open(&path).unwrap().with_coords(rx,rz);
    match region.get_chunk(lx,lz).unwrap() {
      None => println!("({cx},{cz}) EMPTY/None"),
      Some(data) => {
        let nbt=read_nbt(&data).unwrap();
        let status = compound_get(&nbt.compound,"Status").map(|t| format!("{:?}",t)).unwrap_or("?".into());
        let sections=match compound_get(&nbt.compound,"sections") {
          Some(Tag::List(List::Compound(l)))=>l.len(), _=>0
        };
        println!("({cx},{cz}) status={status} sections={sections} bytes={}", data.len());
      }
    }
  }
}
