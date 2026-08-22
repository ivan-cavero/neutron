//! Tree trunk placement — Straight / DarkOak / Fancy (TrunkPlacer ports).
use super::*;
use crate::feature_rng::FeatureRandom;
use crate::region_buf::RegionBuf;
use crate::surface::BlockId;
use serde_json::Value;

pub(super) fn place_below_trunk(ctx: &mut TreeCtx<'_>, x: i32, y: i32, z: i32, cfg: &Value) {
    let below = ctx.region.get(x, y, z);
    if cannot_replace_below_tree_trunk(below) {
        return;
    }
    if y < WORLD_BOTTOM || y >= WORLD_TOP {
        return;
    }
    let dirt = below_trunk_block(cfg).unwrap_or(BlockId::Dirt);
    ctx.region.set(x, y, z, dirt);
    ctx.trunks.push((x, y, z));
}

fn place_log(ctx: &mut TreeCtx<'_>, x: i32, y: i32, z: i32) -> bool {
    if !valid_tree_pos(ctx.region.get(x, y, z)) {
        return false;
    }
    ctx.region.set(x, y, z, ctx.log);
    ctx.trunks.push((x, y, z));
    true
}

pub(super) fn place_straight_trunk(
    ctx: &mut TreeCtx<'_>,
    x: i32,
    y: i32,
    z: i32,
    tree_height: i32,
    cfg: &Value,
) -> Vec<FoliageAttachment> {
    place_below_trunk(ctx, x, y - 1, z, cfg);
    for dy in 0..tree_height {
        place_log(ctx, x, y + dy, z);
    }
    vec![FoliageAttachment {
        x,
        y: y + tree_height,
        z,
        radius_offset: 0,
        double_trunk: false,
    }]
}

pub(super) fn place_dark_oak_trunk(
    ctx: &mut TreeCtx<'_>,
    x: i32,
    y: i32,
    z: i32,
    tree_height: i32,
    cfg: &Value,
) -> Vec<FoliageAttachment> {
    place_below_trunk(ctx, x, y - 1, z, cfg);
    place_below_trunk(ctx, x + 1, y - 1, z, cfg);
    place_below_trunk(ctx, x, y - 1, z + 1, cfg);
    place_below_trunk(ctx, x + 1, y - 1, z + 1, cfg);

    // Direction.Plane.HORIZONTAL = NORTH, EAST, SOUTH, WEST
    let dir = ctx.rng.next_int(4);
    let (sx, sz) = match dir {
        0 => (0, -1), // NORTH
        1 => (1, 0),  // EAST
        2 => (0, 1),  // SOUTH
        _ => (-1, 0), // WEST
    };
    let lean_height = tree_height - ctx.rng.next_int(4);
    let mut lean_steps = 2 - ctx.rng.next_int(3);
    let mut tx = x;
    let mut tz = z;
    let ey = y + tree_height - 1;

    for dy in 0..tree_height {
        if dy >= lean_height && lean_steps > 0 {
            tx += sx;
            tz += sz;
            lean_steps -= 1;
        }
        let yy = y + dy;
        if !is_air_or_leaves(ctx.region.get(tx, yy, tz)) {
            continue;
        }
        place_log(ctx, tx, yy, tz);
        place_log(ctx, tx + 1, yy, tz);
        place_log(ctx, tx, yy, tz + 1);
        place_log(ctx, tx + 1, yy, tz + 1);
    }

    let mut attachments = vec![FoliageAttachment {
        x: tx,
        y: ey,
        z: tz,
        radius_offset: 0,
        double_trunk: true,
    }];

    for ox in -1..=2 {
        for oz in -1..=2 {
            if (ox >= 0 && ox <= 1 && oz >= 0 && oz <= 1) || ctx.rng.next_int(3) > 0 {
                continue;
            }
            let length = ctx.rng.next_int(3) + 2;
            for branch_y in 0..length {
                place_log(ctx, x + ox, ey - branch_y - 1, z + oz);
            }
            attachments.push(FoliageAttachment {
                x: x + ox,
                y: ey,
                z: z + oz,
                radius_offset: 0,
                double_trunk: false,
            });
        }
    }
    attachments
}

struct FancyFoliageCoord {
    x: i32,
    y: i32,
    z: i32,
    branch_base: i32,
}

