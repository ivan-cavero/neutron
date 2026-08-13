// Copyright (c) 2026 Neutron Contributors -- MIT License
//
// Lighting engine — Starlight-inspired BFS propagation.
//
// Provides both sky light and block light computation for a chunk column.
// Sky light starts at 15 at the top of the world and decreases through
// opaque blocks. Block light propagates from light sources outward.

use std::collections::VecDeque;

use crate::block;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Number of block positions in a 16x16x16 chunk section.
const SECTION_VOLUME: usize = 16 * 16 * 16;

/// Number of nibble-packed bytes per section (2 light values per byte).
const SECTION_NIBBLE_BYTES: usize = SECTION_VOLUME / 2;

/// Number of sections in a vertical column (Y=-64 to Y=320, 384 blocks).
const SECTIONS_PER_COLUMN: usize = 24;

/// World bottom Y coordinate.
const WORLD_BOTTOM: i32 = -64;

/// World top Y coordinate (exclusive).
const WORLD_TOP: i32 = 320;

/// Maximum light level.
const MAX_LIGHT: u8 = 15;

// ---------------------------------------------------------------------------
// ChunkSection — a 16x16x16 slice of blocks
// ---------------------------------------------------------------------------

/// A chunk section: 16x16x16 blocks stored as a flat `Vec<u16>`.
pub struct ChunkSection {
    /// Block state IDs. Index = `y * 256 + z * 16 + x` (section-local).
    pub blocks: Vec<u16>,
}

impl ChunkSection {
    /// Create an empty section (all air).
    pub fn empty() -> Self {
        Self {
            blocks: vec![0; SECTION_VOLUME],
        }
    }

    /// Get the block at section-local coordinates (x, y, z in 0..16).
    pub fn block_at(&self, x: u32, y: u32, z: u32) -> u16 {
        debug_assert!(x < 16 && y < 16 && z < 16);
        self.blocks[(y * 16 * 16 + z * 16 + x) as usize]
    }

    /// Set the block at section-local coordinates.
    pub fn set_block(&mut self, x: u32, y: u32, z: u32, block_id: u16) {
        debug_assert!(x < 16 && y < 16 && z < 16);
        self.blocks[(y * 16 * 16 + z * 16 + x) as usize] = block_id;
    }
}

// ---------------------------------------------------------------------------
// Nibble array helpers
// ---------------------------------------------------------------------------

/// Read a nibble (0-15) from a packed byte array.
#[inline]
fn get_nibble(data: &[u8], index: usize) -> u8 {
    let byte = data[index / 2];
    if index & 1 == 0 {
        byte & 0x0F
    } else {
        byte >> 4
    }
}

/// Write a nibble (0-15) into a packed byte array.
#[inline]
fn set_nibble(data: &mut [u8], index: usize, value: u8) {
    debug_assert!(value <= MAX_LIGHT);
    let byte_idx = index / 2;
    let byte = data[byte_idx];
    if index & 1 == 0 {
        data[byte_idx] = (byte & 0xF0) | (value & 0x0F);
    } else {
        data[byte_idx] = (byte & 0x0F) | ((value & 0x0F) << 4);
    }
}

// ---------------------------------------------------------------------------
// LightEngine
// ---------------------------------------------------------------------------

/// The lighting engine.
///
/// Manages sky light and block light for a single chunk column (24 sections).
/// Uses BFS-based propagation inspired by Starlight.
pub struct LightEngine {
    /// Sky light data for each section. One `[u8; 2048]` per section (nibble-packed).
    sky_light: Vec<[u8; SECTION_NIBBLE_BYTES]>,

    /// Block light data for each section. One `[u8; 2048]` per section (nibble-packed).
    block_light: Vec<[u8; SECTION_NIBBLE_BYTES]>,

    /// Per-section dirty flags: `true` if the section needs re-propagation.
    dirty: Vec<bool>,
}

