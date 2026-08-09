// Copyright (c) 2026 Neutron Contributors -- MIT License
//
// Mob spawning engine for Minecraft 26.2.
//
// Handles spawn cycle ticking, light/biome/distance checks, pack spawning,
// spawn caps, and despawn logic. All world data is accessed through the
// `SpawnAccess` trait so the engine stays decoupled from chunk storage.

use std::collections::HashMap;

// ---------------------------------------------------------------------------
// BiomeId (local definition matching neutron-worldgen::biome::BiomeId)
// ---------------------------------------------------------------------------

/// Biome IDs matching Minecraft's internal registry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum BiomeId {
    Ocean = 0,
    Plains = 1,
    Desert = 2,
    Forest = 3,
    Taiga = 4,
    Swamp = 5,
    River = 6,
    Beach = 7,
    DeepOcean = 8,
    SnowyPlains = 9,
    Jungle = 10,
    Savanna = 11,
    DarkForest = 12,
    StonyShore = 13,
    Meadow = 14,
    FrozenOcean = 15,
    FrozenRiver = 16,
    IceSpikes = 17,
    OldGrowthBirchForest = 18,
    OldGrowthPineForest = 19,
    WindsweptHills = 20,
    Grove = 21,
    SnowySlopes = 22,
    JaggedPeaks = 23,
    FrozenPeaks = 24,
    StonyPeaks = 25,
    Badlands = 26,
    ErodedBadlands = 27,
    WoodedBadlands = 28,
    MushroomFields = 29,
    CherryGrove = 30,
    DeepDark = 31,
    MangroveSwamp = 32,
    BirchForest = 33,
    LushCaves = 34,
    DripstoneCaves = 35,
}

impl BiomeId {
    /// Whether this biome is an ocean biome.
    pub fn is_ocean(self) -> bool {
        matches!(self, Self::Ocean | Self::DeepOcean | Self::FrozenOcean)
    }

    /// Whether this biome is cold (snowy/frozen).
    pub fn is_cold(self) -> bool {
        matches!(
            self,
            Self::SnowyPlains
                | Self::IceSpikes
                | Self::FrozenOcean
                | Self::FrozenRiver
                | Self::Grove
                | Self::SnowySlopes
                | Self::JaggedPeaks
                | Self::FrozenPeaks
        )
    }

    /// Whether this biome is warm (hot/dry).
    pub fn is_warm(self) -> bool {
        matches!(
            self,
            Self::Desert
                | Self::Savanna
                | Self::Badlands
                | Self::ErodedBadlands
                | Self::WoodedBadlands
                | Self::StonyPeaks
        )
    }
}

// ---------------------------------------------------------------------------
// Mob categories & types
// ---------------------------------------------------------------------------

/// Vanilla mob categories that govern spawn caps and tick frequency.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MobCategory {
    /// Hostile mobs: zombies, skeletons, creepers, etc.
    Monster,
    /// Passive mobs: cows, pigs, sheep, etc.
    Creature,
    /// Ambient mobs: bats.
    Ambient,
    /// Water creatures: squid, dolphins.
    WaterCreature,
    /// Small water mobs: tropical fish, cod, salmon.
    WaterAmbient,
    /// Glow squid (underground only).
    UndergroundCreature,
    /// Axolotl (lush caves).
    Axolotl,
}

/// Concrete mob types that can spawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MobType {
    // --- Hostile ---
    Zombie,
    Skeleton,
    Creeper,
    Spider,
    Enderman,
    Witch,
    ZombieVillager,
    // --- Passive ---
    Cow,
    Pig,
    Sheep,
    Chicken,
    Horse,
    Rabbit,
    Fox,
    Bee,
    // --- Ambient ---
    Bat,
    // --- Water ---
    Squid,
    GlowSquid,
    Cod,
    Salmon,
    TropicalFish,
    Pufferfish,
    Dolphin,
    Axolotl,
}

impl MobType {
    /// Return the vanilla category for this mob type.
    pub fn category(self) -> MobCategory {
        match self {
            Self::Zombie
            | Self::Skeleton
            | Self::Creeper
            | Self::Spider
            | Self::Enderman
            | Self::Witch
            | Self::ZombieVillager => MobCategory::Monster,

            Self::Cow
            | Self::Pig
            | Self::Sheep
            | Self::Chicken
            | Self::Horse
            | Self::Rabbit
            | Self::Fox
            | Self::Bee => MobCategory::Creature,

            Self::Bat => MobCategory::Ambient,

            Self::Squid | Self::Dolphin => MobCategory::WaterCreature,

            Self::Cod
            | Self::Salmon
            | Self::TropicalFish
            | Self::Pufferfish => MobCategory::WaterAmbient,

            Self::GlowSquid => MobCategory::UndergroundCreature,

            Self::Axolotl => MobCategory::Axolotl,
        }
    }

