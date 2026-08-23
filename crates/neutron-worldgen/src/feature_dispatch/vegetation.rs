//! Direction shuffling + inline vegetation-family features
//! (glow lichen / vines / root system / vegetation_patch).
use super::*;
use crate::feature_catalog;
use crate::feature_rng::FeatureRandom;
use crate::generator::{WORLD_BOTTOM, WORLD_TOP};
use crate::legacy_rng::LegacyRandom;
use crate::region_buf::RegionBuf;
use crate::sculk;
use crate::surface::BlockId;
use crate::worldgen::WorldgenState;
use serde_json::Value;


// ---------------------------------------------------------------------------
// B4 ports: multiface_growth (glow_lichen), vines, root_system
// ---------------------------------------------------------------------------

fn shuffle_dirs(rng: &mut FeatureRandom, dirs: &[usize]) -> Vec<usize> {
    let mut order = dirs.to_vec();
    crate::deco_util::shuffle(&mut order, rng);
    order
}

fn dir_opposite(d: usize) -> usize {
    crate::deco_util::opposite(d)
}

fn dir_axis(d: usize) -> usize {
    match d {
        0 | 1 => 1,
        2 | 3 => 2,
        _ => 0,
    }
}


/// `MultifaceGrowthFeature.place` / `placeGrowthIfPossible` (26.2 bytecode).
///
/// Search loop is `mutable.setWithOffset(origin, searchDir)` every iteration
/// (not `move`) — only the adjacent cell is tested, `search_range` times.
/// `validDirections` order: UP (ceiling), DOWN (floor), then HORIZONTAL
/// NORTH/EAST/SOUTH/WEST. sculk_vein is skipped (sculk.rs owns it).
pub(super) fn place_multiface_growth(
    rng: &mut FeatureRandom,
    region: &mut RegionBuf,
    x: i32,
    y: i32,
    z: i32,
    cfg: &Value,
) {
    let c = &cfg["config"];
    let Some(place_block) = c["block"].as_str().and_then(BlockId::from_name) else {
        return;
    };
    // NOTE: sculk_vein flows through here too — vanilla MultifaceGrowthFeature
    // handles ALL multiface blocks identically (glow_lichen, sculk_vein, ...).
    let here = region.get(x, y, z);
    if !matches!(here, BlockId::Air | BlockId::CaveAir | BlockId::Water) {
        return;
    }
    let search_range = c["search_range"].as_i64().unwrap_or(10) as i32;
    let on_floor = c["can_place_on_floor"].as_bool().unwrap_or(false);
    let on_ceiling = c["can_place_on_ceiling"].as_bool().unwrap_or(false);
    let on_wall = c["can_place_on_wall"].as_bool().unwrap_or(false);
    let chance = c["chance_of_spreading"].as_f64().unwrap_or(0.5) as f32;
    let allowed: Vec<BlockId> = c["can_be_placed_on"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().and_then(BlockId::from_name))
                .collect()
        })
        .unwrap_or_default();
    // Ctor: if ceiling add UP; if floor add DOWN; if wall HORIZONTAL N,E,S,W.
    let mut valid = Vec::new();
    if on_ceiling {
        valid.push(1usize); // UP
    }
    if on_floor {
        valid.push(0); // DOWN
    }
    if on_wall {
        valid.extend_from_slice(&[2, 5, 3, 4]); // NORTH EAST SOUTH WEST
    }
    if valid.is_empty() {
        return;
    }

    let air_or_water_or_self = |b: BlockId| {
        matches!(b, BlockId::Air | BlockId::CaveAir | BlockId::Water) || b == place_block
    };

    let try_place = |rng: &mut FeatureRandom,
                     region: &mut RegionBuf,
                     px: i32,
                     py: i32,
                     pz: i32,
                     dirs: &[usize]|
     -> bool {
        for &d in dirs {
            let (dx, dy, dz) = crate::multiface_spreader::DIRS[d];
            if !allowed.contains(&region.get(px + dx, py + dy, pz + dz)) {
                continue;
            }
            // getStateForPlacement null → vanilla returns false immediately
            // (does not try later dirs). Air/water + canBePlacedOn neighbour
            // is never null for glow_lichen.
            region.set(px, py, pz, place_block);
            if rng.next_f32() < chance {
                // Direction.allShuffled consumes 5× nextInt; placing the
                // spread cell is a separate block write (DefaultSpreaderConfig).
                lichen_spread(rng, region, px, py, pz, d, place_block);
            }
            return true;
        }
        false
    };

    let dirs0 = shuffle_dirs(rng, &valid);
    if try_place(rng, region, x, y, z, &dirs0) {
        return;
    }
    for &search in &dirs0 {
        let except: Vec<usize> = valid
            .iter()
            .copied()
            .filter(|&d| d != dir_opposite(search))
            .collect();
        let place_dirs = shuffle_dirs(rng, &except);
        let (sdx, sdy, sdz) = crate::multiface_spreader::DIRS[search];
        // Bytecode: setWithOffset(origin, searchDir) every i — adjacent only.
        let px = x + sdx;
        let py = y + sdy;
        let pz = z + sdz;
        for _ in 0..search_range {
            let st = region.get(px, py, pz);
            if !air_or_water_or_self(st) {
                break;
            }
            if try_place(rng, region, px, py, pz, &place_dirs) {
                return;
            }
        }
    }
}

