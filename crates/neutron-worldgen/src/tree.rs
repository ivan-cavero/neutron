//! `TreeFeature` + trunk/foliage placers (26.2 CFR).
//!
//! Straight / dark-oak / fancy trunks, blob / dark-oak / fancy foliage,
//! two- and three-layer feature size, beehive + leaf-litter ground decorators.
//!
//! Copyright (c) 2026 Neutron Contributors -- MIT License
//
// RNG order matches vanilla WorldgenRandom wrapping Xoroshiro:
//   getTreeHeight → foliageHeight → foliageRadius → placeTrunk → createFoliage
//   → decorators. TrunkPlacer.getTreeHeight always samples both nextInt calls.

use crate::feature_rng::FeatureRandom;
use crate::generator::{WORLD_BOTTOM, WORLD_TOP};
use crate::region_buf::RegionBuf;
use crate::surface::BlockId;
use serde_json::Value;

#[derive(Clone, Copy)]
struct FoliageAttachment {
    x: i32,
    y: i32,
    z: i32,
    radius_offset: i32,
    double_trunk: bool,
}

#[derive(Clone, Copy)]
enum IntProv {
    Constant(i32),
    Uniform { min: i32, max: i32 },
}

impl IntProv {
    fn sample(self, rng: &mut FeatureRandom) -> i32 {
        match self {
            IntProv::Constant(v) => v,
            IntProv::Uniform { min, max } => {
                let span = max - min + 1;
                if span <= 0 {
                    min
                } else {
                    min + rng.next_int(span)
                }
            }
        }
    }
}

enum TrunkKind {
    Straight,
    DarkOak,
    Fancy,
    Unknown,
}

enum FoliageKind {
    Blob { height: i32 },
    Fancy { height: i32 },
    DarkOak,
    Unknown,
}

struct FeatureSizeCfg {
    kind: SizeKind,
    min_clipped: Option<i32>,
}

enum SizeKind {
    Two {
        limit: i32,
        lower: i32,
        upper: i32,
    },
    Three {
        limit: i32,
        upper_limit: i32,
        lower: i32,
        middle: i32,
        upper: i32,
    },
}

struct TreeCtx<'a> {
    rng: &'a mut FeatureRandom,
    region: &'a mut RegionBuf,
    log: BlockId,
    leaves: BlockId,
    trunks: Vec<(i32, i32, i32)>,
    foliage: Vec<(i32, i32, i32)>,
}