impl LightEngine {
    /// Create a new light engine with all-zero light data.
    pub fn new() -> Self {
        let zero_section = [0u8; SECTION_NIBBLE_BYTES];
        Self {
            sky_light: vec![zero_section; SECTIONS_PER_COLUMN],
            block_light: vec![zero_section; SECTIONS_PER_COLUMN],
            dirty: vec![false; SECTIONS_PER_COLUMN],
        }
    }

    /// Initialize sky light for a column of chunk sections.
    ///
    /// Topmost sections get full sky light (15). Light decreases downward
    /// through opaque blocks. Transparent blocks reduce by 1.
    ///
    /// `sections` is indexed by section index (0 = bottom, 23 = top).
    pub fn init_sky_light(&mut self, sections: &[ChunkSection; SECTIONS_PER_COLUMN]) {
        // Clear all sky light.
        for s in &mut self.sky_light {
            s.fill(0);
        }

        // Top-down pass: start from the top section.
        for section_idx in (0..SECTIONS_PER_COLUMN).rev() {
            let section_y = section_index_to_y(section_idx);

            for ly in (0..16u32).rev() {
                let wy = section_y + ly as i32;
                if wy >= WORLD_TOP {
                    // Above world top: no light.
                    continue;
                }

                for z in 0..16u32 {
                    for x in 0..16u32 {
                        let block = sections[section_idx].block_at(x, ly, z);
                        let local_idx = (ly * 16 * 16 + z * 16 + x) as usize;

                        if block == 0 {
                            // Air: inherit from above.
                            let above_level = if ly == 15 {
                                // Top of section: look at the section above.
                                if section_idx + 1 < SECTIONS_PER_COLUMN {
                                    let above_y = 0u32; // top of section above
                                    let above_idx = (above_y * 16 * 16 + z * 16 + x) as usize;
                                    get_nibble(&self.sky_light[section_idx + 1], above_idx)
                                } else {
                                    // Top of world: full sky light.
                                    MAX_LIGHT
                                }
                            } else {
                                let above_idx = ((ly + 1) * 16 * 16 + z * 16 + x) as usize;
                                get_nibble(&self.sky_light[section_idx], above_idx)
                            };

                            set_nibble(&mut self.sky_light[section_idx], local_idx, above_level);
                        } else if block::is_transparent(block) {
                            // Transparent block: reduce light by 1.
                            let above_level = if ly == 15 {
                                if section_idx + 1 < SECTIONS_PER_COLUMN {
                                    let above_y = 0u32;
                                    let above_idx = (above_y * 16 * 16 + z * 16 + x) as usize;
                                    get_nibble(&self.sky_light[section_idx + 1], above_idx)
                                } else {
                                    MAX_LIGHT
                                }
                            } else {
                                let above_idx = ((ly + 1) * 16 * 16 + z * 16 + x) as usize;
                                get_nibble(&self.sky_light[section_idx], above_idx)
                            };

                            let reduced = above_level.saturating_sub(1);
                            set_nibble(&mut self.sky_light[section_idx], local_idx, reduced);
                        } else {
                            // Opaque block: blocks light completely.
                            set_nibble(&mut self.sky_light[section_idx], local_idx, 0);
                        }
                    }
                }
            }
        }
    }

