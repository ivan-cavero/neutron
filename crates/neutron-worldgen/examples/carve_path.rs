//! Seed 424242: replay cave/canyon worm motion for sources that can
//! reach chunks (0,0) or (0,1). Geometric path ignores `can_reach`;
//! abort / ellipsoid early-out are evaluated per target afterwards.
//!
//! Duplicates `mth_sin`/`mth_cos`/`can_reach`/`create_tunnel`/`do_canyon`
//! motion from `carvers.rs` (examples cannot call `pub(crate)` items).
//! Does not call `apply_carvers_region`.
//!
//!   cargo run --release -p neutron-worldgen --example carve_path

use neutron_worldgen::generator::WORLD_BOTTOM;
use neutron_worldgen::legacy_rng::LegacyRandom;
use std::sync::OnceLock;

const APPLY_RANGE: i32 = 8;
const SEED: i64 = 424242;
const RANGE_BLOCKS: i32 = 7 * 16;
const BAND_LO: f64 = -16.0;
const BAND_HI: f64 = 16.0;

const SIN_SCALE: f64 = 10430.378350470453;
const COS_OFFSET: f64 = 16384.0;

const WATER_CELLS: [(i32, i32, i32); 11] = [
    (12, 1, 15),
    (10, 2, 15),
    (8, 3, 14),
    (2, 5, 14),
    (5, 5, 14),
    (1, 5, 15),
    (8, 3, 15),
    (2, 5, 15),
    (5, 5, 15),
    (1, 6, 21),
    (3, 6, 23),
];

struct CaveCfg {
    name: &'static str,
    probability: f32,
    y_min: i32,
    y_max: i32,
}

fn sin_table() -> &'static [f32; 65536] {
    static TABLE: OnceLock<[f32; 65536]> = OnceLock::new();
    TABLE.get_or_init(|| {
        let mut t = [0f32; 65536];
        for i in 0..65536 {
            t[i] = (i as f64 / SIN_SCALE).sin() as f32;
        }
        t
    })
}

#[inline]
fn mth_sin_d(v: f64) -> f32 {
    let idx = ((v * SIN_SCALE) as i64 as u64 & 0xFFFF) as usize;
    sin_table()[idx]
}

#[inline]
fn mth_cos_d(v: f64) -> f32 {
    let idx = ((v * SIN_SCALE + COS_OFFSET) as i64 as u64 & 0xFFFF) as usize;
    sin_table()[idx]
}

#[inline]
fn mth_sin_f(v: f32) -> f32 {
    mth_sin_d(v as f64)
}

#[inline]
fn mth_cos_f(v: f32) -> f32 {
    mth_cos_d(v as f64)
}

fn sample_y(rng: &mut LegacyRandom, y_min: i32, y_max: i32) -> i32 {
    if y_max <= y_min {
        return y_min;
    }
    y_min + rng.next_int(y_max - y_min + 1)
}

fn get_thickness(rng: &mut LegacyRandom) -> f32 {
    let mut t = rng.next_f32() * 2.0 + rng.next_f32();
    if rng.next_int(10) == 0 {
        t *= rng.next_f32() * rng.next_f32() * 3.0 + 1.0;
    }
    t
}

fn sample_trapezoid_thickness(rng: &mut LegacyRandom) -> f32 {
    let t = (rng.next_f32() + rng.next_f32()) * 3.0;
    t.clamp(0.0, 6.0)
}

/// `WorldCarver.canReach` — `carvers.rs` `can_reach`.
fn can_reach(
    target_cx: i32,
    target_cz: i32,
    x: f64,
    z: f64,
    branch_index: i32,
    branch_count: i32,
    thickness: f32,
) -> bool {
    let mid_x = (target_cx * 16 + 8) as f64;
    let mid_z = (target_cz * 16 + 8) as f64;
    let dx = x - mid_x;
    let dz = z - mid_z;
    let remaining = (branch_count - branch_index) as f64;
    let max_r = (thickness + 2.0 + 16.0) as f64;
    dx * dx + dz * dz - remaining * remaining <= max_r * max_r
}

fn ellipsoid_early_out(target_cx: i32, target_cz: i32, cx: f64, cz: f64, horiz: f64) -> bool {
    let mid_x = (target_cx * 16 + 8) as f64;
    let mid_z = (target_cz * 16 + 8) as f64;
    let reach = 16.0 + horiz * 2.0;
    (cx - mid_x).abs() > reach || (cz - mid_z).abs() > reach
}

