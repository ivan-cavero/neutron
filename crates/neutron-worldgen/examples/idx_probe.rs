fn main() {
    println!(
        "idx_vein={:?}",
        neutron_worldgen::feature_catalog::global_feature_index(
            neutron_worldgen::feature_catalog::step::UNDERGROUND_DECORATION,
            "sculk_vein"
        )
    );
    println!(
        "idx_patch={:?}",
        neutron_worldgen::feature_catalog::global_feature_index(
            neutron_worldgen::feature_catalog::step::UNDERGROUND_DECORATION,
            "sculk_patch_deep_dark"
        )
    );
}