/// `MultifaceSpreader.spreadFromFaceTowardRandomDirection`:
/// `Direction.allShuffled` then first successful SAME_POSITION / SAME_PLANE /
/// WRAP_AROUND (skip same-axis). Attach uses isFaceSturdy ≈ `is_solid_block`.
fn lichen_spread(
    rng: &mut FeatureRandom,
    region: &mut RegionBuf,
    x: i32,
    y: i32,
    z: i32,
    start_face: usize,
    place_block: BlockId,
) {
    let order = shuffle_dirs(rng, &[0, 1, 2, 3, 4, 5]);
    // Stream.findFirst: first successful spreadFromFaceTowardDirection.
    for spread_dir in order {
        if dir_axis(start_face) == dir_axis(spread_dir) {
            continue;
        }
        let candidates = [
            (x, y, z, spread_dir),
            {
                let (dx, dy, dz) = crate::multiface_spreader::DIRS[spread_dir];
                (x + dx, y + dy, z + dz, start_face)
            },
            {
                let (sdx, sdy, sdz) = crate::multiface_spreader::DIRS[spread_dir];
                let (fdx, fdy, fdz) = crate::multiface_spreader::DIRS[start_face];
                (
                    x + sdx + fdx,
                    y + sdy + fdy,
                    z + sdz + fdz,
                    dir_opposite(spread_dir),
                )
            },
        ];
        for (sx, sy, sz, face) in candidates {
            let cur = region.get(sx, sy, sz);
            if !matches!(
                cur,
                BlockId::Air
                    | BlockId::CaveAir
                    | BlockId::Water
                    | BlockId::GlowLichen
                    | BlockId::SculkVein
            ) && cur != place_block
            {
                continue;
            }
            let (dx, dy, dz) = crate::multiface_spreader::DIRS[face];
            if !is_solid_block(region.get(sx + dx, sy + dy, sz + dz)) {
                continue;
            }
            region.set(sx, sy, sz, place_block);
            return; // first success, like Optional.findFirst
        }
    }
}

/// `VinesFeature.place` (26.2): origin must be empty; attaches to the first
/// acceptable neighbor among all directions except DOWN (isAcceptableNeighbour
/// = solid face or vine). No RNG consumed.
pub(super) fn place_vines(rng: &mut FeatureRandom, region: &mut RegionBuf, x: i32, y: i32, z: i32) {
    let _ = rng;
    if !region.get(x, y, z).is_air() {
        return;
    }
    for &(dx, dy, dz) in &[(0, 1, 0), (0, 0, -1), (1, 0, 0), (0, 0, 1), (-1, 0, 0)] {
        let nb = region.get(x + dx, y + dy, z + dz);
        if blocks_motion(nb) || nb == BlockId::Vine {
            region.set(x, y, z, BlockId::Vine);
            break;
        }
    }
}

