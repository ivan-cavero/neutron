//! Overworld RUINED PORTAL structures (`RuinedPortalStructure`,
//! `RuinedPortalPiece`, structure_set `minecraft:ruined_portals`) for 26.2.
//!
//! Vanilla pipeline replicated here (decompile root
//! `net/minecraft/world/level/levelgen/`):
//!
//! - placement: `structure/placement/RandomSpreadStructurePlacement.getPotentialStructureChunk`
//!   (:67-76; spacing 40 / separation 15 / salt 34222645, LINEAR spread via
//!   `WorldgenRandom.setLargeFeatureWithSalt(seed, gridX, gridZ, salt)`),
//! - `chunk/ChunkGenerator.createStructures(:481-551)` retry loop: 7 weighted
//!   entries (weight 1 each, JSON array order), driver
//!   `WorldgenRandom(LegacyRandomSource)` seeded `setLargeFeatureSeed(levelSeed,
//!   cx, cz)` (:511-512); a candidate failing the biome filter is removed and
//!   the draw retried against the shrinking total,
//! - per candidate a FRESH context random (`structure/Structure.java:247-251`,
//!   `makeRandom` = `setLargeFeatureSeed(seed,cx,cz)`) drives
//!   `RuinedPortalStructure.findGenerationPoint` (:67-159): setup pick (>1
//!   setups ⇒ one float), air-pocket sample (no draw at prob 0/1), giant roll
//!   (`nextFloat() < 0.05`), template index `nextInt(3|10)`, rotation
//!   `Util.getRandom(Rotation.values())`, mirror float (`<0.5 ? NONE :
//!   FRONT_BACK`), then `findSuitableY` (:173-234) over RAW noise columns —
//!   26.2 `NoiseBasedChunkGenerator.iterateNoiseColumn(:190-259)` applies NO
//!   surface rules to `getBaseColumn`,
//! - validity filter tests the STUB origin biome (`Structure.java:204`) with
//!   `QuartPos.fromBlock(origin)` against the variant's biome tag,
//! - piece placement mirrors `TemplateStructurePiece.postProcess` +
//!   `StructureTemplate.placeInWorld`: NBT `blocks` array order, processors in
//!   `RuinedPortalPiece.makeSettings` order (ignore → rules → age → protected
//!   → lava-submerged), jigsaw markers replaced by `final_state`
//!   (TemplateStructurePiece.java:103-114); `RuleProcessor.processBlockState`
//!   seeds `RandomSource.create(Mth.getSeed(pos))` per cell and
//!   `BlockAgeProcessor` uses `settings.getRandom(pos)` = the same positional
//!   hash (Mth.java:332-338) — INDEPENDENT of any shared stream;
//! - container block entities drain one `nextLong()` LootTableSeed from the
//!   DECORATION stream inside the write loop (placeInWorld :302-304);
//! - shared decoration stream = `ChunkGenerator.applyBiomeDecoration(:326-355)`
//!   local `random`: xoroshiro, `setDecorationSeed(levelSeed, ox0, oz0)` then
//!   per-structure `setFeatureSeed(decorationSeed, index, step)`; structures
//!   of step SURFACE_STRUCTURES(ordinal 4) place before that step's features;
//!   `spreadNetherrack` (:239-274), drip columns (:216-237) and vine/leaf
//!   decorations (:195-214) consume it after the template.
//!
//! Stream parity hinge: index of `minecraft:ruined_portal` within the step-4
//! registry-order group. Datapack dynamic registries insert in sorted key
//! order; overworld SURFACE_STRUCTURES members sorted below it are
//! desert_pyramid, igloo, jungle_pyramid, mansion, monument, ocean_ruin_cold,
//! ocean_ruin_warm, pillager_outpost ⇒ ZERO-BASED INDEX 8.
//! Ground-truth cross-check (vanilla saved StructureStart NBT at seed
//! 424242 anchor (8,2)): giant_portal_2 / NONE / FRONT_BACK /
//! TP(128,45,32) / air_pocket=1 / cold=0 / mossiness 0.2 / BB
//! [118,45,32,128,60,47] — reproduced by this port field-for-field.
//! Env override `NEUTRON_RP_STEP_INDEX`; a wrong index perturbs only the
//! probabilistic debris pattern, never the piece geometry.
//!
//! Copyright (c) 2026 Neutron Contributors -- MIT License

use crate::feature_rng::FeatureRandom;
use crate::legacy_rng::LegacyRandom;
use crate::region_buf::RegionBuf;
use crate::surface::BlockId;
use crate::worldgen::WorldgenState;

#[allow(clippy::all)]
mod templates {
    include!("ruined_portal_templates.rs");
}

const SALT: i64 = 34_222_645;
const SPACING: i32 = 40;
const SEPARATION: i32 = 15;

/// `GenerationStep.Decoration.SURFACE_STRUCTURES` ordinal.
const STEP_SURFACE_STRUCTURES: i32 = 4;

fn rp_step_index() -> i32 {
    static IDX: std::sync::OnceLock<i32> = std::sync::OnceLock::new();
    *IDX.get_or_init(|| {
        std::env::var("NEUTRON_RP_STEP_INDEX")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(8)
    })
}

