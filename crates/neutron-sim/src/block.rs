// Copyright (c) 2026 Neutron Contributors -- MIT License
//
// Block properties for the lighting engine.
//
// Maps block state IDs to their transparency and light emission values.
// Block IDs must match the vanilla block registry for Minecraft 26.2.

/// Check if a block is transparent to light.
///
/// Transparent blocks allow light to pass through, but may reduce the light
/// level by 1 (e.g., water, ice). Fully opaque blocks block light completely.
pub fn is_transparent(block_id: u16) -> bool {
    match block_id {
        // Air and gases
        0 |  // air
        959 // cave_air (same as air for lighting)
        => true,

        // Glass and panes
        20 |  // glass
        95 |  // stained_glass variants
        102 // glass_pane
        => true,

        // Water and ice
        50 |  // water
        54 |  // ice
        57 |  // packed_ice
        58 // blue_ice
        => true,

        // Leaves and vegetation
        41 |  // oak_leaves
        18 |  // grass
        31 |  // tall_grass
        37 |  // dandelion
        38 // poppy
        => true,

        // Torches and lanterns (transparent but emit light)
        76 |  // lantern
        242 // soul_lantern
        => true,

        // Signs and banners
        63 |  // oak_wall_sign
        68 // standing_sign
        => true,

        // Crops and saplings
        319 | // wheat
        336 // oak_sapling
        => true,

        // Carpet and snow layers
        171 | // carpet
        78 // snow_layer
        => true,

        // Redstone components (transparent)
        55 |  // redstone_wire
        69 |  // lever
        96 |  // wooden_door
        208 // tripwire
        => true,

        // Slabs and stairs (partial transparency — light passes through)
        126 | // wooden_slab
        128 // stone_stairs
        => true,

        // Fences and walls
        85 |  // fence
        139 // cobblestone_wall
        => true,

        // Rails
        27 |  // powered_rail
        28 |  // detector_rail
        66 // rail
        => true,

        // Pressure plates
        72 |  // wooden_pressure_plate
        278 // stone_pressure_plate
        => true,

        // Buttons
        77 |  // stone_button
        143 // wooden_button
        => true,

        // Anvils
        145 | // anvil
        146 // chipped_anvil
        => true,

        // Brewing stand, enchanting table, beacon
        117 | // brewing_stand
        116 | // enchanting_table
        138 // beacon
        => true,

        _ => false,
    }
}

/// Get the light level emitted by a block (0-15).
///
/// Light sources emit light that propagates outward, decreasing by 1 per block.
/// The maximum light level in Minecraft is 15 (e.g., glowstone, sea lantern).
pub fn light_emission(block_id: u16) -> u8 {
    match block_id {
        // Full brightness (15)
        11 |  // glowstone
        76 |  // lantern
        124 | // redstone_lamp
        138 | // beacon
        110 | // lava
        152 | // sea_lantern
        51 |  // fire
        834 | // campfire (lit)
        839 | // shroomlight
        831 | // conduit
        91 |  // jack_o_lantern
        874 | // crying_obsidian
        128 | // dragon_egg
        863 | // froglight (ochre)
        864 | // froglight (verdant)
        865 // froglight (pearlescent)
        => 15,

        // High brightness (14)
        50 |  // torch
        77 |  // soul_torch
        89 // end_rod
        => 14,

        // Medium brightness (7)
        75 |  // redstone_torch
        116 | // enchanting_table
        869 | // glow_lichen
        859 // cave_vines
        => 7,

        // Low brightness (6)
        861 // sculk_catalyst
        => 6,

        // Minimal brightness (1)
        117 | // brewing_stand
        39 // brown_mushroom
        => 1,

        _ => 0,
    }
}
