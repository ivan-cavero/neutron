use neutron_world::{Region, nbt::{read_nbt, compound_get}};
use neutron_world::nbt::ussr_nbt::owned::Tag;
use neutron_worldgen::{ChunkGenerator};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let seed: i64 = args.get(1).map(|s| s.parse().unwrap()).unwrap_or(12345);
    let cx: i32 = args.get(2).map(|s| s.parse().unwrap()).unwrap_or(0);
    let cz: i32 = args.get(3).map(|s| s.parse().unwrap()).unwrap_or(0);

    // read vanilla chunk
    let region_path = format!("tools/nbt-ref/vanilla1/world/dimensions/minecraft/overworld/region/r.{}.{}.mca", cx.div_euclid(32), cz.div_euclid(32));
    let region = Region::open(std::path::Path::new(&region_path)).expect("open region");
    let data = region.get_chunk(cx.rem_euclid(32), cz.rem_euclid(32)).expect("get chunk").expect("chunk exists");
    let nbt = read_nbt(&data).expect("parse nbt");
    let hm = compound_get(&nbt.compound, "Heightmaps").expect("heightmaps");
    let hmc = match hm { Tag::Compound(c) => c, _ => panic!("heightmaps not compound") };
    println!("heightmap keys: {:?}", hmc.tags.iter().map(|(k, _)| k.to_string()).collect::<Vec<_>>());
    // Prefer MOTION_BLOCKING_NO_LEAVES so trees/leaves don't inflate vanilla
    // heights above the terrain surface Neutron generates (pre-features).
    let motion = compound_get(hmc, "MOTION_BLOCKING_NO_LEAVES")
        .or_else(|| compound_get(hmc, "OCEAN_FLOOR"))
        .or_else(|| compound_get(hmc, "MOTION_BLOCKING"))
        .expect("heightmap");
    let longs = match motion { Tag::LongArray(l) => l, _ => panic!("not longarray") };
    let longs: Vec<i64> = longs.to_vec();
    // unpack 9-bit: 256 values, 7 per long; packed = absoluteY+1 - minY
    let mut vanilla_heights = vec![0i32; 256];
    for i in 0..256 {
        let long_idx = i / 7;
        let bit = (i % 7) * 9;
        let v = (longs[long_idx] >> bit) & 0x1FF;
        vanilla_heights[i] = v as i32;
    }

    // my chunk
    let mut gen = ChunkGenerator::new(seed);
    let mine = gen.generate_chunk(cx, cz);

    // Vanilla packs heightmap as (absoluteY + 1 - minY) with minY=-64,
    // i.e. packed = absolute_solid_y + 1 + 64 = absolute_solid_y + 65.
    // Neutron stores absolute solid Y; comparable "Y+1 absolute" values:
    //   vanilla_abs_y1 = packed + minY = packed - 64
    //   neutron_abs_y1 = heightmap + 1
    const MIN_Y: i32 = -64;
    let mut same = 0; let mut diff = 0;
    for i in 0..256 {
        let vh = vanilla_heights[i] + MIN_Y; // absolute Y+1
        let mh = mine.heightmap[i] as i32 + 1;
        if vh == mh { same += 1 } else { diff += 1; }
    }
    println!("heightmap comparison (absolute Y+1): same={same} diff={diff}");
    if diff > 0 {
        let mut shown = 0;
        for i in 0..256 {
            let vh = vanilla_heights[i] + MIN_Y;
            let mh = mine.heightmap[i] as i32 + 1;
            if vh != mh && shown < 40 {
                println!(
                    "  col {i} (x={},z={}): vanilla={vh} mine={mh} (packed={})",
                    i % 16,
                    i / 16,
                    vanilla_heights[i]
                );
                shown += 1;
            }
        }
    }
}