    /// Default pack size range (min, max) for this mob type.
    pub fn pack_range(self) -> (u32, u32) {
        match self {
            Self::Zombie | Self::Skeleton | Self::Creeper | Self::Spider => (2, 4),
            Self::Enderman => (1, 1),
            Self::Witch | Self::ZombieVillager => (1, 1),
            Self::Cow | Self::Pig | Self::Sheep | Self::Chicken => (4, 4),
            Self::Horse => (2, 6),
            Self::Rabbit => (2, 3),
            Self::Fox => (2, 4),
            Self::Bee => (1, 1),
            Self::Bat => (8, 8),
            Self::Squid => (1, 2),
            Self::GlowSquid => (1, 2),
            Self::Cod | Self::Salmon => (3, 5),
            Self::TropicalFish => (2, 5),
            Self::Pufferfish => (1, 1),
            Self::Dolphin => (1, 2),
            Self::Axolotl => (3, 5),
        }
    }
}

// ---------------------------------------------------------------------------
// World-access trait
// ---------------------------------------------------------------------------

/// Trait for querying block and light data from the world.
///
/// Implementations provide access to chunk storage without coupling the
/// spawn engine to a specific chunk format.
pub trait SpawnAccess {
    /// Get the block state ID at (x, y, z).
    fn get_block(&self, x: i32, y: i32, z: i32) -> u16;

    /// Get the sky light level (0-15) at (x, y, z).
    fn get_sky_light(&self, x: i32, y: i32, z: i32) -> u8;

    /// Get the block light level (0-15) at (x, y, z).
    fn get_block_light(&self, x: i32, y: i32, z: i32) -> u8;

    /// Get the biome at (x, z).
    fn get_biome(&self, x: i32, z: i32) -> BiomeId;

    /// Whether the block at (x, y, z) is solid (not air/water/etc.).
    fn is_solid(&self, x: i32, y: i32, z: i32) -> bool;

    /// The highest non-solid Y at (x, z) -- used to find spawn surface.
    fn get_heightmap_y(&self, x: i32, z: i32) -> i32;
}

// ---------------------------------------------------------------------------
// Spawn candidate
// ---------------------------------------------------------------------------

/// A proposed mob spawn produced by the engine.
#[derive(Debug, Clone)]
pub struct SpawnCandidate {
    /// X coordinate.
    pub x: i32,
    /// Y coordinate.
    pub y: i32,
    /// Z coordinate.
    pub z: i32,
    /// Type of mob to spawn.
    pub mob_type: MobType,
    /// Category for cap tracking.
    pub category: MobCategory,
}

// ---------------------------------------------------------------------------
// Spawn engine
// ---------------------------------------------------------------------------

/// Core spawn engine: tracks caps, runs spawn cycles, computes despawn.
pub struct SpawnEngine {
    /// Per-category spawn caps.
    mob_caps: HashMap<MobCategory, i32>,
    /// Current per-category spawn counts (tracked across ticks).
    spawned_counts: HashMap<MobCategory, i32>,
    /// Tick counter for passive spawn cycles (every 400 ticks).
    spawn_cycle: u32,
}

impl SpawnEngine {
    /// Create a new engine with vanilla 26.2 spawn caps.
    pub fn new() -> Self {
        let mut caps = HashMap::new();
        caps.insert(MobCategory::Monster, 70);
        caps.insert(MobCategory::Creature, 10); // per player
        caps.insert(MobCategory::Ambient, 15);
        caps.insert(MobCategory::WaterCreature, 5); // per player
        caps.insert(MobCategory::WaterAmbient, 20); // per player
        caps.insert(MobCategory::UndergroundCreature, 5); // per player
        caps.insert(MobCategory::Axolotl, 5); // per player

        let mut counts = HashMap::new();
        for cat in caps.keys() {
            counts.insert(*cat, 0);
        }

        Self {
            mob_caps: caps,
            spawned_counts: counts,
            spawn_cycle: 0,
        }
    }

    /// Reset spawned counts (e.g. on world load).
    pub fn reset_counts(&mut self) {
        for v in self.spawned_counts.values_mut() {
            *v = 0;
        }
    }

    /// Adjust cap for a per-player category based on player count.
    pub fn effective_cap(&self, category: MobCategory, player_count: i32) -> i32 {
        let base = *self.mob_caps.get(&category).unwrap_or(&0);
        match category {
            MobCategory::Monster | MobCategory::Ambient => base,
            _ => base * player_count.max(1),
        }
    }