    /// Propagate block light from a source position using BFS.
    ///
    /// The light starts at `level` and decreases by 1 per block. Propagation
    /// stops at level 0 or when hitting opaque blocks.
    ///
    /// NOTE: Storage is column-based (24 sections for one x,z column).
    /// For now, propagation works within a single column (y-axis only).
    /// Full 3D propagation across columns requires a 3D section grid.
    pub fn propagate_block_light(
        &mut self,
        x: i32,
        y: i32,
        z: i32,
        level: u8,
        sections: &[ChunkSection; SECTIONS_PER_COLUMN],
    ) {
        if level == 0 || level > MAX_LIGHT {
            return;
        }

        let mut light_map: std::collections::HashMap<(i32, i32, i32), u8> =
            std::collections::HashMap::new();
        let mut queue: VecDeque<(i32, i32, i32, u8)> = VecDeque::new();

        light_map.insert((x, y, z), level);
        queue.push_back((x, y, z, level));

        while let Some((cx, cy, cz, current)) = queue.pop_front() {
            if current <= 1 {
                continue;
            }

            let next = current - 1;

            // Propagate in all 6 directions
            for (dx, dy, dz) in [
                (1i32, 0i32, 0i32),
                (-1, 0, 0),
                (0, 1, 0),
                (0, -1, 0),
                (0, 0, 1),
                (0, 0, -1),
            ] {
                let nx = cx + dx;
                let ny = cy + dy;
                let nz = cz + dz;

                if ny < WORLD_BOTTOM || ny >= WORLD_TOP {
                    continue;
                }

                // Skip positions outside column bounds (cross-column not supported yet)
                if nx < 0 || nx >= 16 || nz < 0 || nz >= 16 {
                    continue;
                }

                let sy = match world_to_section(ny) {
                    Some((s, _)) => s,
                    None => continue,
                };

                let lx = nx as u32;
                let lz = nz as u32;
                let ly = (ny - WORLD_BOTTOM).rem_euclid(16) as u32;

                let block = sections[sy].block_at(lx, ly, lz);
                if block != 0 && !block::is_transparent(block) {
                    continue;
                }

                let entry = light_map.entry((nx, ny, nz)).or_insert(0);
                if *entry < next {
                    *entry = next;
                    queue.push_back((nx, ny, nz, next));
                    // Debug: trace propagation to x=3
                    if nx == 3 && ny == 0 && nz == 0 {
                        eprintln!(
                            "BFS SET (3,0,0)={} from ({},{},{}) cur={}",
                            next, cx, cy, cz, current
                        );
                    }
                }
            }
        }

        // Phase 2: Write all light values to the engine.
        for ((px, py, pz), light_val) in &light_map {
            if *light_val == 0 {
                continue;
            }
            // Skip positions outside column bounds
            if *px < 0 || *px >= 16 || *pz < 0 || *pz >= 16 {
                continue;
            }
            let sy = match world_to_section(*py) {
                Some((s, _)) => s,
                None => continue,
            };
            let lx = *px as u32;
            let lz = *pz as u32;
            let ly = (*py - WORLD_BOTTOM).rem_euclid(16) as u32;
            let idx = (ly * 16 * 16 + lz * 16 + lx) as usize;
            if get_nibble(&self.block_light[sy], idx) < *light_val {
                set_nibble(&mut self.block_light[sy], idx, *light_val);
            }
        }
    }

    /// Handle a block change: remove old light, place new block, re-propagate.
    ///
    /// This implements the incremental update strategy: when a block changes,
    /// we remove the light that the old block was blocking (or the light from
    /// a removed source), then re-propagate from nearby sources.
    pub fn on_block_change(
        &mut self,
        x: i32,
        y: i32,
        z: i32,
        _old_block: u16,
        new_block: u16,
        sections: &[ChunkSection; SECTIONS_PER_COLUMN],
    ) {
        let (_sx, _sl_y, _sl_z) = world_to_section_local_xz(x, y, z);
        let sy = match world_to_section(y) {
            Some((s, _)) => s,
            None => return,
        };

        // Step 1: Remove old light around the changed position.
        self.remove_light_around(x, y, z);

        // Step 2: If the new block emits light, propagate from it.
        let emission = block::light_emission(new_block);
        if emission > 0 {
            self.propagate_block_light(x, y, z, emission, sections);
        }

        // Step 3: Mark nearby sections as dirty for sky light re-propagation.
        let affected_start = sy.saturating_sub(2);
        let affected_end = (sy + 2).min(SECTIONS_PER_COLUMN - 1);
        for s in affected_start..=affected_end {
            self.dirty[s] = true;
        }
    }

