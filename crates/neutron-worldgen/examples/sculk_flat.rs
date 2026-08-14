// cargo run -p neutron-worldgen --example sculk_flat --release
fn main() {
    let (sculk, vein, growth, roll, draws) = neutron_worldgen::sculk::probe_flat_floor_patch();
    println!("sculk={sculk} vein={vein} growth={growth} catalyst_roll={roll:.6} draws={draws}");
}