/// `RootSystemFeature.place` (26.2): azalea tree + rooted_dirt columns +
/// hanging roots. Origin must be air; scans up the column for a valid tree
/// position (allowed_tree_position + spaceForTree + solid below); if the tree
/// places, fills rooted_dirt from the origin up and scatters hanging roots.
pub(super) fn place_root_system(
    rng: &mut FeatureRandom,
    region: &mut RegionBuf,
    state: Option<&WorldgenState>,
    x: i32,
    y: i32,
    z: i32,
    cfg: &Value,
) {
    if !region.get(x, y, z).is_air() {
        return;
    }
    let c = &cfg["config"];
    let max_height = c["root_column_max_height"].as_i64().unwrap_or(100) as i32;
    let required_space = c["required_vertical_space_for_tree"].as_i64().unwrap_or(3) as i32;
    let allowed_water = c["allowed_vertical_water_for_tree"].as_i64().unwrap_or(2) as i32;
    let root_radius = c["root_radius"].as_i64().unwrap_or(3) as i32;
    let root_attempts = c["root_placement_attempts"].as_i64().unwrap_or(20) as i32;
    let hang_radius = c["hanging_root_radius"].as_i64().unwrap_or(3) as i32;
    let hang_span = c["hanging_roots_vertical_span"].as_i64().unwrap_or(2) as i32;
    let hang_attempts = c["hanging_root_placement_attempts"].as_i64().unwrap_or(20) as i32;

    let mut ty = y;
    let mut placed = false;
    'col: for _ in 0..max_height {
        ty += 1;
        if ty > WORLD_TOP {
            break;
        }
        // level.getHeight(WORLD_SURFACE) < ty -> fail (surface below the scan).
        let ws = heightmap_top(region, x, z, HeightmapKind::WorldSurface)
            .map(|h| h + 1)
            .unwrap_or(-1);
        if ws < ty {
            break;
        }
        // allowed_tree_position: any_of(air, replaceable_by_trees) at pos AND
        // azalea_grows_on at below.
        let here = region.get(x, ty, z);
        let pos_ok = here.is_air() || crate::tree::valid_tree_pos(here);
        let below_ok = is_in_tag(region.get(x, ty - 1, z), "#minecraft:azalea_grows_on");
        if !pos_ok || !below_ok {
            continue;
        }
        // spaceForTree: required_space air/water above.
        let mut space_ok = true;
        for i in 1..=required_space {
            let b = region.get(x, ty + i, z);
            if !b.is_air() && !(b == BlockId::Water && i <= allowed_water) {
                space_ok = false;
                break;
            }
        }
        if !space_ok {
            continue;
        }
        let below = region.get(x, ty - 1, z);
        if below == BlockId::Lava || !blocks_motion(below) {
            continue;
        }
        // Place the tree (azalea_tree is a tree config; inline placed feature).
        if let Some(feat) = c["feature"]["feature"].as_str() {
            if let Some(tcfg) = feature_catalog::load_configured_feature(feat) {
                dispatch_configured(
                    rng,
                    region,
                    state,
                    x,
                    ty,
                    z,
                    &tcfg,
                    step::VEGETAL_DECORATION,
                );
            }
        }
        // placeDirt: rooted_dirt columns from the origin up to the tree base.
        for cy in y..ty {
            for _ in 0..root_attempts {
                let rx = x + rng.next_int(root_radius) - rng.next_int(root_radius);
                let rz = z + rng.next_int(root_radius) - rng.next_int(root_radius);
                if is_in_tag(region.get(rx, cy, rz), "#minecraft:azalea_root_replaceable") {
                    region.set(rx, cy, rz, BlockId::RootedDirt);
                }
            }
        }
        placed = true;
        break 'col;
    }
    if placed {
        // placeRoots: hanging roots scattered around the origin.
        for _ in 0..hang_attempts {
            let rx = x + rng.next_int(hang_radius) - rng.next_int(hang_radius);
            let ry = y + rng.next_int(hang_span) - rng.next_int(hang_span);
            let rz = z + rng.next_int(hang_radius) - rng.next_int(hang_radius);
            if region.get(rx, ry, rz).is_air() && blocks_motion(region.get(rx, ry + 1, rz)) {
                region.set(rx, ry, rz, BlockId::HangingRoots);
            }
        }
    }
}


/// Minimal replica of Java `HashSet<BlockPos>` iteration order, used by
/// `VegetationPatchFeature.placeGroundPatch` (surface set). Java HashMap:
/// initial capacity 16, load factor 0.75, capacity doubles when size >
/// capacity*0.75; bucket = spread(hashCode) & (capacity-1); iteration is
/// bucket order, insertion order within a bucket. `Vec3i.hashCode()` =
/// `(y + z*31)*31 + x`. Dedup matters: distributeVegetation consumes RNG once
/// per unique element.
struct JavaBlockPosSet {
    buckets: Vec<Vec<(i32, i32, i32)>>,
    capacity: usize,
    size: usize,
}

impl JavaBlockPosSet {
    fn new() -> Self {
        Self {
            buckets: Vec::new(),
            capacity: 0,
            size: 0,
        }
    }

