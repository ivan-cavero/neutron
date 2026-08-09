// Copyright (c) 2026 Neutron Contributors -- MIT License
//
// MultifaceSpreader — port of net.minecraft.world.level.block.MultifaceSpreader
// (CFR: tools/vanilla-extract/decompiled/.../MultifaceSpreader.java).
//
// Face state is a u8 mask (bit i = Direction.values()[i]), not full BlockState.
// Used by sculk veins and MultifaceGrowthFeature.

use crate::feature_rng::FeatureRandom;
use crate::region_buf::RegionBuf;
use crate::surface::BlockId;
use std::collections::HashMap;

/// Direction.values() order: DOWN, UP, NORTH, SOUTH, WEST, EAST.
pub const DIRS: [(i32, i32, i32); 6] = [
    (0, -1, 0),
    (0, 1, 0),
    (0, 0, -1),
    (0, 0, 1),
    (-1, 0, 0),
    (1, 0, 0),
];

pub type FaceMap = HashMap<(i32, i32, i32), u8>;

/// MultifaceSpreader.SpreadType
#[derive(Clone, Copy, Debug)]
pub enum SpreadType {
    SamePosition,
    SamePlane,
    WrapAround,
}

pub const DEFAULT_SPREAD_ORDER: [SpreadType; 3] = [
    SpreadType::SamePosition,
    SpreadType::SamePlane,
    SpreadType::WrapAround,
];

/// MultifaceSpreader.SpreadPos
#[derive(Clone, Copy, Debug)]
pub struct SpreadPos {
    pub x: i32,
    pub y: i32,
    pub z: i32,
    /// Face index 0..5 (direction the multiface attaches).
    pub face: usize,
}

impl SpreadType {
    /// SpreadType.getSpreadPos(pos, spreadDirection, fromFace)
    pub fn get_spread_pos(
        self,
        x: i32,
        y: i32,
        z: i32,
        spread_dir: usize,
        from_face: usize,
    ) -> SpreadPos {
        let (sdx, sdy, sdz) = DIRS[spread_dir];
        let (fdx, fdy, fdz) = DIRS[from_face];
        match self {
            SpreadType::SamePosition => SpreadPos {
                x,
                y,
                z,
                face: spread_dir,
            },
            SpreadType::SamePlane => SpreadPos {
                x: x + sdx,
                y: y + sdy,
                z: z + sdz,
                face: from_face,
            },
            SpreadType::WrapAround => {
                let opp = opposite(spread_dir);
                SpreadPos {
                    x: x + sdx + fdx,
                    y: y + sdy + fdy,
                    z: z + sdz + fdz,
                    face: opp,
                }
            }
        }
    }
}

fn opposite(dir: usize) -> usize {
    match dir {
        0 => 1,
        1 => 0,
        2 => 3,
        3 => 2,
        4 => 5,
        5 => 4,
        _ => dir,
    }
}

/// Config for sculk vein spreaders.
#[derive(Clone, Copy)]
pub struct VeinSpreadConfig {
    pub spread_types: &'static [SpreadType],
    /// SculkVeinSpreaderConfig.isOtherBlockValidAsSource: !is(SCULK_VEIN)
    pub other_block_valid_as_source: bool,
    /// Use SCULK_REPLACEABLE_WORLD_GEN attach rules for placement validity.
    pub worldgen: bool,
}

impl VeinSpreadConfig {
    /// sameSpaceSpreader: ONLY SAME_POSITION
    pub fn same_space() -> Self {
        static T: [SpreadType; 1] = [SpreadType::SamePosition];
        Self {
            spread_types: &T,
            other_block_valid_as_source: true, // !is(vein) for air/sculk/etc.
            worldgen: true,
        }
    }

    /// veinSpreader: DEFAULT_SPREAD_ORDER
    pub fn vein() -> Self {
        Self {
            spread_types: &DEFAULT_SPREAD_ORDER,
            other_block_valid_as_source: true,
            worldgen: true,
        }
    }
}

/// MultifaceSpreader bound to sculk_vein face map.
pub struct MultifaceSpreader {
    config: VeinSpreadConfig,
}

impl MultifaceSpreader {
    pub fn new(config: VeinSpreadConfig) -> Self {
        Self { config }
    }

