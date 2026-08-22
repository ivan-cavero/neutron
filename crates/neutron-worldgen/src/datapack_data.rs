//! Embedded vanilla 26.2 datapack worldgen JSONs (`include_str!`).
//!
//! Source: the vanilla server jar's `data/minecraft/worldgen/` (26.2).
//! One macro invocation drives lookup AND enumeration: adding a file means
//! dropping it under `src/data/worldgen/` and appending one literal below.

macro_rules! embedded_jsons {
    ($($path:literal),* $(,)?) => {
        /// Every embedded datapack path, relative to `worldgen/`.
        pub const EMBEDDED_PATHS: &[&str] = &[$($path),*];

        /// Return the embedded JSON content for a datapack path
        /// (relative to `worldgen/`).
        pub fn datapack_json(path: &str) -> Option<&'static str> {
            match path {
                $($path => Some(include_str!(concat!("data/worldgen/", $path))),)*
                _ => None,
            }
        }
    };
}

embedded_jsons!(
    "density_function/end/base_3d_noise.json",
    "density_function/end/sloped_cheese.json",
    "density_function/nether/base_3d_noise.json",
    "density_function/overworld/base_3d_noise.json",
    "density_function/overworld/caves/entrances.json",
    "density_function/overworld/caves/noodle.json",
    "density_function/overworld/caves/pillars.json",
    "density_function/overworld/caves/spaghetti_2d.json",
    "density_function/overworld/caves/spaghetti_2d_thickness_modulator.json",
    "density_function/overworld/caves/spaghetti_roughness_function.json",
    "density_function/overworld/continents.json",
    "density_function/overworld/depth.json",
    "density_function/overworld/erosion.json",
    "density_function/overworld/factor.json",
    "density_function/overworld/jaggedness.json",
    "density_function/overworld/offset.json",
    "density_function/overworld/ridges.json",
    "density_function/overworld/ridges_folded.json",
    "density_function/overworld/sloped_cheese.json",
    "density_function/overworld_amplified/depth.json",
    "density_function/overworld_amplified/factor.json",
    "density_function/overworld_amplified/jaggedness.json",
    "density_function/overworld_amplified/offset.json",
    "density_function/overworld_amplified/sloped_cheese.json",
    "density_function/overworld_large_biomes/continents.json",
    "density_function/overworld_large_biomes/depth.json",
    "density_function/overworld_large_biomes/erosion.json",
    "density_function/overworld_large_biomes/factor.json",
    "density_function/overworld_large_biomes/jaggedness.json",
    "density_function/overworld_large_biomes/offset.json",
    "density_function/overworld_large_biomes/sloped_cheese.json",
    "density_function/shift_x.json",
    "density_function/shift_z.json",
    "density_function/y.json",
    "density_function/zero.json",
    "multi_noise_biome_source_parameter_list/nether.json",
    "multi_noise_biome_source_parameter_list/overworld.json",
    "noise/aquifer_barrier.json",
    "noise/aquifer_fluid_level_floodedness.json",
    "noise/aquifer_fluid_level_spread.json",
    "noise/aquifer_lava.json",
    "noise/badlands_pillar.json",
    "noise/badlands_pillar_roof.json",
    "noise/badlands_surface.json",
    "noise/calcite.json",
    "noise/cave_cheese.json",
    "noise/cave_entrance.json",
    "noise/cave_layer.json",
    "noise/clay_bands_offset.json",
    "noise/continentalness.json",
    "noise/continentalness_large.json",
    "noise/erosion.json",
    "noise/erosion_large.json",
    "noise/gravel.json",
    "noise/gravel_layer.json",
    "noise/ice.json",
    "noise/iceberg_pillar.json",
    "noise/iceberg_pillar_roof.json",
    "noise/iceberg_surface.json",
    "noise/jagged.json",
    "noise/nether/temperature.json",
    "noise/nether/vegetation.json",
    "noise/nether_state_selector.json",
    "noise/nether_wart.json",
    "noise/netherrack.json",
    "noise/noodle.json",
    "noise/noodle_ridge_a.json",
    "noise/noodle_ridge_b.json",
    "noise/noodle_thickness.json",
    "noise/offset.json",
    "noise/ore_gap.json",
    "noise/ore_vein_a.json",
    "noise/ore_vein_b.json",
    "noise/ore_veininess.json",
    "noise/packed_ice.json",
    "noise/patch.json",
    "noise/pillar.json",
    "noise/pillar_rareness.json",
    "noise/pillar_thickness.json",
    "noise/powder_snow.json",
    "noise/ridge.json",
    "noise/soul_sand_layer.json",
    "noise/spaghetti_2d.json",
    "noise/spaghetti_2d_elevation.json",
    "noise/spaghetti_2d_modulator.json",
    "noise/spaghetti_2d_thickness.json",
    "noise/spaghetti_3d_1.json",
    "noise/spaghetti_3d_2.json",
    "noise/spaghetti_3d_rarity.json",
    "noise/spaghetti_3d_thickness.json",
    "noise/spaghetti_roughness.json",
    "noise/spaghetti_roughness_modulator.json",
    "noise/sulfur_cave_gradient.json",
    "noise/surface.json",
    "noise/surface_secondary.json",
    "noise/surface_swamp.json",
    "noise/temperature.json",
    "noise/temperature_large.json",
    "noise/vegetation.json",
    "noise/vegetation_large.json",
    "noise_settings_overworld.json",
);

/// All embedded JSON paths (relative to worldgen/).
pub fn all_paths() -> &'static [&'static str] {
    EMBEDDED_PATHS
}
