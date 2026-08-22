//! Mirror of ProbeHashOrder.java — prints tree::java_hash_order for an
//! x,y,z insertion list. Usage: hash_echo <inserts.txt>
use neutron_worldgen::tree::java_hash::java_hash_order;

fn main() {
    let path = std::env::args().nth(1).expect("input file");
    let text = std::fs::read_to_string(path).expect("read");
    let mut items = Vec::new();
    for l in text.lines() {
        let l = l.trim();
        if l.is_empty() {
            continue;
        }
        let mut it = l.split(',');
        let x: i32 = it.next().unwrap().parse().unwrap();
        let y: i32 = it.next().unwrap().parse().unwrap();
        let z: i32 = it.next().unwrap().parse().unwrap();
        items.push((x, y, z));
    }
    for (x, y, z) in java_hash_order(items) {
        println!("CELL {x},{y},{z}");
    }
}