    pub fn same_space() -> Self {
        Self::new(VeinSpreadConfig::same_space())
    }

    pub fn vein() -> Self {
        Self::new(VeinSpreadConfig::vein())
    }

    fn has_face_mask(mask: u8, face: usize) -> bool {
        mask & (1u8 << face) != 0
    }

    fn can_spread_from(&self, region: &RegionBuf, faces: &FaceMap, x: i32, y: i32, z: i32, face: usize) -> bool {
        let b = region.get(x, y, z);
        if self.config.other_block_valid_as_source && b != BlockId::SculkVein {
            return true;
        }
        let mask = faces.get(&(x, y, z)).copied().unwrap_or(0);
        Self::has_face_mask(mask, face)
    }

    /// isValidStateForPlacement: neighbour in face direction is sturdy solid.
    fn is_valid_placement_face(region: &RegionBuf, x: i32, y: i32, z: i32, face: usize) -> bool {
        let (dx, dy, dz) = DIRS[face];
        let n = region.get(x + dx, y + dy, z + dz);
        is_sturdy_attach(n)
    }

    fn state_can_be_replaced(&self, region: &RegionBuf, x: i32, y: i32, z: i32) -> bool {
        let b = region.get(x, y, z);
        matches!(b, BlockId::Air | BlockId::Water | BlockId::SculkVein)
            // SculkVeinSpreaderConfig rejects sculk/catalyst at attach target side —
            // handled in can_spread_into via against-state checks for wrap types
            || b == BlockId::SculkVein
    }

    fn can_spread_into(
        &self,
        region: &RegionBuf,
        _sx: i32,
        _sy: i32,
        _sz: i32,
        sp: SpreadPos,
    ) -> bool {
        if !self.state_can_be_replaced(region, sp.x, sp.y, sp.z) {
            return false;
        }
        // Against block must not be sculk/catalyst (SculkVeinSpreaderConfig)
        let (dx, dy, dz) = DIRS[sp.face];
        let against = region.get(sp.x + dx, sp.y + dy, sp.z + dz);
        if matches!(
            against,
            BlockId::Sculk | BlockId::SculkCatalyst
        ) {
            return false;
        }
        Self::is_valid_placement_face(region, sp.x, sp.y, sp.z, sp.face)
    }

    fn place_block(
        &self,
        region: &mut RegionBuf,
        faces: &mut FaceMap,
        sp: SpreadPos,
    ) -> bool {
        if !self.can_spread_into(region, 0, 0, 0, sp) {
            return false;
        }
        let key = (sp.x, sp.y, sp.z);
        let prev = faces.get(&key).copied().unwrap_or(0);
        let bit = 1u8 << sp.face;
        if prev & bit != 0 && region.get(sp.x, sp.y, sp.z) == BlockId::SculkVein {
            return false; // already has face
        }
        let first = prev == 0;
        faces.insert(key, prev | bit);
        let b = region.get(sp.x, sp.y, sp.z);
        if matches!(b, BlockId::Air | BlockId::Water) {
            region.set(sp.x, sp.y, sp.z, BlockId::SculkVein);
            if first {
                // counted once per new vein cell (optional stat — sculk module tracks)
            }
        }
        true
    }

    fn get_spread_from_face_toward_direction(
        &self,
        region: &RegionBuf,
        faces: &FaceMap,
        x: i32,
        y: i32,
        z: i32,
        starting_face: usize,
        spread_dir: usize,
    ) -> Option<SpreadPos> {
        // same axis → empty
        if axis(starting_face) == axis(spread_dir) {
            return None;
        }
        let b = region.get(x, y, z);
        let mask = faces.get(&(x, y, z)).copied().unwrap_or(0);
        let other_ok = self.config.other_block_valid_as_source && b != BlockId::SculkVein;
        let has_start = Self::has_face_mask(mask, starting_face);
        let has_spread = Self::has_face_mask(mask, spread_dir);
        if !(other_ok || (has_start && !has_spread)) {
            return None;
        }
        for &ty in self.config.spread_types {
            let sp = ty.get_spread_pos(x, y, z, spread_dir, starting_face);
            if self.can_spread_into(region, x, y, z, sp) {
                return Some(sp);
            }
        }
        None
    }

