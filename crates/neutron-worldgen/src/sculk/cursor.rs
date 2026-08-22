//! Charge cursors — SculkBlock/ChargeCursor tick simulation.
use super::*;
use super::blocks::*;
use super::gates::*;
use super::place::*;
use crate::feature_catalog;
use crate::feature_rng::FeatureRandom;
use crate::region_buf::RegionBuf;
use crate::surface::BlockId;
use crate::worldgen::WorldgenState;

pub(super) struct Cursor {
    pub(super) x: i32,
    pub(super) y: i32,
    pub(super) z: i32,
    pub(super) charge: i32,
    pub(super) decay_delay: i32,
    pub(super) update_delay: i32,
    pub(super) facings: Option<u8>,
}

pub(super) fn run_patch(
    rng: &mut FeatureRandom,
    region: &mut RegionBuf,
    faces: &mut FaceMap,
    ox: i32,
    oy: i32,
    oz: i32,
    cfg: &PatchConfig,
) {
    let total = cfg.spread_rounds + cfg.growth_rounds;
    for round in 0..total {
        let mut cursors: Vec<Cursor> = Vec::new();
        for _ in 0..cfg.charge_count {
            let mut charge = cfg.amount_per_charge;
            while charge > 0 && cursors.len() < MAX_CURSORS {
                let cur = charge.min(1000);
                cursors.push(Cursor {
                    x: ox,
                    y: oy,
                    z: oz,
                    charge: cur,
                    decay_delay: 1,
                    update_delay: 0,
                    facings: None,
                });
                charge -= cur;
            }
        }
        let spread_veins = round < cfg.spread_rounds;
        for attempt in 0..cfg.spread_attempts {
            ATT_I.store(attempt, Ordering::Relaxed);
            update_cursors(rng, region, faces, ox, oy, oz, &mut cursors, spread_veins);
            dump_tick_world_if_requested(region, faces, attempt);
            if cursor_draws_on() {
                for c in cursors.iter() {
                    eprintln!(
                        "CURA i={} att={} {},{},{} ch={} dec={} upd={} faces={}",
                        PATCH_I.load(Ordering::Relaxed),
                        attempt,
                        c.x,
                        c.y,
                        c.z,
                        c.charge,
                        c.decay_delay,
                        c.update_delay,
                        c.facings.map(|f| f as i32).unwrap_or(-1)
                    );
                }
            }
            if std::env::var_os("NEUTRON_SCULK_STEPS").is_some()
                && (attempt < 8
                    || attempt == 15
                    || attempt == 31
                    || attempt == 63
                    || std::env::var_os("NEUTRON_SCULK_ALL_TICKS").is_some())
            {
                let mut sc = 0u32;
                let mut vn = 0u32;
                for z in region.origin_z..region.origin_z + region.side {
                    for y in WORLD_BOTTOM..crate::generator::WORLD_TOP {
                        for x in region.origin_x..region.origin_x + region.side {
                            match region.get(x, y, z) {
                                BlockId::Sculk => sc += 1,
                                BlockId::SculkVein => vn += 1,
                                _ => {}
                            }
                        }
                    }
                }
                eprintln!(
                    "after {} cursors={} sculk={} vein={} nextBits={}",
                    attempt + 1,
                    cursors.len(),
                    sc,
                    vn,
                    rng.draw_count()
                );
                if attempt == 2 {
                    eprint!("after3 sculk:");
                    for z in region.origin_z..region.origin_z + region.side {
                        for y in (oy - 16)..=(oy + 16) {
                            for x in region.origin_x..region.origin_x + region.side {
                                if region.get(x, y, z) == BlockId::Sculk {
                                    eprint!(" {x},{y},{z}");
                                }
                            }
                        }
                    }
                    eprintln!();
                    eprint!("after3 vein:");
                    for z in region.origin_z..region.origin_z + region.side {
                        for y in (oy - 16)..=(oy + 16) {
                            for x in region.origin_x..region.origin_x + region.side {
                                if region.get(x, y, z) == BlockId::SculkVein {
                                    let m = faces.get(&(x, y, z)).copied().unwrap_or(0);
                                    eprint!(" {x},{y},{z}#{m}");
                                }
                            }
                        }
                    }
                    eprintln!();
                    for c in cursors.iter() {
                        eprintln!("  live {},{},{} ch={}", c.x, c.y, c.z, c.charge);
                    }
                }
                if attempt < 1 {
                    eprint!("after1 vein:");
                    for z in region.origin_z..region.origin_z + region.side {
                        for y in (oy - 16)..=(oy + 16) {
                            for x in region.origin_x..region.origin_x + region.side {
                                if region.get(x, y, z) == BlockId::SculkVein {
                                    let m = faces.get(&(x, y, z)).copied().unwrap_or(0);
                                    eprint!(" {x},{y},{z}#{m}");
                                }
                            }
                        }
                    }
                    eprintln!();
                    eprint!("after1 sculk:");
                    for z in region.origin_z..region.origin_z + region.side {
                        for y in (oy - 16)..=(oy + 16) {
                            for x in region.origin_x..region.origin_x + region.side {
                                if region.get(x, y, z) == BlockId::Sculk {
                                    eprint!(" {x},{y},{z}");
                                }
                            }
                        }
                    }
                    eprintln!();
                }
            }
            // Vanilla still executes all spread_attempts after the cursor
            // list becomes empty. There are no further RNG draws in that
            // case, but the differential harness needs all 64 snapshots.
            if cursors.is_empty() && std::env::var_os("NEUTRON_SCULK_TICK_DUMPS").is_none() {
                break;
            }
            if std::env::var_os("NEUTRON_SCULK_DUMP_PATCH").is_some()
                && (attempt == 63
                    || attempt == 37
                    || attempt == 5
                    || std::env::var_os("NEUTRON_SCULK_DUMP_ALL").is_some())
            {
                let mut cells: Vec<(i32, i32, i32, u8, &str)> = Vec::new();
                for z in region.origin_z..region.origin_z + region.side {
                    for y in WORLD_BOTTOM..crate::generator::WORLD_TOP {
                        for x in region.origin_x..region.origin_x + region.side {
                            match region.get(x, y, z) {
                                BlockId::Sculk => cells.push((x, y, z, 0, "sculk")),
                                BlockId::SculkVein => cells.push((
                                    x,
                                    y,
                                    z,
                                    faces.get(&(x, y, z)).copied().unwrap_or(0),
                                    "vein",
                                )),
                                _ => {}
                            }
                        }
                    }
                }
                cells.sort();
                eprintln!(
                    "PATCHEND a={} o=({ox},{oy},{oz}) sculk={} vein={}",
                    attempt,
                    cells.iter().filter(|c| c.4 == "sculk").count(),
                    cells.iter().filter(|c| c.4 == "vein").count()
                );
                for (x, y, z, m, k) in cells {
                    eprintln!("CELL {x},{y},{z} {k}#{m}");
                }
            }
        }
    }

    let dump = std::env::var_os("NEUTRON_SCULK_PATCHES").is_some();
    let roll = rng.next_f32();
    LAST_CATALYST_ROLL.store((roll * 1_000_000.0) as u32, Ordering::Relaxed);
    if dump {
        eprintln!(
            "catalyst_roll={roll:.6} <= {} ? {} below={:?}",
            cfg.catalyst_chance,
            roll <= cfg.catalyst_chance,
            region.get(ox, oy - 1, oz)
        );
    }
    if roll <= cfg.catalyst_chance {
        // isCollisionShapeFullBlock below
        if is_collision_full_block(region.get(ox, oy - 1, oz)) {
            region.set(ox, oy, oz, BlockId::SculkCatalyst);
            faces.remove(&(ox, oy, oz));
        }
    }
    // SculkPatchFeature.place: extraRareGrowths.sample then offset(nextInt(5)-2, 0, nextInt(5)-2).
    // deep_dark JSON is 0 (ConstantInt.sample — no RNG).
    let extra = cfg.extra_rare_growths;
    for _ in 0..extra {
        let px = ox + rng.next_int(5) - 2;
        let pz = oz + rng.next_int(5) - 2;
        if region.get(px, oy, pz) != BlockId::Air {
            continue;
        }
        if is_collision_full_block(region.get(px, oy - 1, pz)) {
            region.set(px, oy, pz, BlockId::SculkShrieker);
        }
    }
}