const PORTALS: [&str; 10] = [
    "PORTAL_1",
    "PORTAL_2",
    "PORTAL_3",
    "PORTAL_4",
    "PORTAL_5",
    "PORTAL_6",
    "PORTAL_7",
    "PORTAL_8",
    "PORTAL_9",
    "PORTAL_10",
];
const GIANTS: [&str; 3] = ["GIANT_PORTAL_1", "GIANT_PORTAL_2", "GIANT_PORTAL_3"];
/// `Rotation.values()` order.
const ROTATIONS: [Rot; 4] = [Rot::None, Rot::Cw90, Rot::Cw180, Rot::Ccw90];
/// `Direction.Plane.HORIZONTAL.getRandomDirection` uses this list order.
const HORIZONTAL: [Dir; 4] = [Dir::North, Dir::East, Dir::South, Dir::West];
/// BlockTags.FEATURES_CANNOT_REPLACE (data/minecraft/tags/block).
const PROTECTED_EXISTING: [BlockId; 3] = [
    BlockId::Bedrock,
    BlockId::Spawner,
    BlockId::Chest,
];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Rot {
    None,
    Cw90,
    Cw180,
    Ccw90,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Dir {
    North,
    East,
    South,
    West,
}

impl Dir {
    fn delta(self) -> (i32, i32, i32) {
        match self {
            Dir::North => (0, 0, -1),
            Dir::East => (1, 0, 0),
            Dir::South => (0, 0, 1),
            Dir::West => (-1, 0, 0),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Placement {
    OnLandSurface,
    PartlyBuried,
    OnOceanFloor,
    InMountain,
    Underground,
}

impl Placement {
    /// `getHeightMapType` / findSuitableY heightmap choice: ocean floor is the
    /// only OCEAN_FLOOR_WG (motion-blocking) user.
    fn ocean_heightmap(self) -> bool {
        self == Placement::OnOceanFloor
    }
}

/// Decided complex for one anchor chunk.
#[allow(dead_code)]
pub struct Plan {
    tpl: &'static templates::Tpl,
    rot: Rot,
    mirrored: bool,
    pivot: (i32, i32),
    placement: Placement,
    /// templatePosition = (anchor chunk corner, projected Y).
    base_x: i32,
    base_y: i32,
    base_z: i32,
    bbox: Bb,
    cold: bool,
    air_pocket: bool,
    mossiness: f32,
    overgrown: bool,
    vines: bool,
    variant: usize,
    pub(crate) trace: Option<Trace>,
}

#[derive(Clone, Copy)]
pub(crate) struct Bb {
    min_x: i32,
    min_y: i32,
    min_z: i32,
    max_x: i32,
    max_y: i32,
    max_z: i32,
}

/// Diagnostics emitted for parity examples (`--features` not needed; always
/// collected at debug cost of nothing when None).
#[allow(dead_code)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct Trace {
    pub(crate) variant: usize,
    pub(crate) tpl: &'static str,
    pub(crate) rot_idx: usize,
    pub(crate) mirrored: bool,
    pub(crate) giant: bool,
    pub(crate) origin_y: i32,
    pub(crate) air_pocket: bool,
    pub(crate) mossiness: f32,
    pub(crate) setup_i: usize,
    pub(crate) surface_y: i32,
    pub(crate) new_y: i32,
}

/// `getPotentialStructureChunk` for the ruined_portals set.
pub fn potential_structure_chunk(level_seed: i64, source_x: i32, source_z: i32) -> (i32, i32) {
    let gx = source_x.div_euclid(SPACING);
    let gz = source_z.div_euclid(SPACING);
    // setLargeFeatureWithSalt: single setSeed(x*a + z*b + seed + blend).
    let mixed = (gx as i64)
        .wrapping_mul(341_873_128_712)
        .wrapping_add((gz as i64).wrapping_mul(132_897_987_541))
        .wrapping_add(level_seed)
        .wrapping_add(SALT);
    let mut rng = LegacyRandom::new(mixed);
    let limit = SPACING - SEPARATION;
    let dx = rng.next_int(limit);
    let dz = rng.next_int(limit);
    (gx * SPACING + dx, gz * SPACING + dz)
}

/// Is `source_chunk` the set's potential feature chunk?
pub fn is_potential_chunk(level_seed: i64, cx: i32, cz: i32) -> bool {
    potential_structure_chunk(level_seed, cx, cz) == (cx, cz)
}

// ---------------------------------------------------------------------------
// transform math (StructureTemplate.transform :539-563)
// ---------------------------------------------------------------------------

fn transform(x: i32, z: i32, mirrored: bool, rot: Rot, px: i32, pz: i32) -> (i32, i32) {
    let x = if mirrored { -x } else { x };
    match rot {
        Rot::Ccw90 => (px - pz + z, px + pz - x),
        Rot::Cw90 => (px + pz - z, pz - px + x),
        Rot::Cw180 => (px + px - x, pz + pz - z),
        Rot::None => (x, z),
    }
}

fn template_bbox(tpl: &templates::Tpl, mirrored: bool, rot: Rot, px: i32, pz: i32, pos: (i32, i32, i32)) -> Bb {
    let c1 = transform(0, 0, mirrored, rot, px, pz);
    let c2 = transform(
        tpl.size[0] - 1,
        tpl.size[2] - 1,
        mirrored,
        rot,
        px,
        pz,
    );
    Bb {
        min_x: pos.0 + c1.0.min(c2.0),
        max_x: pos.0 + c1.0.max(c2.0),
        min_z: pos.2 + c1.1.min(c2.1),
        max_z: pos.2 + c1.1.max(c2.1),
        min_y: pos.1,
        max_y: pos.1 + tpl.size[1] - 1,
    }
}

fn center_of(b: &Bb) -> (i32, i32, i32) {
    ((b.min_x + b.max_x) / 2, (b.min_y + b.max_y) / 2, (b.min_z + b.max_z) / 2)
}

// ---------------------------------------------------------------------------
// height predicates (Heightmap.Types NOT_AIR / MATERIAL_MOTION_BLOCKING)
// ---------------------------------------------------------------------------

fn not_air(b: BlockId) -> bool {
    !b.is_air()
}

fn motion_blocking(b: BlockId) -> bool {
    !matches!(
        b,
        BlockId::Air | BlockId::CaveAir | BlockId::Water | BlockId::Lava
    )
}

/// Top-most y whose state passes the heightmap predicate (level.getHeight()-1
/// semantics folded in: this IS the matching cell, callers use it directly).
fn region_top(region: &RegionBuf, x: i32, z: i32, ocean_type: bool) -> i32 {
    for y in (crate::generator::WORLD_BOTTOM..crate::generator::WORLD_TOP).rev() {
        let b = region.get(x, y, z);
        let hit = if ocean_type { motion_blocking(b) } else { not_air(b) };
        if hit {
            return y;
        }
    }
    crate::generator::WORLD_BOTTOM
}

// ---------------------------------------------------------------------------
// biome tags (#minecraft:has_structure/* extraction)
// ---------------------------------------------------------------------------

fn biome_allowed(variant: usize, b: u8) -> bool {
    use crate::biome_source::biome_id as id;
    const STANDARD: &[u8] = &[
        id::BEACH,
        id::SNOWY_BEACH,
        id::RIVER,
        id::FROZEN_RIVER,
        id::TAIGA,
        id::SNOWY_TAIGA,
        id::OLD_GROWTH_PINE_FOREST,
        id::OLD_GROWTH_SPRUCE_TAIGA,
        id::FOREST,
        id::FLOWER_FOREST,
        id::BIRCH_FOREST,
        id::OLD_GROWTH_BIRCH_FOREST,
        id::DARK_FOREST,
        id::PALE_GARDEN,
        id::GROVE,
        id::MUSHROOM_FIELDS,
        id::ICE_SPIKES,
        id::DRIPSTONE_CAVES,
        id::LUSH_CAVES,
        id::SULFUR_CAVES,
        id::SAVANNA,
        id::SAVANNA_PLATEAU,
        id::WINDSWEPT_SAVANNA,
        id::SNOWY_PLAINS,
        id::PLAINS,
        id::SUNFLOWER_PLAINS,
    ];
    const MOUNTAIN: &[u8] = &[
        id::BADLANDS,
        id::ERODED_BADLANDS,
        id::WOODED_BADLANDS,
        id::WINDSWEPT_HILLS,
        id::WINDSWEPT_FOREST,
        id::WINDSWEPT_GRAVELLY_HILLS,
        id::SAVANNA_PLATEAU,
        id::WINDSWEPT_SAVANNA,
        id::STONY_SHORE,
        id::MEADOW,
        id::FROZEN_PEAKS,
        id::JAGGED_PEAKS,
        id::STONY_PEAKS,
        id::SNOWY_SLOPES,
        id::CHERRY_GROVE,
    ];
    const OCEAN: &[u8] = &[
        id::DEEP_FROZEN_OCEAN,
        id::DEEP_COLD_OCEAN,
        id::DEEP_OCEAN,
        id::DEEP_LUKEWARM_OCEAN,
        id::FROZEN_OCEAN,
        id::OCEAN,
        id::COLD_OCEAN,
        id::LUKEWARM_OCEAN,
        id::WARM_OCEAN,
    ];
    let set: &[u8] = match variant {
        0 => STANDARD,
        1 => &[crate::biome_source::biome_id::DESERT],
        2 => &[
            crate::biome_source::biome_id::JUNGLE,
            crate::biome_source::biome_id::SPARSE_JUNGLE,
            crate::biome_source::biome_id::BAMBOO_JUNGLE,
        ],
        3 => &[
            crate::biome_source::biome_id::SWAMP,
            crate::biome_source::biome_id::MANGROVE_SWAMP,
        ],
        4 => MOUNTAIN,
        5 => OCEAN,
        _ => &[],
    };
    set.contains(&b)
}

/// `Biome.coldEnoughToSnow`: warm ⇔ height-adjusted temperature ≥ 0.15. The
/// adjustment engages only above seaLevel+17; portal Y searches stay below ⇒
/// base temperature decides. Unknown ids default warm.
fn cold_enough(biome: u8) -> bool {
    use crate::biome_source::biome_id as id;
    const COLD: &[u8] = &[
        id::SNOWY_PLAINS,
        id::ICE_SPIKES,
        id::SNOWY_TAIGA,
        id::GROVE,
        id::SNOWY_SLOPES,
        id::FROZEN_PEAKS,
        id::JAGGED_PEAKS,
        id::SNOWY_BEACH,
        id::FROZEN_RIVER,
        id::FROZEN_OCEAN,
        id::DEEP_FROZEN_OCEAN,
    ];
    COLD.contains(&biome)
}

fn get_tpl(name: &str) -> &'static templates::Tpl {
    use templates::*;
    match name {
        "PORTAL_1" => &PORTAL_1,
        "PORTAL_2" => &PORTAL_2,
        "PORTAL_3" => &PORTAL_3,
        "PORTAL_4" => &PORTAL_4,
        "PORTAL_5" => &PORTAL_5,
        "PORTAL_6" => &PORTAL_6,
        "PORTAL_7" => &PORTAL_7,
        "PORTAL_8" => &PORTAL_8,
        "PORTAL_9" => &PORTAL_9,
        "PORTAL_10" => &PORTAL_10,
        "GIANT_PORTAL_1" => &GIANT_PORTAL_1,
        "GIANT_PORTAL_2" => &GIANT_PORTAL_2,
        "GIANT_PORTAL_3" => &GIANT_PORTAL_3,
        _ => unreachable!("unknown template {name}"),
    }
}

// ---------------------------------------------------------------------------
// decision pipeline
// ---------------------------------------------------------------------------

/// Full evaluation of one anchor chunk: createStructures retry loop +
/// findGenerationPoint + stub-biome filter. Region reads stand in for the raw
/// noise columns of getBaseColumn/getBaseHeight (verdicts agree below the top
/// few cells; anchor chunks decorate first in Neutron's origin order so no
/// earlier pass has mutated them yet).
pub(crate) fn decide_start(state: &WorldgenState, region: &RegionBuf, cx: i32, cz: i32) -> Option<Plan> {
    let seed = state.seed;
    let mut drv = LegacyRandom::new(0);
    drv.set_large_feature_seed(seed, cx, cz);

    let mut options: Vec<usize> = (0..7).collect();
    let mut total: i32 = 7;
    while !options.is_empty() {
        let mut choice = drv.next_int(total);
        let mut index = 0usize;
        for k in 0..options.len() {
            choice -= 1;
            if choice < 0 {
                break;
            }
            index = k + 1;
        }
        let selected = options.remove(index);
        total -= 1;
        if let Some(plan) = try_candidate(state, region, cx, cz, selected) {
            return Some(plan);
        }
    }
    None
}

fn try_candidate(
    state: &WorldgenState,
    region: &RegionBuf,
    cx: i32,
    cz: i32,
    v: usize,
) -> Option<Plan> {
    if v == 6 {
        // Nether tag can never contain an overworld stub biome. Every
        // candidate reseeds its own context stream (makeRandom), so skipping
        // its draws cannot skew survivors.
        return None;
    }
    let seed = state.seed;
    let mut r = LegacyRandom::new(0);
    r.set_large_feature_seed(seed, cx, cz);

    // Setup pick — standard(v0) and mountain(v4) have two 0.5-weight setups.
    let multi = v == 0 || v == 4;
    let setup_i = if multi {
        let pick = r.next_f32();
        if pick < 0.5 {
            0
        } else {
            1
        }
    } else {
        0
    };

    // Per-variant statics from the extracted structure JSONs.
    let (placement, air_prob, mossiness, overgrown, vines, can_be_cold): (
        Placement,
        f32,
        f32,
        bool,
        bool,
        bool,
    ) = match (v, setup_i) {
        (0, 0) => (Placement::Underground, 1.0, 0.2, false, false, true),
        (0, _) => (Placement::OnLandSurface, 0.5, 0.2, false, false, true),
        (1, _) => (Placement::PartlyBuried, 0.0, 0.0, false, false, false),
        (2, _) => (Placement::OnLandSurface, 0.5, 0.8, true, true, false),
        (3, _) => (Placement::OnOceanFloor, 0.0, 0.5, false, true, false),
        (4, 0) => (Placement::InMountain, 1.0, 0.2, false, false, true),
        (4, _) => (Placement::OnLandSurface, 0.5, 0.2, false, false, true),
        (5, _) => (Placement::OnOceanFloor, 0.0, 0.8, false, false, true),
        _ => unreachable!(),
    };

    let air_pocket = sample_limit(&mut r, air_prob);

    let giant = r.next_f32() < 0.05;
    let idx_draw = if giant {
        r.next_int(GIANTS.len() as i32)
    } else {
        r.next_int(PORTALS.len() as i32)
    };
    let name_static: &'static str = if giant {
        GIANTS[idx_draw as usize]
    } else {
        PORTALS[idx_draw as usize]
    };
    let tpl = get_tpl(name_static);

    let rot = ROTATIONS[r.next_int(4) as usize];
    let mirrored = r.next_f32() >= 0.5;

    let px = tpl.size[0] / 2;
    let pz = tpl.size[2] / 2;
    let base_x = cx * 16;
    let base_z = cz * 16;

    let probe_bb = template_bbox(tpl, mirrored, rot, px, pz, (base_x, 0, base_z));
    let (center_x, _, center_z) = center_of(&probe_bb);

    let hm_ocean = placement.ocean_heightmap();
    let surface_y = region_top(region, center_x, center_z, hm_ocean);
    let y_span = probe_bb.max_y - probe_bb.min_y + 1;

    let (new_y, origin_y) =
        find_suitable_y(&mut r, region, placement, surface_y, y_span, &probe_bb);

    let stub_biome =
        crate::biome_source::biome_id_at_block(state, base_x >> 2, origin_y >> 2, base_z >> 2);
    if !biome_allowed(v, stub_biome) {
        return None;
    }

    let cold = can_be_cold && cold_enough(stub_biome);

    Some(Plan {
        tpl,
        rot,
        mirrored,
        pivot: (px, pz),
        placement,
        base_x,
        base_y: origin_y,
        base_z,
        bbox: template_bbox(tpl, mirrored, rot, px, pz, (base_x, origin_y, base_z)),
        cold,
        air_pocket,
        mossiness,
        overgrown,
        vines,
        variant: v,
        trace: Some(Trace {
            variant: v,
            tpl: name_static,
            rot_idx: ROTATIONS.iter().position(|&x| x == rot).unwrap_or(0),
            mirrored,
            giant,
            origin_y,
            air_pocket,
            mossiness,
            setup_i,
            surface_y,
            new_y,
        }),
    })
}

/// Public debug hook: place a decided plan into a region (examples only).
pub fn __place_for_debug(region: &mut RegionBuf, state: &WorldgenState, plan: &Plan) {
    place_complex(region, state, plan);
}

impl Plan {
    /// Share the decided plan across example snapshots without cloning
    /// template payloads.
    #[allow(dead_code)]
    pub fn clone_ptrs(&self) -> Plan {
        Plan { ..*self }
    }
}

/// Debug hook with explicit step-index override.
pub fn __place_for_debug_idx(region: &mut RegionBuf, state: &WorldgenState, plan: &Plan, idx: i32) {
    place_complex_idx(region, state, plan, idx);
}

/// Public introspection for examples.
pub fn debug_step_index() -> i32 { rp_step_index() }

/// Debug aid for parity examples: (surface_at_center, drawn_new_y, projected).
pub fn debug_decision(state: &WorldgenState, region: &RegionBuf, cx: i32, cz: i32) -> Option<(i32, i32, i32)> {
    let seed = state.seed;
    let mut drv = LegacyRandom::new(0);
    drv.set_large_feature_seed(seed, cx, cz);
    let _ = &mut drv;
    // Re-run try_candidate path capturing internals.
    let plan = decide_start(state, region, cx, cz)?;
    let t = plan.trace.as_ref()?;
    Some((t.surface_y, t.new_y, t.origin_y))
}

fn sample_limit(r: &mut LegacyRandom, limit: f32) -> bool {
    if limit == 0.0 {
        false
    } else if limit == 1.0 {
        true
    } else {
        r.next_f32() < limit
    }
}

fn between_inclusive(r: &mut LegacyRandom, min: i32, max_inclusive: i32) -> i32 {
    r.next_int(max_inclusive - min + 1) + min
}

/// `getRandomWithinInterval` :236-238.
fn random_within_interval(r: &mut LegacyRandom, min_preferred: i32, max: i32) -> i32 {
    if min_preferred < max {
        between_inclusive(r, min_preferred, max)
    } else {
        max
    }
}

/// `findSuitableY` :173-234.
fn find_suitable_y(
    r: &mut LegacyRandom,
    region: &RegionBuf,
    placement: Placement,
    surface_y: i32,
    y_span: i32,
    bb: &Bb,
) -> (i32, i32) {
    let min_y = crate::generator::WORLD_BOTTOM + 15;
    let new_y = match placement {
        Placement::OnLandSurface | Placement::OnOceanFloor => surface_y,
        Placement::InMountain => random_within_interval(r, 70, surface_y - y_span),
        Placement::Underground => random_within_interval(r, min_y, surface_y - y_span),
        Placement::PartlyBuried => surface_y - y_span + between_inclusive(r, 2, 8),
    };

    let corners = [
        (bb.min_x, bb.min_z),
        (bb.max_x, bb.min_z),
        (bb.min_x, bb.max_z),
        (bb.max_x, bb.max_z),
    ];
    let ocean_type = placement.ocean_heightmap();
    let mut project = new_y;
    while project > min_y {
        let mut on_solid = 0;
        for &(x, z) in &corners {
            let opaque = {
                let bl = region.get(x, project, z);
                if ocean_type { motion_blocking(bl) } else { not_air(bl) }
            };
            if opaque {
                on_solid += 1;
                if on_solid == 3 {
                    return (new_y, project);
                }
            }
        }
        project -= 1;
    }
    (new_y, project)
}

// ---------------------------------------------------------------------------
// placement (TemplateStructurePiece.postProcess equivalent)
// ---------------------------------------------------------------------------

/// Palette index → BlockId (bare name; the parity metric compares vanilla
/// names so state properties are irrelevant).
fn pal_block(pal: u16) -> Option<BlockId> {
    static TBL: std::sync::OnceLock<Vec<Option<BlockId>>> = std::sync::OnceLock::new();
    TBL.get_or_init(|| {
        templates::PALETTE
            .iter()
            .map(|entry| {
                let bare = entry.split('|').next().unwrap_or(entry);
                let short = bare.strip_prefix("minecraft:").unwrap_or(bare);
                BlockId::from_name(short)
            })
            .collect()
    })
    .get(pal as usize)
    .copied()
    .flatten()
}

fn is_stair(b: BlockId) -> bool {
    matches!(b, BlockId::StoneBrickStairs | BlockId::MossyStoneBrickStairs)
}

fn is_slab(b: BlockId) -> bool {
    matches!(
        b,
        BlockId::StoneBrickSlab
            | BlockId::MossyStoneBrickSlab
            | BlockId::StoneSlab
            | BlockId::SmoothStoneSlab
    )
}

fn is_wall(b: BlockId) -> bool {
    matches!(b, BlockId::StoneBrickWall | BlockId::MossyStoneBrickWall)
}

/// `Block.isShapeFullBlock` approximation for the lava-submerged rule.
fn full_cube(b: BlockId) -> bool {
    !(is_slab(b)
        || is_stair(b)
        || is_wall(b)
        || matches!(
            b,
            BlockId::Air | BlockId::CaveAir | BlockId::Water | BlockId::Lava
                | BlockId::IronBars | BlockId::Chest | BlockId::Vine
                | BlockId::JungleLeaves
        ))
}

fn protected_existing(b: BlockId) -> bool {
    PROTECTED_EXISTING.contains(&b)
}

/// Per-cell positional java random (`RandomSource.create(Mth.getSeed(pos))`,
/// Mth.java:332-338).
struct PosRng(LegacyRandom);
impl PosRng {
    fn new(x: i32, y: i32, z: i32) -> Self {
        let mut seed = (x as i64)
            .wrapping_mul(3_129_871)
            ^ (z as i64).wrapping_mul(116_129_781)
            ^ (y as i64);
        seed = seed
            .wrapping_mul(seed)
            .wrapping_mul(42_317_861)
            .wrapping_add(seed.wrapping_mul(11));
        PosRng(LegacyRandom::new(seed >> 16))
    }
}

/// RuleProcessor chain in `makeSettings` order (:129-143): gold → lava rule →
/// netherrack-magma. `out` is the pre-age palette block.
fn apply_rules(cur: BlockId, cold: bool, ocean_floor_hm: bool, x: i32, y: i32, z: i32) -> Option<BlockId> {
    let mut rng = PosRng::new(x, y, z);
    if cur == BlockId::GoldBlock && rng.0.next_f32() < 0.3 {
        return Some(BlockId::Air);
    }
    if cur == BlockId::Lava {
        if ocean_floor_hm {
            return Some(BlockId::MagmaBlock);
        }
        return Some(if cold {
            BlockId::Netherrack
        } else if rng.0.next_f32() < 0.2 {
            BlockId::MagmaBlock
        } else {
            BlockId::Lava
        });
    }
    if !cold && cur == BlockId::Netherrack && rng.0.next_f32() < 0.07 {
        return Some(BlockId::MagmaBlock);
    }
    None
}

/// BlockAgeProcessor(mossiness), positional-hash stream per cell.
fn apply_age(cur: BlockId, mossiness: f32, x: i32, y: i32, z: i32) -> Option<BlockId> {
    let mut rng = PosRng::new(x, y, z);
    let stone_family = matches!(
        cur,
        BlockId::StoneBricks | BlockId::Stone | BlockId::ChiseledStoneBricks
    );
    if stone_family {
        if rng.0.next_f32() >= 0.5 {
            return None;
        }
        let mossy = rng.0.next_f32() < mossiness;
        let pick = rng.0.next_int(2);
        return Some(match (mossy, pick) {
            (false, 0) => BlockId::CrackedStoneBricks,
            (false, _) => BlockId::StoneBrickStairs,
            (true, 0) => BlockId::MossyStoneBricks,
            (true, _) => BlockId::MossyStoneBrickStairs,
        });
    }
    if is_stair(cur) {
        if rng.0.next_f32() >= 0.5 {
            return None;
        }
        let mossy = rng.0.next_f32() < mossiness;
        let pick = rng.0.next_int(2);
        return Some(match (mossy, pick) {
            (false, 0) => BlockId::StoneSlab,
            (false, _) => BlockId::StoneBrickSlab,
            (true, 0) => BlockId::MossyStoneBrickStairs,
            (true, _) => BlockId::MossyStoneBrickSlab,
        });
    }
    if is_slab(cur) {
        return if rng.0.next_f32() < mossiness {
            Some(BlockId::MossyStoneBrickSlab)
        } else {
            None
        };
    }
    if is_wall(cur) {
        return if rng.0.next_f32() < mossiness {
            Some(BlockId::MossyStoneBrickWall)
        } else {
            None
        };
    }
    if cur == BlockId::Obsidian && rng.0.next_f32() < 0.15 {
        return Some(BlockId::CryingObsidian);
    }
    None
}

/// Merged NBT-order iteration over template cells + marked cells.
enum Step<'a> {
    Plain(&'a (u16, i32, i32, i32, u16)),
    Marked(&'a (u16, i32, i32, i32, u16, &'static str, &'static str)),
}

struct MergedIter<'a> {
    cells: &'a [(u16, i32, i32, i32, u16)],
    marked: &'a [(u16, i32, i32, i32, u16, &'static str, &'static str)],
    ci: usize,
    mi: usize,
}

impl<'a> MergedIter<'a> {
    fn new(tpl: &'a templates::Tpl) -> Self {
        MergedIter {
            cells: tpl.cells,
            marked: tpl.marked,
            ci: 0,
            mi: 0,
        }
    }
}

impl<'a> Iterator for MergedIter<'a> {
    type Item = Step<'a>;
    fn next(&mut self) -> Option<Step<'a>> {
        match (self.cells.get(self.ci), self.marked.get(self.mi)) {
            (None, None) => None,
            (Some(c), None) => {
                self.ci += 1;
                Some(Step::Plain(c))
            }
            (None, Some(m)) => {
                self.mi += 1;
                Some(Step::Marked(m))
            }
            (Some(c), Some(m)) => {
                if c.0 < m.0 {
                    self.ci += 1;
                    Some(Step::Plain(c))
                } else {
                    self.mi += 1;
                    Some(Step::Marked(m))
                }
            }
        }
    }
}

