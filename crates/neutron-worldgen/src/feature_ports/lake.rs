//! LakeFeature port.
use super::*;
use crate::feature_catalog;
use crate::feature_dispatch;
use crate::feature_dispatch::*;
use crate::feature_rng::FeatureRandom;
use crate::generator::{WORLD_BOTTOM, WORLD_TOP};
use crate::legacy_rng::LegacyRandom;
use crate::region_buf::RegionBuf;
use crate::surface::BlockId;
use crate::worldgen::WorldgenState;
use serde_json::Value;
use crate::feature_dispatch::*;


// ---------------------------------------------------------------------------
// lake
// ---------------------------------------------------------------------------

/// `LakeFeature.place` (26.2). `state` is only used for the water-freeze pass.
pub(crate) fn place_lake(
    rng: &mut FeatureRandom,
    region: &mut RegionBuf,
    state: Option<&WorldgenState>,
    x: i32,
    y: i32,
    z: i32,
    cfg: &Value,
) {
    let c = &cfg["config"];
    if y <= WORLD_BOTTOM + 4 {
        return;
    }
    let ox = x - 8;
    let oy = y - 4;
    let oz = z - 8;
    let mut grid = vec![false; 16 * 8 * 16];
    let spots = rng.next_int(4) + 4;
    for _ in 0..spots {
        let xr = rng.next_f64() * 6.0 + 3.0;
        let yr = rng.next_f64() * 4.0 + 2.0;
        let zr = rng.next_f64() * 6.0 + 3.0;
        let xp = rng.next_f64() * (16.0 - xr - 2.0) + 1.0 + xr / 2.0;
        let yp = rng.next_f64() * (8.0 - yr - 4.0) + 2.0 + yr / 2.0;
        let zp = rng.next_f64() * (16.0 - zr - 2.0) + 1.0 + zr / 2.0;
        for xx in 1..15 {
            for zz in 1..15 {
                for yy in 1..7 {
                    let xd = (xx as f64 - xp) / (xr / 2.0);
                    let yd = (yy as f64 - yp) / (yr / 2.0);
                    let zd = (zz as f64 - zp) / (zr / 2.0);
                    let d = xd * xd + yd * yd + zd * zd;
                    if d < 1.0 {
                        grid[(xx * 16 + zz) * 8 + yy] = true;
                    }
                }
            }
        }
    }
    let Some(fluid) = block_from_to_place(rng, &c["fluid"]) else {
        return;
    };
    let edge = |xx: usize, zz: usize, yy: usize| -> bool {
        !grid[(xx * 16 + zz) * 8 + yy]
            && ((xx < 15 && grid[((xx + 1) * 16 + zz) * 8 + yy])
                || (xx > 0 && grid[((xx - 1) * 16 + zz) * 8 + yy])
                || (zz < 15 && grid[(xx * 16 + zz + 1) * 8 + yy])
                || (zz > 0 && grid[(xx * 16 + (zz - 1)) * 8 + yy])
                || (yy < 7 && grid[(xx * 16 + zz) * 8 + yy + 1])
                || (yy > 0 && grid[(xx * 16 + zz) * 8 + (yy - 1)]))
    };
    for xx in 0..16 {
        for zz in 0..16 {
            for yy in 0..8 {
                if edge(xx, zz, yy) {
                    let b = region.get(ox + xx as i32, oy + yy as i32, oz + zz as i32);
                    if yy >= 4 && b.is_fluid() {
                        return;
                    }
                    if yy < 4 && !blocks_motion(b) && b != fluid {
                        return;
                    }
                    if !eval_block_predicate(
                        region,
                        ox + xx as i32,
                        oy + yy as i32,
                        oz + zz as i32,
                        &c["can_place_feature"],
                    ) {
                        return;
                    }
                }
            }
        }
    }
    for xx in 0..16 {
        for zz in 0..16 {
            for yy in 0..8 {
                if grid[(xx * 16 + zz) * 8 + yy] {
                    let (px, py, pz) = (ox + xx as i32, oy + yy as i32, oz + zz as i32);
                    if eval_block_predicate(region, px, py, pz, &c["can_replace_with_air_or_fluid"]) {
                        // LakeFeature.AIR is Blocks.CAVE_AIR (not plain air).
                        let place_air = yy >= 4;
                        region.set(px, py, pz, if place_air { BlockId::CaveAir } else { fluid });
                    }
                }
            }
        }
    }
    if let Some(barrier) = block_from_to_place(rng, &c["barrier"]) {
        if barrier != BlockId::Air {
            for xx in 0..16 {
                for zz in 0..16 {
                    for yy in 0..8 {
                        if edge(xx, zz, yy) && (yy < 4 || rng.next_int(2) != 0) {
                            let (px, py, pz) = (ox + xx as i32, oy + yy as i32, oz + zz as i32);
                            let b = region.get(px, py, pz);
                            if blocks_motion(b)
                                && eval_block_predicate(region, px, py, pz, &c["can_replace_with_barrier"])
                            {
                                region.set(px, py, pz, barrier);
                            }
                        }
                    }
                }
            }
        }
    }
    if fluid == BlockId::Water {
        if let Some(st) = state {
            for xx in 0..16 {
                for zz in 0..16 {
                    let (px, py, pz) = (ox + xx as i32, oy + 4, oz + zz as i32);
                    if !super::simple::biome_warm_enough(st, px, py, pz)
                        && eval_block_predicate(region, px, py, pz, &c["can_replace_with_air_or_fluid"])
                    {
                        region.set(px, py, pz, BlockId::Ice);
                    }
                }
            }
        }
    }
}
