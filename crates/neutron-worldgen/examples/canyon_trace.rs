//! Canyon pipeline bit-trace for seed 424242, target chunk from args.
//! Mirrors `crates/neutron-worldgen/src/carvers.rs` canyon path exactly:
//! apply_carvers_for_target (:152-184 seed/order), canyon_from_chunk
//! (:598-640), sample_trapezoid_thickness (:646-654), do_canyon (:656-724),
//! init_width_factors (:726-736), update_vertical_radius (:739-752),
//! can_reach (:449-467), carve_ellipsoid_canyon range math (:766-787).
//! Emits raw-bit hex records mechanically comparable with
//! tools/worldgen-probe/src/ProbeCanyonTrace.java.
//!
//!   cargo build --release -p neutron-worldgen --example canyon_trace && \
//!   target/release/examples/canyon_trace 424242 7 2 > /tmp/canyon_rust.txt

use neutron_worldgen::legacy_rng::LegacyRandom;
use std::sync::OnceLock;

const APPLY_RANGE: i32 = 8;
const RANGE_BLOCKS: i32 = 7 * 16;
const GEN_DEPTH: i32 = 384;
const WORLD_BOTTOM: i32 = -64;

const SIN_SCALE: f64 = 10430.378350470453;
const COS_OFFSET: f64 = 16384.0;

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

fn hf(v: f32) -> String {
    format!("{:08x}", v.to_bits())
}
fn hd(v: f64) -> String {
    format!("{:016x}", v.to_bits())
}

#[derive(Clone, Copy)]
struct RangeOut {
    early_out: bool,
    min_lx: i32,
    max_lx: i32,
    min_y: i32,
    max_y: i32,
    min_lz: i32,
    max_lz: i32,
}

/// Mirror of carve_ellipsoid_canyon range computation (carvers.rs:766-787).
fn ellipsoid_ranges(cx: f64, cy: f64, cz: f64, horiz: f64, vert: f64, tcx: i32, tcz: i32) -> RangeOut {
    let mid_x = (tcx * 16 + 8) as f64;
    let mid_z = (tcz * 16 + 8) as f64;
    let reach = 16.0 + horiz * 2.0;
    if (cx - mid_x).abs() > reach || (cz - mid_z).abs() > reach {
        return RangeOut { early_out: true, min_lx: 0, max_lx: 0, min_y: 0, max_y: 0, min_lz: 0, max_lz: 0 };
    }
    let min_bx = tcx * 16;
    let min_bz = tcz * 16;
    let min_lx = (((cx - horiz).floor() as i32) - min_bx - 1).max(0);
    let max_lx = (((cx + horiz).floor() as i32) - min_bx).min(15);
    let min_y = (((cy - vert).floor() as i32) - 1).max(WORLD_BOTTOM + 1);
    let max_y = (((cy + vert).floor() as i32) + 1).min(WORLD_BOTTOM + 384 - 1 - 7);
    let min_lz = (((cz - horiz).floor() as i32) - min_bz - 1).max(0);
    let max_lz = (((cz + horiz).floor() as i32) - min_bz).min(15);
    RangeOut { early_out: false, min_lx, max_lx, min_y, max_y, min_lz, max_lz }
}

/// Mirror of can_reach (carvers.rs:449-467).
fn can_reach(tcx: i32, tcz: i32, x: f64, z: f64, branch_index: i32, branch_count: i32, thickness: f32) -> bool {
    let mid_x = (tcx * 16 + 8) as f64;
    let mid_z = (tcz * 16 + 8) as f64;
    let dx = x - mid_x;
    let dz = z - mid_z;
    let remaining = (branch_count - branch_index) as f64;
    let max_r = (thickness + 2.0 + 16.0) as f64;
    dx * dx + dz * dz - remaining * remaining <= max_r * max_r
}

/// Mirror of sample_trapezoid_thickness (carvers.rs:646-654).
fn sample_trapezoid_thickness(rng: &mut LegacyRandom) -> f32 {
    let min = 0.0f32;
    let max = 6.0f32;
    let plateau = 2.0f32;
    let range = max - min;
    let plateau_start = (range - plateau) / 2.0;
    let plateau_end = range - plateau_start;
    min + rng.next_f32() * plateau_end + rng.next_f32() * plateau_start
}

