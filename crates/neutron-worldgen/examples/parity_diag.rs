use neutron_worldgen::{ChunkGenerator, surface::BlockId};
fn main() {
    let gen = ChunkGenerator::new(12345);
    let chunk = gen.generate_chunk(6, -2);
    // print solid height and raw solid occupancy for Y bands
    let mut solid_below0 = 0u32;
    let mut solid_above0 = 0u32;
    let mut stone = 0u32;
    let mut air = 0u32;
    let mut water = 0u32;
    for y in -64..320 {
        for z in 0..16u32 {
            for x in 0..16u32 {
                let b = chunk.block_at(x, y, z);
                match b {
                    BlockId::Air => air += 1,
                    BlockId::Water | BlockId::Lava => water += 1,
                    BlockId::Stone => {
                        stone += 1;
                        if y < 0 { solid_below0 += 1; } else { solid_above0 += 1; }
                    }
                    _ => {
                        if y < 0 { solid_below0 += 1; } else { solid_above0 += 1; }
                    }
                }
            }
        }
    }
    println!("neutron air={air} stone={stone} water={water} solid_y<0={solid_below0} solid_y>=0={solid_above0}");
    // dump surface column map as CSV of heights
    print!("HEIGHTS:");
    for z in 0..16 {
        for x in 0..16 {
            print!(" {}", chunk.heightmap[z*16+x]);
        }
    }
    println!();
    // dump whether solid at (x,y,z) for y in 100..140 as bitstring per column - sample agreement
    // print block at surface and 5 below for a few columns
    for (x,z) in [(0u32,0u32),(5,5),(15,15),(10,3),(8,8)] {
        let h = chunk.heightmap[(z as usize)*16+(x as usize)];
        print!("col ({x},{z}) h={h}: ");
        for dy in 0..6 {
            let y = h as i32 - dy;
            print!("{:?} ", chunk.block_at(x, y, z));
        }
        println!();
    }
}