/// WorldCarver.carveEllipsoid local x/z range in the target chunk is non-empty.
fn local_xz_nonempty(target_cx: i32, target_cz: i32, cx: f64, cz: f64, horiz: f64) -> bool {
    let min_bx = target_cx * 16;
    let min_bz = target_cz * 16;
    let min_lx = ((cx - horiz).floor() as i32 - min_bx - 1).max(0);
    let max_lx = ((cx + horiz).floor() as i32 - min_bx).min(15);
    let min_lz = ((cz - horiz).floor() as i32 - min_bz - 1).max(0);
    let max_lz = ((cz + horiz).floor() as i32 - min_bz).min(15);
    min_lx <= max_lx && min_lz <= max_lz
}

fn chunk_coord(v: f64) -> i32 {
    (v.floor() as i32).div_euclid(16)
}

fn in_band_y(y: f64) -> bool {
    y >= BAND_LO && y < BAND_HI
}

/// Ellipsoid Y range overlaps [-16,16). Vanilla loop is `for (y = maxY; y > minY; --)`.
fn y_overlaps_band(cy: f64, vert: f64) -> bool {
    let min_y = (cy - vert).floor() as i32 - 1;
    let max_y = (cy + vert).floor() as i32 + 1;
    max_y > min_y && max_y > -16 && min_y < 16
}

fn dist3(x: f64, y: f64, z: f64, cx: i32, cy: i32, cz: i32) -> f64 {
    let dx = x - cx as f64;
    let dy = y - cy as f64;
    let dz = z - cz as f64;
    (dx * dx + dy * dy + dz * dz).sqrt()
}

#[derive(Clone)]
struct Closest {
    dist: f64,
    sx: f64,
    sy: f64,
    sz: f64,
    kind: &'static str,
    scx: i32,
    scz: i32,
    px: i32,
    py: i32,
    pz: i32,
    tun: i32,
}

#[derive(Clone, Copy)]
struct StartRef {
    kind: &'static str,
    scx: i32,
    scz: i32,
    x: i32,
    y: i32,
    z: i32,
}

struct WormStats {
    kind: &'static str,
    scx: i32,
    scz: i32,
    px: i32,
    py: i32,
    pz: i32,
    tun: i32,
    steps: i32,
    ymin: i32,
    ymax: i32,
    in00: u32,
    in01: u32,
    band00: u32,
    band01: u32,
    abort00: bool,
    abort01: bool,
    early: u32,
    carve_band00: u32,
    carve_band01: u32,
    /// Geometric band steps that happen after that target's can_reach abort.
    post_abort_band00: u32,
    post_abort_band01: u32,
    /// Steps that pass ellipsoid early-out vs target and whose Y overlaps [-16,16)
    /// (center may be outside the target chunk). This is what CARVE_BAND_CELL sees
    /// after can_reach, before the local x/z empty-range check.
    reach00: u32,
    reach01: u32,
    write00: u32,
    write01: u32,
}

struct Acc {
    closest: [Option<Closest>; 11],
    nearest_start: [Option<(f64, StartRef)>; 11],
    worms: Vec<WormStats>,
    starts: u32,
    starts_n32: u32,
    starts_n32_any_band: u32,
}

impl Acc {
    fn new() -> Self {
        Self {
            closest: Default::default(),
            nearest_start: Default::default(),
            worms: Vec::new(),
            starts: 0,
            starts_n32: 0,
            starts_n32_any_band: 0,
        }
    }

    fn note_start(&mut self, kind: &'static str, scx: i32, scz: i32, x: i32, y: i32, z: i32) {
        self.starts += 1;
        let sr = StartRef {
            kind,
            scx,
            scz,
            x,
            y,
            z,
        };
        for (i, &(wx, wy, wz)) in WATER_CELLS.iter().enumerate() {
            let d = dist3(x as f64, y as f64, z as f64, wx, wy, wz);
            match &self.nearest_start[i] {
                Some((best, _)) if *best <= d => {}
                _ => self.nearest_start[i] = Some((d, StartRef { ..sr })),
            }
        }
    }

    fn note_step(
        &mut self,
        kind: &'static str,
        scx: i32,
        scz: i32,
        px: i32,
        py: i32,
        pz: i32,
        tun: i32,
        x: f64,
        y: f64,
        z: f64,
    ) {
        for (i, &(wx, wy, wz)) in WATER_CELLS.iter().enumerate() {
            let d = dist3(x, y, z, wx, wy, wz);
            let better = match &self.closest[i] {
                Some(c) => d < c.dist,
                None => true,
            };
            if better {
                self.closest[i] = Some(Closest {
                    dist: d,
                    sx: x,
                    sy: y,
                    sz: z,
                    kind,
                    scx,
                    scz,
                    px,
                    py,
                    pz,
                    tun,
                });
            }
        }
    }
}

/// Consume canyon widthFactors RNG (`init_width_factors` in carvers.rs).
fn consume_width_factors(rng: &mut LegacyRandom, gen_depth: i32, width_smoothness: i32) {
    let mut _w = 1.0f32;
    for y in 0..gen_depth {
        if y == 0 || rng.next_int(width_smoothness) == 0 {
            _w = 1.0 + rng.next_f32() * rng.next_f32();
        }
    }
}

