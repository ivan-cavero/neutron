// Copyright (c) 2026 Neutron Contributors -- MIT License
//
// Fluid mechanics for Minecraft 26.2.
//
// Implements water and lava flow, bubble columns, waterlogging,
// and fluid tick scheduling. Fluids are tracked per-block and
// spread according to Minecraft's flow rules.

use std::collections::{HashMap, VecDeque};

// ---------------------------------------------------------------------------
// Block IDs used by fluid mechanics (must match the vanilla registry).
// These overlap with the IDs in neutron-worldgen/src/surface.rs but are
// re-declared here to keep neutron-sim self-contained for fluid logic.
// ---------------------------------------------------------------------------

/// Air
const BLOCK_AIR: u16 = 0;
/// Water
const BLOCK_WATER: u16 = 50;
/// Lava
const BLOCK_LAVA: u16 = 51;
/// Soul sand – creates upward bubble column when under water
const BLOCK_SOUL_SAND: u16 = 88;
/// Magma block – creates downward bubble column when under water
const BLOCK_MAGMA: u16 = 110;

// Waterloggable block IDs (subset – stairs, slabs, fences, walls, buttons, etc.)
const WATERLOGGABLE_BLOCKS: &[u16] = &[
    126, // oak_slab
    127, // stone_slab
    128, // oak_stairs
    85,  // oak_fence
    139, // cobblestone_wall
    77,  // stone_button
    143, // oak_button
    63,  // standing_sign
    68,  // wall_sign
    102, // glass_pane
    55,  // redstone_wire (waterloggable in vanilla)
];

// ---------------------------------------------------------------------------
// Fluid types and state
// ---------------------------------------------------------------------------

/// The type of fluid occupying a block.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FluidType {
    /// Water fluid.
    Water,
    /// Lava fluid.
    Lava,
}

/// Minecraft dimension, affects fluid spread rates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Dimension {
    /// Overworld (and other non-nether dimensions).
    Overworld,
    /// The Nether — lava spreads faster here.
    Nether,
}

/// Maximum horizontal spread distance for lava (vanilla: 4 blocks).
/// Lava at level 4 does not spread further horizontally.
const LAVA_MIN_HORIZONTAL_LEVEL: u8 = 4;

/// The state of a single fluid block.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FluidState {
    /// The type of fluid.
    pub fluid_type: FluidType,
    /// Flow level: 1–8 where 8 is a source block.
    pub level: u8,
}

impl FluidState {
    /// Create a new fluid state.
    pub fn new(fluid_type: FluidType, level: u8) -> Self {
        Self { fluid_type, level }
    }

    /// Returns `true` if this is a source block (level == 8).
    pub fn is_source(&self) -> bool {
        self.level == 8
    }

    /// Returns `true` if this is a flowing block (level < 8).
    pub fn is_flowing(&self) -> bool {
        self.level > 0 && self.level < 8
    }

    /// Returns `true` if this state represents a non-empty fluid.
    pub fn is_fluid(&self) -> bool {
        self.level > 0
    }
}

// ---------------------------------------------------------------------------
// Block access trait
// ---------------------------------------------------------------------------

/// Trait for accessing and mutating blocks in the world.
///
/// Fluids need to read adjacent blocks to decide where to spread and
/// sometimes replace air/empty blocks with flowing fluid.  The concrete
/// implementation lives in the world crate; the fluid engine only needs
/// this trait.
pub trait BlockAccess {
    /// Get the block state ID at `(x, y, z)`.
    fn get_block(&self, x: i32, y: i32, z: i32) -> u16;

    /// Set the block state ID at `(x, y, z)`.
    fn set_block(&mut self, x: i32, y: i32, z: i32, block_id: u16);

    /// Returns `true` if the block at `(x, y, z)` is air (or cave air).
    fn is_air(&self, x: i32, y: i32, z: i32) -> bool;

    /// Returns `true` if the block at `(x, y, z)` is liquid (water or lava block).
    fn is_liquid(&self, x: i32, y: i32, z: i32) -> bool;
}

// ---------------------------------------------------------------------------
// Fluid engine
// ---------------------------------------------------------------------------

/// The fluid simulation engine.
///
/// Tracks all fluid positions in a world region and processes tick-based
/// spreading, bubble columns, waterlogging, and flow direction queries.
pub struct FluidEngine {
    /// All known fluid positions and their states.
    fluids: HashMap<(i32, i32, i32), FluidState>,
    /// Tick scheduling queue: `(x, y, z, scheduled_tick)`.
    tick_queue: VecDeque<(i32, i32, i32, u32)>,
    /// Current game tick counter (monotonically increasing).
    current_tick: u32,
    /// The dimension this engine operates in (affects lava spread rates).
    dimension: Dimension,
}

