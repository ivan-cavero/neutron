use neutron_worldgen::{ChunkGenerator, surface::BlockId, generator::WORLD_BOTTOM};
use neutron_world::nbt::ussr_nbt::owned::{List, Tag};
use neutron_world::nbt::{compound_get, read_nbt};
use neutron_world::Region;
use std::path::PathBuf;
fn main() {
  let g = ChunkGenerator::new(12345);
  let ch = g.generate_chunk(6,-2);
  let path = PathBuf::from("tools/nbt-ref/vanilla1/world/dimensions/minecraft/overworld/region/r.0.-1.mca");
  let region = Region::open(&path).unwrap().with_coords(0,-1);
  let data = region.get_chunk(6,30).unwrap().unwrap();
  let nbt = read_nbt(&data).unwrap();
  let sections = match compound_get(&nbt.compound,"sections") { Some(Tag::List(List::Compound(l)))=>l, _=>panic!() };
  let mut van = vec![String::new(); 98304];
  for sec in sections {
    let y_sec = match compound_get(sec,"Y") { Some(Tag::Byte(y))=> *y as i8 as i32, _=>continue };
    let Some(Tag::Compound(bs)) = compound_get(sec,"block_states") else { continue };
    let Some(Tag::List(List::Compound(palette))) = compound_get(bs,"palette") else { continue };
    let names: Vec<String> = palette.iter().map(|pc| match compound_get(pc,"Name") { Some(Tag::String(s))=>s.to_string(), _=>"minecraft:air".into() }).collect();
    let nstates = names.len();
    for i in 0..4096u32 {
      let name = if nstates==1 { names[0].clone() } else {
        let bits = ((nstates-1).ilog2()+1).max(4) as u32;
        let Tag::LongArray(data)=compound_get(bs,"data").unwrap() else { panic!() };
        let longs: Vec<i64>=data.to_vec(); let epl=64/bits; let mask=(1u64<<bits)-1;
        let li=(i/epl) as usize; let bo=(i%epl)*bits;
        let idx=((longs[li] as u64)>>bo)&mask; names[idx as usize].clone()
      };
      let ly=(i>>8) as i32; let lz=((i>>4)&15) as usize; let lx=(i&15) as usize;
      let y=y_sec*16+ly; let idx=((y-WORLD_BOTTOM) as usize)*256+lz*16+lx;
      if idx < van.len() { van[idx]=name; }
    }
  }
  let dirs=[(0i32,-1,0),(0,1,0),(0,0,-1),(0,0,1),(-1,0,0),(1,0,0)];
  let mut van_sculk_neu_wall=0u32;
  let mut van_sculk_neu_air=0u32;
  let mut van_sculk_neu_buried=0u32;
  let mut van_sculk_neu_sculk=0u32;
  let mut hist = std::collections::HashMap::new();
  for y in WORLD_BOTTOM..320 {
    for z in 0..16i32 {
      for x in 0..16i32 {
        let idx=((y-WORLD_BOTTOM) as usize)*256+(z as usize)*16+(x as usize);
        let s = van[idx].strip_prefix("minecraft:").unwrap_or(&van[idx]);
        if s != "sculk" && s != "sculk_catalyst" { continue; }
        let nb = ch.block_at(x as u32,y,z as u32);
        *hist.entry(format!("{nb:?}")).or_insert(0u32) += 1;
        if matches!(nb, BlockId::Sculk|BlockId::SculkCatalyst) { van_sculk_neu_sculk+=1; continue; }
        if matches!(nb, BlockId::Air|BlockId::Water|BlockId::SculkVein) { van_sculk_neu_air+=1; continue; }
        // solid - wall?
        let mut open=false;
        for (dx,dy,dz) in dirs {
          let nx=x+dx; let nz=z+dz; let ny=y+dy;
          if nx<0||nx>=16||nz<0||nz>=16||ny<WORLD_BOTTOM||ny>=320 { continue; }
          let n2=ch.block_at(nx as u32,ny,nz as u32);
          if matches!(n2, BlockId::Air|BlockId::Water|BlockId::SculkVein|BlockId::Sculk) { open=true; break; }
        }
        if open { van_sculk_neu_wall+=1; } else { van_sculk_neu_buried+=1; }
      }
    }
  }
  println!("van sculk cells as neu: sculk={van_sculk_neu_sculk} wall_solid={van_sculk_neu_wall} buried_solid={van_sculk_neu_buried} air={van_sculk_neu_air}");
  let mut v: Vec<_>=hist.into_iter().collect(); v.sort_by(|a,b| b.1.cmp(&a.1));
  for (k,c) in v.iter().take(10) { println!("  {c:4} {k}"); }
}
