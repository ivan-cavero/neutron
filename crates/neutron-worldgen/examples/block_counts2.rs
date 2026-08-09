use neutron_worldgen::{ChunkGenerator, surface::BlockId};
use std::collections::HashMap;
fn main() {
    let gen = ChunkGenerator::new(12345);
    let chunk = gen.generate_chunk(6, -2);
    let mut c: HashMap<&str, u32> = HashMap::new();
    for y in -64..320 {
        for z in 0..16u32 {
            for x in 0..16u32 {
                let b = chunk.block_at(x,y,z);
                let name = format!("{:?}", b);
                *c.entry(Box::leak(name.into_boxed_str())).or_insert(0) += 1;
            }
        }
    }
    let mut v: Vec<_> = c.into_iter().collect();
    v.sort_by(|a,b| b.1.cmp(&a.1));
    for (n,k) in v {
        if k > 0 { println!("{n}: {k}"); }
    }
}