impl FluidEngine {
    /// Create an empty fluid engine for the overworld dimension.
    pub fn new() -> Self {
        Self::with_dimension(Dimension::Overworld)
    }

    /// Create an empty fluid engine for the given dimension.
    pub fn with_dimension(dimension: Dimension) -> Self {
        Self {
            fluids: HashMap::new(),
            tick_queue: VecDeque::new(),
            current_tick: 0,
            dimension,
        }
    }

    /// Advance the engine by one game tick.
    ///
    /// This processes all fluid ticks that are due and schedules new ticks
    /// for spreading fluid.
    pub fn tick(&mut self, world: &mut dyn BlockAccess) {
        self.current_tick += 1;

        // Process all ticks that are due at or before the current tick.
        let mut to_process: Vec<(i32, i32, i32)> = Vec::new();
        while let Some(&(x, y, z, tick)) = self.tick_queue.front() {
            if tick > self.current_tick {
                break;
            }
            self.tick_queue.pop_front();
            to_process.push((x, y, z));
        }

        for (x, y, z) in to_process {
            if let Some(state) = self.fluids.get(&(x, y, z)).copied() {
                self.process_fluid_tick(x, y, z, state, world);
            }
        }
    }

    /// Place a fluid source block at the given position.
    ///
    /// Creates a source block (level = 8) and schedules it for spreading.
    pub fn place_source(&mut self, x: i32, y: i32, z: i32, fluid_type: FluidType) {
        let state = FluidState::new(fluid_type, 8);
        self.fluids.insert((x, y, z), state);

        // Schedule this source block for tick processing.
        let delay = Self::spread_delay(fluid_type, self.dimension);
        self.tick_queue
            .push_back((x, y, z, self.current_tick + delay));
    }

    /// Get the fluid state at the given position, if any.
    pub fn get_fluid(&self, x: i32, y: i32, z: i32) -> Option<&FluidState> {
        self.fluids.get(&(x, y, z))
    }

    /// Check if a block ID is waterloggable.
    pub fn is_waterloggable(block_id: u16) -> bool {
        WATERLOGGABLE_BLOCKS.contains(&block_id)
    }

    /// Waterlog the block at the given position.
    ///
    /// If the block is waterloggable, it becomes waterlogged (level = 8) and
    /// is registered as a fluid in the engine. The block in the world is not
    /// removed – the water exists *inside* the block.
    pub fn waterlog(&mut self, x: i32, y: i32, z: i32, world: &dyn BlockAccess) -> bool {
        let block_id = world.get_block(x, y, z);
        if !Self::is_waterloggable(block_id) {
            return false;
        }
        let state = FluidState::new(FluidType::Water, 8);
        self.fluids.insert((x, y, z), state);
        true
    }

    /// Get the flow direction at a position.
    ///
    /// Returns a unit-ish vector `(dx, dy, dz)` representing the direction
    /// and magnitude of the fluid flow. Used to push entities.  Returns
    /// `(0.0, 0.0, 0.0)` for source blocks or when there is no flow.
    pub fn get_flow_direction(&self, x: i32, y: i32, z: i32) -> (f64, f64, f64) {
        let state = match self.fluids.get(&(x, y, z)) {
            Some(s) if s.is_flowing() => s,
            _ => return (0.0, 0.0, 0.0),
        };

        let fluid_type = state.fluid_type;
        let current_level = state.level;

        let mut dx: f64 = 0.0;
        let mut dy: f64 = 0.0;
        let mut dz: f64 = 0.0;

        // Check down first – flowing water always descends.
        if let Some(down_state) = self.fluids.get(&(x, y - 1, z)) {
            if down_state.fluid_type == fluid_type && down_state.level < current_level {
                dy = -1.0;
                return (dx, dy, dz);
            }
        }

        // Check horizontal neighbours and pick the steepest descent.
        let neighbours = [
            (1, 0, 0),
            (-1, 0, 0),
            (0, 0, 1),
            (0, 0, -1),
        ];

        let mut best_level = current_level;
        for &(nx, ny, nz) in &neighbours {
            let pos = (x + nx, y + ny, z + nz);
            if let Some(ns) = self.fluids.get(&pos) {
                if ns.fluid_type == fluid_type && ns.level < current_level && ns.level > 0 {
                    // Prefer lower level (steeper descent).
                    if ns.level < best_level {
                        best_level = ns.level;
                        dx = nx as f64;
                        dy = 0.0;
                        dz = nz as f64;
                    }
                }
            }
        }

        // Normalise to unit length.
        let len = (dx * dx + dy * dy + dz * dz).sqrt();
        if len > 0.0 {
            (dx / len, dy / len, dz / len)
        } else {
            (0.0, 0.0, 0.0)
        }
    }

