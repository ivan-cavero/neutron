//! Feature ports: desert_well / freeze_top_layer / spike / bamboo / monster_room.
use super::*;
use crate::feature_catalog;
use crate::feature_dispatch;
use crate::feature_dispatch::*;
use crate::feature_rng::FeatureRandom;
use crate::generator::{WORLD_BOTTOM, WORLD_TOP};
use crate::legacy_rng::LegacyRandom;
use crate::region_buf::RegionBuf;
use crate::surface::BlockId;
use crate::worldgen::WorldgenState;
use serde_json::Value;


// ---------------------------------------------------------------------------
// desert_well
// ---------------------------------------------------------------------------

/// `DesertWellFeature.place` (26.2). No RNG except the two suspicious-sand
/// picks at the end.
pub(crate) fn place_desert_well(
    rng: &mut FeatureRandom,
    region: &mut RegionBuf,
    x: i32,
    y: i32,
    z: i32,
) {
    let mut oy = y + 1;
    while region.get(x, oy, z).is_air() && oy > WORLD_BOTTOM + 2 {
        oy -= 1;
    }
    if region.get(x, oy, z) != BlockId::Sand {
        return;
    }
    for dx in -2..=2 {
        for dz in -2..=2 {
            if region.get(x + dx, oy - 1, z + dz).is_air()
                && region.get(x + dx, oy - 2, z + dz).is_air()
            {
                return;
            }
        }
    }
    for dy in -2..=0 {
        for dx in -2..=2 {
            for dz in -2..=2 {
                region.set(x + dx, oy + dy, z + dz, BlockId::Sandstone);
            }
        }
    }
    region.set(x, oy, z, BlockId::Water);
    for &(dx, dz) in &[(1, 0), (-1, 0), (0, 1), (0, -1)] {
        region.set(x + dx, oy, z + dz, BlockId::Water);
    }
    region.set(x, oy - 1, z, BlockId::Sand);
    for &(dx, dz) in &[(1, 0), (-1, 0), (0, 1), (0, -1)] {
        region.set(x + dx, oy - 1, z + dz, BlockId::Sand);
    }
    for dx in -2..=2 {
        for dz in -2..=2 {
            if dx == -2 || dx == 2 || dz == -2 || dz == 2 {
                region.set(x + dx, oy + 1, z + dz, BlockId::Sandstone);
            }
        }
    }
    region.set(x + 2, oy + 1, z, BlockId::SandstoneSlab);
    region.set(x - 2, oy + 1, z, BlockId::SandstoneSlab);
    region.set(x, oy + 1, z + 2, BlockId::SandstoneSlab);
    region.set(x, oy + 1, z - 2, BlockId::SandstoneSlab);
    for dx in -1..=1 {
        for dz in -1..=1 {
            if dx == 0 && dz == 0 {
                region.set(x + dx, oy + 4, z + dz, BlockId::Sandstone);
            } else {
                region.set(x + dx, oy + 4, z + dz, BlockId::SandstoneSlab);
            }
        }
    }
    for dy in 1..=3 {
        region.set(x - 1, oy + dy, z - 1, BlockId::Sandstone);
        region.set(x - 1, oy + dy, z + 1, BlockId::Sandstone);
        region.set(x + 1, oy + dy, z - 1, BlockId::Sandstone);
        region.set(x + 1, oy + dy, z + 1, BlockId::Sandstone);
    }
    let waters = [(x, oy, z), (x + 1, oy, z), (x, oy, z + 1), (x, oy, z - 1), (x - 1, oy, z)];
    let p1 = waters[rng.next_int(5) as usize];
    region.set(p1.0, p1.1 - 1, p1.2, BlockId::SuspiciousSand);
    let p2 = waters[rng.next_int(5) as usize];
    region.set(p2.0, p2.1 - 2, p2.2, BlockId::SuspiciousSand);
}

// ---------------------------------------------------------------------------
// freeze_top_layer
// ---------------------------------------------------------------------------