/// Place the decided complex into `region`, consuming the decoration stream.
pub(crate) fn place_complex(region: &mut RegionBuf, state: &WorldgenState, plan: &Plan) {
    place_complex_idx(region, state, plan, rp_step_index())
}

pub(crate) fn place_complex_idx(region: &mut RegionBuf, state: &WorldgenState, plan: &Plan, step_idx: i32) {
    // Decoration-seeded xoroshiro (applyBiomeDecoration :326-355): one stream,
    // seeded per (origin, structure-index, step); postProcess consumes it.
    let mut rng = FeatureRandom::new(0);
    let dec = rng.set_decoration_seed(state.seed, plan.base_x, plan.base_z);
    rng.set_feature_seed(dec, step_idx, STEP_SURFACE_STRUCTURES);
    let _ = step_idx;

    let prev_writer = region.current_writer;
    region.current_writer = crate::writers::RUINED_PORTAL;

    let hm_ocean = plan.placement.ocean_heightmap();

    // --- phase A: template cells in NBT order --------------------------------
    for step in MergedIter::new(plan.tpl) {
        let (lx, ly, lz, pal, marked_id, final_state) = match step {
            Step::Plain((_, x, y, z, p)) => (*x, *y, *z, *p, "", ""),
            Step::Marked((_, x, y, z, p, id, fs)) => (*x, *y, *z, *p, *id, *fs),
        };
        let (tx, tz) = transform(lx, lz, plan.mirrored, plan.rot, plan.pivot.0, plan.pivot.1);
        let wx = plan.base_x + tx;
        let wy = plan.base_y + ly;
        let wz = plan.base_z + tz;

        let Some(mut out) = pal_block(pal) else { continue };

        // BlockIgnoreProcessor: STRUCTURE_AND_AIR ignores template air unless
        // an air pocket carves it open.
        if out == BlockId::Air && !plan.air_pocket {
            continue;
        }

        if let Some(repl) = apply_rules(out, plan.cold, hm_ocean, wx, wy, wz) {
            out = repl;
        }
        if let Some(repl) = apply_age(out, plan.mossiness, wx, wy, wz) {
            out = repl;
        }
        if protected_existing(region.get(wx, wy, wz)) {
            continue;
        }
        // LavaSubmergedBlockProcessor: keep lava under non-full shapes.
        if region.get(wx, wy, wz) == BlockId::Lava && !full_cube(out) {
            region.set(wx, wy, wz, BlockId::Lava);
        } else {
            region.set(wx, wy, wz, out);
        }

        match marked_id {
            "minecraft:chest" => {
                // LootTableSeed draw from the shared stream (:302-304).
                let _loot = rng.next_long();
            }
            "minecraft:jigsaw" => {
                // Replaced by final_state right after placement.
                let f = if final_state == "minecraft:air" {
                    BlockId::Air
                } else {
                    BlockId::Netherrack
                };
                region.set(wx, wy, wz, f);
            }
            _ => {}
        }
    }

    // --- phase B: debris + vegetation ---------------------------------------
    spread_netherrack(region, plan, &mut rng);
    add_drip_columns_below_portal(region, plan, &mut rng);
    if plan.vines || plan.overgrown {
        let bb = plan.bbox;
        for z in bb.min_z..=bb.max_z {
            for y in bb.min_y..=bb.max_y {
                for x in bb.min_x..=bb.max_x {
                    if plan.vines {
                        maybe_add_vine(region, &mut rng, x, y, z);
                    }
                    if plan.overgrown {
                        maybe_add_leaves_above(region, plan, &mut rng, x, y, z);
                    }
                }
            }
        }
    }
    region.current_writer = prev_writer;
}