    // -------------------------------------------------------------------
    // Bubble columns
    // -------------------------------------------------------------------

    /// Detect bubble column at a given position.
    ///
    /// A bubble column exists if there is water above a soul sand (upward) or
    /// magma block (downward). Returns `Some(direction)` where direction is
    /// positive for upward, negative for downward.
    pub fn detect_bubble_column(
        &self,
        x: i32,
        y: i32,
        z: i32,
        world: &dyn BlockAccess,
    ) -> Option<i32> {
        // The block below must be soul sand or magma, and there must be
        // water in the current position.
        let below_block = world.get_block(x, y - 1, z);
        let current_block = world.get_block(x, y, z);

        // The current block must be water (block) or contain a fluid.
        let has_water_here = current_block == BLOCK_WATER
            || self
                .fluids
                .get(&(x, y, z))
                .is_some_and(|s| s.fluid_type == FluidType::Water && s.level > 0);

        if !has_water_here {
            return None;
        }

        match below_block {
            BLOCK_SOUL_SAND => Some(1),  // upward
            BLOCK_MAGMA => Some(-1),     // downward
            _ => None,
        }
    }

    /// Get the bubble column velocity at a position.
    ///
    /// Returns the Y velocity applied to entities inside the bubble column.
    /// Soul sand pushes up (`+0.7`), magma pulls down (`-0.25`).
    pub fn bubble_column_velocity(
        &self,
        x: i32,
        y: i32,
        z: i32,
        world: &dyn BlockAccess,
    ) -> f64 {
        match self.detect_bubble_column(x, y, z, world) {
            Some(1) => 0.7,   // soul sand – upward
            Some(-1) => -0.25, // magma – downward
            _ => 0.0,
        }
    }

    // -------------------------------------------------------------------
    // Water level helpers
    // -------------------------------------------------------------------

    /// Remove fluid at the given position.
    pub fn remove_fluid(&mut self, x: i32, y: i32, z: i32) {
        self.fluids.remove(&(x, y, z));
    }

    /// Return the number of tracked fluid blocks.
    pub fn fluid_count(&self) -> usize {
        self.fluids.len()
    }

    /// Return the current game tick.
    pub fn current_tick(&self) -> u32 {
        self.current_tick
    }

    // -------------------------------------------------------------------
    // Internal helpers
    // -------------------------------------------------------------------

    /// Ticks between spread attempts for a given fluid type and dimension.
    ///
    /// Vanilla rates:
    /// - Water: 5 ticks (same in all dimensions)
    /// - Lava overworld: 30 ticks
    /// - Lava nether: 10 ticks
    fn spread_delay(fluid_type: FluidType, dimension: Dimension) -> u32 {
        match fluid_type {
            FluidType::Water => 5,
            FluidType::Lava => match dimension {
                Dimension::Overworld => 30,
                Dimension::Nether => 10,
            },
        }
    }

    /// Process a single fluid tick for a block.
    fn process_fluid_tick(
        &mut self,
        x: i32,
        y: i32,
        z: i32,
        state: FluidState,
        world: &mut dyn BlockAccess,
    ) {
        if state.is_flowing() {
            // A flowing block tries to spread further.
            self.spread_fluid(x, y, z, state, world);
        } else if state.is_source() {
            // A source block tries to create flowing neighbours.
            self.spread_from_source(x, y, z, state, world);
        }
    }

    /// Spread fluid from a source block (creates flowing neighbours).
    fn spread_from_source(
        &mut self,
        x: i32,
        y: i32,
        z: i32,
        state: FluidState,
        world: &mut dyn BlockAccess,
    ) {
        let neighbours = [
            (0, -1, 0),  // down
            (1, 0, 0),   // north
            (-1, 0, 0),  // south
            (0, 0, 1),   // east
            (0, 0, -1),  // west
        ];

        for &(nx, ny, nz) in &neighbours {
            let px = x + nx;
            let py = y + ny;
            let pz = z + nz;

            if self.can_fluid_enter(px, py, pz, state.fluid_type, world) {
                let new_level = if ny == -1 {
                    // Downward flow keeps full level (it's like a waterfall).
                    8
                } else {
                    // Horizontal flow decreases by 1.
                    state.level - 1
                };

                // Lava max horizontal spread: vanilla limits lava to 4 blocks
                // horizontal. Lava at level 4 does not spread further horizontally.
                let is_horizontal = ny != -1;
                if is_horizontal
                    && state.fluid_type == FluidType::Lava
                    && new_level < LAVA_MIN_HORIZONTAL_LEVEL
                {
                    continue;
                }

                if new_level > 0 {
                    self.set_fluid(px, py, pz, state.fluid_type, new_level, world);
                    let delay = Self::spread_delay(state.fluid_type, self.dimension);
                    self.tick_queue
                        .push_back((px, py, pz, self.current_tick + delay));
                }
            }
        }
    }