fn update_vertical_radius(
    rng: &mut LegacyRandom,
    base_vert: f64,
    branch_count: f32,
    step: f32,
) -> f64 {
    let t = 1.0 - 2.0 * (0.5 - step / branch_count).abs();
    let factor = 1.0f32 + 0.0 * t;
    let r = 0.75 + rng.next_f32() * 0.25;
    base_vert * factor as f64 * r as f64
}

struct TunnelArgs {
    kind: &'static str,
    scx: i32,
    scz: i32,
    px: i32,
    py: i32,
    pz: i32,
    seed: i64,
    x: f64,
    y: f64,
    z: f64,
    horiz_mult: f64,
    vert_mult: f64,
    thickness: f32,
    yaw: f32,
    pitch: f32,
    branch_index: i32,
    branch_count: i32,
    y_scale: f64,
    /// If parent aborted vs target before the fork, children would not run for that target.
    spawn00: bool,
    spawn01: bool,
}

fn simulate_cave_tunnel(acc: &mut Acc, next_tun: &mut i32, a: TunnelArgs) {
    let tun = *next_tun;
    *next_tun += 1;

    let mut rng = LegacyRandom::new(a.seed);
    let steeper_at = rng.next_int(a.branch_count / 2) + a.branch_count / 4;
    let rare = rng.next_int(6) == 0;
    let mut yaw_vel = 0.0f32;
    let mut pitch_vel = 0.0f32;
    let mut yaw = a.yaw;
    let mut pitch = a.pitch;
    let mut x = a.x;
    let mut y = a.y;
    let mut z = a.z;

    let mut steps = 0i32;
    let mut ymin = i32::MAX;
    let mut ymax = i32::MIN;
    let mut in00 = 0u32;
    let mut in01 = 0u32;
    let mut band00 = 0u32;
    let mut band01 = 0u32;
    let mut abort00 = false;
    let mut abort01 = false;
    let mut alive00 = a.spawn00;
    let mut alive01 = a.spawn01;
    let mut early = 0u32;
    let mut carve_band00 = 0u32;
    let mut carve_band01 = 0u32;
    let mut post_abort_band00 = 0u32;
    let mut post_abort_band01 = 0u32;
    let mut reach00 = 0u32;
    let mut reach01 = 0u32;
    let mut write00 = 0u32;
    let mut write01 = 0u32;

    if !a.spawn00 {
        abort00 = true;
    }
    if !a.spawn01 {
        abort01 = true;
    }

    let mut i = a.branch_index;
    let mut forked = false;
    while i < a.branch_count {
        let angle = (3.1415927f32 * i as f32 / a.branch_count as f32) as f64;
        let sin_v = mth_sin_d(angle);
        let horiz_base = 1.5 + (sin_v * a.thickness) as f64;
        let vert_base = horiz_base * a.y_scale;
        let horiz = horiz_base * a.horiz_mult;
        let vert = vert_base * a.vert_mult;

        let cos_pitch = mth_cos_f(pitch);
        x += (mth_cos_f(yaw) * cos_pitch) as f64;
        y += mth_sin_f(pitch) as f64;
        z += (mth_sin_f(yaw) * cos_pitch) as f64;

        pitch *= if rare { 0.92 } else { 0.7 };
        pitch += pitch_vel * 0.1;
        yaw += yaw_vel * 0.1;
        pitch_vel *= 0.9;
        yaw_vel *= 0.75;
        pitch_vel += (rng.next_f32() - rng.next_f32()) * rng.next_f32() * 2.0;
        yaw_vel += (rng.next_f32() - rng.next_f32()) * rng.next_f32() * 4.0;

        steps += 1;
        let y_i = y.floor() as i32;
        ymin = ymin.min(y_i);
        ymax = ymax.max(y_i);
        acc.note_step(a.kind, a.scx, a.scz, a.px, a.py, a.pz, tun, x, y, z);

        let cx = chunk_coord(x);
        let cz = chunk_coord(z);
        let band = in_band_y(y);
        if cx == 0 && cz == 0 {
            in00 += 1;
            if band {
                band00 += 1;
                if !alive00 {
                    post_abort_band00 += 1;
                }
            }
        }
        if cx == 0 && cz == 1 {
            in01 += 1;
            if band {
                band01 += 1;
                if !alive01 {
                    post_abort_band01 += 1;
                }
            }
        }

        if i == steeper_at && a.thickness > 1.0 {
            let child_seed1 = rng.next_long();
            let thick1 = rng.next_f32() * 0.5 + 0.5;
            let child_seed2 = rng.next_long();
            let thick2 = rng.next_f32() * 0.5 + 0.5;
            acc.worms.push(WormStats {
                kind: a.kind,
                scx: a.scx,
                scz: a.scz,
                px: a.px,
                py: a.py,
                pz: a.pz,
                tun,
                steps,
                ymin: if steps == 0 { a.py } else { ymin },
                ymax: if steps == 0 { a.py } else { ymax },
                in00,
                in01,
                band00,
                band01,
                abort00,
                abort01,
                early,
                carve_band00,
                carve_band01,
                post_abort_band00,
                post_abort_band01,
                reach00,
                reach01,
                write00,
                write01,
            });
            simulate_cave_tunnel(
                acc,
                next_tun,
                TunnelArgs {
                    kind: a.kind,
                    scx: a.scx,
                    scz: a.scz,
                    px: a.px,
                    py: a.py,
                    pz: a.pz,
                    seed: child_seed1,
                    x,
                    y,
                    z,
                    horiz_mult: a.horiz_mult,
                    vert_mult: a.vert_mult,
                    thickness: thick1,
                    yaw: yaw - 1.5707964,
                    pitch: pitch / 3.0,
                    branch_index: i,
                    branch_count: a.branch_count,
                    y_scale: 1.0,
                    spawn00: alive00,
                    spawn01: alive01,
                },
            );
            simulate_cave_tunnel(
                acc,
                next_tun,
                TunnelArgs {
                    kind: a.kind,
                    scx: a.scx,
                    scz: a.scz,
                    px: a.px,
                    py: a.py,
                    pz: a.pz,
                    seed: child_seed2,
                    x,
                    y,
                    z,
                    horiz_mult: a.horiz_mult,
                    vert_mult: a.vert_mult,
                    thickness: thick2,
                    yaw: yaw + 1.5707964,
                    pitch: pitch / 3.0,
                    branch_index: i,
                    branch_count: a.branch_count,
                    y_scale: 1.0,
                    spawn00: alive00,
                    spawn01: alive01,
                },
            );
            forked = true;
            break;
        }

        if rng.next_int(4) == 0 {
            i += 1;
            continue;
        }

        if alive00 && !can_reach(0, 0, x, z, i, a.branch_count, a.thickness) {
            alive00 = false;
            abort00 = true;
        }
        if alive01 && !can_reach(0, 1, x, z, i, a.branch_count, a.thickness) {
            alive01 = false;
            abort01 = true;
        }

        if alive00 {
            if ellipsoid_early_out(0, 0, x, z, horiz) {
                early += 1;
            } else {
                if y_overlaps_band(y, vert) {
                    reach00 += 1;
                    if local_xz_nonempty(0, 0, x, z, horiz) {
                        write00 += 1;
                    }
                }
                if cx == 0 && cz == 0 && band {
                    carve_band00 += 1;
                }
            }
        }
        if alive01 {
            if ellipsoid_early_out(0, 1, x, z, horiz) {
                early += 1;
            } else {
                if y_overlaps_band(y, vert) {
                    reach01 += 1;
                    if local_xz_nonempty(0, 1, x, z, horiz) {
                        write01 += 1;
                    }
                }
                if cx == 0 && cz == 1 && band {
                    carve_band01 += 1;
                }
            }
        }

        i += 1;
    }

    if !forked {
        if steps == 0 {
            ymin = a.py;
            ymax = a.py;
        }
        acc.worms.push(WormStats {
            kind: a.kind,
            scx: a.scx,
            scz: a.scz,
            px: a.px,
            py: a.py,
            pz: a.pz,
            tun,
            steps,
            ymin,
            ymax,
            in00,
            in01,
            band00,
            band01,
            abort00,
            abort01,
            early,
            carve_band00,
            carve_band01,
            post_abort_band00,
            post_abort_band01,
            reach00,
            reach01,
            write00,
            write01,
        });
    }
}

