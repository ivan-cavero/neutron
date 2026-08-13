//! Trace carves from source (6,-1) into target (6,-2).
use neutron_worldgen::carvers::{self, CARVE_STARTS, CARVE_WRITES};
use neutron_worldgen::generator::WORLD_BOTTOM;
use neutron_worldgen::legacy_rng::LegacyRandom;
use neutron_worldgen::region_buf::RegionBuf;
use neutron_worldgen::surface::BlockId;
use std::sync::atomic::Ordering;

// We need to call into carvers more deeply. For now, solid region + apply
// and also manual reimplementation of one start with tracing.

fn mth_sin_d(v: f64) -> f32 {
    // approximate with lib sin for trace (same module uses table)
    v.sin() as f32
}
fn mth_cos_d(v: f64) -> f32 {
    v.cos() as f32
}
fn mth_sin_f(v: f32) -> f32 {
    mth_sin_d(v as f64)
}
fn mth_cos_f(v: f32) -> f32 {
    mth_cos_d(v as f64)
}

fn can_reach(tcx: i32, tcz: i32, x: f64, z: f64, i: i32, bc: i32, th: f32) -> bool {
    let mid_x = (tcx * 16 + 8) as f64;
    let mid_z = (tcz * 16 + 8) as f64;
    let dx = x - mid_x;
    let dz = z - mid_z;
    let rem = (bc - i) as f64;
    let max_r = (th + 2.0 + 16.0) as f64;
    dx * dx + dz * dz - rem * rem <= max_r * max_r
}