    /// Spread fluid from a flowing block (continues the flow).
    fn spread_fluid(
        &mut self,
        x: i32,
        y: i32,
        z: i32,
        state: FluidState,
        world: &mut dyn BlockAccess,
    ) {
        let neighbours = [
            (0, -1, 0),  // down
            (1, 0, 0),   // north
            (-1, 0, 0),  // south
            (0, 0, 1),   // east
            (0, 0, -1),  // west
        ];

        for &(nx, ny, nz) in &neighbours {
            let px = x + nx;
            let py = y + ny;
            let pz = z + nz;

            if self.can_fluid_enter(px, py, pz, state.fluid_type, world) {
                let new_level = if ny == -1 {
                    // Downward flow: keep the same level (waterfall behaviour).
                    state.level
                } else {
                    state.level.saturating_sub(1)
                };

                // Lava max horizontal spread: vanilla limits lava to 4 blocks
                // horizontal. Lava at level 4 does not spread further horizontally.
                let is_horizontal = ny != -1;
                if is_horizontal
                    && state.fluid_type == FluidType::Lava
                    && new_level < LAVA_MIN_HORIZONTAL_LEVEL
                {
                    continue;
                }

                if new_level > 0 {
                    // Only update if we are providing a better (higher) level.
                    let should_update = match self.fluids.get(&(px, py, pz)) {
                        None => true,
                        Some(existing) => new_level > existing.level,
                    };

                    if should_update {
                        self.set_fluid(px, py, pz, state.fluid_type, new_level, world);
                        let delay = Self::spread_delay(state.fluid_type, self.dimension);
                        self.tick_queue
                            .push_back((px, py, pz, self.current_tick + delay));
                    }
                }
            }
        }
    }

    /// Check whether a fluid of the given type can enter a position.
    fn can_fluid_enter(
        &self,
        x: i32,
        y: i32,
        z: i32,
        fluid_type: FluidType,
        world: &dyn BlockAccess,
    ) -> bool {
        let block = world.get_block(x, y, z);

        // Cannot flow into solid blocks (only air and water-like blocks).
        match block {
            BLOCK_AIR => true,
            BLOCK_WATER => {
                // Can replace water with same type (for level updates).
                fluid_type == FluidType::Water
            }
            _ => {
                // Check if the position already contains the same fluid.
                if let Some(existing) = self.fluids.get(&(x, y, z)) {
                    existing.fluid_type == fluid_type
                } else {
                    false
                }
            }
        }
    }

    /// Set a fluid in the world at the given position.
    ///
    /// This updates both the internal fluid map and the world block (placing
    /// a water or lava block if the target was air).
    fn set_fluid(
        &mut self,
        x: i32,
        y: i32,
        z: i32,
        fluid_type: FluidType,
        level: u8,
        world: &mut dyn BlockAccess,
    ) {
        let state = FluidState::new(fluid_type, level);
        self.fluids.insert((x, y, z), state);

        // If the world block was air, place the appropriate liquid block.
        let current_block = world.get_block(x, y, z);
        if current_block == BLOCK_AIR {
            match fluid_type {
                FluidType::Water => world.set_block(x, y, z, BLOCK_WATER),
                FluidType::Lava => world.set_block(x, y, z, BLOCK_LAVA),
            }
        }
    }
}

impl Default for FluidEngine {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// A simple in-memory world for testing.
    struct TestWorld {
        blocks: HashMap<(i32, i32, i32), u16>,
    }

    impl TestWorld {
        fn new() -> Self {
            Self {
                blocks: HashMap::new(),
            }
        }

        /// Fill a region with air.
        fn fill_air(&mut self, x1: i32, y1: i32, z1: i32, x2: i32, y2: i32, z2: i32) {
            for x in x1..=x2 {
                for y in y1..=y2 {
                    for z in z1..=z2 {
                        self.blocks.insert((x, y, z), BLOCK_AIR);
                    }
                }
            }
        }

        /// Fill a region with a specific block.
        fn fill_block(
            &mut self,
            x1: i32,
            y1: i32,
            z1: i32,
            x2: i32,
            y2: i32,
            z2: i32,
            block: u16,
        ) {
            for x in x1..=x2 {
                for y in y1..=y2 {
                    for z in z1..=z2 {
                        self.blocks.insert((x, y, z), block);
                    }
                }
            }
        }
    }

    impl BlockAccess for TestWorld {
        fn get_block(&self, x: i32, y: i32, z: i32) -> u16 {
            *self.blocks.get(&(x, y, z)).unwrap_or(&BLOCK_AIR)
        }