fn simulate_canyon(
    acc: &mut Acc,
    kind: &'static str,
    scx: i32,
    scz: i32,
    px: i32,
    py: i32,
    pz: i32,
    seed: i64,
    mut x: f64,
    mut y: f64,
    mut z: f64,
    thickness: f32,
    mut yaw: f32,
    mut pitch: f32,
    branch_count: i32,
    y_scale: f64,
) {
    let tun = 0;
    let mut rng = LegacyRandom::new(seed);
    consume_width_factors(&mut rng, 384, 3);
    let mut yaw_vel = 0.0f32;
    let mut pitch_vel = 0.0f32;

    let mut steps = 0i32;
    let mut ymin = i32::MAX;
    let mut ymax = i32::MIN;
    let mut in00 = 0u32;
    let mut in01 = 0u32;
    let mut band00 = 0u32;
    let mut band01 = 0u32;
    let mut abort00 = false;
    let mut abort01 = false;
    let mut alive00 = true;
    let mut alive01 = true;
    let mut early = 0u32;
    let mut carve_band00 = 0u32;
    let mut carve_band01 = 0u32;
    let mut post_abort_band00 = 0u32;
    let mut post_abort_band01 = 0u32;
    let mut reach00 = 0u32;
    let mut reach01 = 0u32;
    let mut write00 = 0u32;
    let mut write01 = 0u32;

    let mut i = 0i32;
    while i < branch_count {
        let sin_v = mth_sin_d((3.1415927f32 * i as f32 / branch_count as f32) as f64);
        let mut horiz = 1.5 + (sin_v * thickness) as f64;
        let mut vert = horiz * y_scale;
        let hrf = 0.75 + rng.next_f32() * 0.25;
        horiz *= hrf as f64;
        vert = update_vertical_radius(&mut rng, vert, branch_count as f32, i as f32);
        let _ = vert;

        let cos_pitch = mth_cos_f(pitch);
        x += (mth_cos_f(yaw) * cos_pitch) as f64;
        y += mth_sin_f(pitch) as f64;
        z += (mth_sin_f(yaw) * cos_pitch) as f64;

        pitch *= 0.7;
        pitch += pitch_vel * 0.05;
        yaw += yaw_vel * 0.05;
        pitch_vel *= 0.8;
        yaw_vel *= 0.5;
        pitch_vel += (rng.next_f32() - rng.next_f32()) * rng.next_f32() * 2.0;
        yaw_vel += (rng.next_f32() - rng.next_f32()) * rng.next_f32() * 4.0;

        steps += 1;
        let y_i = y.floor() as i32;
        ymin = ymin.min(y_i);
        ymax = ymax.max(y_i);
        acc.note_step(kind, scx, scz, px, py, pz, tun, x, y, z);

        let cx = chunk_coord(x);
        let cz = chunk_coord(z);
        let band = in_band_y(y);
        if cx == 0 && cz == 0 {
            in00 += 1;
            if band {
                band00 += 1;
                if !alive00 {
                    post_abort_band00 += 1;
                }
            }
        }
        if cx == 0 && cz == 1 {
            in01 += 1;
            if band {
                band01 += 1;
                if !alive01 {
                    post_abort_band01 += 1;
                }
            }
        }

        if rng.next_int(4) == 0 {
            i += 1;
            continue;
        }

        if alive00 && !can_reach(0, 0, x, z, i, branch_count, thickness) {
            alive00 = false;
            abort00 = true;
        }
        if alive01 && !can_reach(0, 1, x, z, i, branch_count, thickness) {
            alive01 = false;
            abort01 = true;
        }

        if alive00 {
            if ellipsoid_early_out(0, 0, x, z, horiz) {
                early += 1;
            } else {
                if y_overlaps_band(y, vert) {
                    reach00 += 1;
                    if local_xz_nonempty(0, 0, x, z, horiz) {
                        write00 += 1;
                    }
                }
                if cx == 0 && cz == 0 && band {
                    carve_band00 += 1;
                }
            }
        }
        if alive01 {
            if ellipsoid_early_out(0, 1, x, z, horiz) {
                early += 1;
            } else {
                if y_overlaps_band(y, vert) {
                    reach01 += 1;
                    if local_xz_nonempty(0, 1, x, z, horiz) {
                        write01 += 1;
                    }
                }
                if cx == 0 && cz == 1 && band {
                    carve_band01 += 1;
                }
            }
        }

        i += 1;
    }

    if steps == 0 {
        ymin = py;
        ymax = py;
    }
    acc.worms.push(WormStats {
        kind,
        scx,
        scz,
        px,
        py,
        pz,
        tun,
        steps,
        ymin,
        ymax,
        in00,
        in01,
        band00,
        band01,
        abort00,
        abort01,
        early,
        carve_band00,
        carve_band01,
        post_abort_band00,
        post_abort_band01,
        reach00,
        reach01,
        write00,
        write01,
    });
}