    /// Check whether a position satisfies the light-level requirement.
    ///
    /// Vanilla rules:
    /// - Hostile (Monster): `sky_light <= 7 AND block_light == 0`
    /// - Passive (Creature): `sky_light > 7` (sky light only)
    /// - Ambient (Bats): `sky_light <= 7 AND block_light == 0`
    /// - UndergroundCreature: `sky_light <= 7 AND block_light == 0`
    /// - Water mobs: no strict light requirement
    pub fn meets_light_requirements(
        &self,
        pos: (i32, i32, i32),
        category: MobCategory,
        world: &dyn SpawnAccess,
    ) -> bool {
        let sky_light = world.get_sky_light(pos.0, pos.1, pos.2);
        let block_light = world.get_block_light(pos.0, pos.1, pos.2);
        match category {
            // Hostile: sky_light <= 7 AND block_light == 0
            MobCategory::Monster => sky_light <= 7 && block_light == 0,
            // Passive: sky_light > 7 (only sky light matters)
            MobCategory::Creature => sky_light > 7,
            // Ambient: sky_light <= 7 AND block_light == 0
            MobCategory::Ambient => sky_light <= 7 && block_light == 0,
            // Water mobs: no strict light requirement
            MobCategory::WaterCreature
            | MobCategory::WaterAmbient
            | MobCategory::Axolotl => true,
            // Underground creature: sky_light <= 7 AND block_light == 0
            MobCategory::UndergroundCreature => sky_light <= 7 && block_light == 0,
        }
    }

    /// Check distance constraints relative to any player.
    pub fn distance_ok(entity_pos: (i32, i32, i32), players: &[(i32, i32, i32)]) -> bool {
        if players.is_empty() {
            return false;
        }
        let min_sq = 24 * 24; // 24 blocks minimum
        let max_sq = 128 * 128; // 128 blocks maximum
        players.iter().all(|p| {
            let dx = entity_pos.0 - p.0;
            let dy = entity_pos.1 - p.1;
            let dz = entity_pos.2 - p.2;
            let d_sq = dx * dx + dy * dy + dz * dz;
            d_sq >= min_sq && d_sq <= max_sq
        })
    }

    /// Check a spawn candidate position: solid floor, non-solid at spawn,
    /// light requirements, distance from players, cap not exceeded.
    pub fn is_valid_spawn(
        &self,
        pos: (i32, i32, i32),
        category: MobCategory,
        players: &[(i32, i32, i32)],
        world: &dyn SpawnAccess,
    ) -> bool {
        // Must have a solid block below.
        if !world.is_solid(pos.0, pos.1 - 1, pos.2) {
            return false;
        }
        // Spawn position itself must not be solid.
        if world.is_solid(pos.0, pos.1, pos.2) {
            return false;
        }
        // 2 blocks tall -- block above must also be non-solid.
        if world.is_solid(pos.0, pos.1 + 1, pos.2) {
            return false;
        }
        // Light check.
        if !self.meets_light_requirements(pos, category, world) {
            return false;
        }
        // Distance check.
        if !Self::distance_ok(pos, players) {
            return false;
        }
        // Cap check.
        let player_count = players.len() as i32;
        let cap = self.effective_cap(category, player_count);
        let count = *self.spawned_counts.get(&category).unwrap_or(&0);
        if count >= cap {
            return false;
        }
        true
    }

    /// Look for a valid spawn position near a candidate XZ, searching
    /// downward from the heightmap.
    pub fn find_spawn_y(
        &self,
        x: i32,
        z: i32,
        category: MobCategory,
        players: &[(i32, i32, i32)],
        world: &dyn SpawnAccess,
    ) -> Option<(i32, i32, i32)> {
        let top = world.get_heightmap_y(x, z);
        // Scan downward from top to find a valid 2-high opening.
        for y in (0..=top).rev() {
            let pos = (x, y, z);
            if self.is_valid_spawn(pos, category, players, world) {
                return Some(pos);
            }
        }
        None
    }