        fn set_block(&mut self, x: i32, y: i32, z: i32, block_id: u16) {
            self.blocks.insert((x, y, z), block_id);
        }

        fn is_air(&self, x: i32, y: i32, z: i32) -> bool {
            self.get_block(x, y, z) == BLOCK_AIR
        }

        fn is_liquid(&self, x: i32, y: i32, z: i32) -> bool {
            matches!(self.get_block(x, y, z), BLOCK_WATER | BLOCK_LAVA)
        }
    }

    // -----------------------------------------------------------------
    // Test 1: Water flow spreads from source
    // -----------------------------------------------------------------
    #[test]
    fn test_water_spreads_from_source() {
        let mut world = TestWorld::new();
        // 5x5x5 air region.
        world.fill_air(-2, -2, -2, 2, 2, 2);

        let mut engine = FluidEngine::new();
        engine.place_source(0, 0, 0, FluidType::Water);

        // The source itself should be registered.
        assert!(engine.get_fluid(0, 0, 0).is_some());
        assert!(engine.get_fluid(0, 0, 0).unwrap().is_source());

        // Water spread_delay is 5 ticks. Tick until source is processed.
        for _ in 0..6 {
            engine.tick(&mut world);
        }

        // Check that water spread downward.
        assert!(
            engine.get_fluid(0, -1, 0).is_some(),
            "Water should flow downward from source"
        );

        // And at least one horizontal neighbour.
        let spread = [(1, 0, 0), (-1, 0, 0), (0, 0, 1), (0, 0, -1)]
            .iter()
            .any(|&(dx, dy, dz)| engine.get_fluid(dx, dy, dz).is_some());
        assert!(spread, "Water should spread horizontally from source");
    }

    // -----------------------------------------------------------------
    // Test 2: Water level decreases by 1 per block
    // -----------------------------------------------------------------
    #[test]
    fn test_water_level_decreases() {
        let mut world = TestWorld::new();
        world.fill_air(-5, -2, -5, 5, 2, 5);

        let mut engine = FluidEngine::new();
        engine.place_source(0, 0, 0, FluidType::Water);

        // Water spread_delay is 5 ticks; let it spread for several cycles.
        for _ in 0..20 {
            engine.tick(&mut world);
        }

        // Source should still be level 8.
        let source = engine.get_fluid(0, 0, 0).unwrap();
        assert_eq!(source.level, 8);

        // The block directly below the source should be level 8 (waterfall).
        let below = engine.get_fluid(0, -1, 0);
        assert!(
            below.is_some(),
            "Block below source should have water"
        );

        // A horizontal neighbour one block away from source should be level 7.
        // Check one axis to verify the level decreases.
        if let Some(state) = engine.get_fluid(1, 0, 0) {
            assert!(
                state.level <= 7,
                "First horizontal block should have level <= 7, got {}",
                state.level
            );
            assert!(
                state.level >= 5,
                "First horizontal block should have level >= 5, got {}",
                state.level
            );
        }
    }

    // -----------------------------------------------------------------
    // Test 3: Lava spread is slower than water
    // -----------------------------------------------------------------
    #[test]
    fn test_lava_spread_slower_than_water() {
        let mut world = TestWorld::new();
        world.fill_air(-5, -2, -5, 5, 2, 5);

        let mut water_engine = FluidEngine::new();
        water_engine.place_source(0, 0, 0, FluidType::Water);

        let mut lava_engine = FluidEngine::new();
        lava_engine.place_source(0, 0, 0, FluidType::Lava);

        // Water spread_delay=5, lava spread_delay=30. After 5 ticks water
        // has spread once; lava has not spread at all.
        for _ in 0..6 {
            water_engine.tick(&mut world);
        }

        // Reset world for lava.
        world.fill_air(-5, -2, -5, 5, 2, 5);

        for _ in 0..6 {
            lava_engine.tick(&mut world);
        }

        let water_count = water_engine.fluid_count();
        let lava_count = lava_engine.fluid_count();

        assert!(
            water_count >= lava_count,
            "Water ({} blocks) should spread at least as much as lava ({} blocks) after 6 ticks",
            water_count,
            lava_count
        );

        // Lava should not have spread at all in 6 ticks (spread_delay=30).
        assert!(
            lava_count == 1, // source only
            "Lava should not spread in 6 ticks (spread_delay=30), got {} blocks",
            lava_count
        );
    }