fn print_worm(w: &WormStats) {
    let abort = if w.abort00 || w.abort01 { 1 } else { 0 };
    println!(
        "WORM {} source=({},{}) pos=({},{},{}) tun={} steps={} ymin={} ymax={} in00={} in01={} band00={} band01={} can_reach_abort={} early={} abort00={} abort01={} carve_band00={} carve_band01={} post_abort_band00={} post_abort_band01={} reach00={} reach01={} write00={} write01={}",
        w.kind,
        w.scx,
        w.scz,
        w.px,
        w.py,
        w.pz,
        w.tun,
        w.steps,
        w.ymin,
        w.ymax,
        w.in00,
        w.in01,
        w.band00,
        w.band01,
        abort,
        w.early,
        if w.abort00 { 1 } else { 0 },
        if w.abort01 { 1 } else { 0 },
        w.carve_band00,
        w.carve_band01,
        w.post_abort_band00,
        w.post_abort_band01,
        w.reach00,
        w.reach01,
        w.write00,
        w.write01,
    );
}

fn replay_cave_instance(
    acc: &mut Acc,
    rng: &mut LegacyRandom,
    kind: &'static str,
    scx: i32,
    scz: i32,
    x: i32,
    y: i32,
    z: i32,
) {
    let xf = x as f64;
    let yf = y as f64;
    let zf = z as f64;
    let horiz_mult = 0.7 + rng.next_f32() * (1.4 - 0.7);
    let vert_mult = 0.8 + rng.next_f32() * (1.3 - 0.8);
    let _floor_level = -1.0 + rng.next_f32() * (-0.4 - -1.0);

    let mut tunnel_count = 1;
    if rng.next_int(4) == 0 {
        let _y_scale = 0.1 + rng.next_f32() * (0.9 - 0.1);
        let _thickness = 1.0 + rng.next_f32() * 6.0;
        tunnel_count += rng.next_int(4);
    }

    let mut next_tun = 0i32;
    let worm_at = acc.worms.len();
    for _ in 0..tunnel_count {
        let yaw = rng.next_f32() * 6.2831855;
        let pitch = (rng.next_f32() - 0.5) / 4.0;
        let thickness = get_thickness(rng);
        let branch_count = RANGE_BLOCKS - rng.next_int(RANGE_BLOCKS / 4);
        let seed = rng.next_long();
        simulate_cave_tunnel(
            acc,
            &mut next_tun,
            TunnelArgs {
                kind,
                scx,
                scz,
                px: x,
                py: y,
                pz: z,
                seed,
                x: xf,
                y: yf,
                z: zf,
                horiz_mult: horiz_mult as f64,
                vert_mult: vert_mult as f64,
                thickness,
                yaw,
                pitch,
                branch_index: 0,
                branch_count,
                y_scale: 1.0,
                spawn00: true,
                spawn01: true,
            },
        );
    }
    for w in &acc.worms[worm_at..] {
        print_worm(w);
    }
}