pub(super) fn place_fancy_trunk(
    ctx: &mut TreeCtx<'_>,
    ox: i32,
    oy: i32,
    oz: i32,
    tree_height: i32,
    cfg: &Value,
) -> Vec<FoliageAttachment> {
    let height = tree_height + 2;
    let trunk_height = mth_floor_f64((height as f64) * 0.618);
    place_below_trunk(ctx, ox, oy - 1, oz, cfg);
    let clusters_per_y = 1i32.min(mth_floor_f64(
        1.382 + (1.0 * (height as f64) / 13.0).powi(2),
    ));
    let trunk_top = oy + trunk_height;
    let mut foliage_coords = Vec::new();
    let mut relative_y = height - 5;
    foliage_coords.push(FancyFoliageCoord {
        x: ox,
        y: oy + relative_y,
        z: oz,
        branch_base: trunk_top,
    });
    while relative_y >= 0 {
        let tree_shape = fancy_tree_shape(height, relative_y);
        if tree_shape >= 0.0 {
            for _ in 0..clusters_per_y {
                let radius = 1.0 * (tree_shape as f64) * (ctx.rng.next_f32() as f64 + 0.328);
                let angle = f64::from(ctx.rng.next_f32() * 2.0) * std::f64::consts::PI;
                let fx = radius * angle.sin() + 0.5;
                let fz = radius * angle.cos() + 0.5;
                let check_start = (
                    ox + mth_floor_f64(fx),
                    oy + relative_y - 1,
                    oz + mth_floor_f64(fz),
                );
                let check_end = (check_start.0, check_start.1 + 5, check_start.2);
                if !make_limb(ctx, check_start, check_end, false) {
                    continue;
                }
                let dx = ox - check_start.0;
                let dz = oz - check_start.2;
                let branch_height =
                    check_start.1 as f64 - ((dx * dx + dz * dz) as f64).sqrt() * 0.381;
                let branch_top = if branch_height > trunk_top as f64 {
                    trunk_top
                } else {
                    branch_height as i32
                };
                let check_base = (ox, branch_top, oz);
                if !make_limb(ctx, check_base, check_start, false) {
                    continue;
                }
                foliage_coords.push(FancyFoliageCoord {
                    x: check_start.0,
                    y: check_start.1,
                    z: check_start.2,
                    branch_base: check_base.1,
                });
            }
        }
        relative_y -= 1;
    }

    make_limb(ctx, (ox, oy, oz), (ox, oy + trunk_height, oz), true);
    for end in &foliage_coords {
        let base = (ox, end.branch_base, oz);
        let end_pos = (end.x, end.y, end.z);
        if base == end_pos || !trim_branches(height, end.branch_base - oy) {
            continue;
        }
        make_limb(ctx, base, end_pos, true);
    }

    let mut attachments = Vec::new();
    for c in &foliage_coords {
        if trim_branches(height, c.branch_base - oy) {
            attachments.push(FoliageAttachment {
                x: c.x,
                y: c.y,
                z: c.z,
                radius_offset: 0,
                double_trunk: false,
            });
        }
    }
    attachments
}

fn trim_branches(height: i32, local_y: i32) -> bool {
    (local_y as f64) >= (height as f64) * 0.2
}

fn fancy_tree_shape(height: i32, y: i32) -> f32 {
    if (y as f32) < (height as f32) * 0.3 {
        return -1.0;
    }
    let radius = height as f32 / 2.0;
    let adjacent = radius - y as f32;
    if adjacent == 0.0 {
        return radius * 0.5;
    }
    if adjacent.abs() >= radius {
        return 0.0;
    }
    (radius * radius - adjacent * adjacent).sqrt() * 0.5
}

fn make_limb(
    ctx: &mut TreeCtx<'_>,
    start: (i32, i32, i32),
    end: (i32, i32, i32),
    do_place: bool,
) -> bool {
    if !do_place && start == end {
        return true;
    }
    let dx = end.0 - start.0;
    let dy = end.1 - start.1;
    let dz = end.2 - start.2;
    let steps = dx.abs().max(dy.abs()).max(dz.abs());
    if steps == 0 {
        if do_place {
            place_log(ctx, start.0, start.1, start.2);
        } else if !is_free(ctx.region.get(start.0, start.1, start.2)) {
            return false;
        }
        return true;
    }
    let fdx = dx as f32 / steps as f32;
    let fdy = dy as f32 / steps as f32;
    let fdz = dz as f32 / steps as f32;
    for i in 0..=steps {
        let px = start.0 + mth_floor_f32(0.5 + i as f32 * fdx);
        let py = start.1 + mth_floor_f32(0.5 + i as f32 * fdy);
        let pz = start.2 + mth_floor_f32(0.5 + i as f32 * fdz);
        if do_place {
            place_log(ctx, px, py, pz);
        } else if !is_free(ctx.region.get(px, py, pz)) {
            return false;
        }
    }
    true
}