    /// Get mob types that can spawn in a given biome for a given category.
    pub fn get_biome_mobs(&self, biome: BiomeId, category: MobCategory) -> Vec<MobType> {
        match category {
            MobCategory::Monster => {
                // Hostiles are mostly universal; some biome-specific extras.
                let mut mobs = vec![
                    MobType::Zombie,
                    MobType::Skeleton,
                    MobType::Creeper,
                    MobType::Spider,
                ];
                match biome {
                    BiomeId::Swamp | BiomeId::MangroveSwamp => {
                        mobs.push(MobType::Witch);
                    }
                    BiomeId::DarkForest | BiomeId::DeepDark => {
                        mobs.push(MobType::Enderman);
                    }
                    _ => {}
                }
                mobs
            }
            MobCategory::Creature => {
                let mut mobs = vec![
                    MobType::Cow,
                    MobType::Pig,
                    MobType::Sheep,
                    MobType::Chicken,
                ];
                match biome {
                    BiomeId::Forest
                    | BiomeId::BirchForest
                    | BiomeId::DarkForest
                    | BiomeId::OldGrowthBirchForest => {
                        mobs.push(MobType::Fox);
                        mobs.push(MobType::Bee);
                    }
                    BiomeId::Plains | BiomeId::Meadow => {
                        mobs.push(MobType::Horse);
                        mobs.push(MobType::Rabbit);
                    }
                    BiomeId::Taiga | BiomeId::OldGrowthPineForest | BiomeId::SnowyPlains => {
                        mobs.push(MobType::Rabbit);
                        mobs.push(MobType::Fox);
                    }
                    BiomeId::Jungle => {
                        mobs.push(MobType::Rabbit);
                    }
                    BiomeId::Desert | BiomeId::Savanna => {
                        mobs.push(MobType::Horse);
                    }
                    _ => {}
                }
                mobs
            }
            MobCategory::Ambient => vec![MobType::Bat],
            MobCategory::WaterCreature => {
                if biome.is_ocean() {
                    vec![MobType::Squid, MobType::Dolphin]
                } else {
                    vec![MobType::Squid]
                }
            }
            MobCategory::WaterAmbient => {
                if biome.is_cold() {
                    vec![MobType::Cod, MobType::Salmon]
                } else if biome.is_warm() {
                    vec![MobType::TropicalFish, MobType::Pufferfish]
                } else {
                    vec![MobType::Cod, MobType::Salmon]
                }
            }
            MobCategory::UndergroundCreature => vec![MobType::GlowSquid],
            MobCategory::Axolotl => vec![MobType::Axolotl],
        }
    }

    /// Despawn decision: instant > 128, 1/800 per tick 32-128 (after 30s), never < 32.
    ///
    /// `ticks_since_player_near` is the number of ticks since any player was
    /// within 32 blocks. Vanilla only begins the random despawn roll after
    /// 30 seconds (600 ticks) with no player nearby.
    pub fn should_despawn(
        &self,
        entity_pos: (i32, i32, i32),
        player_pos: (i32, i32, i32),
        ticks_since_player_near: u32,
    ) -> bool {
        let dx = entity_pos.0 - player_pos.0;
        let dy = entity_pos.1 - player_pos.1;
        let dz = entity_pos.2 - player_pos.2;
        let d_sq = dx * dx + dy * dy + dz * dz;

        if d_sq > 128 * 128 {
            // Instant despawn beyond 128 blocks.
            return true;
        }
        if d_sq > 32 * 32 {
            // 1/800 chance per tick, but only after 30 seconds (600 ticks)
            // with no player within 32 blocks.
            if ticks_since_player_near < 600 {
                return false;
            }
            // Use a deterministic hash-based "random" for reproducibility.
            let hash = (entity_pos.0 as u64)
                .wrapping_mul(6364136223846793005)
                .wrapping_add(ticks_since_player_near as u64)
                .wrapping_mul(1442695040888963407)
                ^ (entity_pos.2 as u64);
            return (hash % 800) == 0;
        }
        // Within 32 blocks: never despawn.
        false
    }

    /// Generate a pack of spawn candidates at a random XZ near a player.
    ///
    /// Picks a random offset in [32, 128] range, finds a valid Y, then
    /// tries to create a pack of 2-4 mobs of the same type.
    fn spawn_pack_for_player(
        &mut self,
        player_pos: (i32, i32, i32),
        players: &[(i32, i32, i32)],
        category: MobCategory,
        world: &dyn SpawnAccess,
    ) -> Vec<SpawnCandidate> {
        let player_count = players.len() as i32;
        let cap = self.effective_cap(category, player_count);
        let count = *self.spawned_counts.get(&category).unwrap_or(&0);
        if count >= cap {
            return vec![];
        }

        // Pick a random offset in the spawn ring [24, 128] around the player.
        let offset_x = pseudo_random_offset(player_pos.0, self.spawn_cycle, 0);
        let offset_z = pseudo_random_offset(player_pos.2, self.spawn_cycle, 1);
        let x = player_pos.0 + offset_x;
        let z = player_pos.2 + offset_z;

        // Determine biome and pick a mob type.
        let biome = world.get_biome(x, z);
        let mobs = self.get_biome_mobs(biome, category);
        if mobs.is_empty() {
            return vec![];
        }
        let mob_idx = pseudo_random_index(
            player_pos.0.wrapping_add(player_pos.2),
            self.spawn_cycle,
            mobs.len(),
        );
        let mob_type = mobs[mob_idx];

        // Find a valid Y.
        let Some(pos) = self.find_spawn_y(x, z, category, players, world) else {
            return vec![];
        };

        // Build pack.
        let (pack_min, pack_max) = mob_type.pack_range();
        let pack_size = pseudo_random_pack_size(
            player_pos.0,
            player_pos.2,
            self.spawn_cycle,
            pack_min,
            pack_max,
        );

        let mut candidates = Vec::with_capacity(pack_size as usize);
        for i in 0..pack_size {
            // Vanilla: pack members offset up to ±5 blocks from the leader.
            let cx = pos.0 + pseudo_random_offset(pos.0, self.spawn_cycle, i * 7 + 3).clamp(-5, 5);
            let cz = pos.2 + pseudo_random_offset(pos.2, self.spawn_cycle, i * 11 + 5).clamp(-5, 5);
            let cy = pos.1;
            let candidate = SpawnCandidate {
                x: cx,
                y: cy,
                z: cz,
                mob_type,
                category,
            };
            candidates.push(candidate);
        }

        // Update cap counter.
        *self.spawned_counts.entry(category).or_insert(0) += pack_size as i32;

        candidates
    }

