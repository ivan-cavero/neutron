use neutron_world::nbt::ussr_nbt::owned::{List, Tag};
use neutron_world::nbt::{compound_get, read_nbt};
use neutron_world::Region;
use neutron_worldgen::surface::BlockId;
use neutron_worldgen::{generator::WORLD_BOTTOM, ChunkGenerator, density::DensityEnv};
use neutron_worldgen::generator::{lerp, ChunkGenerator as CG};
use std::path::PathBuf;

fn is_solid_name(n: &str) -> bool {
    let n = n.strip_prefix("minecraft:").unwrap_or(n);
    if n=="air"||n=="cave_air"||n=="void_air"||n=="water"||n=="lava" { return false; }
    // veg counts as solid for shape of occupancy
    true
}
fn is_solid_neu(b: BlockId) -> bool {
    !matches!(b, BlockId::Air|BlockId::Water|BlockId::Lava)
}
fn is_fluid_name(n: &str) -> bool {
    let n = n.strip_prefix("minecraft:").unwrap_or(n);
    n=="water"||n=="lava"
}

fn main() {
    let path = PathBuf::from("tools/nbt-ref/vanilla1/world/dimensions/minecraft/overworld/region/r.0.-1.mca");
    let region = Region::open(&path).unwrap().with_coords(0,-1);
    let data = region.get_chunk(6,30).unwrap().unwrap();
    let nbt = read_nbt(&data).unwrap();
    let sections = match compound_get(&nbt.compound,"sections") {
        Some(Tag::List(List::Compound(l))) => l, _ => panic!()
    };
    let mut van = vec!["minecraft:air".to_string(); 98304];
    for sec in sections {
        let y_sec = match compound_get(sec,"Y") { Some(Tag::Byte(y))=>*y as i8 as i32, _=>continue };
        let Some(Tag::Compound(bs)) = compound_get(sec,"block_states") else {continue};
        let Some(Tag::List(List::Compound(palette))) = compound_get(bs,"palette") else {continue};
        let names: Vec<String> = palette.iter().map(|pc| match compound_get(pc,"Name") {
            Some(Tag::String(s))=>s.to_string(), _=>"minecraft:air".into()
        }).collect();
        let nstates = names.len();
        for i in 0..4096u32 {
            let name = if nstates==1 { names[0].clone() } else {
                let bits=((nstates-1).ilog2()+1).max(4) as u32;
                let Tag::LongArray(data)=compound_get(bs,"data").unwrap() else {panic!()};
                let longs:Vec<i64>=data.to_vec(); let epl=64/bits; let mask=(1u64<<bits)-1;
                let li=(i/epl) as usize; let bo=(i%epl)*bits;
                let idx=((longs[li] as u64)>>bo)&mask;
                names[idx as usize].clone()
            };
            let ly=(i>>8) as i32; let lz=((i>>4)&15) as usize; let lx=(i&15) as usize;
            let y=y_sec*16+ly;
            let idx=((y-WORLD_BOTTOM) as usize)*256+lz*16+lx;
            if idx<van.len() { van[idx]=name; }
        }
    }
    let gen = ChunkGenerator::new(12345);
    let chunk = gen.generate_chunk(6,-2);
    let mut shown=0;
    let mut neu_solid_van_air=0;
    let mut neu_air_van_solid=0;
    let mut fluid_mismatch=0;
    // sample density at mismatches
    let st = &gen.state;
    for y in WORLD_BOTTOM..320 {
        for z in 0..16usize {
            for x in 0..16usize {
                let idx=((y-WORLD_BOTTOM) as usize)*256+z*16+x;
                let vn=&van[idx];
                let nb=chunk.block_at(x as u32,y,z as u32);
                let vs=is_solid_name(vn);
                let ns=is_solid_neu(nb);
                let vf=is_fluid_name(vn);
                let nf=matches!(nb, BlockId::Water|BlockId::Lava);
                if vs==ns && vf==nf { continue; }
                if ns && !vs && !vf { neu_solid_van_air+=1; }
                if !ns && !nf && vs { neu_air_van_solid+=1; }
                if vf!=nf { fluid_mismatch+=1; }
                if shown<40 {
                    let wx=6*16+x as i32; let wz=-2*16+z as i32;
                    // eval final density naive (no grid interp) for debug
                    let mut env = DensityEnv::new(wx,y,wz, st.noises.noises());
                    let d = st.eval(&st.router.final_density, wx,y,wz);
                    println!("mismatch ({x},{y},{z}) world({wx},{y},{wz}) van={vn} neu={nb:?} density_direct={d:.6}");
                    shown+=1;
                }
            }
        }
    }
    println!("neu_solid_van_air={neu_solid_van_air} neu_air_van_solid={neu_air_van_solid} fluid_mismatch={fluid_mismatch}");
}
