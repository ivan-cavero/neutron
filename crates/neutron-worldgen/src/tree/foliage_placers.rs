//! Tree foliage placement — blob / fancy / dark-oak (FoliagePlacer ports).
use super::*;
use crate::feature_rng::FeatureRandom;
use crate::region_buf::RegionBuf;
use crate::surface::BlockId;
use serde_json::Value;

fn try_place_leaf(ctx: &mut TreeCtx<'_>, x: i32, y: i32, z: i32) -> bool {
    if !valid_tree_pos(ctx.region.get(x, y, z)) {
        return false;
    }
    ctx.region.set(x, y, z, ctx.leaves);
    ctx.foliage.push((x, y, z));
    true
}

fn place_leaves_row(
    ctx: &mut TreeCtx<'_>,
    origin: (i32, i32, i32),
    current_radius: i32,
    yo: i32,
    double_trunk: bool,
    skip: SkipFn,
) {
    let extra = if double_trunk { 1 } else { 0 };
    for dx in -current_radius..=current_radius + extra {
        for dz in -current_radius..=current_radius + extra {
            if should_skip_location_signed(ctx.rng, dx, yo, dz, current_radius, double_trunk, skip)
            {
                continue;
            }
            try_place_leaf(ctx, origin.0 + dx, origin.1 + yo, origin.2 + dz);
        }
    }
}

#[derive(Clone, Copy)]
enum SkipFn {
    Blob,
    Fancy,
    DarkOak,
}

fn should_skip_location_signed(
    rng: &mut FeatureRandom,
    dx: i32,
    y: i32,
    dz: i32,
    current_radius: i32,
    double_trunk: bool,
    skip: SkipFn,
) -> bool {
    if matches!(skip, SkipFn::DarkOak)
        && y == 0
        && double_trunk
        && (dx == -current_radius || dx >= current_radius)
        && (dz == -current_radius || dz >= current_radius)
    {
        return true;
    }
    let (min_dx, min_dz) = if double_trunk {
        (dx.abs().min((dx - 1).abs()), dz.abs().min((dz - 1).abs()))
    } else {
        (dx.abs(), dz.abs())
    };
    should_skip_location(rng, min_dx, y, min_dz, current_radius, double_trunk, skip)
}

fn should_skip_location(
    rng: &mut FeatureRandom,
    dx: i32,
    y: i32,
    dz: i32,
    current_radius: i32,
    double_trunk: bool,
    skip: SkipFn,
) -> bool {
    match skip {
        SkipFn::Blob => {
            dx == current_radius && dz == current_radius && (rng.next_int(2) == 0 || y == 0)
        }
        SkipFn::Fancy => {
            let a = dx as f32 + 0.5;
            let b = dz as f32 + 0.5;
            a * a + b * b > (current_radius * current_radius) as f32
        }
        SkipFn::DarkOak => {
            if y == -1 && !double_trunk {
                dx == current_radius && dz == current_radius
            } else if y == 1 {
                dx + dz > current_radius * 2 - 2
            } else {
                false
            }
        }
    }
}

pub(super) fn create_blob_foliage(
    ctx: &mut TreeCtx<'_>,
    att: FoliageAttachment,
    foliage_height: i32,
    leaf_radius: i32,
    offset: i32,
) {
    let origin = (att.x, att.y, att.z);
    let mut yo = offset;
    while yo >= offset - foliage_height {
        let current_radius = (leaf_radius + att.radius_offset - 1 - yo / 2).max(0);
        place_leaves_row(
            ctx,
            origin,
            current_radius,
            yo,
            att.double_trunk,
            SkipFn::Blob,
        );
        yo -= 1;
    }
}

pub(super) fn create_fancy_foliage(
    ctx: &mut TreeCtx<'_>,
    att: FoliageAttachment,
    foliage_height: i32,
    leaf_radius: i32,
    offset: i32,
) {
    let origin = (att.x, att.y, att.z);
    let mut yo = offset;
    while yo >= offset - foliage_height {
        let current_radius = leaf_radius
            + if yo == offset || yo == offset - foliage_height {
                0
            } else {
                1
            };
        place_leaves_row(
            ctx,
            origin,
            current_radius,
            yo,
            att.double_trunk,
            SkipFn::Fancy,
        );
        yo -= 1;
    }
}

pub(super) fn create_dark_oak_foliage(
    ctx: &mut TreeCtx<'_>,
    att: FoliageAttachment,
    leaf_radius: i32,
    offset: i32,
) {
    let origin = (att.x, att.y + offset, att.z);
    if att.double_trunk {
        place_leaves_row(ctx, origin, leaf_radius + 2, -1, true, SkipFn::DarkOak);
        place_leaves_row(ctx, origin, leaf_radius + 3, 0, true, SkipFn::DarkOak);
        place_leaves_row(ctx, origin, leaf_radius + 2, 1, true, SkipFn::DarkOak);
        if next_boolean(ctx.rng) {
            place_leaves_row(ctx, origin, leaf_radius, 2, true, SkipFn::DarkOak);
        }
    } else {
        place_leaves_row(ctx, origin, leaf_radius + 2, -1, false, SkipFn::DarkOak);
        place_leaves_row(ctx, origin, leaf_radius + 1, 0, false, SkipFn::DarkOak);
    }
}