/// Mirror of init_width_factors (carvers.rs:726-736).
fn init_width_factors(rng: &mut LegacyRandom, gen_depth: i32, width_smoothness: i32, scx: i32, scz: i32) -> Vec<f32> {
    let mut factors = vec![0.0f32; gen_depth as usize];
    let mut w = 1.0f32;
    let mut renew = 0u32;
    for y in 0..gen_depth {
        if y == 0 || rng.next_int(width_smoothness) == 0 {
            w = 1.0 + rng.next_f32() * rng.next_f32();
            renew += 1;
        }
        factors[y as usize] = w * w;
    }
    for i in 96..=127 {
        println!("W src=({scx},{scz}) idx={i} wf={}", hf(factors[i as usize]));
    }
    let mut wsum: u32 = 0;
    for f in &factors {
        wsum = wsum.wrapping_add(f.to_bits());
    }
    println!("WSUM src=({scx},{scz}) sum={wsum:08x} nrenew={renew}");
    factors
}

/// Mirror of update_vertical_radius (carvers.rs:739-752).
fn update_vertical_radius(rng: &mut LegacyRandom, base_vert: f64, branch_count: f32, step: f32) -> f64 {
    let t = 1.0 - 2.0 * (0.5 - step / branch_count).abs();
    let factor = 1.0f32 + 0.0 * t;
    let r = 0.75 + rng.next_f32() * 0.25;
    base_vert * factor as f64 * r as f64
}

/// Mirror of do_canyon (carvers.rs:656-724).
fn do_canyon(
    seed: i64,
    mut x: f64,
    mut y: f64,
    mut z: f64,
    thickness: f32,
    mut yaw: f32,
    mut pitch: f32,
    branch_index: i32,
    branch_count: i32,
    y_scale: f64,
    tcx: i32,
    tcz: i32,
    scx: i32,
    scz: i32,
) {
    let mut rng = LegacyRandom::new(seed);
    let width_factors = init_width_factors(&mut rng, GEN_DEPTH, 3, scx, scz);
    let mut yaw_vel = 0.0f32;
    let mut pitch_vel = 0.0f32;
    let mut reach_steps = 0u32;
    let mut first_hit_step: i32 = -1;
    let mut last_hit_step: i32 = -1;
    let mut skipdraw_steps = 0u32;

    let mut i = branch_index;
    while i < branch_count {
        // carvers.rs:680-682
        let sin_v = mth_sin_d((3.1415927f32 * i as f32 / branch_count as f32) as f64);
        let mut horiz = 1.5 + (sin_v * thickness) as f64;
        let vert_base = horiz * y_scale;
        // carvers.rs:684-685 horizontalRadiusFactor uniform [0.75, 1.0)
        let hrf = 0.75 + rng.next_f32() * 0.25;
        horiz *= hrf as f64;
        // carvers.rs:686
        let vert = update_vertical_radius(&mut rng, vert_base, branch_count as f32, i as f32);

        // carvers.rs:688-691
        let cos_pitch = mth_cos_f(pitch);
        x += (mth_cos_f(yaw) * cos_pitch) as f64;
        y += mth_sin_f(pitch) as f64;
        z += (mth_sin_f(yaw) * cos_pitch) as f64;

        // carvers.rs:693-699
        pitch *= 0.7;
        pitch += pitch_vel * 0.05;
        yaw += yaw_vel * 0.05;
        pitch_vel *= 0.8;
        yaw_vel *= 0.5;
        pitch_vel += (rng.next_f32() - rng.next_f32()) * rng.next_f32() * 2.0;
        yaw_vel += (rng.next_f32() - rng.next_f32()) * rng.next_f32() * 4.0;

        if rng.next_int(4) == 0 {
            // carvers.rs:701-704
            skipdraw_steps += 1;
            println!(
                "S src=({scx},{scz}) i={i} x={} y={} z={} yaw=PITCHHELD hr={} vr={} draw=0 reach=0 rngc=0 lx=[-,-] lz=[-,-] ly=[-,-]",
                hd(x), hd(y), hd(z), hd(horiz), hd(vert)
            );
            i += 1;
            continue;
        }
        if !can_reach(tcx, tcz, x, z, i, branch_count, thickness) {
            // carvers.rs:705-707
            println!(
                "S src=({scx},{scz}) i={i} x={} y={} z={} yaw=PITCHHELD hr={} vr={} draw=1 reach=0 rngc=0 lx=[-,-] lz=[-,-] ly=[-,-]",
                hd(x), hd(y), hd(z), hd(horiz), hd(vert)
            );
            println!(
                "ABORT src=({scx},{scz}) at_step={i} reach_steps={reach_steps} first_hit_step={first_hit_step} last_hit_step={last_hit_step} skipdraw_steps={skipdraw_steps}"
            );
            return;
        }
        let r = ellipsoid_ranges(x, y, z, horiz, vert, tcx, tcz);
        let writes_local = !r.early_out && r.min_lx <= r.max_lx && r.min_lz <= r.max_lz && r.max_y > r.min_y;
        if writes_local {
            if first_hit_step < 0 {
                first_hit_step = i;
            }
            last_hit_step = i;
        }
        if r.early_out {
            println!(
                "S src=({scx},{scz}) i={i} x={} y={} z={} yaw=PITCHHELD hr={} vr={} draw=1 reach=1 rngc=0 lx=[-,-] lz=[-,-] ly=[-,-]",
                hd(x), hd(y), hd(z), hd(horiz), hd(vert)
            );
        } else {
            println!(
                "S src=({scx},{scz}) i={i} x={} y={} z={} yaw=PITCHHELD hr={} vr={} draw=1 reach=1 rngc=1 lx=[{},{}] lz=[{},{}] ly=[{},{}]",
                hd(x), hd(y), hd(z), hd(horiz), hd(vert), r.min_lx, r.max_lx, r.min_lz, r.max_lz, r.min_y, r.max_y
            );
        }
        reach_steps += 1;
        let _ = &width_factors;
        i += 1;
    }
    println!(
        "END src=({scx},{scz}) steps_run={} reach_steps={reach_steps} first_hit_step={first_hit_step} last_hit_step={last_hit_step} skipdraw_steps={skipdraw_steps}",
        branch_count - branch_index
    );
}