/// `SnowAndFreezeFeature.place` (26.2): 16×16 MOTION_BLOCKING columns; ice
/// below where `!warmEnoughToRain` on the biome at the TOP block, snow +
/// snowy-grass when `shouldSnow`.
///
/// ponytail: `Biome.getHeightAdjustedTemperature` (>snow-line noise term) and
/// the FROZEN temperature modifier are not applied (needs PerlinSimplexNoise);
/// exact below y=81 outside frozen oceans.
pub(crate) fn place_freeze_top_layer(
    region: &mut RegionBuf,
    state: &WorldgenState,
    x: i32,
    y: i32,
    z: i32,
) {
    let _ = y;
    for dx in 0..16 {
        for dz in 0..16 {
            let bx = x + dx;
            let bz = z + dz;
            let Some(sy) = heightmap_top(region, bx, bz, HeightmapKind::MotionBlocking) else {
                continue;
            };
            // Vanilla samples the biome AT topPos (level.getBiome(topPos)).
            let bid = crate::biome_manager::biome_id_at_block(state, bx, sy, bz);
            let name = crate::feature_dispatch::biome_id_to_name(bid);
            let (temperature, has_precip) = crate::feature_catalog::biome_climate(name);
            let warm_enough = temperature >= 0.15;
            // shouldFreeze(level, belowPos, false): water + !warmEnoughToRain
            // (block light < 10 is trivially true during worldgen).
            let below = sy - 1;
            if !warm_enough && region.get(bx, below, bz) == BlockId::Water {
                region.set(bx, below, bz, BlockId::Ice);
            }

            // shouldSnow(topPos): precipitation==SNOW && coldEnoughToSnow &&
            // (air | snow) && SNOW.canSurvive (solid ground below).
            let top = region.get(bx, sy, bz);
            let below_block = region.get(bx, below, bz);
            let ground = !below_block.is_air()
                && below_block != BlockId::Water
                && below_block != BlockId::Lava;
            if has_precip
                && !warm_enough
                && (top.is_air() || top == BlockId::Snow)
                && ground
            {
                region.set(bx, sy, bz, BlockId::Snow);
            }
        }
    }
}

/// `Biome.warmEnoughToRain` (base-temperature form): biome temperature at
/// `(x,y,z)` >= 0.15.
pub(crate) fn biome_warm_enough(state: &WorldgenState, x: i32, y: i32, z: i32) -> bool {
    let bid = crate::biome_manager::biome_id_at_block(state, x, y, z);
    let name = crate::feature_dispatch::biome_id_to_name(bid);
    let (temperature, _) = crate::feature_catalog::biome_climate(name);
    temperature >= 0.15
}

// ---------------------------------------------------------------------------
// spike (ice_spike)
// ---------------------------------------------------------------------------