    /// Remove all block light in a radius around a position.
    ///
    /// This clears block light within MAX_LIGHT distance.
    fn remove_light_around(&mut self, x: i32, y: i32, z: i32) {
        let radius = MAX_LIGHT as i32;

        for dy in -radius..=radius {
            for dx in -radius..=radius {
                for dz in -radius..=radius {
                    // Manhattan distance check for efficiency.
                    if dx.abs() + dy.abs() + dz.abs() > radius {
                        continue;
                    }

                    let nx = x + dx;
                    let ny = y + dy;
                    let nz = z + dz;

                    if ny < WORLD_BOTTOM || ny >= WORLD_TOP {
                        continue;
                    }

                    let (sx, sl_y, sl_z) = world_to_section_local_xz(nx, ny, nz);
                    let sy = match world_to_section(ny) {
                        Some((s, _)) => s,
                        None => continue,
                    };

                    let idx = (sl_y * 16 * 16 + sl_z * 16 + sx) as usize;
                    set_nibble(&mut self.block_light[sy], idx, 0);
                }
            }
        }
    }

    /// Get sky light level at world coordinates.
    pub fn get_sky_light(&self, x: i32, y: i32, z: i32) -> u8 {
        if y < WORLD_BOTTOM || y >= WORLD_TOP {
            return 0;
        }

        let (sx, sl_y, sl_z) = world_to_section_local_xz(x, y, z);
        let sy = match world_to_section(y) {
            Some((s, _)) => s,
            None => return 0,
        };

        let idx = (sl_y * 16 * 16 + sl_z * 16 + sx) as usize;
        get_nibble(&self.sky_light[sy], idx)
    }

    /// Get block light level at world coordinates.
    pub fn get_block_light(&self, x: i32, y: i32, z: i32) -> u8 {
        if y < WORLD_BOTTOM || y >= WORLD_TOP {
            return 0;
        }

        let (sx, sl_y, sl_z) = world_to_section_local_xz(x, y, z);
        let sy = match world_to_section(y) {
            Some((s, _)) => s,
            None => return 0,
        };

        let idx = (sl_y * 16 * 16 + sl_z * 16 + sx) as usize;
        get_nibble(&self.block_light[sy], idx)
    }

    /// Get the maximum light level at a position (sky or block, whichever is higher).
    pub fn get_light_level(&self, x: i32, y: i32, z: i32) -> u8 {
        self.get_sky_light(x, y, z)
            .max(self.get_block_light(x, y, z))
    }

    /// Get sky light data for a section (for sending to clients).
    pub fn get_sky_light_section(&self, section_y: i32) -> Option<&[u8; SECTION_NIBBLE_BYTES]> {
        let idx = y_to_section_index(section_y)?;
        self.sky_light.get(idx)
    }

    /// Get block light data for a section.
    pub fn get_block_light_section(&self, section_y: i32) -> Option<&[u8; SECTION_NIBBLE_BYTES]> {
        let idx = y_to_section_index(section_y)?;
        self.block_light.get(idx)
    }

    /// Check if a section is dirty (needs re-propagation).
    pub fn is_dirty(&self, section_y: i32) -> bool {
        y_to_section_index(section_y)
            .map(|i| self.dirty[i])
            .unwrap_or(false)
    }

    /// Clear the dirty flag for a section.
    pub fn clear_dirty(&mut self, section_y: i32) {
        if let Some(idx) = y_to_section_index(section_y) {
            self.dirty[idx] = false;
        }
    }

    /// Compute xxHash64 of all light data for parity verification.
    pub fn hash_light_data(&self) -> u64 {
        use std::hash::Hasher as _;
        use xxhash_rust::xxh3::Xxh3;

        let mut hasher = Xxh3::new();

        // Hash sky light.
        for section in &self.sky_light {
            hasher.update(section);
        }

        // Hash block light.
        for section in &self.block_light {
            hasher.update(section);
        }

        hasher.finish()
    }
}

