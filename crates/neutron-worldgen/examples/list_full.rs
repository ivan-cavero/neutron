use neutron_world::nbt::ussr_nbt::owned::Tag;
use neutron_world::nbt::{compound_get, read_nbt};
use neutron_world::Region;
use std::collections::HashMap;
use std::path::PathBuf;
fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let base = args.first().map(|s| s.as_str()).unwrap_or("tools/nbt-ref/vanilla1/world/dimensions/minecraft/overworld/region");
    let mut by_status: HashMap<String, Vec<(i32, i32, usize)>> = HashMap::new();
    for rx in -1i32..=0 {
        for rz in -1i32..=0 {
            let path=PathBuf::from(format!("{base}/r.{rx}.{rz}.mca"));
            if !path.exists() {
                continue;
            }
            let region = Region::open(&path).unwrap().with_coords(rx, rz);
            for lz in 0..32i32 {
                for lx in 0..32i32 {
                    if let Ok(Some(data)) = region.get_chunk(lx, lz) {
                        if let Ok(nbt) = read_nbt(&data) {
                            let status = match compound_get(&nbt.compound, "Status") {
                                Some(Tag::String(s)) => s.to_string(),
                                _ => "unknown".into(),
                            };
                            let cx = rx * 32 + lx;
                            let cz = rz * 32 + lz;
                            by_status
                                .entry(status)
                                .or_default()
                                .push((cx, cz, data.len()));
                        }
                    }
                }
            }
        }
    }
    for (st, mut v) in by_status {
        v.sort_by_key(|c| -(c.2 as i64));
        println!(
            "{st}: count={} largest={} at ({},{})",
            v.len(),
            v[0].2,
            v[0].0,
            v[0].1
        );
        if st.contains("full")
            || st.contains("light")
            || st.contains("spawn")
            || st.contains("carver")
        {
            for c in v.iter().take(8) {
                println!("    ({},{}) {}b", c.0, c.1, c.2);
            }
        }
    }
}

