use neutron_worldgen::deco_schedule;

fn main() {
    let seq = deco_schedule::decorate_sequence();
    let mut rank = std::collections::HashMap::new();
    for (i, &p) in seq.iter().enumerate() {
        rank.insert(p, i as i64);
    }
    let csv = std::fs::read_to_string("/tmp/opencode/deco_pairs_424242.csv").expect("csv");
    let mut violation_votes: std::collections::HashMap<(i64, i64, i32, i32, i32, i32), i64> = std::collections::HashMap::new();
    for line in csv.lines().skip(1) {
        if line.starts_with('#') || line.starts_with("ccx") || line.trim().is_empty() {
            continue;
        }
        let f: Vec<&str> = line.split(',').collect();
        if f.len() < 8 { continue; }
        let w: (i32, i32) = (f[2].parse().unwrap(), f[3].parse().unwrap());
        let l: (i32, i32) = (f[5].parse().unwrap(), f[6].parse().unwrap());
        let (wr, lr) = match (rank.get(&w), rank.get(&l)) {
            (Some(a), Some(b)) => (*a, *b),
            _ => continue,
        };
        if wr <= lr {
            let dd = (w.0 - l.0, w.1 - l.1);
            *violation_votes.entry((wr, lr, dd.0, dd.1, w.1.abs(), l.1.abs())).or_insert(0) += 1;
        }
    }
    let mut vv: Vec<_> = violation_votes.iter().collect();
    vv.sort_by(|a, b| b.1.cmp(a.1));
    println!("distinct violating pairs: {}", vv.len());
    for ((wr, lr, ddx, ddz, _wz, _lz), v) in vv.iter().take(50) {
        println!("  wrank={wr} lrank={lr} rank_gap={} ddx={ddx} ddz={ddz} votes={v}", lr - wr);
    }
}