/// Mirror of canyon_from_chunk (carvers.rs:598-640). Returns true when it fires.
fn canyon_from_chunk(level_seed: i64, scx: i32, scz: i32, tcx: i32, tcz: i32) -> bool {
    let mut rng = LegacyRandom::new(0);
    rng.set_large_feature_seed(level_seed.wrapping_add(2), scx, scz);
    let f = rng.next_f32();
    if f > 0.01 {
        return false; // carvers.rs:176
    }
    println!("START src=({scx},{scz}) f={}", hf(f));
    let range_blocks = RANGE_BLOCKS;
    let x = (scx * 16 + rng.next_int(16)) as f64;
    let y = (10 + rng.next_int(67 - 10 + 1)) as f64;
    let z = (scz * 16 + rng.next_int(16)) as f64;
    let yaw = rng.next_f32() * 6.2831855;
    let pitch = -0.125 + rng.next_f32() * 0.25;
    let y_scale = 3.0f64;
    let thickness = sample_trapezoid_thickness(&mut rng);
    let distance_factor = 0.75 + rng.next_f32() * 0.25;
    let branch_count = ((range_blocks as f32) * distance_factor) as i32;
    let seed = rng.next_long();
    println!(
        "P src=({scx},{scz}) x={} y={} z={} yaw={} pitch={} th={} dist={branch_count} tseed={seed:016x}",
        hd(x), hd(y), hd(z), hf(yaw), hf(pitch), hf(thickness)
    );
    do_canyon(seed, x, y, z, thickness, yaw, pitch, 0, branch_count, y_scale, tcx, tcz, scx, scz);
    true
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let seed: i64 = args.get(1).map(|s| s.parse().unwrap()).unwrap_or(424242);
    let tcx: i32 = args.get(2).map(|s| s.parse().unwrap()).unwrap_or(7);
    let tcz: i32 = args.get(3).map(|s| s.parse().unwrap()).unwrap_or(2);
    println!("PROBE=rust seed={seed} target=({tcx},{tcz})");
    // Neutron order (carvers.rs:152-153): dz OUTER, dx INNER.
    let mut fired = 0;
    for dz in -APPLY_RANGE..=APPLY_RANGE {
        for dx in -APPLY_RANGE..=APPLY_RANGE {
            if canyon_from_chunk(seed, tcx + dx, tcz + dz, tcx, tcz) {
                fired += 1;
            }
        }
    }
    println!("FIRED {fired}");
}