    fn hash(x: i32, y: i32, z: i32) -> u32 {
        // Java int arithmetic wraps mod 2^32; i64 then truncate is identical.
        let h = ((y as i64 + z as i64 * 31) * 31 + x as i64) as u32;
        h ^ (h >> 16) // HashMap.hash spread
    }

    fn insert(&mut self, x: i32, y: i32, z: i32) {
        if self.buckets.is_empty() {
            self.capacity = 16; // HashMap first put -> resize() to 16
            self.buckets = vec![Vec::new(); 16];
        }
        let bi = (Self::hash(x, y, z) as usize) & (self.capacity - 1);
        if self.buckets[bi]
            .iter()
            .any(|&(a, b, c)| a == x && b == y && c == z)
        {
            return; // duplicate: no add, no size change
        }
        self.buckets[bi].push((x, y, z));
        self.size += 1;
        if self.size > self.capacity * 3 / 4 {
            let new_cap = self.capacity * 2;
            let mut new_buckets = vec![Vec::new(); new_cap];
            for bucket in self.buckets.drain(..) {
                for e in bucket {
                    let h = Self::hash(e.0, e.1, e.2);
                    new_buckets[(h as usize) & (new_cap - 1)].push(e);
                }
            }
            self.buckets = new_buckets;
            self.capacity = new_cap;
        }
    }

    fn iter(&self) -> impl Iterator<Item = (i32, i32, i32)> + '_ {
        self.buckets.iter().flatten().copied()
    }
}