/// `SpikeFeature.place` (26.2).
pub(crate) fn place_spike(
    rng: &mut FeatureRandom,
    region: &mut RegionBuf,
    x: i32,
    y: i32,
    z: i32,
    cfg: &Value,
) {
    let c = &cfg["config"];
    let state = c["state"]["Name"]
        .as_str()
        .and_then(BlockId::from_name)
        .unwrap_or(BlockId::PackedIce);
    let mut oy = y;
    while region.get(x, oy, z).is_air() && oy > WORLD_BOTTOM + 2 {
        oy -= 1;
    }
    if !eval_block_predicate(region, x, oy, z, &c["can_place_on"]) {
        return;
    }
    oy += rng.next_int(4);
    let height = rng.next_int(4) + 7;
    let mut width = height / 4 + rng.next_int(2);
    if width > 1 && rng.next_int(60) == 0 {
        oy += 10 + rng.next_int(30);
    }
    for y_off in 0..height {
        let scale = (1.0 - y_off as f32 / height as f32) * width as f32;
        let new_width = scale.ceil() as i32;
        for xo in -new_width..=new_width {
            let dx = xo.abs() as f32 - 0.25;
            for zo in -new_width..=new_width {
                let dz = zo.abs() as f32 - 0.25;
                let in_circle = (xo == 0 && zo == 0) || (dx * dx + dz * dz <= scale * scale);
                let edge = xo == -new_width
                    || xo == new_width
                    || zo == -new_width
                    || zo == new_width;
                if in_circle && (!edge || !(rng.next_f32() > 0.75)) {
                    let b = region.get(x + xo, oy + y_off, z + zo);
                    if b.is_air()
                        || eval_block_predicate(region, x + xo, oy + y_off, z + zo, &c["can_replace"])
                    {
                        region.set(x + xo, oy + y_off, z + zo, state);
                    }
                    if y_off != 0 && new_width > 1 {
                        let b2 = region.get(x + xo, oy - y_off, z + zo);
                        if b2.is_air()
                            || eval_block_predicate(region, x + xo, oy - y_off, z + zo, &c["can_replace"])
                        {
                            region.set(x + xo, oy - y_off, z + zo, state);
                        }
                    }
                }
            }
        }
    }
    let mut pillar_width = width - 1;
    if pillar_width < 0 {
        pillar_width = 0;
    } else if pillar_width > 1 {
        pillar_width = 1;
    }
    for xo in -pillar_width..=pillar_width {
        for zo in -pillar_width..=pillar_width {
            let mut cy = oy - 1;
            let mut run_length = 50;
            if xo.abs() == 1 && zo.abs() == 1 {
                run_length = rng.next_int(5);
            }
            while cy > 50 {
                let b = region.get(x + xo, cy, z + zo);
                if !(b.is_air()
                    || eval_block_predicate(region, x + xo, cy, z + zo, &c["can_replace"]))
                    && b != state
                {
                    break;
                }
                region.set(x + xo, cy, z + zo, state);
                cy -= 1;
                run_length -= 1;
                if run_length <= 0 {
                    cy -= rng.next_int(5) + 1;
                    run_length = rng.next_int(5);
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// bamboo
// ---------------------------------------------------------------------------

/// `BambooFeature.place` (26.2). Block states collapse to `BlockId::Bamboo`
/// (age/leaves/stage are palette properties only).
pub(crate) fn place_bamboo(
    rng: &mut FeatureRandom,
    region: &mut RegionBuf,
    x: i32,
    y: i32,
    z: i32,
    cfg: &Value,
) {
    let prob = cfg["config"]["probability"].as_f64().unwrap_or(0.0);
    // BambooStalkBlock.canSurvive: the block below must support bamboo
    // (approx: not air — matches vanilla behavior on the ground).
    if !region.get(x, y, z).is_air() || region.get(x, y - 1, z).is_air() {
        return;
    }
    let height = rng.next_int(12) + 5;
    if rng.next_f32() < prob as f32 {
        let r = rng.next_int(4) + 1;
        for xx in (x - r)..=(x + r) {
            for zz in (z - r)..=(z + r) {
                let xd = xx - x;
                let zd = zz - z;
                if xd * xd + zd * zd <= r * r {
                    if let Some(sy) = heightmap_top(region, xx, zz, HeightmapKind::WorldSurface) {
                        let py = sy - 1;
                        if is_in_tag(region.get(xx, py, zz), "#minecraft:beneath_bamboo_podzol_replaceable")
                        {
                            region.set(xx, py, zz, BlockId::Podzol);
                        }
                    }
                }
            }
        }
    }
    let mut by = y;
    for _ in 0..height {
        if !region.get(x, by, z).is_air() {
            break;
        }
        region.set(x, by, z, BlockId::Bamboo);
        by += 1;
    }
    if by - y >= 3 {
        region.set(x, by, z, BlockId::Bamboo);
        region.set(x, by - 1, z, BlockId::Bamboo);
        region.set(x, by - 2, z, BlockId::Bamboo);
    }
}

// ---------------------------------------------------------------------------
// monster_room
// ---------------------------------------------------------------------------

/// `MonsterRoomFeature.place` (26.2). Chest/spawner loot entities are not
/// modelled (block parity only).
pub(crate) fn place_monster_room(
    rng: &mut FeatureRandom,
    region: &mut RegionBuf,
    x: i32,
    y: i32,
    z: i32,
) {
    let xr = rng.next_int(2) + 2;
    let zr = rng.next_int(2) + 2;
    let min_x = -xr - 1;
    let max_x = xr + 1;
    let min_z = -zr - 1;
    let max_z = zr + 1;
    let mut hole_count = 0;
    for dx in min_x..=max_x {
        for dy in -1..=4 {
            for dz in min_z..=max_z {
                let solid_b = blocks_motion(region.get(x + dx, y + dy, z + dz));
                if dy == -1 && !solid_b {
                    return;
                }
                if dy == 4 && !solid_b {
                    return;
                }
                if (dx == min_x || dx == max_x || dz == min_z || dz == max_z)
                    && dy == 0
                    && region.get(x + dx, y + dy, z + dz).is_air()
                    && region.get(x + dx, y + dy + 1, z + dz).is_air()
                {
                    hole_count += 1;
                }
            }
        }
    }
    if !(1..=5).contains(&hole_count) {
        return;
    }
    for dx in min_x..=max_x {
        for dy in (3..=-1).rev() {
            for dz in min_z..=max_z {
                let is_wall = dx == min_x
                    || dy == -1
                    || dz == min_z
                    || dx == max_x
                    || dy == 4
                    || dz == max_z;
                if is_wall {
                    if y + dy >= WORLD_BOTTOM && !blocks_motion(region.get(x + dx, y + dy - 1, z + dz))
                    {
                        region.set(x + dx, y + dy, z + dz, BlockId::CaveAir);
                    } else {
                        let ws = region.get(x + dx, y + dy, z + dz);
                        if blocks_motion(ws) && ws != BlockId::Chest {
                            if dy == -1 && rng.next_int(4) != 0 {
                                region.set(x + dx, y + dy, z + dz, BlockId::MossyCobblestone);
                            } else {
                                region.set(x + dx, y + dy, z + dz, BlockId::Cobblestone);
                            }
                        }
                    }
                } else {
                    let ws = region.get(x + dx, y + dy, z + dz);
                    if ws != BlockId::Chest && ws != BlockId::Spawner {
                        region.set(x + dx, y + dy, z + dz, BlockId::CaveAir);
                    }
                }
            }
        }
    }
    'chest: for _ in 0..2 {
        for _ in 0..3 {
            let xc = x + rng.next_int(xr * 2 + 1) - xr;
            let zc = z + rng.next_int(zr * 2 + 1) - zr;
            if region.get(xc, y, zc).is_air() {
                let mut wall_count = 0;
                for &(dx, dz) in &[(1, 0), (-1, 0), (0, 1), (0, -1)] {
                    if blocks_motion(region.get(xc + dx, y, zc + dz)) {
                        wall_count += 1;
                    }
                }
                if wall_count == 1 {
                    region.set(xc, y, zc, BlockId::Chest);
                    break 'chest;
                }
            }
        }
    }
    region.set(x, y, z, BlockId::Spawner);
}