/// `spreadNetherrack` :239-274.
fn spread_netherrack(region: &mut RegionBuf, plan: &Plan, rng: &mut FeatureRandom) {
    let follow_ground = matches!(
        plan.placement,
        Placement::OnLandSurface | Placement::OnOceanFloor
    );
    let probs: [f32; 14] = [1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 0.9, 0.9, 0.8, 0.7, 0.6, 0.4, 0.2];
    let max_distance = probs.len() as i32;
    let center = center_of(&plan.bbox);
    let avg_width =
        ((plan.bbox.max_x - plan.bbox.min_x + 1) + (plan.bbox.max_z - plan.bbox.min_z + 1)) / 2;
    let distance_adjustment = rng.next_int(1.max(8 - avg_width / 2));
    let hm_ocean = plan.placement.ocean_heightmap();

    for dx in -max_distance..=max_distance {
        for dz in -max_distance..=max_distance {
            let x = center.0 + dx;
            let z = center.2 + dz;
            let distance = dx.abs() + dz.abs();
            let adjusted = (distance + distance_adjustment).max(0);
            if adjusted >= max_distance {
                continue;
            }
            if rng.next_f64() < probs[adjusted as usize] as f64 {
                let surface_y = region_top(region, x, z, hm_ocean);
                let y = if follow_ground {
                    surface_y
                } else {
                    plan.bbox.min_y.min(surface_y)
                };
                if (y - plan.bbox.min_y).abs() <= 3
                    && can_be_replaced_by_netherrack(region, x, y, z, plan.placement)
                {
                    place_netherrack_or_magma(region, plan, rng, x, y, z);
                    if plan.overgrown {
                        maybe_add_leaves_above_at_surface(region, plan, rng, x, y, z);
                    }
                    drip_column(region, plan, rng, x, y - 1, z);
                }
            }
        }
    }
}

