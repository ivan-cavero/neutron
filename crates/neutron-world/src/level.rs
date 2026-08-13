// Copyright (c) 2026 Neutron Contributors — MIT License
//
// level.dat reading and writing.
//
// The file is gzip-compressed NBT. The root compound contains a single
// child called "Data" which holds all the world settings.

use std::fs;
use std::path::Path;

use ussr_nbt::owned::Compound;

use crate::error::{WorldError, WorldResult};
use crate::nbt;

/// Game modes as stored in level.dat.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameMode {
    Survival = 0,
    Creative = 1,
    Adventure = 2,
    Spectator = 3,
}

impl GameMode {
    /// Convert from the integer stored in NBT.
    pub fn from_i32(v: i32) -> Option<Self> {
        match v {
            0 => Some(Self::Survival),
            1 => Some(Self::Creative),
            2 => Some(Self::Adventure),
            3 => Some(Self::Spectator),
            _ => None,
        }
    }
}

/// Difficulty as stored in level.dat.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Difficulty {
    Peaceful = 0,
    Easy = 1,
    Normal = 2,
    Hard = 3,
}

impl Difficulty {
    pub fn from_i32(v: i32) -> Option<Self> {
        match v {
            0 => Some(Self::Peaceful),
            1 => Some(Self::Easy),
            2 => Some(Self::Normal),
            3 => Some(Self::Hard),
            _ => None,
        }
    }
}

/// A parsed representation of `level.dat`.
///
/// All fields mirror the vanilla Data compound tag. Fields use sensible defaults
/// that match a fresh vanilla world.
#[derive(Debug, Clone)]
pub struct LevelDat {
    /// DataVersion (vanilla int). Set to the version that last saved this file.
    pub data_version: i32,

    /// World version (format version). Vanilla uses 19133 for Anvil.
    pub version: i32,

    /// Level name (folder name without trailing slash).
    pub level_name: String,

    /// World seed (stored as Long under "Data.seed").
    pub seed: i64,

    /// Game mode (0=survival, 1=creative, 2=adventure, 3=spectator).
    pub game_type: GameMode,

    /// Difficulty (0-3).
    pub difficulty: Difficulty,

    /// Whether difficulty is locked.
    pub difficulty_locked: bool,

    /// Spawn X coordinate.
    pub spawn_x: i32,

    /// Spawn Y coordinate.
    pub spawn_y: i32,

    /// Spawn Z coordinate.
    pub spawn_z: i32,

    /// Day time (ticks, 0-24000).
    pub day_time: i64,

    /// World age (ticks since world creation).
    pub time: i64,

    /// Whether the world is hardcore mode.
    pub hardcore: bool,

    /// Whether cheats are enabled.
    pub allow_commands: bool,

    /// Game rules stored as a compound tag.
    pub game_rules: Compound,

    /// Whether the world has been initialized (border, etc.).
    pub initialized: bool,

    /// Whether the world allows flight.
    pub allow_flight: bool,

    /// Whether to show death screen.
    pub show_death_screen: bool,

    /// Whether rain is enabled.
    pub rain_enabled: bool,

    /// Rain time (ticks until rain changes).
    pub rain_time: i32,

    /// Whether thunder is enabled.
    pub thunder_enabled: bool,

    /// Thunder time (ticks until thunder changes).
    pub thunder_time: i32,

    /// Version info compound (Id, Name, Snapshot, Series).
    pub version_info: Option<Compound>,
}

impl Default for LevelDat {
    fn default() -> Self {
        Self {
            data_version: 3955, // 26.2 data version (approximate)
            version: 19133,     // Anvil format
            level_name: "world".to_string(),
            seed: 0,
            game_type: GameMode::Survival,
            difficulty: Difficulty::Normal,
            difficulty_locked: false,
            spawn_x: 0,
            spawn_y: 64,
            spawn_z: 0,
            day_time: 0,
            time: 0,
            hardcore: false,
            allow_commands: false,
            game_rules: nbt::new_compound(),
            initialized: true,
            allow_flight: false,
            show_death_screen: true,
            rain_enabled: false,
            rain_time: 0,
            thunder_enabled: false,
            thunder_time: 0,
            version_info: None,
        }
    }
}

impl LevelDat {
    /// Read a `level.dat` file from disk.
    pub fn read(path: &Path) -> WorldResult<Self> {
        let bytes = fs::read(path)?;
        Self::from_bytes(&bytes)
    }

    /// Parse a `level.dat` from a gzip-compressed byte buffer.
    pub fn from_bytes(bytes: &[u8]) -> WorldResult<Self> {
        let nbt = nbt::read_gzip_nbt(bytes)?;
        let data = &nbt.compound;

        // The Data compound is stored as a Tag::Compound inside the root compound.
        let data_key = ussr_nbt::mutf8::MString::from("Data");
        let data = data
            .tags
            .iter()
            .find(|(name, _)| name == &data_key)
            .and_then(|(_, tag)| match tag {
                ussr_nbt::owned::Tag::Compound(c) => Some(c),
                _ => None,
            })
            .ok_or_else(|| WorldError::MissingField {
                field: "Data".to_string(),
            })?;

        Self::from_compound(data)
    }

