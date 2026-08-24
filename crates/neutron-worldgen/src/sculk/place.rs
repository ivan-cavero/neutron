//! Sculk vein/growth/patch placement (SculkPatchFeature, vein placement).
use super::*;
use super::gates::*;

pub(super) fn valid_growth_dirs(can_floor: bool, can_ceiling: bool, can_wall: bool) -> Vec<(i32, i32, i32)> {
    let mut v = Vec::with_capacity(6);
    // javap MultifaceGrowthConfiguration.validDirections:
    //   UP if ceiling, DOWN if floor, then Plane.HORIZONTAL.
    if can_ceiling {
        v.push(DIRS[1]); // UP
    }
    if can_floor {
        v.push(DIRS[0]); // DOWN
    }
    if can_wall {
        v.push(DIRS[2]); // NORTH
        v.push(DIRS[5]); // EAST
        v.push(DIRS[3]); // SOUTH
        v.push(DIRS[4]); // WEST
    }
    v
}

pub(super) fn shuffle_dirs_list(
    rng: &mut FeatureRandom,
    dirs: &[(i32, i32, i32)],
) -> Vec<(i32, i32, i32)> {
    let mut d = dirs.to_vec();
    crate::deco_util::shuffle(&mut d, rng);
    d
}

pub(super) fn place_sculk_vein(
    rng: &mut FeatureRandom,
    region: &mut RegionBuf,
    state: &WorldgenState,
    faces: &mut FaceMap,
    ox0: i32,
    oz0: i32,
    cfg: &VeinConfig,
) {
    let gate = |x: i32, y: i32, z: i32| is_deep_dark_at(state, x, y, z);
    place_sculk_vein_gated(rng, region, faces, ox0, oz0, cfg, &gate);
}

/// MultifaceGrowthFeature driver with an injectable position gate
/// (vanilla: `minecraft:biome` placement modifier == deep_dark check).
pub(super) fn place_sculk_vein_gated(
    rng: &mut FeatureRandom,
    region: &mut RegionBuf,
    faces: &mut FaceMap,
    ox0: i32,
    oz0: i32,
    cfg: &VeinConfig,
    gate: &dyn Fn(i32, i32, i32) -> bool,
) {
    // sculk_vein config: floor+ceiling+wall all true
    let base_dirs = valid_growth_dirs(true, true, true);
    let count = cfg.count_min + rng.next_int(cfg.count_max - cfg.count_min + 1);
    for _ in 0..count {
        let x = ox0 + rng.next_int(16);
        let z = oz0 + rng.next_int(16);
        let y = WORLD_BOTTOM + rng.next_int(256 - WORLD_BOTTOM + 1);
        if !gate(x, y, z) {
            continue;
        }
        if std::env::var_os("NEUTRON_SCULK_ATT").is_some() && ox0 == 96 && oz0 == -32 {
            eprintln!("ATT vein {x} {y} {z}");
        }
        // MultifaceGrowthFeature.place: origin must be air/water
        if !is_air_or_water(region.get(x, y, z)) {
            continue;
        }
        let search_dirs = shuffle_dirs_list(rng, &base_dirs);
        if place_growth(
            rng,
            region,
            faces,
            x,
            y,
            z,
            &search_dirs,
            cfg.chance_of_spreading,
        ) {
            continue;
        }
        // MultifaceGrowthFeature.place (bytecode 26.2): for each searchDirection,
        // loop search_range times with setWithOffset(origin, searchDirection) —
        // always origin±1, NOT multi-step accumulation (CFR matches javap).
        'search: for &(dx, dy, dz) in &search_dirs {
            let opp = (-dx, -dy, -dz);
            let placement_dirs = shuffle_dirs_list(
                rng,
                &base_dirs
                    .iter()
                    .copied()
                    .filter(|d| *d != opp)
                    .collect::<Vec<_>>(),
            );
            for _ in 0..cfg.search_range {
                let nx = x + dx;
                let ny = y + dy;
                let nz = z + dz;
                let b = region.get(nx, ny, nz);
                // solid (not vein) ends this search direction
                if !is_air_or_water(b) && b != BlockId::SculkVein {
                    break;
                }
                if place_growth(
                    rng,
                    region,
                    faces,
                    nx,
                    ny,
                    nz,
                    &placement_dirs,
                    cfg.chance_of_spreading,
                ) {
                    break 'search;
                }
            }
        }
    }
}

pub(super) fn place_growth(
    rng: &mut FeatureRandom,
    region: &mut RegionBuf,
    faces: &mut FaceMap,
    x: i32,
    y: i32,
    z: i32,
    placement_dirs: &[(i32, i32, i32)],
    chance: f32,
) -> bool {
    let b = region.get(x, y, z);
    if !is_air_or_water(b) && b != BlockId::SculkVein {
        return false;
    }
    for &(dx, dy, dz) in placement_dirs {
        if !is_vein_placeable_on(region.get(x + dx, y + dy, z + dz)) {
            continue;
        }
        let Some(fi) = dir_index(dx, dy, dz) else {
            continue;
        };
        // getStateForPlacement == null → return false (do not try the next dir)
        let bit = 1u8 << fi;
        let prev = faces.get(&(x, y, z)).copied().unwrap_or(0);
        if b == BlockId::SculkVein && prev & bit != 0 {
            return false;
        }
        faces.insert((x, y, z), prev | bit);
        if is_air_or_water(b) {
            region.set(x, y, z, BlockId::SculkVein);
            SCULK_VEIN_PLACED.fetch_add(1, Ordering::Relaxed);
        }
        if rng.next_f32() < chance {
            MultifaceSpreader::vein()
                .spread_from_face_toward_random_direction(rng, region, faces, x, y, z, fi);
        }
        return true;
    }
    false
}