    // -----------------------------------------------------------------
    // Test 4: Waterlogging a block
    // -----------------------------------------------------------------
    #[test]
    fn test_waterlogging() {
        let mut world = TestWorld::new();
        world.set_block(0, 0, 0, 126); // oak_slab (waterloggable)
        world.set_block(1, 0, 0, 1);   // stone (not waterloggable)

        let mut engine = FluidEngine::new();

        // Waterlog the slab.
        let result = engine.waterlog(0, 0, 0, &world);
        assert!(result, "Should be able to waterlog an oak slab");

        let fluid = engine.get_fluid(0, 0, 0).unwrap();
        assert_eq!(fluid.fluid_type, FluidType::Water);
        assert_eq!(fluid.level, 8);
        assert!(fluid.is_source());

        // Cannot waterlog stone.
        let result2 = engine.waterlog(1, 0, 0, &world);
        assert!(!result2, "Should not be able to waterlog stone");

        // The block in the world should still be the slab (not replaced).
        assert_eq!(world.get_block(0, 0, 0), 126);
    }

    // -----------------------------------------------------------------
    // Test 5: Flow direction calculation
    // -----------------------------------------------------------------
    #[test]
    fn test_flow_direction() {
        let mut world = TestWorld::new();
        world.fill_air(-3, -3, -3, 3, 3, 3);

        let mut engine = FluidEngine::new();
        engine.place_source(0, 1, 0, FluidType::Water);

        // Let water spread (spread_delay=5, need enough ticks for multiple cycles).
        for _ in 0..15 {
            engine.tick(&mut world);
        }

        // Flow direction at the source should be (0, 0, 0) — source blocks
        // don't have a flow direction.
        let (dx, dy, dz) = engine.get_flow_direction(0, 1, 0);
        assert_eq!((dx, dy, dz), (0.0, 0.0, 0.0));

        // A flowing block should have a non-zero flow direction.
        // Check any neighbour that has water.
        let mut found_flow = false;
        for &(nx, ny, nz) in &[(1, 0, 0), (-1, 0, 0), (0, 0, 1), (0, 0, -1)] {
            if let Some(state) = engine.get_fluid(nx, ny, nz) {
                if state.is_flowing() {
                    let (fdx, fdy, fdz) = engine.get_flow_direction(nx, ny, nz);
                    let magnitude = (fdx * fdx + fdy * fdy + fdz * fdz).sqrt();
                    if magnitude > 0.0 {
                        found_flow = true;
                        // The direction should point back toward the source or down.
                        assert!(
                            magnitude > 0.9,
                            "Flow direction should be unit length, got magnitude {}",
                            magnitude
                        );
                    }
                }
            }
        }
        assert!(
            found_flow,
            "At least one flowing neighbour should have a non-zero flow direction"
        );
    }

    // -----------------------------------------------------------------
    // Test 6: Bubble column detection
    // -----------------------------------------------------------------
    #[test]
    fn test_bubble_column_detection() {
        let mut world = TestWorld::new();
        world.fill_air(-1, -1, -1, 1, 3, 1);

        // Place water at y=1 and y=2.
        world.set_block(0, 1, 0, BLOCK_WATER);
        world.set_block(0, 2, 0, BLOCK_WATER);

        // Place soul sand below at y=0.
        world.set_block(0, 0, 0, BLOCK_SOUL_SAND);

        let mut engine = FluidEngine::new();
        engine.place_source(0, 2, 0, FluidType::Water);

        // Detect bubble column at y=1 (above soul sand).
        let col = engine.detect_bubble_column(0, 1, 0, &world);
        assert_eq!(col, Some(1), "Soul sand should create upward bubble column");

        let velocity = engine.bubble_column_velocity(0, 1, 0, &world);
        assert!(velocity > 0.0, "Soul sand should push entities upward");

        // Now test magma (downward).
        world.set_block(0, 0, 0, BLOCK_MAGMA);
        let col2 = engine.detect_bubble_column(0, 1, 0, &world);
        assert_eq!(col2, Some(-1), "Magma should create downward bubble column");

        let velocity2 = engine.bubble_column_velocity(0, 1, 0, &world);
        assert!(
            velocity2 < 0.0,
            "Magma should pull entities downward"
        );

        // No bubble column when below block is stone.
        world.set_block(0, 0, 0, 1); // stone
        let col3 = engine.detect_bubble_column(0, 1, 0, &world);
        assert_eq!(col3, None, "Stone should not create a bubble column");
    }

    // -----------------------------------------------------------------
    // Test 7: Water spreads downward first (waterfall behaviour)
    // -----------------------------------------------------------------
    #[test]
    fn test_water_flows_down_first() {
        let mut world = TestWorld::new();
        world.fill_air(-2, -5, -2, 2, 1, 2);

        let mut engine = FluidEngine::new();
        engine.place_source(0, 0, 0, FluidType::Water);

        // Water spread_delay is 5 ticks. Tick until source is processed.
        for _ in 0..6 {
            engine.tick(&mut world);
        }

        let below = engine.get_fluid(0, -1, 0);
        assert!(
            below.is_some(),
            "Water should flow downward from source"
        );

        // Downward flow should keep level 8 (waterfall).
        if let Some(state) = below {
            assert_eq!(
                state.level, 8,
                "Downward flow from source should be level 8, got {}",
                state.level
            );
        }
    }