    /// Build a `LevelDat` from an already-parsed "Data" compound tag.
    fn from_compound(data: &Compound) -> WorldResult<Self> {
        let data_version = nbt::get_int_or(data, "DataVersion", 0);
        let version = nbt::get_int_or(data, "version", 19133);
        let level_name = nbt::get_string_or(data, "LevelName", "world");
        let seed = nbt::get_long_or(data, "seed", 0);
        let game_type =
            GameMode::from_i32(nbt::get_int_or(data, "GameType", 0)).unwrap_or(GameMode::Survival);
        let difficulty = Difficulty::from_i32(nbt::get_int_or(data, "Difficulty", 2))
            .unwrap_or(Difficulty::Normal);
        let difficulty_locked = nbt::get_byte_or(data, "DifficultyLocked", 0) != 0;
        let spawn_x = nbt::get_int_or(data, "SpawnX", 0);
        let spawn_y = nbt::get_int_or(data, "SpawnY", 64);
        let spawn_z = nbt::get_int_or(data, "SpawnZ", 0);
        let day_time = nbt::get_long_or(data, "DayTime", 0);
        let time = nbt::get_long_or(data, "Time", 0);
        let hardcore = nbt::get_byte_or(data, "hardcore", 0) != 0;
        let allow_commands = nbt::get_byte_or(data, "allowCommands", 0) != 0;
        let initialized = nbt::get_byte_or(data, "initialized", 1) != 0;
        let allow_flight = nbt::get_byte_or(data, "allowFlight", 0) != 0;
        let show_death_screen = nbt::get_byte_or(data, "showDeathMessages", 1) != 0;
        let rain_enabled = nbt::get_byte_or(data, "raining", 0) != 0;
        let rain_time = nbt::get_int_or(data, "rainTime", 0);
        let thunder_enabled = nbt::get_byte_or(data, "thundering", 0) != 0;
        let thunder_time = nbt::get_int_or(data, "thunderTime", 0);

        let game_rules = nbt::get_compound(data, "GameRules")
            .ok()
            .cloned()
            .unwrap_or_else(nbt::new_compound);

        let version_info = nbt::get_compound(data, "Version").ok().cloned();

        Ok(Self {
            data_version,
            version,
            level_name,
            seed,
            game_type,
            difficulty,
            difficulty_locked,
            spawn_x,
            spawn_y,
            spawn_z,
            day_time,
            time,
            hardcore,
            allow_commands,
            game_rules,
            initialized,
            allow_flight,
            show_death_screen,
            rain_enabled,
            rain_time,
            thunder_enabled,
            thunder_time,
            version_info,
        })
    }

    /// Serialize this `LevelDat` to an `Nbt` (root Compound).
    pub fn to_nbt(&self) -> ussr_nbt::owned::Nbt {
        let mut data = nbt::new_compound();

        nbt::compound_insert(&mut data, "DataVersion", nbt::tag_int(self.data_version));
        nbt::compound_insert(&mut data, "version", nbt::tag_int(self.version));
        nbt::compound_insert(&mut data, "LevelName", nbt::tag_string(&self.level_name));
        nbt::compound_insert(&mut data, "seed", nbt::tag_long(self.seed));
        nbt::compound_insert(&mut data, "GameType", nbt::tag_int(self.game_type as i32));
        nbt::compound_insert(
            &mut data,
            "Difficulty",
            nbt::tag_int(self.difficulty as i32),
        );
        nbt::compound_insert(
            &mut data,
            "DifficultyLocked",
            nbt::tag_byte(self.difficulty_locked as u8),
        );
        nbt::compound_insert(&mut data, "SpawnX", nbt::tag_int(self.spawn_x));
        nbt::compound_insert(&mut data, "SpawnY", nbt::tag_int(self.spawn_y));
        nbt::compound_insert(&mut data, "SpawnZ", nbt::tag_int(self.spawn_z));
        nbt::compound_insert(&mut data, "DayTime", nbt::tag_long(self.day_time));
        nbt::compound_insert(&mut data, "Time", nbt::tag_long(self.time));
        nbt::compound_insert(&mut data, "hardcore", nbt::tag_byte(self.hardcore as u8));
        nbt::compound_insert(
            &mut data,
            "allowCommands",
            nbt::tag_byte(self.allow_commands as u8),
        );
        nbt::compound_insert(
            &mut data,
            "initialized",
            nbt::tag_byte(self.initialized as u8),
        );
        nbt::compound_insert(
            &mut data,
            "allowFlight",
            nbt::tag_byte(self.allow_flight as u8),
        );
        nbt::compound_insert(
            &mut data,
            "showDeathMessages",
            nbt::tag_byte(self.show_death_screen as u8),
        );
        nbt::compound_insert(&mut data, "raining", nbt::tag_byte(self.rain_enabled as u8));
        nbt::compound_insert(&mut data, "rainTime", nbt::tag_int(self.rain_time));
        nbt::compound_insert(
            &mut data,
            "thundering",
            nbt::tag_byte(self.thunder_enabled as u8),
        );
        nbt::compound_insert(&mut data, "thunderTime", nbt::tag_int(self.thunder_time));
        nbt::compound_insert(
            &mut data,
            "GameRules",
            nbt::tag_compound(self.game_rules.clone()),
        );

        if let Some(ref vi) = self.version_info {
            nbt::compound_insert(&mut data, "Version", nbt::tag_compound(vi.clone()));
        }

        let mut root = nbt::new_compound();
        nbt::compound_insert(&mut root, "Data", nbt::tag_compound(data));

        nbt::root_nbt(root)
    }