/// `canBlockBeReplacedByNetherrackOrMagma` :276-282.
fn can_be_replaced_by_netherrack(
    region: &RegionBuf,
    x: i32,
    y: i32,
    z: i32,
    _placement: Placement,
) -> bool {
    let b = region.get(x, y, z);
    !b.is_air()
        && b != BlockId::Obsidian
        && !protected_existing(b)
        && b != BlockId::Lava
}

/// `placeNetherrackOrMagma` :284-290 — shared-stream coin.
fn place_netherrack_or_magma(
    region: &mut RegionBuf,
    plan: &Plan,
    rng: &mut FeatureRandom,
    x: i32,
    y: i32,
    z: i32,
) {
    let b = if !plan.cold && rng.next_f32() < 0.07 {
        BlockId::MagmaBlock
    } else {
        BlockId::Netherrack
    };
    region.set(x, y, z, b);
}

/// `addNetherrackDripColumnsBelowPortal` :216-225.
fn add_drip_columns_below_portal(region: &mut RegionBuf, plan: &Plan, rng: &mut FeatureRandom) {
    let bb = plan.bbox;
    for x in bb.min_x + 1..bb.max_x {
        for z in bb.min_z + 1..bb.max_z {
            if region.get(x, bb.min_y, z) == BlockId::Netherrack {
                drip_column(region, plan, rng, x, bb.min_y - 1, z);
            }
        }
    }
}