fn dump_tick_world_if_requested(region: &RegionBuf, faces: &FaceMap, attempt: i32) {
    if std::env::var_os("NEUTRON_SCULK_TICK_DUMPS").is_none() {
        return;
    }
    let selected = std::env::var("NEUTRON_SCULK_TICK_PATCH")
        .ok()
        .and_then(|s| s.parse::<i32>().ok());
    if selected.is_some_and(|i| i != PATCH_I.load(Ordering::Relaxed)) {
        return;
    }

    let mut cells = Vec::new();
    for z in region.origin_z..region.origin_z + region.side {
        for y in WORLD_BOTTOM..crate::generator::WORLD_TOP {
            for x in region.origin_x..region.origin_x + region.side {
                match region.get(x, y, z) {
                    BlockId::Sculk => cells.push(format!("{x},{y},{z} sculk#0")),
                    BlockId::SculkVein => {
                        let mask = faces.get(&(x, y, z)).copied().unwrap_or(0);
                        cells.push(format!("{x},{y},{z} vein#{mask}"));
                    }
                    _ => {}
                }
            }
        }
    }
    cells.sort_unstable();
    let dir = std::env::var_os("NEUTRON_SCULK_TICK_DUMPS_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_default();
    let path = dir.join(format!(
        "rust-tickfull-{}-{}.txt",
        PATCH_I.load(Ordering::Relaxed),
        attempt
    ));
    std::fs::write(path, cells.join("\n") + "\n").expect("write sculk tick dump");
}

pub(super) fn update_cursors(
    rng: &mut FeatureRandom,
    region: &mut RegionBuf,
    faces: &mut FaceMap,
    ox: i32,
    oy: i32,
    oz: i32,
    cursors: &mut Vec<Cursor>,
    spread_veins: bool,
) {
    let log = cursor_draws_on();
    let mut next: Vec<Cursor> = Vec::new();
    for (idx, mut c) in cursors.drain(..).enumerate() {
        let chess = (c.x - ox).abs().max((c.y - oy).abs()).max((c.z - oz).abs());
        if chess > 1024 {
            continue;
        }
        let before = rng.draw_count();
        if log {
            eprintln!(
                "CDCB i={} att={} cur={} {},{},{} ch={} dec={} upd={} faces={}",
                PATCH_I.load(Ordering::Relaxed),
                ATT_I.load(Ordering::Relaxed),
                idx,
                c.x,
                c.y,
                c.z,
                c.charge,
                c.decay_delay,
                c.update_delay,
                c.facings.map(|f| f as i32).unwrap_or(-1)
            );
        }
        cursor_update(rng, region, faces, ox, oy, oz, &mut c, spread_veins);
        if log {
            eprintln!(
                "CDCE i={} att={} cur={} n={} ch={} pos={},{},{} faces={}",
                PATCH_I.load(Ordering::Relaxed),
                ATT_I.load(Ordering::Relaxed),
                idx,
                rng.draw_count() - before,
                c.charge,
                c.x,
                c.y,
                c.z,
                c.facings.map(|f| f as i32).unwrap_or(-1)
            );
        }
        if c.charge > 0 {
            next.push(c);
        }
    }
    // Worldgen keeps multiple cursors at same pos (no merge when isWorldGeneration)
    if next.len() > MAX_CURSORS {
        next.truncate(MAX_CURSORS);
    }
    *cursors = next;
}

/// ChargeCursor.update
fn cursor_update(
    rng: &mut FeatureRandom,
    region: &mut RegionBuf,
    faces: &mut FaceMap,
    ox: i32,
    oy: i32,
    oz: i32,
    c: &mut Cursor,
    spread_veins: bool,
) {
    if c.charge <= 0 {
        return;
    }
    if c.update_delay > 0 {
        c.update_delay -= 1;
        return;
    }

    let mut here = region.get(c.x, c.y, c.z);
    if std::env::var_os("NEUTRON_SCULK_STEPS").is_some() {
        eprintln!(
            "  upd ({},{},{}) here={here:?} ch={} dec={} faces={:?}",
            c.x, c.y, c.z, c.charge, c.decay_delay, c.facings
        );
    }
    // updateDecayDelay uses the behaviour from *before* the move
    // (and after attemptSpreadVein re-read if canChangeBlockStateOnSpread).
    let mut behaviour_is_sculk = is_sculk_behaviour(here);

    // Vanilla keeps `currentState` as a BlockState SNAPSHOT read here (A),
    // refreshed only after attemptSpreadVein when canChangeBlockStateOnSpread
    // (SculkBlock overrides it to false; veins/DEFAULT keep true). Both
    // onDischarged calls and the final availableFaces use that SNAPSHOT, not
    // the live state — faces added mid-tick (e.g. by attemptPlaceSculk's
    // spreadAll from a new support) are WIPED by the discharge rewrite.
    let mut stale_vein_mask = if here == BlockId::SculkVein {
        faces.get(&(c.x, c.y, c.z)).copied().unwrap_or(0)
    } else {
        0
    };

    if spread_veins {
        if attempt_spread_vein(region, faces, c.x, c.y, c.z, c.facings, here) {
            // SculkBlock.canChangeBlockStateOnSpread == false; default is true.
            if here != BlockId::Sculk {
                here = region.get(c.x, c.y, c.z);
                behaviour_is_sculk = is_sculk_behaviour(here);
                stale_vein_mask = if here == BlockId::SculkVein {
                    faces.get(&(c.x, c.y, c.z)).copied().unwrap_or(0)
                } else {
                    0
                };
            }
        }
    }

    c.charge = attempt_use_charge(rng, region, faces, c, here, spread_veins, ox, oy, oz);
    if c.charge <= 0 {
        if here == BlockId::SculkVein {
            on_discharged_snapshot(region, faces, c.x, c.y, c.z, stale_vein_mask);
        }
        return;
    }

    let mut moved = false;
    if let Some((nx, ny, nz)) = get_valid_movement(rng, region, faces, c.x, c.y, c.z) {
        if here == BlockId::SculkVein {
            on_discharged_snapshot(region, faces, c.x, c.y, c.z, stale_vein_mask);
        }
        c.x = nx;
        c.y = ny;
        c.z = nz;
        moved = true;
        // closerThan(Vec3i(originX, cursorY, originZ), 15) → distSqr < 225
        let dx = (c.x - ox) as f64;
        let dz = (c.z - oz) as f64;
        if dx * dx + dz * dz >= WORLDGEN_MAX_DIST * WORLDGEN_MAX_DIST {
            c.charge = 0;
            return;
        }
        here = region.get(c.x, c.y, c.z);
    }

    // MultifaceBlock.availableFaces(currentState): when the cursor did NOT
    // move, currentState is still the (A/B) SNAPSHOT — faces added mid-tick
    // must not leak into cursor.facings. After a move it is a fresh read.
    // Empty set on non-multiface SculkBehaviour (sculk); catalyst/sensor/
    // shrieker are not SculkBehaviour — facings left unchanged.
    if here == BlockId::SculkVein {
        c.facings = Some(if moved {
            faces.get(&(c.x, c.y, c.z)).copied().unwrap_or(0)
        } else {
            stale_vein_mask
        });
    } else if here == BlockId::Sculk {
        c.facings = Some(0);
    }
    if behaviour_is_sculk {
        c.decay_delay = 1;
    } else {
        c.decay_delay = (c.decay_delay - 1).max(0);
    }
    c.update_delay = 1; // getSculkSpreadDelay
}
