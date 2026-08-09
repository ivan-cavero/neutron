use neutron_worldgen::sculk::{SCULK_ENABLED};
use neutron_worldgen::{ChunkGenerator, surface::BlockId, biome_source::{climate_at_block, find_biome, biome_id}, density::DensityEnv, generator::WORLD_BOTTOM, feature_rng::FeatureRandom, region_buf::RegionBuf};
use neutron_worldgen::{carvers, features};

// Replicate generator mid-pipeline to log spread positions
fn main() {
  assert!(SCULK_ENABLED);
  let g = ChunkGenerator::new(12345);
  let st = &g.state;
  let cx=6i32; let cz=-2i32;
  let mut region = RegionBuf::new(cx, cz, 1);
  for dz in -1..=1 {
    for dx in -1..=1 {
      let ch = {
        // use full generate for each noise chunk - expensive; use generate_chunk and put? can't.
        // Approximate: only analyze center after generate
        ()
      };
      let _ = (dx,dz,ch);
    }
  }
  // Log every sculk in center + sample neighbor gens
  let ch = g.generate_chunk(6,-2);
  let mut sculk_pos=vec![];
  for y in WORLD_BOTTOM..320 {
    for z in 0..16u32 {
      for x in 0..16u32 {
        if matches!(ch.block_at(x,y,z), BlockId::Sculk|BlockId::SculkCatalyst) {
          sculk_pos.push((x as i32,y,z as i32));
        }
      }
    }
  }
  println!("center sculk count={} samples={:?}", sculk_pos.len(), &sculk_pos[..sculk_pos.len().min(20)]);
  // For each of 9 origins, count expected spread seeds with veins present in FINAL region - approximate with generate of each
  for ncz in -3..=-1 {
    for ncx in 5..=7 {
      let mut rng = FeatureRandom::new(12345);
      let ox = ncx*16; let oz = ncz*16;
      let dec = rng.set_decoration_seed(12345, ox, oz);
      rng.set_feature_seed(dec, 1, 7);
      let chn = g.generate_chunk(ncx, ncz);
      let mut hits=0u32;
      for _ in 0..256 {
        let x = ox + rng.next_int(16);
        let z = oz + rng.next_int(16);
        let y = WORLD_BOTTOM + rng.next_int(256-WORLD_BOTTOM+1);
        let lx = x-ox; let lz = z-oz;
        if lx<0||lx>=16||lz<0||lz>=16 { continue; }
        let mut env = DensityEnv::new(x,y,z, st.noises.noises());
        let climate = climate_at_block(&mut env, &st.router.temperature, &st.router.vegetation, &st.router.continents, &st.router.erosion, &st.router.depth, &st.router.ridges);
        if find_biome(&climate) != biome_id::DEEP_DARK { continue; }
        let b = chn.block_at(lx as u32,y,lz as u32);
        let ok = matches!(b, BlockId::Sculk|BlockId::SculkVein|BlockId::SculkCatalyst|BlockId::Air|BlockId::Water);
        if !ok { continue; }
        if matches!(b, BlockId::Sculk|BlockId::SculkVein|BlockId::SculkCatalyst) { hits+=1; continue; }
        // air - solid n?
        let dirs=[(0i32,-1,0),(0,1,0),(0,0,-1),(0,0,1),(-1,0,0),(1,0,0)];
        for (dx,dy,dz) in dirs {
          let nx=lx+dx; let nz=lz+dz; let ny=y+dy;
          if nx<0||nx>=16||nz<0||nz>=16||ny<WORLD_BOTTOM||ny>=320 { continue; }
          let nb=chn.block_at(nx as u32,ny,nz as u32);
          if !matches!(nb, BlockId::Air|BlockId::Water|BlockId::Lava|BlockId::Sculk|BlockId::SculkVein|BlockId::SculkCatalyst|BlockId::OakLeaves|BlockId::Snow|BlockId::PowderSnow|BlockId::ShortGrass) {
            hits+=1; break;
          }
        }
      }
      let mut sc=0u32;
      for y in WORLD_BOTTOM..320 {
        for z in 0..16u32 { for x in 0..16u32 {
          if matches!(chn.block_at(x,y,z), BlockId::Sculk|BlockId::SculkCatalyst) { sc+=1; }
        }}
      }
      println!("chunk ({ncx},{ncz}) patch_hits≈{hits} sculk={sc}");
    }
  }
}