    // -----------------------------------------------------------------
    // Test 8: FluidEngine default
    // -----------------------------------------------------------------
    #[test]
    fn test_default() {
        let engine = FluidEngine::default();
        assert_eq!(engine.fluid_count(), 0);
        assert_eq!(engine.current_tick(), 0);
    }

    // -----------------------------------------------------------------
    // Test 9: FluidState helpers
    // -----------------------------------------------------------------
    #[test]
    fn test_fluid_state_helpers() {
        let source = FluidState::new(FluidType::Water, 8);
        assert!(source.is_source());
        assert!(!source.is_flowing());
        assert!(source.is_fluid());

        let flowing = FluidState::new(FluidType::Water, 4);
        assert!(!flowing.is_source());
        assert!(flowing.is_flowing());
        assert!(flowing.is_fluid());

        let empty = FluidState::new(FluidType::Water, 0);
        assert!(!empty.is_source());
        assert!(!empty.is_flowing());
        assert!(!empty.is_fluid());
    }

    // -----------------------------------------------------------------
    // Test 10: Remove fluid
    // -----------------------------------------------------------------
    #[test]
    fn test_remove_fluid() {
        let mut engine = FluidEngine::new();
        engine.place_source(0, 0, 0, FluidType::Water);
        assert_eq!(engine.fluid_count(), 1);

        engine.remove_fluid(0, 0, 0);
        assert_eq!(engine.fluid_count(), 0);
        assert!(engine.get_fluid(0, 0, 0).is_none());
    }

    // -----------------------------------------------------------------
    // Test 11: Waterlogging multiple blocks
    // -----------------------------------------------------------------
    #[test]
    fn test_waterlogging_multiple_blocks() {
        let mut world = TestWorld::new();
        // Place waterloggable blocks.
        world.set_block(0, 0, 0, 126); // oak_slab
        world.set_block(1, 0, 0, 127); // stone_slab
        world.set_block(2, 0, 0, 128); // oak_stairs
        world.set_block(3, 0, 0, 85);  // oak_fence
        world.set_block(4, 0, 0, 139); // cobblestone_wall
        world.set_block(5, 0, 0, 77);  // stone_button
        world.set_block(6, 0, 0, 143); // oak_button
        world.set_block(7, 0, 0, 63);  // standing_sign
        world.set_block(8, 0, 0, 68);  // wall_sign
        world.set_block(9, 0, 0, 1);   // stone (not waterloggable)

        let mut engine = FluidEngine::new();

        // Waterlog all waterloggable blocks.
        for x in 0..10 {
            engine.waterlog(x, 0, 0, &world);
        }

        // All waterloggable blocks should be waterlogged.
        for x in 0..9 {
            assert!(
                engine.get_fluid(x, 0, 0).is_some(),
                "Block at x={} should be waterlogged",
                x
            );
        }

        // Stone block should NOT be waterlogged.
        assert!(
            engine.get_fluid(9, 0, 0).is_none(),
            "Stone block should not be waterlogged"
        );
    }

    // -----------------------------------------------------------------
    // Test 12: Lava source placement
    // -----------------------------------------------------------------
    #[test]
    fn test_lava_source_placement() {
        let mut engine = FluidEngine::new();
        engine.place_source(0, 0, 0, FluidType::Lava);

        let state = engine.get_fluid(0, 0, 0).unwrap();
        assert_eq!(state.fluid_type, FluidType::Lava);
        assert!(state.is_source());
        assert_eq!(state.level, 8);
    }

    // -----------------------------------------------------------------
    // Test 13: Flow direction is zero for non-flowing blocks
    // -----------------------------------------------------------------
    #[test]
    fn test_flow_direction_empty() {
        let engine = FluidEngine::new();
        let (dx, dy, dz) = engine.get_flow_direction(0, 0, 0);
        assert_eq!((dx, dy, dz), (0.0, 0.0, 0.0));
    }

    // -----------------------------------------------------------------
    // Test 14: Bubble column with no water
    // -----------------------------------------------------------------
    #[test]
    fn test_bubble_column_no_water() {
        let mut world = TestWorld::new();
        world.fill_air(-1, -1, -1, 1, 3, 1);
        world.set_block(0, 0, 0, BLOCK_SOUL_SAND);

        let engine = FluidEngine::new();
        let col = engine.detect_bubble_column(0, 1, 0, &world);
        assert_eq!(col, None, "No bubble column without water above");
    }