/// Port of `VegetationPatchFeature.place` (moss patches, pale moss patches).
///
/// `state` is `None` when invoked from a tree decorator (vanilla
/// `Feature.place` of an inline placed feature with no biome filter).
pub(crate) fn place_vegetation_patch(
    rng: &mut FeatureRandom,
    region: &mut RegionBuf,
    state: Option<&WorldgenState>,
    x: i32,
    y: i32,
    z: i32,
    cfg: &Value,
    gen_step: i32,
) {
    let c = &cfg["config"];
    let surface = c["surface"].as_str().unwrap_or("floor");
    // inwards = surface direction (floor -> down, ceiling -> up).
    let (in_dx, in_dy, in_dz) = if surface == "ceiling" {
        (0, 1, 0)
    } else {
        (0, -1, 0)
    };
    let (out_dx, out_dy, out_dz) = (-in_dx, -in_dy, -in_dz);
    let vertical_range = c["vertical_range"].as_i64().unwrap_or(5) as i32;
    let extra_edge = c["extra_edge_column_chance"].as_f64().unwrap_or(0.0) as f32;
    let extra_bottom = c["extra_bottom_block_chance"].as_f64().unwrap_or(0.0) as f32;
    let depth_prov = &c["depth"];
    let ground_state = block_from_to_place(rng, &c["ground_state"]);
    let replaceable = c["replaceable"].as_str().unwrap_or("");
    let veg_chance = c["vegetation_chance"].as_f64().unwrap_or(0.0) as f32;
    let veg_feature = c["vegetation_feature"].clone();
    let waterlogged = cfg["type"].as_str() == Some("minecraft:waterlogged_vegetation_patch");

    let xr = sample_int_provider(rng, &c["xz_radius"]).max(0) + 1;
    let zr = sample_int_provider(rng, &c["xz_radius"]).max(0) + 1;

    let mut surface_pts = JavaBlockPosSet::new();

    for dx in -xr..=xr {
        let is_x_edge = dx == -xr || dx == xr;
        for dz in -zr..=zr {
            let is_z_edge = dz == -zr || dz == zr;
            let is_edge = is_x_edge || is_z_edge;
            let is_corner = is_x_edge && is_z_edge;
            let is_edge_not_corner = is_edge && !is_corner;
            if is_corner
                || (is_edge_not_corner && (extra_edge == 0.0 || rng.next_f32() > extra_edge))
            {
                continue;
            }
            let (mut px, mut py, mut pz) = (x + dx, y, z + dz);
            // Scan through air inwards (isEmptyBlock == isAir, incl. cave_air).
            let mut off = 0;
            while region.get(px, py, pz).is_air() && off < vertical_range {
                px += in_dx;
                py += in_dy;
                pz += in_dz;
                off += 1;
            }
            // Scan back out through solid.
            off = 0;
            while !region.get(px, py, pz).is_air() && off < vertical_range {
                px += out_dx;
                py += out_dy;
                pz += out_dz;
                off += 1;
            }
            let (bx, by, bz) = (px + in_dx, py + in_dy, pz + in_dz);
            if !region.get(px, py, pz).is_air() {
                continue;
            }
            // belowState.isFaceSturdy(..., outwards). Full cubes yes; leaves /
            // pointed dripstone / sculk_vein / bamboo no. Azalea is sturdy on UP
            // (ProbeSolidFaces 26.2) so a floor can sit on it.
            let below = region.get(bx, by, bz);
            if !(is_face_sturdy(below)
                || matches!(below, BlockId::Azalea | BlockId::FloweringAzalea))
            {
                continue;
            }
            let mut depth = sample_int_provider(rng, depth_prov).max(0);
            if extra_bottom > 0.0 && rng.next_f32() < extra_bottom {
                depth += 1;
            }
            // VegetationPatchFeature.placeGround: same-block skips set+move.
            // Vanilla returns true when the depth loop completes (already-ground
            // still joins the surface set). Insert only on a real place: already-
            // ground membership extra-draws vegetationChance and drops ALL.
            let (gx0, gy0, gz0) = (bx, by, bz);
            let (mut gx, mut gy, mut gz) = (bx, by, bz);
            let mut placed_any = false;
            let mut i = 0;
            while i < depth {
                let cur = region.get(gx, gy, gz);
                if let Some(st) = ground_state {
                    if st == cur {
                        i += 1;
                        continue;
                    }
                    if !is_in_tag(cur, replaceable) {
                        placed_any = i != 0;
                        break;
                    }
                    region.set(gx, gy, gz, st);
                    placed_any = true;
                }
                gx += in_dx;
                gy += in_dy;
                gz += in_dz;
                i += 1;
            }
            if placed_any {
                surface_pts.insert(gx0, gy0, gz0);
            }
        }
    }

    // WaterloggedVegetationPatchFeature.placeGroundPatch: interior (not
    // isExposed N/E/S/W/DOWN) ground cells become water; vegetation is only
    // distributed on those interior positions.
    if waterlogged {
        let mut interior = JavaBlockPosSet::new();
        for (sx, sy, sz) in surface_pts.iter() {
            if !patch_is_exposed(region, sx, sy, sz) {
                interior.insert(sx, sy, sz);
            }
        }
        for (sx, sy, sz) in interior.iter() {
            region.set(sx, sy, sz, BlockId::Water);
        }
        surface_pts = interior;
    }

    // distributeVegetation. Dry: place opposite of inwards (floor → above).
    // WaterloggedVegetationPatchFeature.placeVegetation calls
    // super.placeVegetation(pos.below()) so the +up offset lands ON the water
    // cell (then vanilla waterlogs the placed block).
    for (sx, sy, sz) in surface_pts.iter() {
        if veg_chance > 0.0 && rng.next_f32() < veg_chance {
            let (vx, vy, vz) = if waterlogged {
                (sx, sy, sz)
            } else {
                (sx + out_dx, sy + out_dy, sz + out_dz)
            };
            place_feature_ref(rng, region, state, vx, vy, vz, &veg_feature, gen_step);
        }
    }
}

/// `WaterloggedVegetationPatchFeature.isExposed`: any of N/E/S/W/DOWN is not
/// `isFaceSturdy` on the opposite face. Full cubes are sturdy on every face.
fn patch_is_exposed(region: &RegionBuf, x: i32, y: i32, z: i32) -> bool {
    const DIRS: [(i32, i32, i32); 5] = [(0, 0, -1), (1, 0, 0), (0, 0, 1), (-1, 0, 0), (0, -1, 0)];
    DIRS.iter()
        .any(|&(dx, dy, dz)| !is_face_sturdy(region.get(x + dx, y + dy, z + dz)))
}

/// 26.2 `BlockState.isFaceSturdy` for the worldgen palette (ProbeSolidFaces).
/// Azalea is sturdy on UP only — floor `below` checks add that at the call site.
fn is_face_sturdy(b: BlockId) -> bool {
    match b {
        BlockId::PointedDripstone | BlockId::SculkVein | BlockId::Bamboo => false,
        BlockId::OakLeaves
        | BlockId::DarkOakLeaves
        | BlockId::PaleOakLeaves
        | BlockId::BirchLeaves
        | BlockId::SpruceLeaves
        | BlockId::JungleLeaves
        | BlockId::AcaciaLeaves
        | BlockId::MangroveLeaves
        | BlockId::CherryLeaves => false,
        _ => is_solid_block(b),
    }
}



