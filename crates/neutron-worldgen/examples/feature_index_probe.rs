// Print our FeatureSorter step-9 indices (vs reference index 17 for
// dark_forest_vegetation from ProbeVegPos).
fn main() {
    let list = neutron_worldgen::feature_catalog::features_per_step_at(9);
    for (i, f) in list.iter().enumerate() {
        if f.contains("forest") || f.contains("tree") || f.contains("litter") || f.contains("mushroom") || i < 5 {
            println!("{i:3} {f}");
        }
    }
    println!("---");
    println!(
        "dark_forest_vegetation = {:?}",
        neutron_worldgen::feature_catalog::global_feature_index(9, "dark_forest_vegetation")
    );
    println!(
        "trees_plains = {:?}",
        neutron_worldgen::feature_catalog::global_feature_index(9, "trees_plains")
    );
}