    // -----------------------------------------------------------------
    // Test 15: Lava nether spread rate (10 ticks)
    // -----------------------------------------------------------------
    #[test]
    fn test_lava_nether_spread_rate() {
        let mut world = TestWorld::new();
        world.fill_air(-3, -2, -3, 3, 2, 3);

        let mut engine = FluidEngine::with_dimension(Dimension::Nether);
        engine.place_source(0, 0, 0, FluidType::Lava);

        // Lava nether spread_delay is 10 ticks. After 10 ticks the source
        // should have spread once.
        for _ in 0..11 {
            engine.tick(&mut world);
        }

        let lava_count = engine.fluid_count();
        assert!(
            lava_count > 1,
            "Lava in nether should have spread after 11 ticks, got {} blocks",
            lava_count
        );
        assert!(
            lava_count <= 6,
            "Lava in nether should have spread only once after 11 ticks, got {} blocks",
            lava_count
        );
    }

    // -----------------------------------------------------------------
    // Test 16: Lava overworld spread rate (30 ticks)
    // -----------------------------------------------------------------
    #[test]
    fn test_lava_overworld_spread_rate() {
        let mut world = TestWorld::new();
        world.fill_air(-3, -2, -3, 3, 2, 3);

        let mut engine = FluidEngine::with_dimension(Dimension::Overworld);
        engine.place_source(0, 0, 0, FluidType::Lava);

        // Lava overworld spread_delay is 30 ticks. After 29 ticks the source
        // should NOT have spread yet.
        for _ in 0..29 {
            engine.tick(&mut world);
        }
        assert_eq!(
            engine.fluid_count(),
            1,
            "Lava should not have spread after 29 ticks"
        );

        // After 31 ticks it should have spread once.
        engine.tick(&mut world);
        engine.tick(&mut world);
        let count = engine.fluid_count();
        assert!(
            count > 1,
            "Lava should have spread after 31 ticks, got {} blocks",
            count
        );
    }

    // -----------------------------------------------------------------
    // Test 17: Lava max horizontal spread is 4 blocks
    // -----------------------------------------------------------------
    #[test]
    fn test_lava_max_horizontal_spread() {
        let mut world = TestWorld::new();
        // Wide flat air region for horizontal spreading.
        world.fill_air(-10, 0, -1, 10, 0, 1);

        let mut engine = FluidEngine::new();
        engine.place_source(0, 0, 0, FluidType::Lava);

        // Lava spread_delay=30. Tick enough for 5 spread cycles.
        for _ in 0..155 {
            engine.tick(&mut world);
        }

        // Lava should not spread beyond 4 blocks horizontal.
        // Check x-axis: positions 5 and beyond should be empty.
        for x in 5..=10 {
            assert!(
                engine.get_fluid(x, 0, 0).is_none(),
                "Lava should not spread beyond 4 blocks horizontally, found lava at x={}",
                x
            );
            assert!(
                engine.get_fluid(-x, 0, 0).is_none(),
                "Lava should not spread beyond 4 blocks horizontally, found lava at x={}",
                -x
            );
        }

        // But lava SHOULD have spread to exactly 4 blocks.
        assert!(
            engine.get_fluid(4, 0, 0).is_some(),
            "Lava should spread to 4 blocks horizontally"
        );
        assert!(
            engine.get_fluid(-4, 0, 0).is_some(),
            "Lava should spread to 4 blocks horizontally"
        );
    }

    // -----------------------------------------------------------------
    // Test 18: Water still spreads 7 blocks horizontally (level limit)
    // -----------------------------------------------------------------
    #[test]
    fn test_water_spreads_7_blocks() {
        let mut world = TestWorld::new();
        world.fill_air(-8, 0, -1, 8, 0, 1);

        let mut engine = FluidEngine::new();
        engine.place_source(0, 0, 0, FluidType::Water);

        // Water spread_delay=5. Tick enough for 7+ spread cycles.
        for _ in 0..40 {
            engine.tick(&mut world);
        }

        // Water should spread to 7 blocks horizontal.
        assert!(
            engine.get_fluid(7, 0, 0).is_some(),
            "Water should spread to 7 blocks horizontally"
        );
        assert!(
            engine.get_fluid(-7, 0, 0).is_some(),
            "Water should spread to 7 blocks horizontally"
        );

        // Water should NOT spread beyond 7 blocks.
        assert!(
            engine.get_fluid(8, 0, 0).is_none(),
            "Water should not spread beyond 7 blocks"
        );
        assert!(
            engine.get_fluid(-8, 0, 0).is_none(),
            "Water should not spread beyond 7 blocks"
        );
    }
}