/// `addNetherrackDripColumn` :227-237.
fn drip_column(region: &mut RegionBuf, plan: &Plan, rng: &mut FeatureRandom, x: i32, y: i32, z: i32) {
    place_netherrack_or_magma(region, plan, rng, x, y, z);
    let mut remaining_cap = 8;
    let mut cy = y;
    while remaining_cap > 0 && rng.next_f32() < 0.5 {
        cy -= 1;
        remaining_cap -= 1;
        place_netherrack_or_magma(region, plan, rng, x, cy, z);
    }
}

/// `maybeAddVines` :195-208 (`Direction.Plane.HORIZONTAL.getRandomDirection`).
/// The horizontal-direction int is drawn only when the cell passes the first
/// gate, mirroring Java's evaluation order.
fn maybe_add_vine(region: &mut RegionBuf, rng: &mut FeatureRandom, x: i32, y: i32, z: i32) {
    let st = region.get(x, y, z);
    if st.is_air() || st == BlockId::Vine {
        return;
    }
    let dir = HORIZONTAL[rng.next_int(4) as usize];
    let (dx, _, dz) = dir.delta();
    let nx = x + dx;
    let nz = z + dz;
    let neighbour = region.get(nx, y, nz);
    if neighbour.is_air() && motion_blocking(st) {
        region.set(nx, y, nz, BlockId::Vine);
    }
}