    /// spreadAll — returns number of successful face placements.
    pub fn spread_all(
        &self,
        region: &mut RegionBuf,
        faces: &mut FaceMap,
        x: i32,
        y: i32,
        z: i32,
    ) -> u64 {
        let mut count = 0u64;
        for start_face in 0..6 {
            if !self.can_spread_from(region, faces, x, y, z, start_face) {
                continue;
            }
            for spread_dir in 0..6 {
                if let Some(sp) =
                    self.get_spread_from_face_toward_direction(region, faces, x, y, z, start_face, spread_dir)
                {
                    if self.place_block(region, faces, sp) {
                        count += 1;
                    }
                }
            }
        }
        count
    }

    /// spreadFromFaceTowardRandomDirection
    pub fn spread_from_face_toward_random_direction(
        &self,
        rng: &mut FeatureRandom,
        region: &mut RegionBuf,
        faces: &mut FaceMap,
        x: i32,
        y: i32,
        z: i32,
        starting_face: usize,
    ) -> bool {
        let mut order: Vec<usize> = (0..6).collect();
        let mut i = order.len();
        while i > 1 {
            let j = rng.next_int(i as i32) as usize;
            order.swap(i - 1, j);
            i -= 1;
        }
        for spread_dir in order {
            if let Some(sp) =
                self.get_spread_from_face_toward_direction(region, faces, x, y, z, starting_face, spread_dir)
            {
                if self.place_block(region, faces, sp) {
                    return true;
                }
            }
        }
        false
    }

    /// SculkVeinBlock.regrow — set faces from collection if can attach.
    pub fn regrow(
        region: &mut RegionBuf,
        faces: &mut FaceMap,
        x: i32,
        y: i32,
        z: i32,
        face_bits: u8,
    ) -> bool {
        let mut mask = 0u8;
        for i in 0..6 {
            if face_bits & (1u8 << i) == 0 {
                continue;
            }
            if Self::is_valid_placement_face(region, x, y, z, i) {
                mask |= 1u8 << i;
            }
        }
        if mask == 0 {
            return false;
        }
        faces.insert((x, y, z), mask);
        let b = region.get(x, y, z);
        if matches!(b, BlockId::Air | BlockId::Water) {
            region.set(x, y, z, BlockId::SculkVein);
        }
        true
    }
}

fn axis(dir: usize) -> u8 {
    match dir {
        0 | 1 => 1, // Y
        2 | 3 => 2, // Z
        _ => 0,     // X
    }
}

fn is_sturdy_attach(b: BlockId) -> bool {
    !matches!(
        b,
        BlockId::Air
            | BlockId::Water
            | BlockId::Lava
            | BlockId::SculkVein
            | BlockId::Sculk
            | BlockId::SculkCatalyst
            | BlockId::SculkSensor
            | BlockId::SculkShrieker
            | BlockId::OakLeaves
            | BlockId::DarkOakLeaves
            | BlockId::ShortGrass
            | BlockId::LeafLitter
            | BlockId::Snow
            | BlockId::PowderSnow
    )
}

/// Shuffle Direction.allShuffled
pub fn all_shuffled(rng: &mut FeatureRandom) -> Vec<usize> {
    let mut order: Vec<usize> = (0..6).collect();
    let mut i = order.len();
    while i > 1 {
        let j = rng.next_int(i as i32) as usize;
        order.swap(i - 1, j);
        i -= 1;
    }
    order
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_space_places_faces_on_open_with_solid() {
        let mut region = RegionBuf::new(0, 0, 0);
        // y=10 air over deepslate floor at y=9 (within world range)
        region.set(0, 10, 0, BlockId::Air);
        region.set(0, 9, 0, BlockId::Deepslate);
        let mut faces = FaceMap::new();
        let n = MultifaceSpreader::same_space().spread_all(&mut region, &mut faces, 0, 10, 0);
        assert!(n > 0, "should place at least one face, n={n}");
        assert_eq!(region.get(0, 10, 0), BlockId::SculkVein);
        let m = faces.get(&(0, 10, 0)).copied().unwrap_or(0);
        assert!(m & (1 << 0) != 0, "should have DOWN face, mask={m}");
    }
}