fn main() {
    let source_cx = 6i32;
    let source_cz = -1i32;
    let target_cx = 6i32;
    let target_cz = -2i32;
    let index = 0i64;
    let mut rng = LegacyRandom::new(0);
    rng.set_large_feature_seed(12345i64.wrapping_add(index), source_cx, source_cz);
    let f = rng.next_f32();
    println!("isStart f={f} start={}", f <= 0.15);
    if f > 0.15 {
        return;
    }

    let range_blocks = 112;
    let a = rng.next_int(15) + 1;
    let b = rng.next_int(a) + 1;
    let cave_count = rng.next_int(b);
    println!("cave_count={cave_count} (a={a} b={b})");

    let mut near_target_steps = 0u32;
    let mut carve_calls = 0u32;
    let mut reach_fail = 0u32;

    for cave_i in 0..cave_count {
        let x0 = (source_cx * 16 + rng.next_int(16)) as f64;
        let y0 = {
            let y_min = WORLD_BOTTOM + 8;
            let y_max = 180;
            (y_min + rng.next_int(y_max - y_min + 1)) as f64
        };
        let z0 = (source_cz * 16 + rng.next_int(16)) as f64;
        let horiz_mult = 0.7 + rng.next_f32() * 0.7;
        let vert_mult = 0.8 + rng.next_f32() * 0.5;
        let floor_level = -1.0 + rng.next_f32() * 0.6;
        let mut tunnel_count = 1;
        let mut had_room = false;
        if rng.next_int(4) == 0 {
            had_room = true;
            let _ys = 0.1 + rng.next_f32() * 0.8;
            let _th = 1.0 + rng.next_f32() * 6.0;
            // room at (x0+1, y0, z0)
            tunnel_count += rng.next_int(4);
        }
        println!("  cave#{cave_i} start=({x0:.1},{y0:.1},{z0:.1}) room={had_room} tunnels={tunnel_count}");

        for t in 0..tunnel_count {
            let yaw = rng.next_f32() * 6.2831855;
            let pitch = (rng.next_f32() - 0.5) / 4.0;
            let mut thickness = rng.next_f32() * 2.0 + rng.next_f32();
            if rng.next_int(10) == 0 {
                thickness *= rng.next_f32() * rng.next_f32() * 3.0 + 1.0;
            }
            let branch_count = range_blocks - rng.next_int(range_blocks / 4);
            let seed = rng.next_long();
            println!("    tunnel#{t} yaw={yaw:.3} pitch={pitch:.3} th={thickness:.2} bc={branch_count} seed={seed}");

            // simulate createTunnel
            let mut trng = LegacyRandom::new(seed);
            let steeper = trng.next_int(branch_count / 2) + branch_count / 4;
            let rare = trng.next_int(6) == 0;
            let mut yaw_vel = 0.0f32;
            let mut pitch_vel = 0.0f32;
            let mut x = x0;
            let mut y = y0;
            let mut z = z0;
            let mut yaw = yaw;
            let mut pitch = pitch;
            let mut min_dist = f64::MAX;
            let mut steps_carved = 0u32;
            let mut i = 0i32;
            while i < branch_count {
                let angle = (3.1415927f32 * i as f32 / branch_count as f32) as f64;
                let sin_v = mth_sin_d(angle);
                let horiz_base = 1.5 + (sin_v * thickness) as f64;
                let _vert_base = horiz_base * 1.0;
                let horiz = horiz_base * horiz_mult as f64;

                let cos_pitch = mth_cos_f(pitch);
                x += (mth_cos_f(yaw) * cos_pitch) as f64;
                y += mth_sin_f(pitch) as f64;
                z += (mth_sin_f(yaw) * cos_pitch) as f64;

                pitch *= if rare { 0.92 } else { 0.7 };
                pitch += pitch_vel * 0.1;
                yaw += yaw_vel * 0.1;
                pitch_vel *= 0.9;
                yaw_vel *= 0.75;
                pitch_vel += (trng.next_f32() - trng.next_f32()) * trng.next_f32() * 2.0;
                yaw_vel += (trng.next_f32() - trng.next_f32()) * trng.next_f32() * 4.0;

                let mid_x = (target_cx * 16 + 8) as f64;
                let mid_z = (target_cz * 16 + 8) as f64;
                let dist = ((x - mid_x).powi(2) + (z - mid_z).powi(2)).sqrt();
                if dist < min_dist {
                    min_dist = dist;
                }

                if i == steeper && thickness > 1.0 {
                    println!("      fork at i={i} pos=({x:.1},{y:.1},{z:.1}) dist={dist:.1}");
                    // consume rng for fork seeds/thickness like vanilla then return
                    let _ = trng.next_long();
                    let _ = trng.next_f32();
                    let _ = trng.next_long();
                    let _ = trng.next_f32();
                    break;
                }
                if trng.next_int(4) == 0 {
                    i += 1;
                    continue;
                }
                if !can_reach(target_cx, target_cz, x, z, i, branch_count, thickness) {
                    reach_fail += 1;
                    println!(
                        "      can_reach FAIL i={i} pos=({x:.1},{y:.1},{z:.1}) dist={dist:.1}"
                    );
                    break;
                }
                // early out check
                let reach = 16.0 + horiz * 2.0;
                if (x - mid_x).abs() <= reach && (z - mid_z).abs() <= reach {
                    near_target_steps += 1;
                    steps_carved += 1;
                    carve_calls += 1;
                }
                i += 1;
            }
            println!("      min_dist_to_target={min_dist:.1} steps_in_range={steps_carved}");
        }
    }
    println!("summary: near_target_steps={near_target_steps} carve_calls={carve_calls} reach_fail={reach_fail}");

    // Real apply
    let mut region = RegionBuf::new(target_cx, target_cz, 0);
    for y in WORLD_BOTTOM..320 {
        for z in 0..16 {
            for x in 0..16 {
                region.set(target_cx * 16 + x, y, target_cz * 16 + z, BlockId::Stone);
            }
        }
    }
    CARVE_WRITES.store(0, Ordering::Relaxed);
    CARVE_STARTS.store(0, Ordering::Relaxed);
    carvers::apply_carvers_region(&mut region, 12345);
    println!(
        "real apply writes={} starts={}",
        CARVE_WRITES.load(Ordering::Relaxed),
        CARVE_STARTS.load(Ordering::Relaxed)
    );
}