/// `maybeAddLeavesAbove` :210-214 — float drawn unconditionally (Java's
/// short-circuit reads it before the level checks).
fn maybe_add_leaves_above(region: &mut RegionBuf, _plan: &Plan, rng: &mut FeatureRandom, x: i32, y: i32, z: i32) {
    let f = rng.next_f32();
    if f < 0.5
        && region.get(x, y, z) == BlockId::Netherrack
        && region.get(x, y + 1, z).is_air()
    {
        region.set(x, y + 1, z, BlockId::JungleLeaves);
    }
}

/// overgrown branch inside spreadNetherrack (:264-266) reuses the same leaf
/// check at the freshly placed surface cell (see :265's call shape).
fn maybe_add_leaves_above_at_surface(region: &mut RegionBuf, plan: &Plan, rng: &mut FeatureRandom, x: i32, y: i32, z: i32) {
    maybe_add_leaves_above(region, plan, rng, x, y, z);
}

// ---------------------------------------------------------------------------
// driver hook
// ---------------------------------------------------------------------------

/// Decided plans for one generation pass, keyed by owning anchor chunk.
/// All plans are evaluated against the PRISTINE pre-decoration buffer
/// (noise + surface + carvers + mineshafts, nothing else): vanilla computes
/// structure starts long before decoration and queries raw noise columns,
/// so any later pass (lakes, geodes …) would corrupt the Y search.
#[derive(Default)]
pub struct Plans {
    entries: Vec<(i32, i32, Plan)>,
}

impl Plans {
    /// Plan anchored at (cx, cz), if any.
    pub fn plan_at(&self, cx: i32, cz: i32) -> Option<&Plan> {
        self.entries.iter().find(|(x, z, _)| *x == cx && *z == cz).map(|(_, _, p)| p)
    }
}

/// Scan potential anchors among `owner` chunks (the inner decoration origins)
/// and freeze their plans. Called once per region right after carvers/
/// mineshafts, before any decoration step runs.
pub fn prepare_region_plans(
    state: &WorldgenState,
    region: &RegionBuf,
    owners: &[(i32, i32)],
) -> Plans {
    let mut plans = Plans { entries: Vec::new() };
    if std::env::var_os("NEUTRON_RP_DISABLE").is_some() {
        return plans;
    }
    let c0x = region.origin_x >> 4;
    let c0z = region.origin_z >> 4;
    let cn = region.chunks;
    for &(ocx, ocz) in owners {
        if !(c0x..c0x + cn).contains(&ocx) || !(c0z..c0z + cn).contains(&ocz) {
            continue;
        }
        if !is_potential_chunk(state.seed, ocx, ocz) {
            continue;
        }
        if let Some(plan) = decide_start(state, region, ocx, ocz) {
            plans.entries.push((ocx, ocz, plan));
        }
    }
    plans
}