// ---------------------------------------------------------------------------
// Coordinate helpers
// ---------------------------------------------------------------------------

/// Convert a world Y to section index and local Y within that section.
///
/// Returns `(section_index, local_y)` or `None` if out of range.
fn world_to_section(y: i32) -> Option<(usize, u32)> {
    if y < WORLD_BOTTOM || y >= WORLD_TOP {
        return None;
    }
    let offset = y - WORLD_BOTTOM;
    let section = offset.div_euclid(16) as usize;
    let local_y = offset.rem_euclid(16) as u32;
    Some((section, local_y))
}

/// Convert world coordinates to section-local x, y, z.
///
/// Uses div_euclid for correct negative coordinate handling.
/// x and z are chunk-local (wraps at chunk boundaries).
fn world_to_section_local_xz(x: i32, y: i32, z: i32) -> (u32, u32, u32) {
    let local_x = x.rem_euclid(16) as u32;
    let local_z = z.rem_euclid(16) as u32;
    let local_y = (y - WORLD_BOTTOM).rem_euclid(16) as u32;
    (local_x, local_y, local_z)
}

/// Get the section index for a world x coordinate.
fn x_to_section(x: i32) -> isize {
    x.div_euclid(16) as isize
}

/// Convert world coordinates to section-local coordinates.
fn world_to_section_local(x: i32, y: i32, z: i32) -> (u32, u32, u32) {
    world_to_section_local_xz(x, y, z)
}

/// Convert world Y to section index, returning `None` if out of range.
fn y_to_section_index(y: i32) -> Option<usize> {
    if y < WORLD_BOTTOM || y >= WORLD_TOP {
        return None;
    }
    Some(((y - WORLD_BOTTOM) / 16) as usize)
}