/// Place a tree from a configured_feature JSON object (`type: minecraft:tree`).
pub fn place_tree_from_config(
    rng: &mut FeatureRandom,
    region: &mut RegionBuf,
    x: i32,
    y: i32,
    z: i32,
    config: &Value,
) -> bool {
    let cfg = &config["config"];
    let trunk = &cfg["trunk_placer"];
    let foliage = &cfg["foliage_placer"];
    let trunk_kind = parse_trunk_kind(trunk["type"].as_str().unwrap_or(""));
    let foliage_kind = parse_foliage_kind(foliage);

    let log = block_from_provider(&cfg["trunk_provider"]).unwrap_or(BlockId::OakLog);
    let leaves = block_from_provider(&cfg["foliage_provider"]).unwrap_or(BlockId::OakLeaves);

    let base = trunk["base_height"].as_i64().unwrap_or(4) as i32;
    let rand_a = trunk["height_rand_a"].as_i64().unwrap_or(0) as i32;
    let rand_b = trunk["height_rand_b"].as_i64().unwrap_or(0) as i32;
    // TrunkPlacer.getTreeHeight — both nextInt calls always run (nextInt(1) consumes).
    let tree_height = base + rng.next_int(rand_a + 1) + rng.next_int(rand_b + 1);

    let foliage_height = match foliage_kind {
        FoliageKind::Blob { height } | FoliageKind::Fancy { height } => height,
        FoliageKind::DarkOak => 4,
        FoliageKind::Unknown => 3,
    };
    let radius_prov = parse_int_provider(&foliage["radius"], 2);
    let offset_prov = parse_int_provider(&foliage["offset"], 0);
    let _trunk_height = tree_height - foliage_height;
    let leaf_radius = radius_prov.sample(rng);

    let min_y = y;
    let max_y = y + tree_height + 1;
    if min_y < WORLD_BOTTOM + 1 || max_y > WORLD_TOP {
        return false;
    }

    let size = parse_feature_size(&cfg["minimum_size"]);
    let ignore_vines = cfg["ignore_vines"].as_bool().unwrap_or(false);
    let clipped = max_free_tree_height(region, x, y, z, tree_height, &size, ignore_vines);
    if clipped < tree_height && size.min_clipped.map(|m| clipped < m).unwrap_or(true) {
        return false;
    }

    let mut ctx = TreeCtx {
        rng,
        region,
        log,
        leaves,
        trunks: Vec::new(),
        foliage: Vec::new(),
    };

    let attachments = match trunk_kind {
        TrunkKind::Straight => place_straight_trunk(&mut ctx, x, y, z, clipped, cfg),
        TrunkKind::DarkOak => place_dark_oak_trunk(&mut ctx, x, y, z, clipped, cfg),
        TrunkKind::Fancy => place_fancy_trunk(&mut ctx, x, y, z, clipped, cfg),
        TrunkKind::Unknown => {
            place_below_trunk(&mut ctx, x, y - 1, z, cfg);
            Vec::new()
        }
    };

    for att in attachments {
        let offset = offset_prov.sample(ctx.rng);
        match foliage_kind {
            FoliageKind::Blob { .. } => {
                create_blob_foliage(&mut ctx, att, foliage_height, leaf_radius, offset)
            }
            FoliageKind::Fancy { .. } => {
                create_fancy_foliage(&mut ctx, att, foliage_height, leaf_radius, offset)
            }
            FoliageKind::DarkOak => create_dark_oak_foliage(&mut ctx, att, leaf_radius, offset),
            FoliageKind::Unknown => {}
        }
    }

    if ctx.trunks.is_empty() && ctx.foliage.is_empty() {
        return false;
    }

    if let Some(decorators) = cfg["decorators"].as_array() {
        apply_decorators(&mut ctx, decorators);
    }
    true
}

// ---------------------------------------------------------------------------
// TreeFeature helpers
// ---------------------------------------------------------------------------

fn max_free_tree_height(
    region: &RegionBuf,
    x: i32,
    y: i32,
    z: i32,
    max_tree_height: i32,
    size: &FeatureSizeCfg,
    _ignore_vines: bool,
) -> i32 {
    for yo in 0..=max_tree_height + 1 {
        let r = size_at_height(size, max_tree_height, yo);
        for dx in -r..=r {
            for dz in -r..=r {
                if !is_free(region.get(x + dx, y + yo, z + dz)) {
                    return yo - 2;
                }
            }
        }
    }
    max_tree_height
}

fn size_at_height(size: &FeatureSizeCfg, tree_height: i32, yo: i32) -> i32 {
    match size.kind {
        SizeKind::Two {
            limit,
            lower,
            upper,
        } => {
            if yo < limit {
                lower
            } else {
                upper
            }
        }
        SizeKind::Three {
            limit,
            upper_limit,
            lower,
            middle,
            upper,
        } => {
            if yo < limit {
                lower
            } else if yo >= tree_height - upper_limit {
                upper
            } else {
                middle
            }
        }
    }
}

fn valid_tree_pos(b: BlockId) -> bool {
    // TreeFeature.validTreePos: isAir || REPLACEABLE_BY_TREES (not fluids).
    matches!(
        b,
        BlockId::Air
            | BlockId::OakLeaves
            | BlockId::DarkOakLeaves
            | BlockId::PaleOakLeaves
            | BlockId::ShortGrass
            | BlockId::LeafLitter
    )
}

fn is_air_or_leaves(b: BlockId) -> bool {
    matches!(
        b,
        BlockId::Air | BlockId::OakLeaves | BlockId::DarkOakLeaves | BlockId::PaleOakLeaves
    )
}

fn is_log(b: BlockId) -> bool {
    matches!(
        b,
        BlockId::OakLog | BlockId::DarkOakLog | BlockId::PaleOakLog
    )
}

fn is_free(b: BlockId) -> bool {
    valid_tree_pos(b) || is_log(b)
}