    /// Main tick: called every server tick.
    ///
    /// - Hostile spawns (Monster, Ambient, UndergroundCreature) every tick.
    /// - Passive spawns (Creature, WaterCreature, WaterAmbient, Axolotl) every 400 ticks.
    ///
    /// Returns all `SpawnCandidate`s produced this tick.
    pub fn tick(
        &mut self,
        players: &[(i32, i32, i32)],
        world: &dyn SpawnAccess,
    ) -> Vec<SpawnCandidate> {
        if players.is_empty() {
            return vec![];
        }

        self.spawn_cycle = self.spawn_cycle.wrapping_add(1);
        let mut candidates = Vec::new();

        // Hostile category: every tick.
        for &cat in &[
            MobCategory::Monster,
            MobCategory::Ambient,
            MobCategory::UndergroundCreature,
        ] {
            for player_pos in players {
                let pack = self.spawn_pack_for_player(*player_pos, players, cat, world);
                candidates.extend(pack);
            }
        }

        // Passive categories: every 400 ticks.
        if self.spawn_cycle % 400 == 0 {
            for &cat in &[
                MobCategory::Creature,
                MobCategory::WaterCreature,
                MobCategory::WaterAmbient,
                MobCategory::Axolotl,
            ] {
                for player_pos in players {
                    let pack = self.spawn_pack_for_player(*player_pos, players, cat, world);
                    candidates.extend(pack);
                }
            }
        }

        candidates
    }
}