// ===================== SculkPatchFeature =====================

pub(super) fn place_sculk_patch(
    rng: &mut FeatureRandom,
    region: &mut RegionBuf,
    state: &WorldgenState,
    faces: &mut FaceMap,
    ox0: i32,
    oz0: i32,
    cfg: &PatchConfig,
) {
    let dump = std::env::var_os("NEUTRON_SCULK_PATCHES").is_some();
    for i in 0..cfg.patch_count {
        PATCH_I.store(i, Ordering::Relaxed);
        SCULK_TRIES.fetch_add(1, Ordering::Relaxed);
        let x = ox0 + rng.next_int(16);
        let z = oz0 + rng.next_int(16);
        let y = WORLD_BOTTOM + rng.next_int(256 - WORLD_BOTTOM + 1);
        if dump && ox0 == 96 && oz0 == -32 {
            eprintln!(
                "att o=({ox0},{oz0}) i={i} ({x},{y},{z}) biome={} here={:?}",
                is_deep_dark_at(state, x, y, z) as u8,
                region.get(x, y, z)
            );
        }
        let biome_ok = is_deep_dark_at(state, x, y, z);
        if !biome_ok {
            continue;
        }
        SCULK_BIOME_OK.fetch_add(1, Ordering::Relaxed);
        let here = region.get(x, y, z);
        let spread = can_spread_from(region, x, y, z);
        if !spread {
            if dump && y >= -40 && y < -8 && matches!(here, BlockId::Air | BlockId::CaveAir | BlockId::Water) {
                let mut nbs = Vec::new();
                for &(dx, dy, dz) in &DIRS {
                    nbs.push(region.get(x + dx, y + dy, z + dz));
                }
                eprintln!(
                    "spread_fail o=({ox0},{oz0}) i={i} ({x},{y},{z}) here={here:?} nbs={nbs:?}"
                );
            }
            continue;
        }
        SCULK_SPREAD_OK.fetch_add(1, Ordering::Relaxed);
        if dump {
            rng.reset_draw_count();
            if ox0 == 96 && oz0 == -32 && i == 0 {
                eprint!("nbhd ({x},{y},{z}):");
                for dy in -1..=1 {
                    for dz in -1..=1 {
                        for dx in -1..=1 {
                            eprint!(
                                " ({},{},{})={:?}",
                                x + dx,
                                y + dy,
                                z + dz,
                                region.get(x + dx, y + dy, z + dz)
                            );
                        }
                    }
                }
                eprintln!();
            }
        }
        let trace_this = ox0 == 96
            && oz0 == -32
            && i == 0
            && std::env::var_os("NEUTRON_SCULK_TRACE_W").is_some();
        if trace_this {
            SET_TRACE.store(true, Ordering::Relaxed);
        }
        run_patch(rng, region, faces, x, y, z, cfg);
        if trace_this {
            SET_TRACE.store(false, Ordering::Relaxed);
        }
        if dump {
            eprintln!(
                "sculk_patch o=({ox0},{oz0}) i={i} ({x},{y},{z}) here={here:?} below={:?} draws={}",
                region.get(x, y - 1, z),
                rng.draw_count()
            );
            if ox0 == 96 && oz0 == -32 && i == 0 {
                let mut sc = 0u32;
                let mut vn = 0u32;
                let mut sn = 0u32;
                let mut sh = 0u32;
                let mut cat = 0u32;
                for dy in -16..=16 {
                    for dz in -16..=16 {
                        for dx in -16..=16 {
                            match region.get(x + dx, y + dy, z + dz) {
                                BlockId::Sculk => sc += 1,
                                BlockId::SculkVein => vn += 1,
                                BlockId::SculkSensor => {
                                    sn += 1;
                                    eprintln!("  sensor ({},{},{})", x + dx, y + dy, z + dz);
                                }
                                BlockId::SculkShrieker => {
                                    sh += 1;
                                    eprintln!("  shrieker ({},{},{})", x + dx, y + dy, z + dz);
                                }
                                BlockId::SculkCatalyst => cat += 1,
                                _ => {}
                            }
                        }
                    }
                }
                eprintln!(
                    "first_patch_r16 sculk={sc} vein={vn} sensor={sn} shrieker={sh} cat={cat}"
                );
            }
        }
    }
}

pub(super) fn can_spread_from(region: &RegionBuf, x: i32, y: i32, z: i32) -> bool {
    let b = region.get(x, y, z);
    if is_sculk_behaviour(b) {
        return true;
    }
    // Vanilla: air OR water source; any neighbour with full collision shape.
    // SCULK is a full cube — must count (cascade after earlier patches).
    if !matches!(b, BlockId::Air | BlockId::CaveAir | BlockId::Water) {
        return false;
    }
    DIRS.iter()
        .any(|&(dx, dy, dz)| is_collision_full_block(region.get(x + dx, y + dy, z + dz)))
}
