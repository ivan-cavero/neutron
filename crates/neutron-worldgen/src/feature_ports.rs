//! T4 feature ports (run-058): the `KNOWN_NO_OP` whitelist → real dispatch.
//!
//! Each `place_*` mirrors the 26.2 decompiled `Feature.place` byte-for-byte
//! (same RNG draw order, same block rules). The generator currently runs steps
//! 6/7-sculk/8/9 only, so most of these place at steps 0-5/7/10 that are not
//! yet wired — they are correct but dormant until the step wiring lands
//! (run-049 finding: T4 ports do not move the 424242 measurement).
//!
//! Copyright (c) 2026 Neutron Contributors -- MIT License

use serde_json::Value;

use crate::feature_catalog;
use crate::feature_dispatch::{
    self, biome_name_at, block_from_to_place, blocks_motion, dispatch_configured,
    eval_block_predicate, heightmap_top, is_in_tag, is_solid_block, place_feature_ref,
    sample_height, sample_int_provider, HeightmapKind,
};
use crate::feature_rng::FeatureRandom;
use crate::generator::{WORLD_BOTTOM, WORLD_TOP};
use crate::region_buf::RegionBuf;
use crate::surface::BlockId;
use crate::worldgen::WorldgenState;

const SEA_LEVEL: i32 = 63;

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
    while region.get(x, oy, z) == BlockId::Air && oy > WORLD_BOTTOM + 2 {
        oy -= 1;
    }
    if region.get(x, oy, z) != BlockId::Sand {
        return;
    }
    for dx in -2..=2 {
        for dz in -2..=2 {
            if region.get(x + dx, oy - 1, z + dz) == BlockId::Air
                && region.get(x + dx, oy - 2, z + dz) == BlockId::Air
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
/// below where the biome would freeze, snow + snowy-grass on top.
///
/// ponytail: vanilla samples the per-biome `TEMPERATURE_NOISE` for
/// `coldEnoughToSnow`; we approximate with the router temperature function
/// (frozen biomes sit well below 0.15 there). Exact when the per-biome noise
/// is ported.
pub(crate) fn place_freeze_top_layer(
    region: &mut RegionBuf,
    state: &WorldgenState,
    x: i32,
    y: i32,
    z: i32,
) {
    for dx in 0..16 {
        for dz in 0..16 {
            let bx = x + dx;
            let bz = z + dz;
            let Some(sy) = heightmap_top(region, bx, bz, HeightmapKind::MotionBlocking) else {
                continue;
            };
            if cold_enough(state, bx, sy - 1, bz) && region.get(bx, sy - 1, bz) == BlockId::Water {
                region.set(bx, sy - 1, bz, BlockId::Ice);
            }
            let top = region.get(bx, sy, bz);
            if cold_enough(state, bx, sy, bz) && (top == BlockId::Air || top == BlockId::Snow) {
                region.set(bx, sy, bz, BlockId::Snow);
                // snowy grass: same block name, property only — no palette change.
            }
        }
    }
}

/// `Biome.coldEnoughToSnow` approximation: router temperature < 0.15.
pub(crate) fn cold_enough(state: &WorldgenState, x: i32, y: i32, z: i32) -> bool {
    let mut env = crate::density::DensityEnv::new(x, y, z, state.noises.noises());
    crate::density::compute(&state.router.temperature, &mut env) < 0.15
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
    while region.get(x, oy, z) == BlockId::Air && oy > WORLD_BOTTOM + 2 {
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
                    if b == BlockId::Air
                        || eval_block_predicate(region, x + xo, oy + y_off, z + zo, &c["can_replace"])
                    {
                        region.set(x + xo, oy + y_off, z + zo, state);
                    }
                    if y_off != 0 && new_width > 1 {
                        let b2 = region.get(x + xo, oy - y_off, z + zo);
                        if b2 == BlockId::Air
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
                if !(b == BlockId::Air
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
    if region.get(x, y, z) != BlockId::Air || region.get(x, y - 1, z) == BlockId::Air {
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
        if region.get(x, by, z) != BlockId::Air {
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
                    && region.get(x + dx, y + dy, z + dz) == BlockId::Air
                    && region.get(x + dx, y + dy + 1, z + dz) == BlockId::Air
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
                        region.set(x + dx, y + dy, z + dz, BlockId::Air);
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
                        region.set(x + dx, y + dy, z + dz, BlockId::Air);
                    }
                }
            }
        }
    }
    'chest: for _ in 0..2 {
        for _ in 0..3 {
            let xc = x + rng.next_int(xr * 2 + 1) - xr;
            let zc = z + rng.next_int(zr * 2 + 1) - zr;
            if region.get(xc, y, zc) == BlockId::Air {
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

// ---------------------------------------------------------------------------
// lake
// ---------------------------------------------------------------------------

/// `LakeFeature.place` (26.2). `state` is only used for the water-freeze pass.
pub(crate) fn place_lake(
    rng: &mut FeatureRandom,
    region: &mut RegionBuf,
    state: Option<&WorldgenState>,
    x: i32,
    y: i32,
    z: i32,
    cfg: &Value,
) {
    let c = &cfg["config"];
    if y <= WORLD_BOTTOM + 4 {
        return;
    }
    let ox = x - 8;
    let oy = y - 4;
    let oz = z - 8;
    let mut grid = vec![false; 16 * 8 * 16];
    let spots = rng.next_int(4) + 4;
    for _ in 0..spots {
        let xr = rng.next_f64() * 6.0 + 3.0;
        let yr = rng.next_f64() * 4.0 + 2.0;
        let zr = rng.next_f64() * 6.0 + 3.0;
        let xp = rng.next_f64() * (16.0 - xr - 2.0) + 1.0 + xr / 2.0;
        let yp = rng.next_f64() * (8.0 - yr - 4.0) + 2.0 + yr / 2.0;
        let zp = rng.next_f64() * (16.0 - zr - 2.0) + 1.0 + zr / 2.0;
        for xx in 1..15 {
            for zz in 1..15 {
                for yy in 1..7 {
                    let xd = (xx as f64 - xp) / (xr / 2.0);
                    let yd = (yy as f64 - yp) / (yr / 2.0);
                    let zd = (zz as f64 - zp) / (zr / 2.0);
                    let d = xd * xd + yd * yd + zd * zd;
                    if d < 1.0 {
                        grid[(xx * 16 + zz) * 8 + yy] = true;
                    }
                }
            }
        }
    }
    let Some(fluid) = block_from_to_place(rng, &c["fluid"]) else {
        return;
    };
    let edge = |xx: usize, zz: usize, yy: usize| -> bool {
        !grid[(xx * 16 + zz) * 8 + yy]
            && ((xx < 15 && grid[((xx + 1) * 16 + zz) * 8 + yy])
                || (xx > 0 && grid[((xx - 1) * 16 + zz) * 8 + yy])
                || (zz < 15 && grid[(xx * 16 + zz + 1) * 8 + yy])
                || (zz > 0 && grid[(xx * 16 + (zz - 1)) * 8 + yy])
                || (yy < 7 && grid[(xx * 16 + zz) * 8 + yy + 1])
                || (yy > 0 && grid[(xx * 16 + zz) * 8 + (yy - 1)]))
    };
    for xx in 0..16 {
        for zz in 0..16 {
            for yy in 0..8 {
                if edge(xx, zz, yy) {
                    let b = region.get(ox + xx as i32, oy + yy as i32, oz + zz as i32);
                    if yy >= 4 && b.is_fluid() {
                        return;
                    }
                    if yy < 4 && !blocks_motion(b) && b != fluid {
                        return;
                    }
                    if !eval_block_predicate(
                        region,
                        ox + xx as i32,
                        oy + yy as i32,
                        oz + zz as i32,
                        &c["can_place_feature"],
                    ) {
                        return;
                    }
                }
            }
        }
    }
    for xx in 0..16 {
        for zz in 0..16 {
            for yy in 0..8 {
                if grid[(xx * 16 + zz) * 8 + yy] {
                    let (px, py, pz) = (ox + xx as i32, oy + yy as i32, oz + zz as i32);
                    if eval_block_predicate(region, px, py, pz, &c["can_replace_with_air_or_fluid"]) {
                        let place_air = yy >= 4;
                        region.set(px, py, pz, if place_air { BlockId::Air } else { fluid });
                    }
                }
            }
        }
    }
    if let Some(barrier) = block_from_to_place(rng, &c["barrier"]) {
        if barrier != BlockId::Air {
            for xx in 0..16 {
                for zz in 0..16 {
                    for yy in 0..8 {
                        if edge(xx, zz, yy) && (yy < 4 || rng.next_int(2) != 0) {
                            let (px, py, pz) = (ox + xx as i32, oy + yy as i32, oz + zz as i32);
                            let b = region.get(px, py, pz);
                            if blocks_motion(b)
                                && eval_block_predicate(region, px, py, pz, &c["can_replace_with_barrier"])
                            {
                                region.set(px, py, pz, barrier);
                            }
                        }
                    }
                }
            }
        }
    }
    if fluid == BlockId::Water {
        if let Some(st) = state {
            for xx in 0..16 {
                for zz in 0..16 {
                    let (px, py, pz) = (ox + xx as i32, oy + 4, oz + zz as i32);
                    if cold_enough(st, px, py, pz)
                        && eval_block_predicate(region, px, py, pz, &c["can_replace_with_air_or_fluid"])
                    {
                        region.set(px, py, pz, BlockId::Ice);
                    }
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// sequence
// ---------------------------------------------------------------------------

/// `SequenceFeature.place` (26.2): run each sub placed-feature (placement
/// modifiers included) at the origin, stop on first failure.
pub(crate) fn place_sequence(
    rng: &mut FeatureRandom,
    region: &mut RegionBuf,
    state: Option<&WorldgenState>,
    x: i32,
    y: i32,
    z: i32,
    cfg: &Value,
    gen_step: i32,
) {
    let Some(features) = cfg["config"]["features"].as_array() else {
        return;
    };
    for f in features {
        if !place_inline_placed(rng, region, state, x, y, z, f, gen_step) {
            return;
        }
    }
}

/// Run one inline placed-feature object (full placement pipeline) at `(x,y,z)`.
/// Returns whether the feature placed (SequenceFeature short-circuits).
fn place_inline_placed(
    rng: &mut FeatureRandom,
    region: &mut RegionBuf,
    state: Option<&WorldgenState>,
    x: i32,
    y: i32,
    z: i32,
    placed: &Value,
    gen_step: i32,
) -> bool {
    let base_count = placed["placement"]
        .as_array()
        .map(|mods| {
            let mut product = 1i32;
            for m in mods {
                if m["type"].as_str() == Some("minecraft:count") {
                    product *= sample_int_provider(rng, &m["count"]).max(1);
                }
            }
            product.min(512)
        })
        .unwrap_or(1);
    let mut placed_any = false;
    for _ in 0..base_count {
        let mut px = x;
        let mut py = y;
        let mut pz = z;
        let mut ok = true;
        if let Some(mods) = placed["placement"].as_array() {
            for m in mods {
                match m["type"].as_str().unwrap_or("") {
                    "minecraft:count" => {}
                    "minecraft:in_square" => {
                        px = x + rng.next_int(16);
                        pz = z + rng.next_int(16);
                    }
                    "minecraft:height_range" => {
                        py = sample_height(rng, &m["height"]);
                    }
                    "minecraft:heightmap" => {
                        let kind = feature_dispatch::parse_heightmap_kind(
                            m["heightmap"].as_str().unwrap_or(""),
                        );
                        if let Some(sy) = heightmap_top(region, px, pz, kind) {
                            py = sy + 1;
                        } else {
                            ok = false;
                        }
                    }
                    "minecraft:random_offset" => {
                        px += sample_int_provider(rng, &m["xz_spread"]);
                        py += sample_int_provider(rng, &m["y_spread"]);
                        pz += sample_int_provider(rng, &m["xz_spread"]);
                    }
                    "minecraft:environment_scan" => {
                        let dir = m["direction_of_search"].as_str().unwrap_or("down");
                        let max_steps = m["max_steps"].as_i64().unwrap_or(12) as i32;
                        let target = &m["target_condition"];
                        let true_pred = serde_json::json!({"type":"minecraft:true"});
                        let allowed = m.get("allowed_search_condition").unwrap_or(&true_pred);
                        if !eval_block_predicate(region, px, py, pz, allowed) {
                            ok = false;
                            break;
                        }
                        let mut found = None;
                        let mut spy = py;
                        for _ in 0..max_steps {
                            if eval_block_predicate(region, px, spy, pz, target) {
                                found = Some(spy);
                                break;
                            }
                            spy += if dir == "down" { -1 } else { 1 };
                            if spy < WORLD_BOTTOM || spy > WORLD_TOP {
                                break;
                            }
                            if !eval_block_predicate(region, px, spy, pz, allowed) {
                                break;
                            }
                        }
                        if found.is_none() && eval_block_predicate(region, px, spy, pz, target) {
                            found = Some(spy);
                        }
                        match found {
                            Some(fy) => py = fy,
                            None => ok = false,
                        }
                    }
                    "minecraft:block_predicate_filter" => {
                        if !eval_block_predicate(region, px, py, pz, &m["predicate"]) {
                            ok = false;
                        }
                    }
                    "minecraft:rarity_filter" => {
                        let chance = m["chance"].as_i64().unwrap_or(1) as i32;
                        if chance <= 0 || rng.next_f32() >= 1.0 / chance as f32 {
                            ok = false;
                        }
                    }
                    "minecraft:surface_water_depth_filter" => {
                        let max = m["max_water_depth"].as_i64().unwrap_or(0) as i32;
                        if feature_dispatch::column_water_depth(region, px, pz) > max {
                            ok = false;
                        }
                    }
                    _ => {}
                }
            }
        }
        if !ok {
            continue;
        }
        let mut placed_one = false;
        if let Some(fid) = placed["feature"].as_str() {
            if let Some(cfg) = feature_catalog::load_configured_feature(fid) {
                dispatch_configured(rng, region, state, px, py, pz, &cfg, gen_step);
                placed_one = true;
            }
        } else if placed["feature"].is_object() {
            dispatch_configured(rng, region, state, px, py, pz, &placed["feature"], gen_step);
            placed_one = true;
        }
        placed_any |= placed_one;
    }
    placed_any
}

// ---------------------------------------------------------------------------
// speleothem_cluster
// ---------------------------------------------------------------------------

/// `SpeleothemClusterFeature.place` (26.2) — dripstone_cluster and
/// sulfur_spike_cluster share this algorithm.
pub(crate) fn place_speleothem_cluster(
    rng: &mut FeatureRandom,
    region: &mut RegionBuf,
    x: i32,
    y: i32,
    z: i32,
    cfg: &Value,
) {
    let c = &cfg["config"];
    if !is_empty_or_water(region, x, y, z) {
        return;
    }
    let height = sample_int_provider(rng, &c["height"]);
    let wetness = sample_float_provider(rng, &c["wetness"]);
    let density = sample_float_provider(rng, &c["density"]);
    let x_radius = sample_int_provider(rng, &c["radius"]);
    let z_radius = sample_int_provider(rng, &c["radius"]);
    for dx in -x_radius..=x_radius {
        for dz in -z_radius..=z_radius {
            let chance = chance_of_speleothem(x_radius, z_radius, dx, dz, c);
            place_cluster_column(
                rng, region, x + dx, y, z + dz, dx, dz, wetness, chance, height, density, c,
            );
        }
    }
}

fn chance_of_speleothem(x_radius: i32, z_radius: i32, dx: i32, dz: i32, c: &Value) -> f64 {
    let max_edge = c["max_distance_from_edge_affecting_chance_of_speleothem"]
        .as_f64()
        .unwrap_or(3.0);
    let at_max = c["chance_of_speleothem_at_max_distance_from_center"]
        .as_f64()
        .unwrap_or(0.1);
    let dist_from_edge = (x_radius - dx.abs()).min(z_radius - dz.abs());
    clamped_map(dist_from_edge as f64, 0.0, max_edge, at_max, 1.0)
}

fn place_cluster_column(
    rng: &mut FeatureRandom,
    region: &mut RegionBuf,
    x: i32,
    origin_y: i32,
    z: i32,
    dx: i32,
    dz: i32,
    chance_of_water: f32,
    chance_of_speleothem: f64,
    cluster_height: i32,
    density: f32,
    c: &Value,
) {
    let search_range = c["floor_to_ceiling_search_range"].as_i64().unwrap_or(12) as i32;
    let Some((ceiling, floor)) = scan_column(region, x, z, origin_y, search_range) else {
        return;
    };
    if ceiling.is_none() && floor.is_none() {
        return;
    }
    let base_block = c["base_block"]["Name"]
        .as_str()
        .and_then(BlockId::from_name)
        .unwrap_or(BlockId::DripstoneBlock);
    let pointed_block = c["pointed_block"]["Name"]
        .as_str()
        .and_then(BlockId::from_name)
        .unwrap_or(BlockId::PointedDripstone);
    let replaceable = c["replaceable_blocks"].as_str().unwrap_or("");
    let max_diff = c["max_stalagmite_stalactite_height_diff"]
        .as_i64()
        .unwrap_or(1) as i32;
    let thickness_provider = &c["speleothem_block_layer_thickness"];

    let want_pool = rng.next_f32() < chance_of_water;
    let mut floor = floor;
    if want_pool && floor.is_some() && can_place_pool(region, x, floor.unwrap(), z, c, base_block, pointed_block) {
        let fy = floor.unwrap();
        floor = Some(fy - 1);
        region.set(x, fy, z, BlockId::Water);
    }

    let want_stalactite = rng.next_f64() < chance_of_speleothem;
    let mut stalactite_height = 0;
    if let Some(cy) = ceiling {
        if want_stalactite && region.get(x, cy, z) != BlockId::Lava {
            let thickness = sample_int_provider(rng, thickness_provider);
            replace_with_base(region, x, cy, z, thickness, 1, base_block, replaceable);
            let max_h = match floor {
                Some(fy) => cluster_height.min(cy - fy),
                None => cluster_height,
            };
            stalactite_height = speleothem_height(rng, dx, dz, density, max_h, c);
        }
    }
    let want_stalagmite = rng.next_f64() < chance_of_speleothem;
    let mut stalagmite_height = 0;
    if let Some(fy) = floor {
        if want_stalagmite && region.get(x, fy, z) != BlockId::Lava {
            let thickness = sample_int_provider(rng, thickness_provider);
            replace_with_base(region, x, fy, z, thickness, -1, base_block, replaceable);
            if ceiling.is_some() {
                stalagmite_height = (stalactite_height
                    + rng.next_int(max_diff * 2 + 1) - max_diff)
                    .max(0);
            } else {
                stalagmite_height = speleothem_height(rng, dx, dz, density, cluster_height, c);
            }
        }
    }

    let (actual_stalactite, actual_stalagmite) =
        if let (Some(cy), Some(fy)) = (ceiling, floor) {
            if cy - stalactite_height <= fy + stalagmite_height {
                let lowest_bottom = (cy - stalactite_height).max(fy + 1);
                let highest_top = (fy + stalagmite_height).min(cy - 1);
                let actual_bottom = rng.next_int(highest_top - lowest_bottom + 2) + lowest_bottom;
                let actual_top = actual_bottom - 1;
                (cy - actual_bottom, actual_top - fy)
            } else {
                (stalactite_height, stalagmite_height)
            }
        } else {
            (stalactite_height, stalagmite_height)
        };
    let column_height = ceiling.and_then(|cy| floor.map(|fy| cy - fy));
    let merge_tips = rng.next_boolean()
        && actual_stalactite > 0
        && actual_stalagmite > 0
        && column_height.is_some()
        && actual_stalactite + actual_stalagmite == column_height.unwrap();
    if let Some(cy) = ceiling {
        grow_speleothem(region, x, cy - 1, z, -1, actual_stalactite, merge_tips, base_block, pointed_block, replaceable);
    }
    if let Some(fy) = floor {
        grow_speleothem(region, x, fy + 1, z, 1, actual_stalagmite, merge_tips, base_block, pointed_block, replaceable);
    }
}

/// `Column.scan`: ceiling = first non-empty going up, floor = first non-empty
/// going down (both within `search_range`), starting at the origin y.
/// Returns None when the origin is not inside the column.
fn scan_column(
    region: &RegionBuf,
    x: i32,
    z: i32,
    origin_y: i32,
    search_range: i32,
) -> Option<(Option<i32>, Option<i32>)> {
    if !is_empty_or_water(region, x, origin_y, z) {
        return None;
    }
    let mut y = origin_y;
    let mut i = 1;
    while i < search_range && is_empty_or_water(region, x, y, z) {
        y += 1;
        i += 1;
    }
    let ceiling = if is_neither_empty_nor_water(region, x, y, z) {
        Some(y)
    } else {
        None
    };
    let mut y = origin_y;
    let mut i = 1;
    while i < search_range && is_empty_or_water(region, x, y, z) {
        y -= 1;
        i += 1;
    }
    let floor = if is_neither_empty_nor_water(region, x, y, z) {
        Some(y)
    } else {
        None
    };
    Some((ceiling, floor))
}

fn is_empty_or_water(region: &RegionBuf, x: i32, y: i32, z: i32) -> bool {
    if y < WORLD_BOTTOM || y >= WORLD_TOP {
        return false;
    }
    let b = region.get(x, y, z);
    b == BlockId::Air || b == BlockId::Water
}

fn is_neither_empty_nor_water(region: &RegionBuf, x: i32, y: i32, z: i32) -> bool {
    if y < WORLD_BOTTOM || y >= WORLD_TOP {
        return false;
    }
    let b = region.get(x, y, z);
    b != BlockId::Air && b != BlockId::Water
}

fn can_place_pool(
    region: &RegionBuf,
    x: i32,
    y: i32,
    z: i32,
    c: &Value,
    base_block: BlockId,
    pointed_block: BlockId,
) -> bool {
    let b = region.get(x, y, z);
    if b == BlockId::Water || b == base_block || b == pointed_block {
        return false;
    }
    if region.get(x, y + 1, z) == BlockId::Water {
        return false;
    }
    for &(dx, dz) in &[(1, 0), (-1, 0), (0, 1), (0, -1)] {
        if !can_be_adjacent_to_water(region, x + dx, y, z + dz) {
            return false;
        }
    }
    can_be_adjacent_to_water(region, x, y - 1, z)
}

fn can_be_adjacent_to_water(region: &RegionBuf, x: i32, y: i32, z: i32) -> bool {
    if y < WORLD_BOTTOM || y >= WORLD_TOP {
        return false;
    }
    let b = region.get(x, y, z);
    is_in_tag(b, "#minecraft:base_stone_overworld") || b == BlockId::Water
}

fn replace_with_base(
    region: &mut RegionBuf,
    x: i32,
    y: i32,
    z: i32,
    max_count: i32,
    dir: i32,
    base_block: BlockId,
    replaceable: &str,
) {
    let mut py = y;
    for _ in 0..max_count {
        if !place_base_if_possible(region, x, py, z, base_block, replaceable) {
            return;
        }
        py += dir;
    }
}

fn place_base_if_possible(
    region: &mut RegionBuf,
    x: i32,
    y: i32,
    z: i32,
    base_block: BlockId,
    replaceable: &str,
) -> bool {
    let b = region.get(x, y, z);
    if is_replaceable_by(b, replaceable) {
        region.set(x, y, z, base_block);
        true
    } else {
        false
    }
}

fn is_replaceable_by(b: BlockId, replaceable: &str) -> bool {
    let t = replaceable.strip_prefix("#minecraft:").unwrap_or(replaceable);
    match t {
        "dripstone_replaceable_blocks" => is_in_tag(b, "#minecraft:base_stone_overworld"),
        "sulfur_spike_replaceable_blocks" => {
            matches!(b, BlockId::Sulfur | BlockId::Cinnabar)
        }
        _ => false,
    }
}

fn speleothem_height(
    rng: &mut FeatureRandom,
    dx: i32,
    dz: i32,
    density: f32,
    max_height: i32,
    c: &Value,
) -> i32 {
    if rng.next_f32() > density {
        return 0;
    }
    let dist = dx.abs() + dz.abs();
    let max_bias = c["max_distance_from_center_affecting_height_bias"]
        .as_f64()
        .unwrap_or(8.0);
    let dev = c["height_deviation"].as_f64().unwrap_or(3.0);
    let mean = clamped_map(dist as f64, 0.0, max_bias, max_height as f64 / 2.0, 0.0);
    // ClampedNormalFloat.sample(random, mean, dev, 0, maxHeight)
    let g = rng.next_gaussian() * dev + mean;
    (g.clamp(0.0, max_height as f64)) as i32
}

fn grow_speleothem(
    region: &mut RegionBuf,
    x: i32,
    y: i32,
    z: i32,
    dir: i32,
    height: i32,
    merged_tip: bool,
    base_block: BlockId,
    pointed_block: BlockId,
    replaceable: &str,
) {
    // isBase(state at startPos.relative(tipDirection.opposite))
    let base_y = y - dir;
    let b = region.get(x, base_y, z);
    let is_base = b == base_block || is_replaceable_by(b, replaceable);
    if !is_base {
        return;
    }
    let mut py = y;
    let mut remaining = height;
    if remaining >= 3 {
        region.set(x, py, z, pointed_block);
        py += dir;
        for _ in 0..(remaining - 3) {
            region.set(x, py, z, pointed_block);
            py += dir;
        }
        remaining = 2; // FRUSTUM + TIP
    }
    if remaining >= 2 {
        region.set(x, py, z, pointed_block);
        py += dir;
        remaining = 1;
    }
    if remaining >= 1 {
        region.set(x, py, z, pointed_block);
    }
}

// ---------------------------------------------------------------------------
// large_dripstone
// ---------------------------------------------------------------------------

/// `LargeDripstoneFeature.place` (26.2).
pub(crate) fn place_large_dripstone(
    rng: &mut FeatureRandom,
    region: &mut RegionBuf,
    x: i32,
    y: i32,
    z: i32,
    cfg: &Value,
) {
    let c = &cfg["config"];
    if !is_empty_or_water(region, x, y, z) {
        return;
    }
    let search_range = c["floor_to_ceiling_search_range"].as_i64().unwrap_or(12) as i32;
    let Some((ceiling, floor)) = scan_column(region, x, z, y, search_range) else {
        return;
    };
    let (Some(cy), Some(fy)) = (ceiling, floor) else {
        return;
    };
    let column_height = cy - fy;
    if column_height < 4 {
        return;
    }
    let ratio = c["max_column_radius_to_cave_height_ratio"].as_f64().unwrap_or(0.33);
    let radius_min = c["column_radius"]["min_inclusive"].as_i64().unwrap_or(3) as i32;
    let radius_max = c["column_radius"]["max_inclusive"].as_i64().unwrap_or(16) as i32;
    let max_from_height = ((column_height as f64) * ratio) as i32;
    let max_radius = max_from_height.clamp(radius_min, radius_max);
    let radius = rng.next_int(max_radius - radius_min + 1) + radius_min;

    let stal_blunt = sample_float_provider(rng, &c["stalactite_bluntness"]);
    let stalag_blunt = sample_float_provider(rng, &c["stalagmite_bluntness"]);
    let height_scale = sample_float_provider(rng, &c["height_scale"]);
    let mut stalactite = LargeDripstone::new(
        x,
        cy - 1,
        z,
        false,
        radius,
        stal_blunt as f64,
        height_scale as f64,
    );
    let mut stalagmite = LargeDripstone::new(
        x,
        fy + 1,
        z,
        true,
        radius,
        stalag_blunt as f64,
        height_scale as f64,
    );

    let min_wind_radius = c["min_radius_for_wind"].as_i64().unwrap_or(4) as i32;
    let min_wind_blunt = c["min_bluntness_for_wind"].as_f64().unwrap_or(0.6);
    let wind = if stalactite.is_suitable_for_wind(min_wind_radius, min_wind_blunt)
        && stalagmite.is_suitable_for_wind(min_wind_radius, min_wind_blunt)
    {
        WindOffsetter::new(y, rng, &c["wind_speed"], 16 - radius)
    } else {
        WindOffsetter::no_wind()
    };
    let ok1 = stalactite.move_back_into_stone(region, &wind);
    let ok2 = stalagmite.move_back_into_stone(region, &wind);
    if ok1 {
        stalactite.place_blocks(rng, region, &wind);
    }
    if ok2 {
        stalagmite.place_blocks(rng, region, &wind);
    }
}

struct LargeDripstone {
    root_x: i32,
    root_y: i32,
    root_z: i32,
    pointing_up: bool,
    radius: i32,
    bluntness: f64,
    scale: f64,
}

impl LargeDripstone {
    fn new(
        x: i32,
        y: i32,
        z: i32,
        pointing_up: bool,
        radius: i32,
        bluntness: f64,
        scale: f64,
    ) -> Self {
        Self {
            root_x: x,
            root_y: y,
            root_z: z,
            pointing_up,
            radius,
            bluntness,
            scale,
        }
    }

    fn height(&self) -> i32 {
        self.height_at_radius(0.0)
    }

    fn height_at_radius(&self, check_radius: f32) -> i32 {
        speleothem_height_formula(check_radius as f64, self.radius as f64, self.scale, self.bluntness)
            as i32
    }

    fn is_suitable_for_wind(&self, min_radius: i32, min_blunt: f64) -> bool {
        self.radius >= min_radius && self.bluntness >= min_blunt
    }

    fn move_back_into_stone(&mut self, region: &RegionBuf, wind: &WindOffsetter) -> bool {
        while self.radius > 1 {
            let mut new_root_y = self.root_y;
            let max_tries = 10.min(self.height());
            for _ in 0..max_tries {
                if region.get(self.root_x, new_root_y, self.root_z) == BlockId::Lava {
                    return false;
                }
                let (wx, wz) = wind.offset(self.root_x, new_root_y, self.root_z);
                if circle_mostly_embedded(region, wx, new_root_y, wz, self.radius) {
                    self.root_y = new_root_y;
                    return true;
                }
                new_root_y += if self.pointing_up { -1 } else { 1 };
            }
            self.radius /= 2;
        }
        false
    }

    fn place_blocks(&self, rng: &mut FeatureRandom, region: &mut RegionBuf, wind: &WindOffsetter) {
        for dx in -self.radius..=self.radius {
            for dz in -self.radius..=self.radius {
                let current_radius = ((dx * dx + dz * dz) as f32).sqrt();
                if current_radius > self.radius as f32 {
                    continue;
                }
                let mut height = self.height_at_radius(current_radius);
                if height > 0 {
                    if rng.next_f32() < 0.2 {
                        let f = 0.8 + rng.next_f32() * 0.2;
                        height = (height as f32 * f) as i32;
                    }
                    let mut py = self.root_y;
                    let mut has_been_out_of_stone = false;
                    let max_y = if self.pointing_up {
                        heightmap_top(region, self.root_x + dx, self.root_z + dz, HeightmapKind::WorldSurface)
                            .unwrap_or(WORLD_TOP)
                    } else {
                        i32::MAX
                    };
                    for _ in 0..height {
                        if py >= max_y {
                            break;
                        }
                        let (wx, wz) = wind.offset(self.root_x + dx, py, self.root_z + dz);
                        let b = region.get(wx, py, wz);
                        if b == BlockId::Air || b == BlockId::Water || b == BlockId::Lava {
                            has_been_out_of_stone = true;
                            region.set(wx, py, wz, BlockId::DripstoneBlock);
                        } else if has_been_out_of_stone
                            && is_in_tag(b, "#minecraft:base_stone_overworld")
                        {
                            break;
                        }
                        py += if self.pointing_up { 1 } else { -1 };
                    }
                }
            }
        }
    }
}

struct WindOffsetter {
    origin_y: i32,
    wind_speed: Option<(f64, f64)>, // (x, z)
    max_offset: i32,
}

impl WindOffsetter {
    fn new(origin_y: i32, rng: &mut FeatureRandom, speed_provider: &Value, max_offset: i32) -> Self {
        let speed = sample_float_provider(rng, speed_provider) as f64;
        let direction = rng.next_f32() * std::f32::consts::PI as f32;
        let (s, c) = direction.sin_cos();
        Self {
            origin_y,
            wind_speed: Some((c as f64 * speed, s as f64 * speed)),
            max_offset,
        }
    }

    fn no_wind() -> Self {
        Self {
            origin_y: 0,
            wind_speed: None,
            max_offset: 0,
        }
    }

    fn offset(&self, x: i32, y: i32, z: i32) -> (i32, i32) {
        match self.wind_speed {
            None => (x, z),
            Some((sx, sz)) => {
                let dy = (self.origin_y - y) as f64;
                let dx = (sx * dy).floor().clamp(-self.max_offset as f64, self.max_offset as f64) as i32;
                let dz = (sz * dy).floor().clamp(-self.max_offset as f64, self.max_offset as f64) as i32;
                (x + dx, z + dz)
            }
        }
    }
}

fn circle_mostly_embedded(region: &RegionBuf, x: i32, y: i32, z: i32, radius: i32) -> bool {
    let center = region.get(x, y, z);
    if center == BlockId::Air || center == BlockId::Water || center == BlockId::Lava {
        return false;
    }
    let arc_length = 6.0f32;
    let angle_increment = 6.0f32 / radius as f32;
    let mut angle = 0.0f32;
    while angle < std::f32::consts::PI * 2.0 {
        let c = angle.cos();
        let s = angle.sin();
        let dx = (c * radius as f32) as i32;
        let dz = (s * radius as f32) as i32;
        let b = region.get(x + dx, y, z + dz);
        if b == BlockId::Air || b == BlockId::Water || b == BlockId::Lava {
            return false;
        }
        angle += angle_increment;
    }
    true
}

/// `SpeleothemUtils.getSpeleothemHeight`.
fn speleothem_height_formula(xz_dist: f64, radius: f64, scale: f64, bluntness: f64) -> f64 {
    let mut d = xz_dist;
    if d < bluntness {
        d = bluntness;
    }
    let r = d / radius * 0.384;
    let part1 = 0.75 * r.powf(1.3333333333333333);
    let part2 = r.powf(0.6666666666666666);
    let part3 = 0.3333333333333333 * r.ln();
    let h = (scale * (part1 - part2 - part3)).max(0.0);
    h / 0.384 * radius
}

// ---------------------------------------------------------------------------
// iceberg
// ---------------------------------------------------------------------------

/// `IcebergFeature.place` (26.2). `state` is the configured block
/// (blue_ice / packed_ice); snow_on_top uses `BlockId::Snow`.
pub(crate) fn place_iceberg(
    rng: &mut FeatureRandom,
    region: &mut RegionBuf,
    x: i32,
    z: i32,
    cfg: &Value,
) {
    let main_state = cfg["config"]["state"]["Name"]
        .as_str()
        .and_then(BlockId::from_name)
        .unwrap_or(BlockId::PackedIce);
    let origin_y = SEA_LEVEL;
    let snow_on_top = rng.next_f64() > 0.7;
    let shape_angle = rng.next_f64() * 2.0 * std::f64::consts::PI;
    let shape_ellipse_a = 11 - rng.next_int(5);
    let shape_ellipse_c = 3 + rng.next_int(3);
    let is_ellipse = rng.next_f64() > 0.7;
    let mut over_water_height = if is_ellipse {
        rng.next_int(6) + 6
    } else {
        rng.next_int(15) + 3
    };
    if !is_ellipse && rng.next_f64() > 0.9 {
        over_water_height += rng.next_int(19) + 7;
    }
    let under_water_height = (over_water_height + rng.next_int(11)).min(18);
    let width = (over_water_height + rng.next_int(7) - rng.next_int(5)).min(11);
    let a = if is_ellipse { shape_ellipse_a } else { 11 };

    for xo in -a..a {
        for zo in -a..a {
            for y_off in 0..over_water_height {
                let radius = if is_ellipse {
                    height_dependent_radius_ellipse(y_off, over_water_height, width)
                } else {
                    height_dependent_radius_round(rng, y_off, over_water_height, width)
                };
                if is_ellipse || xo < radius {
                    generate_iceberg_block(
                        rng,
                        region,
                        x,
                        origin_y,
                        z,
                        over_water_height,
                        xo,
                        y_off,
                        zo,
                        radius,
                        a,
                        is_ellipse,
                        shape_ellipse_c,
                        shape_angle,
                        snow_on_top,
                        main_state,
                    );
                }
            }
        }
    }
    smooth_iceberg(region, x, origin_y, z, width, over_water_height, is_ellipse, shape_ellipse_a);
    for xo in -a..a {
        for zo in -a..a {
            for y_off in -1..-under_water_height {
                let new_a = if is_ellipse {
                    ((a as f32) * (1.0 - (y_off * y_off) as f32 / (under_water_height * 8) as f32)).ceil() as i32
                } else {
                    a
                };
                let radius = height_dependent_radius_steep(rng, -y_off, under_water_height, width);
                if xo < radius {
                    generate_iceberg_block(
                        rng,
                        region,
                        x,
                        origin_y,
                        z,
                        under_water_height,
                        xo,
                        y_off,
                        zo,
                        radius,
                        new_a,
                        is_ellipse,
                        shape_ellipse_c,
                        shape_angle,
                        snow_on_top,
                        main_state,
                    );
                }
            }
        }
    }
    let do_cut_out = if is_ellipse {
        rng.next_f64() > 0.1
    } else {
        rng.next_f64() > 0.7
    };
    if do_cut_out {
        generate_cut_out(
            rng,
            region,
            x,
            origin_y,
            z,
            width,
            over_water_height,
            is_ellipse,
            shape_ellipse_a,
            shape_angle,
            shape_ellipse_c,
        );
    }
}

fn height_dependent_radius_round(rng: &mut FeatureRandom, y_off: i32, height: i32, width: i32) -> i32 {
    let k = 3.5 - rng.next_f32();
    let mut scale = (1.0 - (y_off * y_off) as f32 / (height as f32 * k)) * width as f32;
    if height > 15 + rng.next_int(5) {
        let temp_y_off = if y_off < 3 + rng.next_int(6) { y_off / 2 } else { y_off };
        scale = (1.0 - temp_y_off as f32 / (height as f32 * k * 0.4)) * width as f32;
    }
    (scale / 2.0).ceil() as i32
}

fn height_dependent_radius_ellipse(y_off: i32, height: i32, width: i32) -> i32 {
    let scale = (1.0 - (y_off * y_off) as f32 / height as f32) * width as f32;
    (scale / 2.0).ceil() as i32
}

fn height_dependent_radius_steep(rng: &mut FeatureRandom, y_off: i32, height: i32, width: i32) -> i32 {
    let k = 1.0 + rng.next_f32() / 2.0;
    let scale = (1.0 - y_off as f32 / (height as f32 * k)) * width as f32;
    (scale / 2.0).ceil() as i32
}

fn generate_iceberg_block(
    rng: &mut FeatureRandom,
    region: &mut RegionBuf,
    x: i32,
    origin_y: i32,
    z: i32,
    height: i32,
    xo: i32,
    y_off: i32,
    zo: i32,
    radius: i32,
    a: i32,
    is_ellipse: bool,
    shape_ellipse_c: i32,
    shape_angle: f64,
    snow_on_top: bool,
    main_state: BlockId,
) {
    let signed_dist = if is_ellipse {
        let c = get_ellipse_c(y_off, height, shape_ellipse_c);
        signed_distance_ellipse(xo, zo, a, c, shape_angle)
    } else {
        signed_distance_circle(rng, xo, zo, radius)
    };
    if signed_dist < 0.0 {
        let compare_val = if is_ellipse { -0.5 } else { -6.0 - rng.next_int(3) as f64 };
        if signed_dist > compare_val && rng.next_f64() > 0.9 {
            return;
        }
        set_iceberg_block(
            region,
            x + xo,
            origin_y + y_off,
            z + zo,
            rng,
            height - y_off,
            height,
            is_ellipse,
            snow_on_top,
            main_state,
        );
    }
}

fn get_ellipse_c(y_off: i32, height: i32, shape_ellipse_c: i32) -> i32 {
    let mut c = shape_ellipse_c;
    if y_off > 0 && height - y_off <= 3 {
        c -= 4 - (height - y_off);
    }
    c
}

fn signed_distance_circle(rng: &mut FeatureRandom, xo: i32, zo: i32, radius: i32) -> f64 {
    let off = 10.0 * rng.next_f32().clamp(0.2, 0.8) / radius as f32;
    (off as f64) + (xo * xo + zo * zo) as f64 - (radius * radius) as f64
}

fn signed_distance_ellipse(xo: i32, zo: i32, a: i32, c: i32, angle: f64) -> f64 {
    let (s, c_angle) = angle.sin_cos();
    let xr = (xo as f64 * c_angle - zo as f64 * s) / a as f64;
    let zr = (xo as f64 * s + zo as f64 * c_angle) / c as f64;
    xr * xr + zr * zr - 1.0
}

fn set_iceberg_block(
    region: &mut RegionBuf,
    x: i32,
    y: i32,
    z: i32,
    rng: &mut FeatureRandom,
    h_diff: i32,
    height: i32,
    is_ellipse: bool,
    snow_on_top: bool,
    main_state: BlockId,
) {
    let state = region.get(x, y, z);
    if state == BlockId::Air
        || state == BlockId::Snow
        || state == BlockId::Ice
        || state == BlockId::Water
    {
        let randomness = !is_ellipse || rng.next_f64() > 0.05;
        let divisor = if is_ellipse { 3 } else { 2 };
        if snow_on_top
            && state != BlockId::Water
            && h_diff <= rng.next_int(1.max(height / divisor)) + (height as f64 * 0.6) as i32
            && randomness
        {
            region.set(x, y, z, BlockId::Snow);
        } else {
            region.set(x, y, z, main_state);
        }
    }
}

fn is_iceberg_state(b: BlockId) -> bool {
    matches!(b, BlockId::PackedIce | BlockId::Snow | BlockId::BlueIce)
}

fn smooth_iceberg(
    region: &mut RegionBuf,
    x: i32,
    origin_y: i32,
    z: i32,
    width: i32,
    height: i32,
    is_ellipse: bool,
    shape_ellipse_a: i32,
) {
    let a = if is_ellipse { shape_ellipse_a } else { width / 2 };
    for dx in -a..=a {
        for dz in -a..=a {
            for y_off in 0..=height {
                let b = region.get(x + dx, origin_y + y_off, z + dz);
                if is_iceberg_state(b) || b == BlockId::Snow {
                    if region.get(x + dx, origin_y + y_off - 1, z + dz) == BlockId::Air {
                        region.set(x + dx, origin_y + y_off, z + dz, BlockId::Air);
                        region.set(x + dx, origin_y + y_off + 1, z + dz, BlockId::Air);
                    } else if is_iceberg_state(b) {
                        let mut counter = 0;
                        for &(sdx, sdz) in &[(1, 0), (-1, 0), (0, 1), (0, -1)] {
                            if !is_iceberg_state(region.get(x + dx + sdx, origin_y + y_off, z + dz + sdz)) {
                                counter += 1;
                            }
                        }
                        if counter >= 3 {
                            region.set(x + dx, origin_y + y_off, z + dz, BlockId::Air);
                        }
                    }
                }
            }
        }
    }
}

fn generate_cut_out(
    rng: &mut FeatureRandom,
    region: &mut RegionBuf,
    x: i32,
    origin_y: i32,
    z: i32,
    width: i32,
    height: i32,
    is_ellipse: bool,
    shape_ellipse_a: i32,
    shape_angle: f64,
    shape_ellipse_c: i32,
) {
    let random_sign_x = if rng.next_boolean() { -1 } else { 1 };
    let random_sign_z = if rng.next_boolean() { -1 } else { 1 };
    let mut x_off = rng.next_int(1.max(width / 2 - 2));
    if rng.next_boolean() {
        x_off = width / 2 + 1 - rng.next_int(1.max(width - width / 2 - 1));
    }
    let mut z_off = rng.next_int(1.max(width / 2 - 2));
    if rng.next_boolean() {
        z_off = width / 2 + 1 - rng.next_int(1.max(width - width / 2 - 1));
    }
    if is_ellipse {
        x_off = rng.next_int(1.max(shape_ellipse_a - 5));
        z_off = x_off;
    }
    let local_ox = random_sign_x * x_off;
    let local_oz = random_sign_z * z_off;
    let angle = if is_ellipse {
        shape_angle + std::f64::consts::FRAC_PI_2
    } else {
        rng.next_f64() * 2.0 * std::f64::consts::PI
    };
    for y_off in 0..height - 3 {
        let radius = height_dependent_radius_round(rng, y_off, height, width);
        carve_iceberg(
            rng,
            region,
            x,
            origin_y,
            z,
            radius,
            y_off,
            false,
            angle,
            local_ox,
            local_oz,
            shape_ellipse_a,
            shape_ellipse_c,
        );
    }
    for y_off in -1..-(height - rng.next_int(5)) {
        let radius = height_dependent_radius_steep(rng, -y_off, height, width);
        carve_iceberg(
            rng,
            region,
            x,
            origin_y,
            z,
            radius,
            y_off,
            true,
            angle,
            local_ox,
            local_oz,
            shape_ellipse_a,
            shape_ellipse_c,
        );
    }
}

fn carve_iceberg(
    rng: &mut FeatureRandom,
    region: &mut RegionBuf,
    x: i32,
    origin_y: i32,
    z: i32,
    radius: i32,
    y_off: i32,
    under_water: bool,
    angle: f64,
    local_ox: i32,
    local_oz: i32,
    shape_ellipse_a: i32,
    shape_ellipse_c: i32,
) {
    let a = radius + 1 + shape_ellipse_a / 3;
    let c = (radius - 3).min(3) + shape_ellipse_c / 2 - 1;
    for xo in -a..a {
        for zo in -a..a {
            let signed_dist = signed_distance_ellipse(xo - local_ox, zo - local_oz, a, c, angle);
            if signed_dist < 0.0 {
                let b = region.get(x + xo, origin_y + y_off, z + zo);
                if is_iceberg_state(b) || b == BlockId::Snow {
                    if under_water {
                        region.set(x + xo, origin_y + y_off, z + zo, BlockId::Water);
                    } else {
                        region.set(x + xo, origin_y + y_off, z + zo, BlockId::Air);
                        if region.get(x + xo, origin_y + y_off + 1, z + zo) == BlockId::Snow {
                            region.set(x + xo, origin_y + y_off + 1, z + zo, BlockId::Air);
                        }
                    }
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// fossil
// ---------------------------------------------------------------------------

/// `FossilFeature.place` (26.2). Rotation + block_rot processors applied.
pub(crate) fn place_fossil(
    rng: &mut FeatureRandom,
    region: &mut RegionBuf,
    x: i32,
    y: i32,
    z: i32,
    cfg: &Value,
) {
    let c = &cfg["config"];
    let rotation = rng.next_int(4); // Rotation.getRandom
    let fossil_index = rng.next_int(8);
    let (sx, sy, sz, blocks) = crate::fossil_structures::FOSSIL_STRUCTURES[fossil_index as usize];
    let (ox_sx, ox_sy, ox_sz, overlay) = crate::fossil_structures::FOSSIL_OVERLAYS[fossil_index as usize];
    let _ = (sy, ox_sy);
    // Rotated footprint: 90/180/270 swap x/z.
    let (rsx, rsz) = match rotation {
        1 | 3 => (sz, sx),
        _ => (sx, sz),
    };
    let low_corner_x = x - rsx / 2;
    let low_corner_z = z - rsz / 2;
    let mut lowest_surface_y = y;
    for xscan in 0..rsx {
        for zscan in 0..rsz {
            if let Some(h) = heightmap_top(region, low_corner_x + xscan, low_corner_z + zscan, HeightmapKind::OceanFloor) {
                lowest_surface_y = lowest_surface_y.min(h + 1);
            }
        }
    }
    let target_y = (lowest_surface_y - 15 - rng.next_int(10)).max(WORLD_BOTTOM + 10);
    // countEmptyCorners over the structure's bounding box (rotated size).
    let corners = [
        (low_corner_x, target_y, low_corner_z),
        (low_corner_x + rsx - 1, target_y, low_corner_z),
        (low_corner_x, target_y, low_corner_z + rsz - 1),
        (low_corner_x + rsx - 1, target_y, low_corner_z + rsz - 1),
    ];
    let mut empty_corners = 0;
    for &(cx, cy, cz) in &corners {
        let b = region.get(cx, cy, cz);
        if b == BlockId::Air || b == BlockId::Lava || b == BlockId::Water {
            empty_corners += 1;
        }
    }
    let max_empty = c["max_empty_corners_allowed"].as_i64().unwrap_or(4) as i32;
    if empty_corners > max_empty {
        return;
    }
    // Base structure with block_rot integrity 0.9 (fossil_rot).
    place_fossil_blocks(region, blocks, sx, sy, sz, low_corner_x, target_y, low_corner_z, rotation, 0.9, rng, BlockId::BoneBlock);
    // Overlay with integrity 0.1 (fossil_coal / fossil_diamonds).
    let is_diamonds = c["overlay_processors"]
        .as_str()
        .map(|s| s.ends_with("fossil_diamonds"))
        .unwrap_or(false);
    let overlay_block = if is_diamonds {
        BlockId::DeepslateDiamondOre
    } else {
        BlockId::CoalOre
    };
    let overlay4: Vec<(i32, i32, i32, u8)> =
        overlay.iter().map(|&(bx, by, bz)| (bx, by, bz, 1)).collect();
    place_fossil_blocks(region, &overlay4, ox_sx, ox_sy, ox_sz, low_corner_x, target_y, low_corner_z, rotation, 0.1, rng, overlay_block);
}

/// Place a structure with `Rotation` (0-3) and `block_rot` integrity.
fn place_fossil_blocks(
    region: &mut RegionBuf,
    blocks: &[(i32, i32, i32, u8)],
    _sx: i32,
    sy: i32,
    sz: i32,
    low_corner_x: i32,
    target_y: i32,
    low_corner_z: i32,
    rotation: i32,
    integrity: f64,
    rng: &mut FeatureRandom,
    block: BlockId,
) {
    for &(bx, by, bz, _axis) in blocks {
        // block_rot: keep with probability `integrity`, else air.
        if rng.next_f64() >= integrity {
            continue;
        }
        let (rx, rz) = match rotation {
            0 => (bx, bz),
            1 => (sz - 1 - bz, bx),
            2 => (sz - 1 - bx, sz - 1 - bz),
            _ => (bz, sz - 1 - bx),
        };
        let px = low_corner_x + rx;
        let py = target_y + by;
        let pz = low_corner_z + rz;
        if py < WORLD_BOTTOM || py >= WORLD_TOP {
            continue;
        }
        let existing = region.get(px, py, pz);
        if existing == BlockId::Bedrock || existing == BlockId::Spawner || existing == BlockId::Chest {
            continue; // protected_blocks: features_cannot_replace
        }
        region.set(px, py, pz, block);
    }
}

// ---------------------------------------------------------------------------
// geode
// ---------------------------------------------------------------------------

/// `GeodeFeature.place` (26.2). Noise seeded from
/// `WorldgenRandom(LegacyRandomSource(levelSeed))` (legacy LCG path).
pub(crate) fn place_geode(
    rng: &mut FeatureRandom,
    region: &mut RegionBuf,
    state: &WorldgenState,
    x: i32,
    y: i32,
    z: i32,
    cfg: &Value,
) {
    let c = &cfg["config"];
    let blocks = &c["blocks"];
    let layers = &c["layers"];
    let crack = &c["crack"];
    let filling = layers["filling"].as_f64().unwrap_or(1.7);
    let inner_layer = layers["inner_layer"].as_f64().unwrap_or(2.2);
    let middle_layer = layers["middle_layer"].as_f64().unwrap_or(3.2);
    let outer_layer = layers["outer_layer"].as_f64().unwrap_or(4.2);
    let generate_crack_chance = crack["generate_crack_chance"].as_f64().unwrap_or(1.0);
    let base_crack_size = crack["base_crack_size"].as_f64().unwrap_or(2.0);
    let crack_point_offset = crack["crack_point_offset"].as_i64().unwrap_or(2) as i32;

    let num_points = sample_int_provider(rng, &c["distribution_points"]);
    // Legacy LCG noise (per-level seed, NOT the feature RNG).
    let mut legacy = crate::legacy_rng::LegacyRandom::new(state.seed);
    let f1 = legacy.next_long();
    let f2 = legacy.next_long();
    let noise = crate::noise::NormalNoise::create_legacy(f1, f2, -4, &[1.0]);

    let outer_wall_max = c["outer_wall_distance"]["max_inclusive"].as_i64().unwrap_or(6) as f64;
    let crack_size_adjustment = num_points as f64 / outer_wall_max;
    let inner_air = 1.0 / filling.sqrt();
    let innermost_block_layer = 1.0 / (inner_layer + crack_size_adjustment).sqrt();
    let inner_crust = 1.0 / (middle_layer + crack_size_adjustment).sqrt();
    let outer_crust = 1.0 / (outer_layer + crack_size_adjustment).sqrt();
    let crack_size = 1.0
        / (base_crack_size + rng.next_f64() / 2.0 + if num_points > 3 { crack_size_adjustment } else { 0.0 })
            .sqrt();
    let should_generate_crack = rng.next_f32() < generate_crack_chance as f32;

    let mut points: Vec<([i32; 3], i32)> = Vec::new();
    let invalid_threshold = c["invalid_blocks_threshold"].as_i64().unwrap_or(1) as i32;
    let mut num_invalid = 0;
    for _ in 0..num_points {
        let px = x + sample_int_provider(rng, &c["outer_wall_distance"]);
        let py = y + sample_int_provider(rng, &c["outer_wall_distance"]);
        let pz = z + sample_int_provider(rng, &c["outer_wall_distance"]);
        let b = region.get(px, py, pz);
        if b == BlockId::Air || is_geode_invalid(b) {
            num_invalid += 1;
            if num_invalid > invalid_threshold {
                return;
            }
        }
        points.push(([px, py, pz], sample_int_provider(rng, &c["point_offset"])));
    }
    let mut crack_points: Vec<[i32; 3]> = Vec::new();
    if should_generate_crack {
        let offset_index = rng.next_int(4);
        let crack_offset = num_points * 2 + 1;
        let (cx, cz) = match offset_index {
            0 => (crack_offset, 0),
            1 => (0, crack_offset),
            2 => (crack_offset, crack_offset),
            _ => (0, 0),
        };
        crack_points.push([x + cx, y + 7, z + cz]);
        crack_points.push([x + cx, y + 5, z + cz]);
        crack_points.push([x + cx, y + 1, z + cz]);
    }

    let noise_multiplier = c["noise_multiplier"].as_f64().unwrap_or(0.05);
    let use_alternate_chance = c["use_alternate_layer0_chance"].as_f64().unwrap_or(0.0);
    let use_potential_chance = c["use_potential_placements_chance"].as_f64().unwrap_or(0.35);
    let require_alternate = c["placements_require_layer0_alternate"].as_bool().unwrap_or(true);
    let min_gen = c["min_gen_offset"].as_i64().unwrap_or(-16) as i32;
    let max_gen = c["max_gen_offset"].as_i64().unwrap_or(16) as i32;
    let alternate_inner = block_from_state_provider(&blocks["alternate_inner_layer_provider"]).unwrap_or(BlockId::BuddingAmethyst);
    let filling_block = block_from_state_provider(&blocks["filling_provider"]).unwrap_or(BlockId::Air);
    let inner_block = block_from_state_provider(&blocks["inner_layer_provider"]).unwrap_or(BlockId::AmethystBlock);
    let middle_block = block_from_state_provider(&blocks["middle_layer_provider"]).unwrap_or(BlockId::Calcite);
    let outer_block = block_from_state_provider(&blocks["outer_layer_provider"]).unwrap_or(BlockId::SmoothBasalt);
    let inner_placements: Vec<BlockId> = blocks["inner_placements"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|s| s["Name"].as_str().and_then(BlockId::from_name))
                .collect()
        })
        .unwrap_or_default();

    let mut potential_crystals: Vec<[i32; 3]> = Vec::new();
    for px in x + min_gen..=x + max_gen {
        for py in y + min_gen..=y + max_gen {
            for pz in z + min_gen..=z + max_gen {
                let noise_offset = noise.get_value(px as f64, py as f64, pz as f64) * noise_multiplier;
                let mut dist_sum_shell = 0.0;
                for (pt, off) in &points {
                    let d = dist_sqr(px, py, pz, pt[0], pt[1], pt[2]);
                    dist_sum_shell += inv_sqrt(d + *off as f64) + noise_offset;
                }
                let mut dist_sum_crack = 0.0;
                for pt in &crack_points {
                    let d = dist_sqr(px, py, pz, pt[0], pt[1], pt[2]);
                    dist_sum_crack += inv_sqrt(d + crack_point_offset as f64) + noise_offset;
                }
                if !(dist_sum_shell < outer_crust) {
                    // outer shell untouched
                } else if should_generate_crack && dist_sum_crack >= crack_size && dist_sum_shell < inner_air {
                    safe_set_geode(region, px, py, pz, BlockId::Air);
                } else if dist_sum_shell >= inner_air {
                    safe_set_geode(region, px, py, pz, filling_block);
                } else if dist_sum_shell >= innermost_block_layer {
                    let use_alternate = rng.next_f32() < use_alternate_chance as f32;
                    if use_alternate {
                        safe_set_geode(region, px, py, pz, alternate_inner);
                    } else {
                        safe_set_geode(region, px, py, pz, inner_block);
                    }
                    if (!require_alternate || use_alternate) && rng.next_f32() < use_potential_chance as f32 {
                        potential_crystals.push([px, py, pz]);
                    }
                } else if dist_sum_shell >= inner_crust {
                    safe_set_geode(region, px, py, pz, middle_block);
                } else {
                    safe_set_geode(region, px, py, pz, outer_block);
                }
            }
        }
    }
    for crystal in &potential_crystals {
        if inner_placements.is_empty() {
            break;
        }
        let block_state = inner_placements[rng.next_int(inner_placements.len() as i32) as usize];
        for (dx, dy, dz) in DIRS_6 {
            let place_pos = [crystal[0] + dx, crystal[1] + dy, crystal[2] + dz];
            let place_state = region.get(place_pos[0], place_pos[1], place_pos[2]);
            if place_state == BlockId::Air || place_state == BlockId::Water {
                safe_set_geode(region, place_pos[0], place_pos[1], place_pos[2], block_state);
                break;
            }
        }
    }
}

const DIRS_6: [(i32, i32, i32); 6] = [
    (0, 1, 0),
    (0, -1, 0),
    (0, 0, -1),
    (1, 0, 0),
    (0, 0, 1),
    (-1, 0, 0),
];

fn dist_sqr(x1: i32, y1: i32, z1: i32, x2: i32, y2: i32, z2: i32) -> f64 {
    let dx = (x1 - x2) as f64;
    let dy = (y1 - y2) as f64;
    let dz = (z1 - z2) as f64;
    dx * dx + dy * dy + dz * dz
}

fn inv_sqrt(v: f64) -> f64 {
    1.0 / v.sqrt()
}

fn is_geode_invalid(b: BlockId) -> bool {
    matches!(
        b,
        BlockId::Bedrock
            | BlockId::Water
            | BlockId::Lava
            | BlockId::Ice
            | BlockId::PackedIce
            | BlockId::BlueIce
    )
}

fn safe_set_geode(region: &mut RegionBuf, x: i32, y: i32, z: i32, b: BlockId) {
    if y < WORLD_BOTTOM || y >= WORLD_TOP {
        return;
    }
    let existing = region.get(x, y, z);
    // features_cannot_replace: bedrock / spawner / chest.
    if existing == BlockId::Bedrock || existing == BlockId::Spawner || existing == BlockId::Chest {
        return;
    }
    region.set(x, y, z, b);
}

fn block_from_state_provider(v: &Value) -> Option<BlockId> {
    if let Some(state) = v.get("state") {
        return state["Name"].as_str().and_then(BlockId::from_name);
    }
    v["Name"].as_str().and_then(BlockId::from_name)
}

// ---------------------------------------------------------------------------
// shared helpers
// ---------------------------------------------------------------------------

/// `Mth.clampedMap`: outMin + clamp((v-min)/(max-min), 0, 1) * (outMax-outMin).
fn clamped_map(v: f64, min: f64, max: f64, out_min: f64, out_max: f64) -> f64 {
    let t = ((v - min) / (max - min)).clamp(0.0, 1.0);
    out_min + t * (out_max - out_min)
}

/// Sample a float provider: `uniform` (min..max exclusive) or
/// `clamped_normal` (gaussian clamped).
fn sample_float_provider(rng: &mut FeatureRandom, v: &Value) -> f32 {
    if let Some(n) = v.as_f64() {
        return n as f32;
    }
    match v["type"].as_str().unwrap_or("") {
        "minecraft:uniform" => {
            let min = v["min_inclusive"].as_f64().unwrap_or(0.0);
            let max = v["max_exclusive"].as_f64().unwrap_or(1.0);
            (min + rng.next_f32() as f64 * (max - min)) as f32
        }
        "minecraft:clamped_normal" => {
            let mean = v["mean"].as_f64().unwrap_or(0.0);
            let dev = v["deviation"].as_f64().unwrap_or(1.0);
            let min = v["min"].as_f64().unwrap_or(0.0);
            let max = v["max"].as_f64().unwrap_or(1.0);
            let g = rng.next_gaussian() * dev + mean;
            g.clamp(min, max) as f32
        }
        _ => 0.0,
    }
}

/// `Mth.randomBetween(random, min, max)` — float.
#[allow(dead_code)]
fn random_between_f32(rng: &mut FeatureRandom, min: f32, max: f32) -> f32 {
    min + rng.next_f32() * (max - min)
}