/// Convert section index to the world Y of the bottom of that section.
fn section_index_to_y(section_idx: usize) -> i32 {
    WORLD_BOTTOM + (section_idx as i32) * 16
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: create a column of empty sections.
    fn empty_column() -> [ChunkSection; SECTIONS_PER_COLUMN] {
        std::array::from_fn(|_| ChunkSection::empty())
    }

    /// Helper: create a section with a specific block at local coordinates.
    fn section_with_block(x: u32, y: u32, z: u32, block_id: u16) -> ChunkSection {
        let mut s = ChunkSection::empty();
        s.set_block(x, y, z, block_id);
        s
    }

    #[test]
    fn sky_light_top_is_max() {
        let mut engine = LightEngine::new();
        let sections = empty_column();

        engine.init_sky_light(&sections);

        // Top section (y=304..319) should have full sky light in air.
        let level = engine.get_sky_light(0, 319, 0);
        assert_eq!(level, MAX_LIGHT, "top of world should have max sky light");
    }

    #[test]
    fn sky_light_decreases_through_opaque() {
        let mut engine = LightEngine::new();
        let mut sections = empty_column();

        // Place a solid block (stone, id=1) at y=300.
        // Section index for y=300: (300 - (-64)) / 16 = 364/16 = 22
        // Local y: 364 % 16 = 12
        let section_idx = 22;
        let local_y = 12;
        sections[section_idx] = section_with_block(0, local_y, 0, 1);

        engine.init_sky_light(&sections);

        // Light at the block itself should be 0 (opaque).
        let at_block = engine.get_sky_light(0, 300, 0);
        assert_eq!(at_block, 0, "opaque block should have 0 sky light");

        // Light one below should also be 0 (block blocks all light).
        let below = engine.get_sky_light(0, 299, 0);
        assert_eq!(below, 0, "below opaque block should have 0 sky light");

        // Light above should be MAX_LIGHT (air above).
        let above = engine.get_sky_light(0, 301, 0);
        assert_eq!(above, MAX_LIGHT, "air above opaque should have max light");
    }

    #[test]
    fn sky_light_decreases_through_transparent() {
        let mut engine = LightEngine::new();
        let mut sections = empty_column();

        // Place water (id=50, transparent) at y=300.
        let section_idx = 22;
        let local_y = 12;
        sections[section_idx] = section_with_block(0, local_y, 0, 50);

        engine.init_sky_light(&sections);

        // Light at the water should be MAX_LIGHT - 1 (reduced by 1).
        let at_water = engine.get_sky_light(0, 300, 0);
        assert_eq!(
            at_water,
            MAX_LIGHT - 1,
            "transparent block should reduce light by 1"
        );

        // Light below water should be MAX_LIGHT - 1 (air inherits reduced light).
        let below = engine.get_sky_light(0, 299, 0);
        assert_eq!(
            below,
            MAX_LIGHT - 1,
            "below transparent block should inherit the reduced light"
        );
    }

    #[test]
    fn block_light_propagation() {
        let mut engine = LightEngine::new();
        let sections = empty_column();

        // Place a torch (id=50, emission=14) at origin.
        engine.propagate_block_light(0, 0, 0, 14, &sections);

        // Light at source should be 14.
        let at_source = engine.get_block_light(0, 0, 0);
        assert_eq!(at_source, 14, "source should have full emission level");

        // Light 1 block away should be 13.
        let one_away = engine.get_block_light(1, 0, 0);
        assert_eq!(one_away, 13, "1 block from source should be emission - 1");

        // Light 13 blocks away should be 1.
        let far = engine.get_block_light(13, 0, 0);
        assert_eq!(far, 1, "13 blocks from source should be 1");

        // Light 14 blocks away should be 0 (BFS stops at level 1).
        let too_far = engine.get_block_light(14, 0, 0);
        assert_eq!(too_far, 0, "14 blocks from source should be 0");
    }

    #[test]
    fn block_light_blocked_by_opaque() {
        let mut engine = LightEngine::new();
        let mut sections: [ChunkSection; 24] = std::array::from_fn(|_| ChunkSection::empty());

        // Create a TUBE across ALL sections: block ALL positions
        // Then open ONLY (x, 0, 0) for x in 0..5
        for sec in 0..=4usize {
            for ly in 0u32..16 {
                for lz in 0u32..16 {
                    sections[sec].set_block(0, ly, lz, 1);
                    sections[sec].set_block(1, ly, lz, 1);
                    sections[sec].set_block(2, ly, lz, 1);
                    sections[sec].set_block(3, ly, lz, 1);
                    sections[sec].set_block(4, ly, lz, 1);
                }
            }
        }
        // Open the x-axis tunnel at y=0 in section 4
        sections[4].set_block(0, 0, 0, 0);
        sections[4].set_block(1, 0, 0, 0);
        sections[4].set_block(3, 0, 0, 0);
        sections[4].set_block(4, 0, 0, 0);
        // x=2 remains solid (wall)

        // Propagate from (0,0,0).
        engine.propagate_block_light(0, 0, 0, 14, &sections);

        // Light at the solid block should be 0.
        let at_wall = engine.get_block_light(2, 0, 0);
        assert_eq!(at_wall, 0, "opaque block should block light");

        // Light behind the wall should be 0 (shadow).
        let behind = engine.get_block_light(3, 0, 0);
        assert_eq!(behind, 0, "behind opaque block should have 0 light");
    }

    #[test]
    fn block_light_through_transparent() {
        let mut engine = LightEngine::new();
        let mut sections = empty_column();

        // Place water (transparent) at (2, 0, 0).
        // y=0 maps to section index (0 - (-64)) / 16 = 4
        let section_idx = 4;
        let local_y = 0;
        sections[section_idx] = section_with_block(2, local_y, 0, 50);

        engine.propagate_block_light(0, 0, 0, 14, &sections);

        // Light at the water should be 12 (14 - 2 for distance).
        let at_water = engine.get_block_light(2, 0, 0);
        assert_eq!(
            at_water, 12,
            "light through transparent should be distance - 1"
        );

        // Light beyond water should be 11.
        let beyond = engine.get_block_light(3, 0, 0);
        assert_eq!(beyond, 11, "light beyond transparent should continue");
    }

    #[test]
    fn dirty_flag_system() {
        let mut engine = LightEngine::new();

        // y=10 maps to section index (10 - (-64)) / 16 = 4
        let section_y = 10;
        let section_idx = 4;

        assert!(!engine.is_dirty(section_y), "section should start clean");

        engine.dirty[section_idx] = true;
        assert!(
            engine.is_dirty(section_y),
            "section should be dirty after marking"
        );

        engine.clear_dirty(section_y);
        assert!(
            !engine.is_dirty(section_y),
            "section should be clean after clearing"
        );
    }

    #[test]
    fn on_block_change_updates_light() {
        let mut engine = LightEngine::new();
        let mut sections = empty_column();

        // Place a torch at (5, 0, 0).
        // y=0 maps to section index (0 - (-64)) / 16 = 4
        let torch_section = 4;
        let torch_local_y = 0;
        sections[torch_section] = section_with_block(5, torch_local_y, 0, 50);

        // Initialize block light from the torch.
        engine.propagate_block_light(5, 0, 0, 14, &sections);

        // Verify torch emits light.
        let at_torch = engine.get_block_light(5, 0, 0);
        assert_eq!(at_torch, 14, "torch should emit light 14");

        // Now remove the torch (replace with air).
        sections[torch_section] = ChunkSection::empty();

        // Handle block change.
        engine.on_block_change(5, 0, 0, 50, 0, &sections);

        // Light should be cleared around the old torch position.
        let after_remove = engine.get_block_light(5, 0, 0);
        assert_eq!(after_remove, 0, "removed torch should have 0 light");
    }

    #[test]
    fn hash_is_deterministic() {
        let engine1 = LightEngine::new();
        let engine2 = LightEngine::new();

        assert_eq!(
            engine1.hash_light_data(),
            engine2.hash_light_data(),
            "identical engines should have same hash"
        );
    }

    #[test]
    fn hash_differs_with_different_light() {
        let mut engine1 = LightEngine::new();
        let mut engine2 = LightEngine::new();
        let sections = empty_column();

        engine1.propagate_block_light(0, 0, 0, 14, &sections);
        engine2.propagate_block_light(0, 0, 0, 15, &sections);

        assert_ne!(
            engine1.hash_light_data(),
            engine2.hash_light_data(),
            "different light should produce different hashes"
        );
    }

    #[test]
    fn debug_bfs_propagation() {
        let mut engine = LightEngine::new();
        let mut sections: [ChunkSection; 24] = std::array::from_fn(|_| ChunkSection::empty());

        // Create a TUBE: block ALL positions with y!=0 or z!=0 in the x range
        for x in 0i32..16 {
            for ly in 0u32..16 {
                for lz in 0u32..16 {
                    if ly != 0 || lz != 0 {
                        sections[4].set_block(x as u32, ly, lz, 1);
                    }
                }
            }
        }

        // Verify tube is correct: check that (0,1,0) is blocked
        let block_at_010 = sections[4].block_at(0, 1, 0);
        eprintln!("Block at (0,1,0): {}", block_at_010);
        assert!(block_at_010 != 0, "tube should block (0,1,0)");

        engine.propagate_block_light(0, 0, 0, 14, &sections);

        for dist in 0..=14 {
            let level = engine.get_block_light(dist, 0, 0);
            eprintln!("Distance {}: level {}", dist, level);
        }

        assert_eq!(engine.get_block_light(0, 0, 0), 14);
        assert_eq!(engine.get_block_light(1, 0, 0), 13);
        assert_eq!(engine.get_block_light(13, 0, 0), 1);
    }
}
