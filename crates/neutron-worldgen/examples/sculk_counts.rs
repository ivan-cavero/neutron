// Count block types in .neufinal dumps (neutron) vs vanilla log states
use std::collections::HashMap;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    // args: <neufinal> <vanillaLog>
    let fin = std::fs::read(&args[1]).expect("final");
    let mut neu: HashMap<&str, u32> = HashMap::new();
    let mut i = 0;
    let y0 = -64i32;
    while i < fin.len() {
        let v = u16::from_le_bytes([fin[i], fin[i + 1]]);
        i += 2;
        let n = match v {
            74 => "sculk",
            76 => "sculk_vein",
            75 => "sculk_catalyst",
            77 => "sculk_sensor",
            _ => continue,
        };
        *neu.entry(n).or_insert(0) += 1;
        let _ = y0;
    }
    println!("neutron sculk-family counts: {:?}", neu);

    // vanilla log: count by state for cells inside center chunk only would need
    // coords; here just total per state across whole region log
    let mut van: HashMap<String, u32> = HashMap::new();
    for line in std::fs::read_to_string(&args[2]).unwrap().lines() {
        let p: Vec<&str> = line.split('|').collect();
        if p.len() >= 4 {
            let s = p[3].trim_start_matches("minecraft:");
            if s.contains("sculk") || s.contains("glow") || s == "vine" {
                *van.entry(s.to_string()).or_insert(0) += 1;
            }
        }
    }
    println!("vanilla sculk-family writes (all origins): {:?}", van);
}
