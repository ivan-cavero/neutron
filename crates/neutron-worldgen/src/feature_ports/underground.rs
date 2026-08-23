//! Underground ports: speleothem clusters, large dripstone, fossil, geode.
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
    b == BlockId::Air || b == BlockId::CaveAir || b == BlockId::Water
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
                        if b == BlockId::Air || b == BlockId::CaveAir || b == BlockId::Water || b == BlockId::Lava {
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
    if center == BlockId::Air || center == BlockId::CaveAir || center == BlockId::Water || center == BlockId::Lava {
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
        if b == BlockId::Air || b == BlockId::CaveAir || b == BlockId::Lava || b == BlockId::Water {
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
        if b == BlockId::Air || b == BlockId::CaveAir || is_geode_invalid(b) {
            num_invalid += 1;
            if std::env::var_os("NEUTRON_GEODE_TRACE").is_some() {
                eprintln!("[geode] invalid block {b:?} at ({px},{py},{pz}) count={num_invalid}");
            }
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
                } else if dist_sum_shell >= outer_crust {
                    // Vanilla's last branch is dead code (guarded above by the
                    // negated `< outer_crust`) — kept verbatim; cells below
                    // inner_crust keep the surrounding terrain, like vanilla.
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
            if place_state == BlockId::Air || place_state == BlockId::CaveAir || place_state == BlockId::Water {
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