impl Default for SpawnEngine {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Deterministic pseudo-random helpers (no external RNG dependency)
// ---------------------------------------------------------------------------

/// Generate an XZ offset in [-128, +128] for spawn ring placement.
fn pseudo_random_offset(coord: i32, cycle: u32, salt: u32) -> i32 {
    let hash = (coord as u64)
        .wrapping_mul(6364136223846793005)
        .wrapping_add(cycle as u64)
        .wrapping_mul(1442695040888963407)
        .wrapping_add(salt as u64);
    // Map to [24, 128] -- must be at least 24 blocks away.
    let val = (hash % 209) as i32; // 0..208
    let signed = val - 104; // -104..+104
    let abs = signed.abs();
    if abs < 24 {
        signed + if signed >= 0 { 24 } else { -24 }
    } else if abs > 128 {
        signed * 128 / abs
    } else {
        signed
    }
}

/// Pick an index into a slice of length `len`.
fn pseudo_random_index(a: i32, cycle: u32, len: usize) -> usize {
    if len == 0 {
        return 0;
    }
    let hash = (a as u64)
        .wrapping_mul(6364136223846793005)
        .wrapping_add(cycle as u64)
        .wrapping_mul(1442695040888963407);
    (hash as usize) % len
}

/// Pick a pack size in [min, max].
fn pseudo_random_pack_size(
    x: i32,
    z: i32,
    cycle: u32,
    min: u32,
    max: u32,
) -> u32 {
    let hash = (x as u64)
        .wrapping_mul(6364136223846793005)
        .wrapping_add(z as u64)
        .wrapping_mul(1442695040888963407)
        .wrapping_add(cycle as u64);
    let range = max.saturating_sub(min);
    if range == 0 {
        return min;
    }
    min + (hash as u32) % (range + 1)
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// Minimal test world: a flat surface at y=64, all air above.
    struct FlatWorld {
        surface_y: i32,
        biome: BiomeId,
        /// Optional block light override for a specific position.
        block_light_override: Option<((i32, i32, i32), u8)>,
    }

    impl FlatWorld {
        fn new(biome: BiomeId) -> Self {
            Self {
                surface_y: 64,
                biome,
                block_light_override: None,
            }
        }

        fn with_block_light(mut self, pos: (i32, i32, i32), light: u8) -> Self {
            self.block_light_override = Some((pos, light));
            self
        }
    }

    impl SpawnAccess for FlatWorld {
        fn get_block(&self, _x: i32, y: i32, _z: i32) -> u16 {
            if y < self.surface_y {
                1 // stone
            } else {
                0 // air
            }
        }

        fn get_sky_light(&self, _x: i32, y: i32, _z: i32) -> u8 {
            if y >= self.surface_y { 15 } else { 0 }
        }

        fn get_block_light(&self, x: i32, y: i32, z: i32) -> u8 {
            if let Some((pos, light)) = &self.block_light_override {
                if (x, y, z) == *pos {
                    return *light;
                }
            }
            0
        }

        fn get_biome(&self, _x: i32, _z: i32) -> BiomeId {
            self.biome
        }

        fn is_solid(&self, _x: i32, y: i32, _z: i32) -> bool {
            y < self.surface_y
        }

        fn get_heightmap_y(&self, _x: i32, _z: i32) -> i32 {
            self.surface_y
        }
    }

    // -----------------------------------------------------------------------
    // Test 1: hostile spawn at low light
    // -----------------------------------------------------------------------
    #[test]
    fn hostile_spawn_low_light() {
        let engine = SpawnEngine::new();
        // y=63 is below surface (64), so sky_light=0, block_light=0 -> OK.
        assert!(engine.meets_light_requirements(
            (10, 63, 10),
            MobCategory::Monster,
            &FlatWorld::new(BiomeId::Plains),
        ));
        // y=64 is at/above surface, sky_light=15 -> NOT OK for Monster.
        let world_high = FlatWorld::new(BiomeId::Plains);
        assert!(!engine.meets_light_requirements(
            (10, 64, 10),
            MobCategory::Monster,
            &world_high,
        ));
    }

    // -----------------------------------------------------------------------
    // Test 1b: hostile spawn REJECTED when block_light > 0 (vanilla parity)
    // -----------------------------------------------------------------------
    #[test]
    fn hostile_spawn_rejected_with_block_light() {
        let engine = SpawnEngine::new();
        // Underground: sky_light=0 but block_light=3 -> should NOT spawn.
        let world = FlatWorld::new(BiomeId::Plains)
            .with_block_light((10, 63, 10), 3);
        assert!(!engine.meets_light_requirements(
            (10, 63, 10),
            MobCategory::Monster,
            &world,
        ));
        // Same with block_light=1 -> still rejected.
        let world2 = FlatWorld::new(BiomeId::Plains)
            .with_block_light((10, 63, 10), 1);
        assert!(!engine.meets_light_requirements(
            (10, 63, 10),
            MobCategory::Monster,
            &world2,
        ));
        // block_light=0 -> accepted.
        let world3 = FlatWorld::new(BiomeId::Plains);
        assert!(engine.meets_light_requirements(
            (10, 63, 10),
            MobCategory::Monster,
            &world3,
        ));
    }

    // -----------------------------------------------------------------------
    // Test 2: passive spawn at high light (sky_light only)
    // -----------------------------------------------------------------------
    #[test]
    fn passive_spawn_high_light() {
        let engine = SpawnEngine::new();
        let world = FlatWorld::new(BiomeId::Plains);
        // sky_light=15 > 7 -> OK for Creature.
        assert!(engine.meets_light_requirements(
            (10, 64, 10),
            MobCategory::Creature,
            &world,
        ));
        // Low light -> NOT OK.
        assert!(!engine.meets_light_requirements(
            (10, 30, 10),
            MobCategory::Creature,
            &FlatWorld::new(BiomeId::Plains),
        ));
    }

    // -----------------------------------------------------------------------
    // Test 2b: passive spawn IGNORES block light (vanilla parity)
    // -----------------------------------------------------------------------
    #[test]
    fn passive_spawn_ignores_block_light() {
        let engine = SpawnEngine::new();
        // sky_light=15, block_light=10 -> should STILL spawn (sky light only).
        let world = FlatWorld::new(BiomeId::Plains)
            .with_block_light((10, 64, 10), 10);
        assert!(engine.meets_light_requirements(
            (10, 64, 10),
            MobCategory::Creature,
            &world,
        ));
    }

    // -----------------------------------------------------------------------
    // Test 3: spawn distance (24-128 blocks)
    // -----------------------------------------------------------------------
    #[test]
    fn spawn_distance() {
        // At 10 blocks: too close.
        assert!(!SpawnEngine::distance_ok((10, 64, 0), &[(0, 64, 0)]));
        // At 24 blocks: minimum -- OK.
        assert!(SpawnEngine::distance_ok((24, 64, 0), &[(0, 64, 0)]));
        // At 100 blocks: OK.
        assert!(SpawnEngine::distance_ok((100, 64, 0), &[(0, 64, 0)]));
        // At 128 blocks: maximum -- OK.
        assert!(SpawnEngine::distance_ok((128, 64, 0), &[(0, 64, 0)]));
        // At 129 blocks: too far.
        assert!(!SpawnEngine::distance_ok((129, 64, 0), &[(0, 64, 0)]));
    }

    // -----------------------------------------------------------------------
    // Test 4: spawn caps
    // -----------------------------------------------------------------------
    #[test]
    fn spawn_caps() {
        let mut engine = SpawnEngine::new();
        let players = [(0, 64, 0)];

        // Monster cap is 70 per world.
        let cap = engine.effective_cap(MobCategory::Monster, 1);
        assert_eq!(cap, 70);

        // Creature cap is 10 per player.
        assert_eq!(engine.effective_cap(MobCategory::Creature, 1), 10);
        assert_eq!(engine.effective_cap(MobCategory::Creature, 4), 40);

        // WaterAmbient cap is 20 per player (vanilla parity).
        assert_eq!(engine.effective_cap(MobCategory::WaterAmbient, 1), 20);
        assert_eq!(engine.effective_cap(MobCategory::WaterAmbient, 3), 60);

        // UndergroundCreature cap is 5 per player (vanilla parity).
        assert_eq!(engine.effective_cap(MobCategory::UndergroundCreature, 1), 5);
        assert_eq!(engine.effective_cap(MobCategory::UndergroundCreature, 4), 20);

        // Simulate filling the cap.
        engine.spawned_counts.insert(MobCategory::Monster, 70);
        let world = FlatWorld::new(BiomeId::Plains);
        assert!(!engine.is_valid_spawn((50, 64, 50), MobCategory::Monster, &players, &world));
    }

    // -----------------------------------------------------------------------
    // Test 5: pack spawning (2-4 mobs)
    // -----------------------------------------------------------------------
    #[test]
    fn pack_spawning() {
        // Verify pack ranges for hostile mobs.
        let (min, max) = MobType::Zombie.pack_range();
        assert!(min >= 2 && max <= 4);

        // Zombie, Skeleton, Creeper all have pack range 2-4.
        assert_eq!(MobType::Skeleton.pack_range(), (2, 4));
        assert_eq!(MobType::Creeper.pack_range(), (2, 4));
        assert_eq!(MobType::Spider.pack_range(), (2, 4));

        // Enderman is solitary.
        assert_eq!(MobType::Enderman.pack_range(), (1, 1));

        // Passive mobs spawn in larger groups.
        assert_eq!(MobType::Cow.pack_range(), (4, 4));
    }

    // -----------------------------------------------------------------------
    // Test 6: despawn mechanics
    // -----------------------------------------------------------------------
    #[test]
    fn despawn_mechanics() {
        let engine = SpawnEngine::new();

        // Within 32 blocks: never despawn, regardless of ticks.
        assert!(!engine.should_despawn((30, 64, 0), (0, 64, 0), 0));
        assert!(!engine.should_despawn((0, 64, 30), (0, 64, 0), 10000));

        // Beyond 128 blocks: instant despawn.
        assert!(engine.should_despawn((130, 64, 0), (0, 64, 0), 0));
        assert!(engine.should_despawn((0, 64, 130), (0, 64, 0), 0));

        // 32-128 blocks, fewer than 600 ticks: NO despawn (30s timer).
        for i in 33..128 {
            assert!(
                !engine.should_despawn((i, 64, 0), (0, 64, 0), 100),
                "should not despawn at distance {i} with only 100 ticks"
            );
        }

        // 32-128 blocks, after 600+ ticks: 1/800 chance per tick.
        // Run 8000 ticks for several positions and verify some despawn.
        let mut any_despawn = false;
        for i in 33..128 {
            for tick in 600..6400 {
                if engine.should_despawn((i, 64, 0), (0, 64, 0), tick) {
                    any_despawn = true;
                    break;
                }
            }
        }
        assert!(
            any_despawn,
            "expected at least one despawn in 32-128 range after 600+ ticks"
        );
    }

    // -----------------------------------------------------------------------
    // Test 7: biome-specific mobs
    // -----------------------------------------------------------------------
    #[test]
    fn biome_specific_mobs() {
        let engine = SpawnEngine::new();

        // Plains: basic hostiles.
        let plains = engine.get_biome_mobs(BiomeId::Plains, MobCategory::Monster);
        assert!(plains.contains(&MobType::Zombie));
        assert!(plains.contains(&MobType::Creeper));

        // Swamp: has Witch.
        let swamp = engine.get_biome_mobs(BiomeId::Swamp, MobCategory::Monster);
        assert!(swamp.contains(&MobType::Witch));

        // Dark Forest: has Enderman.
        let dark = engine.get_biome_mobs(BiomeId::DarkForest, MobCategory::Monster);
        assert!(dark.contains(&MobType::Enderman));

        // Ocean water creatures.
        let ocean_wc = engine.get_biome_mobs(BiomeId::Ocean, MobCategory::WaterCreature);
        assert!(ocean_wc.contains(&MobType::Dolphin));
        assert!(ocean_wc.contains(&MobType::Squid));

        // Non-ocean: no dolphin.
        let river_wc = engine.get_biome_mobs(BiomeId::River, MobCategory::WaterCreature);
        assert!(!river_wc.contains(&MobType::Dolphin));

        // Cold biome water ambient: cod + salmon.
        let cold_wa = engine.get_biome_mobs(BiomeId::SnowyPlains, MobCategory::WaterAmbient);
        assert!(cold_wa.contains(&MobType::Cod));
        assert!(cold_wa.contains(&MobType::Salmon));

        // Warm biome water ambient: tropical fish + pufferfish.
        let warm_wa = engine.get_biome_mobs(BiomeId::Desert, MobCategory::WaterAmbient);
        assert!(warm_wa.contains(&MobType::TropicalFish));
        assert!(warm_wa.contains(&MobType::Pufferfish));

        // Ambient: always bat.
        let ambient = engine.get_biome_mobs(BiomeId::Plains, MobCategory::Ambient);
        assert_eq!(ambient, vec![MobType::Bat]);
    }

    // -----------------------------------------------------------------------
    // Test 8: mob type category mapping
    // -----------------------------------------------------------------------
    #[test]
    fn mob_type_categories() {
        assert_eq!(MobType::Zombie.category(), MobCategory::Monster);
        assert_eq!(MobType::Cow.category(), MobCategory::Creature);
        assert_eq!(MobType::Bat.category(), MobCategory::Ambient);
        assert_eq!(MobType::Squid.category(), MobCategory::WaterCreature);
        assert_eq!(MobType::Cod.category(), MobCategory::WaterAmbient);
        assert_eq!(MobType::GlowSquid.category(), MobCategory::UndergroundCreature);
        assert_eq!(MobType::Axolotl.category(), MobCategory::Axolotl);
    }

    // -----------------------------------------------------------------------
    // Test 9: full tick cycle produces candidates
    // -----------------------------------------------------------------------
    #[test]
    fn tick_produces_candidates() {
        let mut engine = SpawnEngine::new();
        let world = FlatWorld::new(BiomeId::Plains);
        let players = [(200, 64, 200)];

        // First tick should attempt hostile spawns.
        let candidates = engine.tick(&players, &world);
        // Depending on the deterministic offsets, we may or may not get a
        // valid position, but the engine should not panic.
        for c in &candidates {
            assert_eq!(c.category, MobCategory::Monster);
            assert!(c.mob_type.category() == MobCategory::Monster);
        }
    }

    // -----------------------------------------------------------------------
    // Test 10: passive tick only on cycle 400
    // -----------------------------------------------------------------------
    #[test]
    fn passive_tick_frequency() {
        let mut engine = SpawnEngine::new();
        let world = FlatWorld::new(BiomeId::Plains);
        let players = [(200, 64, 200)];

        // Run 399 ticks -- no Creature spawns.
        let mut saw_creature = false;
        for _ in 0..399 {
            let candidates = engine.tick(&players, &world);
            for c in &candidates {
                if c.category == MobCategory::Creature {
                    saw_creature = true;
                }
            }
        }
        assert!(!saw_creature, "Creature spawns should not happen before tick 400");

        // Tick 400 -- may produce Creature spawns.
        let candidates = engine.tick(&players, &world);
        let has_creature = candidates.iter().any(|c| c.category == MobCategory::Creature);
        // The deterministic offset might not land on a valid position, but
        // the engine attempted it. Just verify no panic and count updated.
        if has_creature {
            assert!(engine.spawned_counts[&MobCategory::Creature] > 0);
        }
    }

    // -----------------------------------------------------------------------
    // Test 11: is_valid_spawn checks solid floor
    // -----------------------------------------------------------------------
    #[test]
    fn valid_spawn_requires_solid_floor() {
        let engine = SpawnEngine::new();
        let players = [(0, 64, 0)];
        // Air at y=200 -- no solid floor below within reach.
        let world = FlatWorld::new(BiomeId::Plains);
        // This position is way above the surface, no solid block below.
        assert!(!engine.is_valid_spawn((50, 200, 50), MobCategory::Monster, &players, &world));
    }
}
