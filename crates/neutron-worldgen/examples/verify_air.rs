use neutron_world::nbt::ussr_nbt::owned::{List, Tag};
use neutron_world::nbt::{compound_get, read_nbt};
use neutron_world::Region;
use std::path::PathBuf;
fn main() {
    let path = PathBuf::from("tools/nbt-ref/vanilla1/world/dimensions/minecraft/overworld/region/r.0.-1.mca");
    let region = Region::open(&path).unwrap().with_coords(0, -1);
    let data = region.get_chunk(6, 30).unwrap().unwrap();
    let nbt = read_nbt(&data).unwrap();
    let sections = match compound_get(&nbt.compound, "sections") {
        Some(Tag::List(List::Compound(l))) => l, _ => panic!()
    };
    // local (6,-41,6): section Y = -3 (since -41/16 = -2.56 -> floor -3), ly = -41 - (-48) = 7
    // section Y = floor(y/16): -41/16 = -2.5625, in integer div_euclid = -3. Yes. ly = -41 - (-48) = 7
    let lx=6u32; let ly=7u32; let lz=6u32;
    let i = (ly<<8)|(lz<<4)|lx;
    for sec in sections {
        let y_sec = match compound_get(sec, "Y") { Some(Tag::Byte(y))=>*y as i8 as i32, _=>continue };
        if y_sec != -3 { continue; }
        let Some(Tag::Compound(bs)) = compound_get(sec, "block_states") else {continue};
        let Some(Tag::List(List::Compound(palette))) = compound_get(bs, "palette") else {continue};
        println!("section Y=-3 palette ({} states):", palette.len());
        for (pi,pc) in palette.iter().enumerate() {
            if let Some(Tag::String(s)) = compound_get(pc, "Name") {
                println!("  [{pi}] {s}");
            }
        }
        let names: Vec<String> = palette.iter().map(|pc| match compound_get(pc, "Name") {
            Some(Tag::String(s))=>s.to_string(), _=>"?".into()
        }).collect();
        let nstates = names.len();
        let bits=((nstates-1).ilog2()+1).max(4) as u32;
        let Tag::LongArray(data)=compound_get(bs,"data").unwrap() else {panic!()};
        let longs:Vec<i64>=data.to_vec(); let epl=64/bits; let mask=(1u64<<bits)-1;
        let li=(i/epl) as usize; let bo=(i%epl)*bits;
        let idx=((longs[li] as u64)>>bo)&mask;
        println!("block index i={i} palette_idx={idx} name={}", names[idx as usize]);
        println!("longs[{li}]={:016x} bits={bits} epl={epl}", longs[li] as u64);
    }
}