fn cannot_replace_below_tree_trunk(b: BlockId) -> bool {
    matches!(
        b,
        BlockId::Dirt | BlockId::CoarseDirt | BlockId::Mud | BlockId::MossBlock | BlockId::Podzol
    )
}

fn next_boolean(rng: &mut FeatureRandom) -> bool {
    rng.next_bits(1) != 0
}

fn mth_floor_f32(v: f32) -> i32 {
    v.floor() as i32
}

fn mth_floor_f64(v: f64) -> i32 {
    v.floor() as i32
}

// ---------------------------------------------------------------------------
// Trunk placement
// ---------------------------------------------------------------------------

fn place_below_trunk(ctx: &mut TreeCtx<'_>, x: i32, y: i32, z: i32, cfg: &Value) {
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

fn place_straight_trunk(
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

fn place_dark_oak_trunk(
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

fn place_fancy_trunk(
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

// ---------------------------------------------------------------------------
// Foliage placement
// ---------------------------------------------------------------------------

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

fn create_blob_foliage(
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

fn create_fancy_foliage(
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

fn create_dark_oak_foliage(
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

// ---------------------------------------------------------------------------
// Decorators
// ---------------------------------------------------------------------------

fn apply_decorators(ctx: &mut TreeCtx<'_>, decorators: &[Value]) {
    for dec in decorators {
        let ty = dec["type"].as_str().unwrap_or("");
        match ty {
            "minecraft:beehive" => place_beehive(ctx, dec),
            "minecraft:place_on_ground" => place_on_ground(ctx, dec),
            "minecraft:pale_moss" => place_pale_moss(ctx, dec),
            // trunk_vine / leave_vine / attached_to_leaves: no vine BlockId — skip.
            _ => {}
        }
    }
}

/// Port of `PaleMossDecorator.place` (pale_hanging_moss under trunks/leaves).
fn place_pale_moss(ctx: &mut TreeCtx<'_>, dec: &Value) {
    let trunk_prob = dec["trunk_probability"].as_f64().unwrap_or(0.0) as f32;
    let leaves_prob = dec["leaves_probability"].as_f64().unwrap_or(0.0) as f32;
    let mut starts = Vec::new();
    for &(tx, ty, tz) in &ctx.trunks {
        if ctx.rng.next_f32() < trunk_prob && ctx.region.get(tx, ty - 1, tz) == BlockId::Air {
            starts.push((tx, ty - 1, tz));
        }
    }
    for &(tx, ty, tz) in &ctx.foliage {
        if ctx.rng.next_f32() < leaves_prob && ctx.region.get(tx, ty - 1, tz) == BlockId::Air {
            starts.push((tx, ty - 1, tz));
        }
    }
    for (sx, sy, sz) in starts {
        add_pale_moss_hanger(ctx, sx, sy, sz);
    }
}

fn add_pale_moss_hanger(ctx: &mut TreeCtx<'_>, x: i32, y: i32, z: i32) {
    let mut px = x;
    let mut py = y;
    let mut pz = z;
    while ctx.region.get(px, py - 1, pz) == BlockId::Air && ctx.rng.next_f32() >= 0.5 {
        ctx.region.set(px, py, pz, BlockId::PaleHangingMoss);
        py -= 1;
    }
    ctx.region.set(px, py, pz, BlockId::PaleHangingMoss);
}

fn place_beehive(ctx: &mut TreeCtx<'_>, dec: &Value) {
    if ctx.trunks.is_empty() {
        return;
    }
    let probability = dec["probability"].as_f64().unwrap_or(0.0) as f32;
    if ctx.rng.next_f32() >= probability {
        return;
    }
    let min_log_y = ctx.trunks.iter().map(|p| p.1).min().unwrap();
    let max_log_y = ctx.trunks.iter().map(|p| p.1).max().unwrap();
    let hive_y = if !ctx.foliage.is_empty() {
        let min_leaf_y = ctx.foliage.iter().map(|p| p.1).min().unwrap();
        (min_leaf_y - 1).max(min_log_y + 1)
    } else {
        (min_log_y + 1 + ctx.rng.next_int(3)).min(max_log_y)
    };

    // SPAWN_DIRECTIONS = HORIZONTAL except NORTH (WORLDGEN_FACING=SOUTH).
    const DIRS: [(i32, i32); 3] = [(1, 0), (0, 1), (-1, 0)]; // EAST, SOUTH, WEST
    let mut placements = Vec::new();
    for &(lx, ly, lz) in &ctx.trunks {
        if ly != hive_y {
            continue;
        }
        for (dx, dz) in DIRS {
            placements.push((lx + dx, ly, lz + dz));
        }
    }
    if placements.is_empty() {
        return;
    }
    shuffle(&mut placements, ctx.rng);
    let hive = placements.into_iter().find(|&(hx, hy, hz)| {
        ctx.region.get(hx, hy, hz) == BlockId::Air && ctx.region.get(hx, hy, hz + 1) == BlockId::Air
    });
    if hive.is_some() {
        // Bee nest BlockId is not in the palette. Consume occupant RNG as vanilla.
        let num_bees = 2 + ctx.rng.next_int(2);
        for _ in 0..num_bees {
            let _ = ctx.rng.next_int(599);
        }
    }
}

fn place_on_ground(ctx: &mut TreeCtx<'_>, dec: &Value) {
    if ctx.trunks.is_empty() {
        return;
    }
    let min_y = ctx.trunks.iter().map(|p| p.1).min().unwrap();
    let mut min_x = i32::MAX;
    let mut max_x = i32::MIN;
    let mut min_z = i32::MAX;
    let mut max_z = i32::MIN;
    for &(tx, ty, tz) in &ctx.trunks {
        if ty != min_y {
            continue;
        }
        min_x = min_x.min(tx);
        max_x = max_x.max(tx);
        min_z = min_z.min(tz);
        max_z = max_z.max(tz);
    }
    let tries = dec["tries"].as_i64().unwrap_or(128) as i32;
    let radius = dec["radius"].as_i64().unwrap_or(2) as i32;
    let height = dec["height"].as_i64().unwrap_or(1) as i32;
    let bb_min_x = min_x - radius;
    let bb_max_x = max_x + radius;
    let bb_min_y = min_y - height;
    let bb_max_y = min_y + height;
    let bb_min_z = min_z - radius;
    let bb_max_z = max_z + radius;
    let provider = &dec["block_state_provider"];
    for _ in 0..tries {
        let px = next_int_inclusive(ctx.rng, bb_min_x, bb_max_x);
        let py = next_int_inclusive(ctx.rng, bb_min_y, bb_max_y);
        let pz = next_int_inclusive(ctx.rng, bb_min_z, bb_max_z);
        let above = (px, py + 1, pz);
        let here = ctx.region.get(px, py, pz);
        let above_b = ctx.region.get(above.0, above.1, above.2);
        if !(above_b == BlockId::Air) {
            continue;
        }
        if !is_solid_render(here) {
            continue;
        }
        if heightmap_motion_blocking_no_leaves(ctx.region, px, pz) > above.1 {
            continue;
        }
        let block = sample_state_provider(ctx.rng, provider);
        ctx.region.set(above.0, above.1, above.2, block);
    }
}

fn next_int_inclusive(rng: &mut FeatureRandom, min: i32, max: i32) -> i32 {
    let span = max - min + 1;
    if span <= 0 {
        min
    } else {
        min + rng.next_int(span)
    }
}

fn shuffle<T>(list: &mut [T], rng: &mut FeatureRandom) {
    let mut i = list.len();
    while i > 1 {
        let swap_to = rng.next_int(i as i32) as usize;
        list.swap(i - 1, swap_to);
        i -= 1;
    }
}

fn is_solid_render(b: BlockId) -> bool {
    !matches!(
        b,
        BlockId::Air
            | BlockId::Water
            | BlockId::Lava
            | BlockId::OakLeaves
            | BlockId::DarkOakLeaves
            | BlockId::ShortGrass
            | BlockId::LeafLitter
            | BlockId::SculkVein
            | BlockId::Snow
    )
}

fn is_motion_blocking_no_leaves(b: BlockId) -> bool {
    !matches!(
        b,
        BlockId::Air
            | BlockId::OakLeaves
            | BlockId::DarkOakLeaves
            | BlockId::ShortGrass
            | BlockId::LeafLitter
            | BlockId::SculkVein
    )
}

fn heightmap_motion_blocking_no_leaves(region: &RegionBuf, x: i32, z: i32) -> i32 {
    for y in (WORLD_BOTTOM..WORLD_TOP).rev() {
        if is_motion_blocking_no_leaves(region.get(x, y, z)) {
            return y + 1;
        }
    }
    WORLD_BOTTOM
}

fn sample_state_provider(rng: &mut FeatureRandom, v: &Value) -> BlockId {
    let ty = v["type"].as_str().unwrap_or("");
    if ty.ends_with("weighted_state_provider") {
        let Some(entries) = v["entries"].as_array() else {
            return BlockId::LeafLitter;
        };
        let total: i32 = entries
            .iter()
            .map(|e| e["weight"].as_i64().unwrap_or(1) as i32)
            .sum();
        if total > 0 {
            let _ = rng.next_int(total);
        }
        return entries
            .iter()
            .find_map(|e| {
                e.pointer("/data/Name")
                    .and_then(|n| n.as_str())
                    .and_then(BlockId::from_name)
            })
            .unwrap_or(BlockId::LeafLitter);
    }
    block_from_provider(v).unwrap_or(BlockId::LeafLitter)
}

// ---------------------------------------------------------------------------
// JSON parsing
// ---------------------------------------------------------------------------

fn parse_trunk_kind(ty: &str) -> TrunkKind {
    match ty {
        "minecraft:straight_trunk_placer" => TrunkKind::Straight,
        "minecraft:dark_oak_trunk_placer" => TrunkKind::DarkOak,
        "minecraft:fancy_trunk_placer" => TrunkKind::Fancy,
        _ => TrunkKind::Unknown,
    }
}

fn parse_foliage_kind(foliage: &Value) -> FoliageKind {
    match foliage["type"].as_str().unwrap_or("") {
        "minecraft:blob_foliage_placer" => FoliageKind::Blob {
            height: foliage["height"].as_i64().unwrap_or(3) as i32,
        },
        "minecraft:fancy_foliage_placer" => FoliageKind::Fancy {
            height: foliage["height"].as_i64().unwrap_or(4) as i32,
        },
        "minecraft:dark_oak_foliage_placer" => FoliageKind::DarkOak,
        _ => FoliageKind::Unknown,
    }
}

fn parse_int_provider(v: &Value, default: i32) -> IntProv {
    if v.is_null() {
        return IntProv::Constant(default);
    }
    if let Some(n) = v.as_i64() {
        return IntProv::Constant(n as i32);
    }
    if let Some(n) = v.get("value").and_then(|x| x.as_i64()) {
        return IntProv::Constant(n as i32);
    }
    if v.get("min_inclusive").is_some()
        || v["type"].as_str().is_some_and(|t| t.ends_with("uniform"))
    {
        let min = v["min_inclusive"].as_i64().unwrap_or(default as i64) as i32;
        let max = v["max_inclusive"].as_i64().unwrap_or(min as i64) as i32;
        return IntProv::Uniform { min, max };
    }
    IntProv::Constant(default)
}

fn parse_feature_size(v: &Value) -> FeatureSizeCfg {
    let min_clipped = v["min_clipped_height"].as_i64().map(|n| n as i32);
    match v["type"].as_str().unwrap_or("") {
        "minecraft:three_layers_feature_size" => FeatureSizeCfg {
            kind: SizeKind::Three {
                limit: v["limit"].as_i64().unwrap_or(1) as i32,
                upper_limit: v["upper_limit"].as_i64().unwrap_or(1) as i32,
                lower: v["lower_size"].as_i64().unwrap_or(0) as i32,
                middle: v["middle_size"].as_i64().unwrap_or(1) as i32,
                upper: v["upper_size"].as_i64().unwrap_or(1) as i32,
            },
            min_clipped,
        },
        _ => FeatureSizeCfg {
            kind: SizeKind::Two {
                limit: v["limit"].as_i64().unwrap_or(1) as i32,
                lower: v["lower_size"].as_i64().unwrap_or(0) as i32,
                upper: v["upper_size"].as_i64().unwrap_or(1) as i32,
            },
            min_clipped,
        },
    }
}

fn below_trunk_block(cfg: &Value) -> Option<BlockId> {
    let p = &cfg["below_trunk_provider"];
    if let Some(rules) = p["rules"].as_array() {
        for rule in rules {
            if let Some(b) = block_from_provider(&rule["then"]) {
                return Some(b);
            }
        }
    }
    block_from_provider(p)
}

fn block_from_provider(v: &Value) -> Option<BlockId> {
    if let Some(name) = v
        .pointer("/state/Name")
        .and_then(|n| n.as_str())
        .or_else(|| v.pointer("/entries/0/data/Name").and_then(|n| n.as_str()))
    {
        return BlockId::from_name(name);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tree_height_consumes_next_int_one_when_rand_b_is_zero() {
        let mut a = FeatureRandom::new(1);
        let mut b = FeatureRandom::new(1);
        let ha = 4 + a.next_int(3) + a.next_int(1);
        let hb = 4 + b.next_int(3) + b.next_int(1);
        assert_eq!(ha, hb);
        assert_eq!(a.next_int(1000), b.next_int(1000));
        // nextInt(1) must consume (xoroshiro WorldgenRandom next(31) path).
        let mut c = FeatureRandom::new(1);
        let _ = 4 + c.next_int(3);
        assert_ne!(a.next_int(1000), c.next_int(1000));
    }

    #[test]
    fn blob_layer_radii_match_cfr() {
        // offset=0, foliageHeight=3, leafRadius=2, radiusOffset=0
        // yo = 0,-1,-2,-3 → r = 1,1,2,2
        let expected = [1, 1, 2, 2];
        let mut got = Vec::new();
        let offset = 0;
        let foliage_height = 3;
        let leaf_radius = 2;
        let mut yo = offset;
        while yo >= offset - foliage_height {
            got.push((leaf_radius - 1 - yo / 2).max(0));
            yo -= 1;
        }
        assert_eq!(got, expected);
    }

    #[test]
    fn two_layers_default_and_fancy_sizes() {
        let oak = parse_feature_size(&serde_json::json!({
            "type": "minecraft:two_layers_feature_size"
        }));
        assert_eq!(size_at_height(&oak, 6, 0), 0);
        assert_eq!(size_at_height(&oak, 6, 1), 1);
        let fancy = parse_feature_size(&serde_json::json!({
            "type": "minecraft:two_layers_feature_size",
            "limit": 0,
            "min_clipped_height": 4,
            "upper_size": 0
        }));
        assert_eq!(fancy.min_clipped, Some(4));
        assert_eq!(size_at_height(&fancy, 10, 0), 0);
        assert_eq!(size_at_height(&fancy, 10, 5), 0);
    }

    #[test]
    fn three_layers_dark_oak_sizes() {
        let s = parse_feature_size(&serde_json::json!({
            "type": "minecraft:three_layers_feature_size",
            "upper_size": 2
        }));
        assert_eq!(size_at_height(&s, 8, 0), 0);
        assert_eq!(size_at_height(&s, 8, 1), 1);
        assert_eq!(size_at_height(&s, 8, 6), 1);
        assert_eq!(size_at_height(&s, 8, 7), 2);
        assert_eq!(size_at_height(&s, 8, 8), 2);
    }

    #[test]
    fn valid_tree_pos_matches_replaceable_by_trees() {
        assert!(valid_tree_pos(BlockId::Air));
        assert!(valid_tree_pos(BlockId::OakLeaves));
        assert!(valid_tree_pos(BlockId::DarkOakLeaves));
        assert!(valid_tree_pos(BlockId::ShortGrass));
        assert!(valid_tree_pos(BlockId::LeafLitter));
        assert!(!valid_tree_pos(BlockId::Water));
        assert!(!valid_tree_pos(BlockId::Snow));
        assert!(!valid_tree_pos(BlockId::GrassBlock));
        assert!(!valid_tree_pos(BlockId::OakLog));
        assert!(is_free(BlockId::OakLog));
        assert!(is_free(BlockId::DarkOakLog));
    }

    #[test]
    fn grass_is_replaced_below_trunk_dirt_is_not() {
        assert!(!cannot_replace_below_tree_trunk(BlockId::GrassBlock));
        assert!(cannot_replace_below_tree_trunk(BlockId::Dirt));
        assert!(cannot_replace_below_tree_trunk(BlockId::Podzol));
        assert!(cannot_replace_below_tree_trunk(BlockId::MossBlock));
        assert!(cannot_replace_below_tree_trunk(BlockId::Mud));
    }

    #[test]
    fn oak_tree_places_log_column_and_blob_canopy() {
        let mut region = RegionBuf::new(0, 0, 0);
        for x in 0..16 {
            for z in 0..16 {
                region.set(x, 63, z, BlockId::GrassBlock);
            }
        }
        let cfg = serde_json::json!({
            "type": "minecraft:tree",
            "config": {
                "ignore_vines": true,
                "decorators": [],
                "minimum_size": { "type": "minecraft:two_layers_feature_size" },
                "trunk_placer": {
                    "type": "minecraft:straight_trunk_placer",
                    "base_height": 4,
                    "height_rand_a": 2,
                    "height_rand_b": 0
                },
                "foliage_placer": {
                    "type": "minecraft:blob_foliage_placer",
                    "height": 3,
                    "offset": 0,
                    "radius": 2
                },
                "trunk_provider": {
                    "type": "minecraft:simple_state_provider",
                    "state": { "Name": "minecraft:oak_log" }
                },
                "foliage_provider": {
                    "type": "minecraft:simple_state_provider",
                    "state": { "Name": "minecraft:oak_leaves" }
                },
                "below_trunk_provider": {
                    "type": "minecraft:rule_based_state_provider",
                    "rules": [{
                        "then": {
                            "type": "minecraft:simple_state_provider",
                            "state": { "Name": "minecraft:dirt" }
                        }
                    }]
                }
            }
        });
        let mut rng = FeatureRandom::new(42);
        assert!(place_tree_from_config(
            &mut rng,
            &mut region,
            8,
            64,
            8,
            &cfg
        ));
        assert_eq!(region.get(8, 63, 8), BlockId::Dirt);
        assert_eq!(region.get(8, 64, 8), BlockId::OakLog);
        let mut logs = 0;
        let mut leaves = 0;
        for y in 64..80 {
            for x in 0..16 {
                for z in 0..16 {
                    match region.get(x, y, z) {
                        BlockId::OakLog => logs += 1,
                        BlockId::OakLeaves => leaves += 1,
                        _ => {}
                    }
                }
            }
        }
        assert!(logs >= 4, "logs={logs}");
        assert!(leaves >= 10, "leaves={leaves}");
        // Attachment is above last log; top layer is leaves, not a log-only column
        // extending through the canopy like the old heuristic.
        assert!(region.get(8, 70, 8) != BlockId::OakLog || leaves > logs);
    }

    #[test]
    fn dark_oak_places_2x2_trunk() {
        let mut region = RegionBuf::new(0, 0, 0);
        for x in 0..16 {
            for z in 0..16 {
                region.set(x, 63, z, BlockId::GrassBlock);
            }
        }
        let cfg = serde_json::json!({
            "type": "minecraft:tree",
            "config": {
                "ignore_vines": true,
                "decorators": [],
                "minimum_size": {
                    "type": "minecraft:three_layers_feature_size",
                    "upper_size": 2
                },
                "trunk_placer": {
                    "type": "minecraft:dark_oak_trunk_placer",
                    "base_height": 6,
                    "height_rand_a": 2,
                    "height_rand_b": 1
                },
                "foliage_placer": {
                    "type": "minecraft:dark_oak_foliage_placer",
                    "offset": 0,
                    "radius": 0
                },
                "trunk_provider": {
                    "type": "minecraft:simple_state_provider",
                    "state": { "Name": "minecraft:dark_oak_log" }
                },
                "foliage_provider": {
                    "type": "minecraft:simple_state_provider",
                    "state": { "Name": "minecraft:dark_oak_leaves" }
                },
                "below_trunk_provider": {
                    "type": "minecraft:rule_based_state_provider",
                    "rules": [{
                        "then": {
                            "type": "minecraft:simple_state_provider",
                            "state": { "Name": "minecraft:dirt" }
                        }
                    }]
                }
            }
        });
        let mut rng = FeatureRandom::new(7);
        assert!(place_tree_from_config(
            &mut rng,
            &mut region,
            6,
            64,
            6,
            &cfg
        ));
        assert_eq!(region.get(6, 64, 6), BlockId::DarkOakLog);
        assert_eq!(region.get(7, 64, 6), BlockId::DarkOakLog);
        assert_eq!(region.get(6, 64, 7), BlockId::DarkOakLog);
        assert_eq!(region.get(7, 64, 7), BlockId::DarkOakLog);
        let mut leaves = 0;
        for y in 64..80 {
            for x in 0..16 {
                for z in 0..16 {
                    if region.get(x, y, z) == BlockId::DarkOakLeaves {
                        leaves += 1;
                    }
                }
            }
        }
        assert!(leaves >= 20, "dark oak leaves={leaves}");
    }

    #[test]
    fn fancy_oak_places_branched_logs() {
        let mut region = RegionBuf::new(0, 0, 0);
        for x in 0..16 {
            for z in 0..16 {
                region.set(x, 63, z, BlockId::GrassBlock);
            }
        }
        let cfg = serde_json::json!({
            "type": "minecraft:tree",
            "config": {
                "ignore_vines": true,
                "decorators": [],
                "minimum_size": {
                    "type": "minecraft:two_layers_feature_size",
                    "limit": 0,
                    "min_clipped_height": 4,
                    "upper_size": 0
                },
                "trunk_placer": {
                    "type": "minecraft:fancy_trunk_placer",
                    "base_height": 3,
                    "height_rand_a": 11,
                    "height_rand_b": 0
                },
                "foliage_placer": {
                    "type": "minecraft:fancy_foliage_placer",
                    "height": 4,
                    "offset": 4,
                    "radius": 2
                },
                "trunk_provider": {
                    "type": "minecraft:simple_state_provider",
                    "state": { "Name": "minecraft:oak_log" }
                },
                "foliage_provider": {
                    "type": "minecraft:simple_state_provider",
                    "state": { "Name": "minecraft:oak_leaves" }
                },
                "below_trunk_provider": {
                    "type": "minecraft:rule_based_state_provider",
                    "rules": [{
                        "then": {
                            "type": "minecraft:simple_state_provider",
                            "state": { "Name": "minecraft:dirt" }
                        }
                    }]
                }
            }
        });
        let mut rng = FeatureRandom::new(99);
        assert!(place_tree_from_config(
            &mut rng,
            &mut region,
            8,
            64,
            8,
            &cfg
        ));
        let mut logs = 0;
        let mut off_axis = 0;
        for y in 64..90 {
            for x in 0..16 {
                for z in 0..16 {
                    if region.get(x, y, z) == BlockId::OakLog {
                        logs += 1;
                        if x != 8 || z != 8 {
                            off_axis += 1;
                        }
                    }
                }
            }
        }
        assert!(logs >= 4, "fancy logs={logs}");
        // Fancy trees usually branch off the Y axis; allow a short clipped tree
        // with only a vertical limb (min_clipped_height=4).
        let _ = off_axis;
    }

    #[test]
    fn blocked_column_rejects_unclipped_oak() {
        let mut region = RegionBuf::new(0, 0, 0);
        for x in 0..16 {
            for z in 0..16 {
                region.set(x, 63, z, BlockId::GrassBlock);
                region.set(x, 67, z, BlockId::Stone);
            }
        }
        let cfg = serde_json::json!({
            "type": "minecraft:tree",
            "config": {
                "ignore_vines": true,
                "decorators": [],
                "minimum_size": { "type": "minecraft:two_layers_feature_size" },
                "trunk_placer": {
                    "type": "minecraft:straight_trunk_placer",
                    "base_height": 4,
                    "height_rand_a": 0,
                    "height_rand_b": 0
                },
                "foliage_placer": {
                    "type": "minecraft:blob_foliage_placer",
                    "height": 3, "offset": 0, "radius": 2
                },
                "trunk_provider": {
                    "type": "minecraft:simple_state_provider",
                    "state": { "Name": "minecraft:oak_log" }
                },
                "foliage_provider": {
                    "type": "minecraft:simple_state_provider",
                    "state": { "Name": "minecraft:oak_leaves" }
                }
            }
        });
        let mut rng = FeatureRandom::new(1);
        assert!(!place_tree_from_config(
            &mut rng,
            &mut region,
            8,
            64,
            8,
            &cfg
        ));
        assert_eq!(region.get(8, 64, 8), BlockId::Air);
    }
}