fn main() {
    let cave_cfgs = [
        CaveCfg {
            name: "cave",
            probability: 0.15,
            y_min: WORLD_BOTTOM + 8,
            y_max: 180,
        },
        CaveCfg {
            name: "cave_extra",
            probability: 0.07,
            y_min: WORLD_BOTTOM + 8,
            y_max: 47,
        },
    ];

    println!("seed={SEED} APPLY_RANGE={APPLY_RANGE} source union cx=-8..=8 cz=-8..=9");
    println!(
        "cave y=[{},{}] p=0.15; cave_extra y=[{},{}] p=0.07; canyon y=[10,67] p=0.01",
        WORLD_BOTTOM + 8,
        180,
        WORLD_BOTTOM + 8,
        47
    );
    println!("path = create_tunnel / do_canyon motion; can_reach evaluated vs targets (0,0) and (0,1)");
    println!("band = chunk (0,0)/(0,1) and y in [-16,16); in00/in01 any Y; geometric path ignores can_reach abort");

    let mut acc = Acc::new();

    for source_cx in -APPLY_RANGE..=APPLY_RANGE {
        for source_cz in -APPLY_RANGE..=(APPLY_RANGE + 1) {
            for (index, cfg) in cave_cfgs.iter().enumerate() {
                let mut rng = LegacyRandom::new(0);
                rng.set_large_feature_seed(
                    SEED.wrapping_add(index as i64),
                    source_cx,
                    source_cz,
                );
                if rng.next_f32() > cfg.probability {
                    continue;
                }
                let a = rng.next_int(15) + 1;
                let b = rng.next_int(a) + 1;
                let cave_count = rng.next_int(b);
                if cave_count == 0 {
                    println!(
                        "START {} source=({},{}) cave_count=0 (no Y)",
                        cfg.name, source_cx, source_cz
                    );
                    continue;
                }
                for _ in 0..cave_count {
                    let x = source_cx * 16 + rng.next_int(16);
                    let y = sample_y(&mut rng, cfg.y_min, cfg.y_max);
                    let z = source_cz * 16 + rng.next_int(16);
                    println!(
                        "START {} source=({},{}) pos=({},{},{})",
                        cfg.name, source_cx, source_cz, x, y, z
                    );
                    acc.note_start(cfg.name, source_cx, source_cz, x, y, z);
                    let worm_at = acc.worms.len();
                    replay_cave_instance(
                        &mut acc,
                        &mut rng,
                        cfg.name,
                        source_cx,
                        source_cz,
                        x,
                        y,
                        z,
                    );
                    if (-32..0).contains(&y) {
                        acc.starts_n32 += 1;
                        let any_band = acc.worms[worm_at..]
                            .iter()
                            .any(|w| w.band00 > 0 || w.band01 > 0);
                        if any_band {
                            acc.starts_n32_any_band += 1;
                        }
                    }
                }
            }
            {
                let mut rng = LegacyRandom::new(0);
                rng.set_large_feature_seed(SEED.wrapping_add(2), source_cx, source_cz);
                if rng.next_f32() <= 0.01 {
                    let x = source_cx * 16 + rng.next_int(16);
                    let y = 10 + rng.next_int(67 - 10 + 1);
                    let z = source_cz * 16 + rng.next_int(16);
                    println!(
                        "START canyon source=({},{}) pos=({},{},{})",
                        source_cx, source_cz, x, y, z
                    );
                    acc.note_start("canyon", source_cx, source_cz, x, y, z);
                    let yaw = rng.next_f32() * 6.2831855;
                    let pitch = -0.125 + rng.next_f32() * 0.25;
                    let y_scale = 3.0f64;
                    let thickness = sample_trapezoid_thickness(&mut rng);
                    let distance_factor = 0.75 + rng.next_f32() * 0.25;
                    let branch_count = ((RANGE_BLOCKS as f32) * distance_factor) as i32;
                    let seed = rng.next_long();
                    let worm_at = acc.worms.len();
                    simulate_canyon(
                        &mut acc,
                        "canyon",
                        source_cx,
                        source_cz,
                        x,
                        y,
                        z,
                        seed,
                        x as f64,
                        y as f64,
                        z as f64,
                        thickness,
                        yaw,
                        pitch,
                        branch_count,
                        y_scale,
                    );
                    for w in &acc.worms[worm_at..] {
                        print_worm(w);
                    }
                    if (-32..0).contains(&y) {
                        acc.starts_n32 += 1;
                        let any_band = acc.worms[worm_at..]
                            .iter()
                            .any(|w| w.band00 > 0 || w.band01 > 0);
                        if any_band {
                            acc.starts_n32_any_band += 1;
                        }
                    }
                }
            }
        }
    }

    println!();
    println!("=== closest approach (any geometric tunnel step → water cell) ===");
    for (i, &(wx, wy, wz)) in WATER_CELLS.iter().enumerate() {
        match &acc.closest[i] {
            Some(c) => println!(
                "CLOSEST water=({},{},{}) dist={:.4} at step=({:.4},{:.4},{:.4}) from START {} source=({},{}) pos=({},{},{}) tun={}",
                wx, wy, wz, c.dist, c.sx, c.sy, c.sz, c.kind, c.scx, c.scz, c.px, c.py, c.pz, c.tun
            ),
            None => println!("CLOSEST water=({},{},{}) none", wx, wy, wz),
        }
        if let Some((d, s)) = &acc.nearest_start[i] {
            println!(
                "  nearest_START dist={:.4} {} source=({},{}) pos=({},{},{})",
                d, s.kind, s.scx, s.scz, s.x, s.y, s.z
            );
        }
    }

    let worms_band00 = acc.worms.iter().filter(|w| w.band00 > 0).count();
    let worms_band01 = acc.worms.iter().filter(|w| w.band01 > 0).count();
    let total_band00: u32 = acc.worms.iter().map(|w| w.band00).sum();
    let total_band01: u32 = acc.worms.iter().map(|w| w.band01).sum();
    let carve_band00: u32 = acc.worms.iter().map(|w| w.carve_band00).sum();
    let carve_band01: u32 = acc.worms.iter().map(|w| w.carve_band01).sum();
    let post00: u32 = acc.worms.iter().map(|w| w.post_abort_band00).sum();
    let post01: u32 = acc.worms.iter().map(|w| w.post_abort_band01).sum();
    let worms_carve00 = acc.worms.iter().filter(|w| w.carve_band00 > 0).count();
    let worms_carve01 = acc.worms.iter().filter(|w| w.carve_band01 > 0).count();
    let total_in00: u32 = acc.worms.iter().map(|w| w.in00).sum();
    let total_in01: u32 = acc.worms.iter().map(|w| w.in01).sum();
    let worms_in00 = acc.worms.iter().filter(|w| w.in00 > 0).count();
    let worms_in01 = acc.worms.iter().filter(|w| w.in01 > 0).count();

    println!();
    println!("SUMMARY");
    println!("starts={}", acc.starts);
    println!("worms_with_band00={worms_band00} worms_with_band01={worms_band01}");
    println!("total_band00_steps={total_band00} total_band01_steps={total_band01}");
    println!(
        "starts_y[-32,0)={} of which any_band_step={}",
        acc.starts_n32, acc.starts_n32_any_band
    );
    match &acc.closest[0] {
        Some(c) => println!(
            "closest_water: (12,1,15) dist={:.4} at step=({:.4},{:.4},{:.4})",
            c.dist, c.sx, c.sy, c.sz
        ),
        None => println!("closest_water: (12,1,15) none"),
    }
    println!("worms={} (incl. forks)", acc.worms.len());
    println!("worms_in00={worms_in00} worms_in01={worms_in01} total_in00_steps={total_in00} total_in01_steps={total_in01}");
    println!("carve_band00_steps={carve_band00} carve_band01_steps={carve_band01} (passed can_reach + not ellipsoid early-out, y[-16,16))");
    println!("worms_with_carve_band00={worms_carve00} worms_with_carve_band01={worms_carve01}");
    println!("post_abort_band00_steps={post00} post_abort_band01_steps={post01} (geometric band after can_reach abort)");
    let reach00: u32 = acc.worms.iter().map(|w| w.reach00).sum();
    let reach01: u32 = acc.worms.iter().map(|w| w.reach01).sum();
    let worms_reach00 = acc.worms.iter().filter(|w| w.reach00 > 0).count();
    let worms_reach01 = acc.worms.iter().filter(|w| w.reach01 > 0).count();
    println!(
        "reach00_steps={reach00} reach01_steps={reach01} (can_reach ok, not ellipsoid early-out, Y overlaps [-16,16); center may be outside target)"
    );
    println!("worms_with_reach00={worms_reach00} worms_with_reach01={worms_reach01}");
    let write00: u32 = acc.worms.iter().map(|w| w.write00).sum();
    let write01: u32 = acc.worms.iter().map(|w| w.write01).sum();
    println!(
        "write00_steps={write00} write01_steps={write01} (reach + local x/z range non-empty in target chunk → carve_ellipsoid Y loop would run)"
    );

    let path_enters = total_band00 > 0 || total_band01 > 0;
    let would_carve = carve_band00 > 0 || carve_band01 > 0;
    let abort_explains = path_enters && !would_carve && (post00 > 0 || post01 > 0);
    let early_explains = path_enters && !would_carve && post00 == 0 && post01 == 0;

    println!();
    println!("FINDING:");
    if path_enters {
        println!(
            "- do any Neutron worms enter (0,0) or (0,1) at y[-16,16)? YES geometric band00_steps={total_band00} band01_steps={total_band01} worms_band00={worms_band00} worms_band01={worms_band01}"
        );
    } else {
        println!(
            "- do any Neutron worms enter (0,0) or (0,1) at y[-16,16)? NO geometric band00_steps=0 band01_steps=0 (in00_steps={total_in00} in01_steps={total_in01} any-Y)"
        );
    }
    if would_carve {
        println!(
            "- CARVE_BAND_CELL=0 is NOT explained by path/can_reach: {carve_band00}+{carve_band01} steps would call carve_ellipsoid inside the band (see carvers.rs create_tunnel + can_reach + carve_ellipsoid early-out)"
        );
    } else if !path_enters {
        println!(
            "- CARVE_BAND_CELL=0 is explained by PATH: worms never walk into (0,0)/(0,1) at y[-16,16). can_reach abort does not matter for the band (no band steps to abort)."
        );
    } else if abort_explains {
        println!(
            "- CARVE_BAND_CELL=0 is explained by can_reach ABORT: geometric path enters the band, but those steps are after can_reach failed vs that target (post_abort_band00={post00} post_abort_band01={post01}). carvers.rs create_tunnel: after skip-1/4, `if !can_reach(...) return;` before carve_ellipsoid."
        );
    } else if early_explains {
        println!(
            "- CARVE_BAND_CELL=0 is explained by ellipsoid EARLY-OUT (or skip-1/4): path enters the band and can_reach does not abort before it, but carve_ellipsoid returns when |cx-mid|>16+horiz*2. carve_band=0."
        );
    } else {
        println!(
            "- CARVE_BAND_CELL=0: path enters band but no carve_ellipsoid would fire in-band (mix of abort/early-out/skip). post_abort_band00={post00} post_abort_band01={post01} carve_band00={carve_band00} carve_band01={carve_band01}"
        );
    }

    if let Some(c) = &acc.closest[0] {
        println!(
            "- nearest WORM STEP to water (12,1,15): dist={:.4} at step=({:.4},{:.4},{:.4}) from START {} source=({},{}) pos=({},{},{}) tun={}",
            c.dist, c.sx, c.sy, c.sz, c.kind, c.scx, c.scz, c.px, c.py, c.pz, c.tun
        );
    }
    if let Some((d, s)) = &acc.nearest_start[0] {
        println!(
            "- nearest START to water (12,1,15): dist={:.4} {} source=({},{}) pos=({},{},{})",
            d, s.kind, s.scx, s.scz, s.x, s.y, s.z
        );
    }
    println!(
        "- cite carvers.rs create_tunnel (move by cos(yaw)*cos(pitch), sin(pitch), sin(yaw)*cos(pitch); fork at steeper_at; skip 1/4; can_reach then carve_ellipsoid) and can_reach: dx²+dz² - remaining² <= (thickness+2+16)²; ellipsoid early-out |cx-midX|>16+horiz*2"
    );
}