    /// Write this `LevelDat` to a file (gzip-compressed NBT).
    pub fn write(&self, path: &Path) -> WorldResult<()> {
        let nbt = self.to_nbt();
        let compressed = nbt::write_gzip_nbt(&nbt)?;

        // Write atomically.
        let tmp_path = path.with_extension("dat.tmp");
        fs::write(&tmp_path, &compressed)?;
        fs::rename(&tmp_path, path)?;

        tracing::debug!("saved level.dat to {}", path.display());
        Ok(())
    }

    /// Create a fresh `LevelDat` for a new world with the given seed.
    pub fn new(seed: i64, level_name: &str) -> Self {
        Self {
            seed,
            level_name: level_name.to_string(),
            ..Default::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_level_dat() {
        let ld = LevelDat::default();
        assert_eq!(ld.version, 19133);
        assert_eq!(ld.level_name, "world");
        assert_eq!(ld.game_type, GameMode::Survival);
        assert_eq!(ld.difficulty, Difficulty::Normal);
        assert_eq!(ld.spawn_y, 64);
    }

    #[test]
    fn test_new_level_dat() {
        let ld = LevelDat::new(12345, "test_world");
        assert_eq!(ld.seed, 12345);
        assert_eq!(ld.level_name, "test_world");
    }

    #[test]
    fn test_roundtrip_bytes() {
        let ld = LevelDat::new(42, "roundtrip_test");
        let nbt = ld.to_nbt();
        let compressed = nbt::write_gzip_nbt(&nbt).unwrap();
        let restored = LevelDat::from_bytes(&compressed).unwrap();

        assert_eq!(restored.seed, 42);
        assert_eq!(restored.level_name, "roundtrip_test");
        assert_eq!(restored.version, 19133);
        assert_eq!(restored.game_type, GameMode::Survival);
    }

    #[test]
    fn test_roundtrip_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("level.dat");

        let ld = LevelDat {
            seed: 999,
            level_name: "file_test".to_string(),
            game_type: GameMode::Creative,
            difficulty: Difficulty::Hard,
            spawn_x: 100,
            spawn_y: 70,
            spawn_z: -200,
            day_time: 6000,
            time: 100000,
            ..Default::default()
        };
        ld.write(&path).unwrap();

        let restored = LevelDat::read(&path).unwrap();
        assert_eq!(restored.seed, 999);
        assert_eq!(restored.level_name, "file_test");
        assert_eq!(restored.game_type, GameMode::Creative);
        assert_eq!(restored.difficulty, Difficulty::Hard);
        assert_eq!(restored.spawn_x, 100);
        assert_eq!(restored.spawn_y, 70);
        assert_eq!(restored.spawn_z, -200);
        assert_eq!(restored.day_time, 6000);
        assert_eq!(restored.time, 100000);
    }

    #[test]
    fn test_game_mode_conversions() {
        assert_eq!(GameMode::from_i32(0), Some(GameMode::Survival));
        assert_eq!(GameMode::from_i32(1), Some(GameMode::Creative));
        assert_eq!(GameMode::from_i32(2), Some(GameMode::Adventure));
        assert_eq!(GameMode::from_i32(3), Some(GameMode::Spectator));
        assert_eq!(GameMode::from_i32(99), None);
    }

    #[test]
    fn test_difficulty_conversions() {
        assert_eq!(Difficulty::from_i32(0), Some(Difficulty::Peaceful));
        assert_eq!(Difficulty::from_i32(1), Some(Difficulty::Easy));
        assert_eq!(Difficulty::from_i32(2), Some(Difficulty::Normal));
        assert_eq!(Difficulty::from_i32(3), Some(Difficulty::Hard));
        assert_eq!(Difficulty::from_i32(99), None);
    }
}