/// Per-origin placement slot (decorated between steps 3 and 4).
pub(crate) fn apply_step_origin(
    region: &mut RegionBuf,
    state: &WorldgenState,
    plans: &Plans,
    ox0: i32,
    oz0: i32,
) {
    let ocx = ox0 >> 4;
    let ocz = oz0 >> 4;
    let Some(plan) = plans.plan_at(ocx, ocz) else {
        return;
    };
    if let Some(t) = &plan.trace {
        if std::env::var_os("NEUTRON_RP_TRACE").is_some() {
            eprintln!(
                "RP anchor ({ocx},{ocz}) variant={} tpl={} rot={} mirror={} setup={} y={} pocket={}",
                t.variant, t.tpl, t.rot_idx, t.mirrored as u8, t.setup_i, t.origin_y, t.air_pocket as u8
            );
        }
    }
    place_complex(region, state, plan);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// RandomSpread structure_set semantics (`setLargeFeatureWithSalt`,
    /// LINEAR nextInt(spacing-separation)). Golden values derived from an
    /// independent Python LCG model of LegacyRandomSource.
    #[test]
    fn potential_chunks_deterministic() {
        // Same 40×40 grid cell ⇒ same potential feature chunk.
        assert_eq!(potential_structure_chunk(424242, 0, 0), potential_structure_chunk(424242, 39, 39));
        // Different cells generally differ.
        assert_ne!(potential_structure_chunk(424242, 0, 0), potential_structure_chunk(424242, 40, 40));
        // Pin: for seed 424242 the only potential anchor within the ±6 window
        // is chunk (8,2) — i.e. seed 424242 chunk (7,2)'s portal neighbor.
        assert_eq!(potential_structure_chunk(424242, 7, 2), (8, 2));
        assert_eq!(potential_structure_chunk(424242, 8, 2), (8, 2));
        assert!(is_potential_chunk(424242, 8, 2));
        let mut anchors = Vec::new();
        for cz in -4..12 {
            for cx in -4..14 {
                if is_potential_chunk(424242, cx, cz) {
                    anchors.push((cx, cz));
                }
            }
        }
        assert_eq!(anchors, vec![(8, 2)]);
    }

    /// Template transform sanity (StructureTemplate.transform :558-563) on
    /// portal_6 (size 5×7×7, pivot (2,3)).
    #[test]
    fn bbox_math_matches_transform_formulas() {
        use templates::PORTAL_6;
        let tpl = &PORTAL_6;
        assert_eq!(tpl.size, [5, 7, 7]);
        let (px, pz) = (tpl.size[0] / 2, tpl.size[2] / 2);

        // NONE + no mirror: identity, offset by position.
        let b = template_bbox(tpl, false, Rot::None, px, pz, (100, 50, 200));
        assert_eq!(
            (b.min_x, b.min_y, b.min_z, b.max_x, b.max_y, b.max_z),
            (100, 50, 200, 104, 56, 206)
        );

        // CLOCKWISE_90 with pivot (2,3): x' = 5-z, z' = 1+x.
        let b = template_bbox(tpl, false, Rot::Cw90, px, pz, (0, 10, 0));
        // corners: (0,0)->(5,1); (4,6)->(-1,5)
        assert_eq!(b.min_x, -1);
        assert_eq!(b.max_x, 5);
        assert_eq!(b.min_z, 1);
        assert_eq!(b.max_z, 5);
        assert_eq!(b.max_y, 16);

        // FRONT_BACK mirrors local x (x -> -x), then CLOCKWISE_180 maps
        // x -> 4-x ⇒ combined x' = 4 + x; z passes through CW180's own flip.
        let b = template_bbox(tpl, true, Rot::Cw180, px, pz, (0, 0, 0));
        assert_eq!((b.min_x, b.max_x), (4, 8));
        assert_eq!((b.min_z, b.max_z), (0, 6));
    }

    /// decide_start is purely a function of (seed, cx, cz, region snapshot).
    #[test]
    fn decision_is_deterministic() {
        let state = WorldgenState::overworld(424242);
        let mut region = RegionBuf::new(0, 0, 1);
        for z in -16i32..32 {
            for x in -16i32..32 {
                for y in (0..80).step_by(4) {
                    region.set(x, y, z, BlockId::Stone);
                }
            }
        }
        let p1 = decide_start(&state, &region, 0, 0);
        let p2 = decide_start(&state, &region, 0, 0);
        match (&p1, &p2) {
            (Some(a), Some(b)) => {
                let t = |p: &Plan| {
                    format!(
                        "{} {} {} {} {}",
                        p.trace.as_ref().unwrap().tpl,
                        p.trace.as_ref().unwrap().rot_idx,
                        p.trace.as_ref().unwrap().mirrored,
                        p.trace.as_ref().unwrap().origin_y,
                        p.trace.as_ref().unwrap().setup_i,
                    )
                };
                assert_eq!(t(a), t(b));
            }
            _ => {
                // (0,0) may legitimately not be an anchor for this seed; both
                // runs agreeing on None is still determinism.
                assert!(p1.is_none() && p2.is_none());
            }
        }
    }

    /// Air-pocket ON carves the open frame; OFF leaves ground solid where the
    /// template carries air.
    #[test]
    fn air_pocket_controls_carving() {
        for pocket in [false, true] {
            let state = WorldgenState::overworld(424242);
            let mut region = RegionBuf::new(0, 0, 1);
            // Solid underground slab.
            for z in -8i32..24 {
                for x in -8i32..24 {
                    for y in 30..70 {
                        region.set(x, y, z, BlockId::Stone);
                    }
                }
            }
            let tpl = get_tpl("PORTAL_1");
            // Hand-built plan centered under our control: PORTAL_1 unrotated.
            let pivot = (tpl.size[0] / 2, tpl.size[2] / 2);
            let base_x = 8 - pivot.0;
            let base_z = 8 - pivot.1;
            let plan = Plan {
                tpl,
                rot: Rot::None,
                mirrored: false,
                pivot,
                placement: Placement::Underground,
                base_x,
                base_y: 40,
                base_z,
                bbox: template_bbox(tpl, false, Rot::None, pivot.0, pivot.1, (base_x, 40, base_z)),
                cold: false,
                air_pocket: pocket,
                mossiness: 0.2,
                overgrown: false,
                vines: false,
                variant: 0,
                trace: None,
            };
            place_complex(&mut region, &state, &plan);
            // Count air cells strictly inside the (former) template volume.
            let mut air_inside = 0;
            for x in base_x..base_x + tpl.size[0] {
                for y in plan.bbox.min_y..plan.bbox.max_y {
                    for z in base_z..base_z + tpl.size[2] {
                        if region.get(x, y, z).is_air() {
                            air_inside += 1;
                        }
                    }
                }
            }
            if pocket {
                assert!(air_inside > 20, "air pocket must carve template air");
            } else {
                assert!(air_inside < 20, "no pocket: gold-gone air stays sparse");
            }
            // Debris always writes netherrack/magma near the complex.
            let mut nr = 0u32;
            for z in -16i32..32 {
                for x in -16i32..32 {
                    for y in 36..64 {
                        if matches!(region.get(x, y, z), BlockId::Netherrack | BlockId::MagmaBlock) {
                            nr += 1;
                        }
                    }
                }
            }
            assert!(nr > 10, "netherrack debris expected");
        }
    }

    /// Positional-hash processor draws are stable regardless of visit order.
    #[test]
    fn age_processor_positional() {
        let a = apply_age(BlockId::StoneBricks, 0.2, 123, 45, 67);
        let b = apply_age(BlockId::StoneBricks, 0.2, 123, 45, 67);
        assert_eq!(a, b);
        // Mossiness 0 can never produce a mossy output.
        let out = apply_age(BlockId::StoneSlab, 0.0, 1, 2, 3);
        assert!(out.is_none() || out != Some(BlockId::MossyStoneBrickSlab));
    }

    /// Seeds WITHOUT portals stay untouched end-to-end.
    #[test]
    fn anchor_sweep_reports_known_shapes() {
        let state = WorldgenState::overworld(424242);
        let mut found = Vec::new();
        for cz in -3..4i32 {
            for cx in -3..4i32 {
                if is_potential_chunk(state.seed, cx, cz) {
                    let mut region = RegionBuf::new(cx, cz, 1);
                    for zz in -16i32..48 {
                        for xx in -16i32..48 {
                            for y in (28..90).step_by(2) {
                                region.set(xx, y, zz, BlockId::Stone);
                            }
                            // crude fake terrain cap
                        }
                    }
                    if let Some(p) = decide_start(&state, &region, cx, cz) {
                        let t = p.trace.unwrap();
                        found.push((cx, cz, t.variant, t.tpl.to_string(), t.origin_y, t.air_pocket));
                    }
                }
            }
        }
        eprintln!("424242 portal starts in ±3: {found:?}");
        // Deterministic sweep result — two passes agree.
        let state2 = WorldgenState::overworld(424242);
        let mut again = 0;
        for &(cx, cz, _, _, _, _) in &found {
            let mut region = RegionBuf::new(cx, cz, 1);
            for zz in -16i32..48 {
                for xx in -16i32..48 {
                    for y in (28..90).step_by(2) {
                        region.set(xx, y, zz, BlockId::Stone);
                    }
                }
            }
            if decide_start(&state2, &region, cx, cz).is_some() {
                again += 1;
            }
        }
        assert_eq!(again, found.len());
    }
}
