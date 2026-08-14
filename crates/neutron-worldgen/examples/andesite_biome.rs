// Biome at the extra andesite_upper origin.
fn main() {
    let st = neutron_worldgen::worldgen::WorldgenState::overworld(12345);
    let x = 105;
    let y = 98;
    let z = -26;
    let id = neutron_worldgen::biome_source::biome_id_at_block(&st, x, y, z);
    println!("biome_id_at_block({x},{y},{z}) = {id}");
    // also sample a column
    for y in [64, 80, 90, 98, 110, 120, 130, 135, 140] {
        let id = neutron_worldgen::biome_source::biome_id_at_block(&st, x, y, z);
        println!("  y={y:3} id={id}");
    }
}
