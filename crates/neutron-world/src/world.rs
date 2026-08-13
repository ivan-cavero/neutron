// Copyright (c) 2026 Neutron Contributors — MIT License
//
// World directory management.
//
// Manages the vanilla directory structure:
//   world/          - Overworld regions + level.dat + session.lock
//   world_nether/   - Nether regions
//   world_the_end/  - The End regions
//
// Each dimension has its own `region/` subdirectory containing `.mca` files.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::error::{WorldError, WorldResult};
use crate::level::LevelDat;
use crate::region::{self, Region};
use crate::session::SessionLock;

/// Minecraft dimensions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Dimension {
    Overworld,
    Nether,
    TheEnd,
}

impl Dimension {
    /// Directory name suffix for this dimension.
    ///
    /// - Overworld: `""` (the base world directory)
    /// - Nether: `"_nether"`
    /// - The End: `"_the_end"`
    pub fn dir_suffix(&self) -> &'static str {
        match self {
            Dimension::Overworld => "",
            Dimension::Nether => "_nether",
            Dimension::TheEnd => "_the_end",
        }
    }

    /// Region subdirectory name (all dimensions use `region/`).
    pub fn region_subdir(&self) -> &'static str {
        "region"
    }

    /// Directory name for this dimension relative to the world root.
    ///
    /// For example, for a world named "world":
    /// - Overworld -> "world"
    /// - Nether -> "world_nether"
    /// - The End -> "world_the_end"
    pub fn dir_name(&self, world_name: &str) -> String {
        format!("{}{}", world_name, self.dir_suffix())
    }
}

/// A Minecraft world, managing the on-disk directory structure.
///
/// Holds loaded region files and provides access to level.dat and session.lock.
pub struct World {
    /// Base path to the world root (e.g., `/path/to/world`).
    path: PathBuf,
    /// World name (typically the directory name).
    name: String,
    /// Level data.
    level: LevelDat,
    /// Session lock.
    _lock: SessionLock,
    /// Loaded regions per dimension, keyed by (rx, rz).
    regions: HashMap<(Dimension, i32, i32), Region>,
}

impl World {
    /// Open an existing world at the given path.
    ///
    /// Acquires the session.lock, reads level.dat, and validates the directory structure.
    pub fn open(path: &Path) -> WorldResult<Self> {
        if !path.exists() {
            return Err(WorldError::WorldNotFound(path.to_path_buf()));
        }

        // Validate directory structure.
        let region_dir = path.join("region");
        if !region_dir.exists() {
            return Err(WorldError::InvalidWorld {
                reason: format!("missing region/ directory in {}", path.display()),
            });
        }

        let level_dat_path = path.join("level.dat");
        if !level_dat_path.exists() {
            return Err(WorldError::InvalidWorld {
                reason: format!("missing level.dat in {}", path.display()),
            });
        }

        // Acquire session lock.
        let lock = SessionLock::acquire(path)?;

        // Read level.dat.
        let level = LevelDat::read(&level_dat_path)?;

        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "world".to_string());

        tracing::info!(
            name = %name,
            seed = level.seed,
            "opened world at {}",
            path.display()
        );

        Ok(Self {
            path: path.to_path_buf(),
            name,
            level,
            _lock: lock,
            regions: HashMap::new(),
        })
    }

    /// Create a new world with the vanilla directory structure.
    ///
    /// Creates the world directory, level.dat, session.lock, and empty region directories.
    pub fn create(path: &Path, seed: i64) -> WorldResult<Self> {
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "world".to_string());

        // Create directory structure.
        // Overworld region dir is inside the world path itself.
        // Nether and TheEnd dirs are siblings of the world directory.
        fs::create_dir_all(path.join("region"))?;
        let parent = path.parent().unwrap_or(path);
        for dim in &[Dimension::Nether, Dimension::TheEnd] {
            let dim_dir = parent.join(dim.dir_name(&name));
            fs::create_dir_all(dim_dir.join("region"))?;
        }

        // Create level.dat.
        let level = LevelDat::new(seed, &name);
        let level_dat_path = path.join("level.dat");
        level.write(&level_dat_path)?;

        // Create session.lock.
        let lock = SessionLock::acquire(path)?;

        tracing::info!(
            name = %name,
            seed,
            "created new world at {}",
            path.display()
        );

        Ok(Self {
            path: path.to_path_buf(),
            name,
            level,
            _lock: lock,
            regions: HashMap::new(),
        })
    }

    /// Get the world name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get the world root path.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Get a reference to the level data.
    pub fn level(&self) -> &LevelDat {
        &self.level
    }

    /// Get a mutable reference to the level data.
    pub fn level_mut(&mut self) -> &mut LevelDat {
        &mut self.level
    }

    /// Get the directory for a specific dimension.
    ///
    /// Overworld is the world path itself; Nether and TheEnd are siblings.
    pub fn dimension_dir(&self, dim: Dimension) -> PathBuf {
        match dim {
            Dimension::Overworld => self.path.clone(),
            _ => {
                let parent = self.path.parent().unwrap_or(&self.path);
                parent.join(dim.dir_name(&self.name))
            }
        }
    }

    /// Get the region directory for a specific dimension.
    pub fn region_dir(&self, dim: Dimension) -> PathBuf {
        self.dimension_dir(dim).join("region")
    }

    /// Get or load a region file at the given dimension and region coordinates.
    ///
    /// Regions are cached in memory. If not loaded, the `.mca` file is read from disk.
    pub fn get_region(&mut self, dim: Dimension, rx: i32, rz: i32) -> WorldResult<&mut Region> {
        if !self.regions.contains_key(&(dim, rx, rz)) {
            let path = region::region_path(&self.region_dir(dim), rx, rz);
            let region = if path.exists() {
                Region::open(&path)?.with_coords(rx, rz)
            } else {
                Region::new(rx, rz)
            };
            self.regions.insert((dim, rx, rz), region);
        }

        Ok(self.regions.get_mut(&(dim, rx, rz)).unwrap())
    }

    /// Save a specific region to disk.
    pub fn save_region(&self, dim: Dimension, rx: i32, rz: i32) -> WorldResult<()> {
        if let Some(region) = self.regions.get(&(dim, rx, rz)) {
            if region.is_dirty() {
                let path = region::region_path(&self.region_dir(dim), rx, rz);
                fs::create_dir_all(path.parent().unwrap_or(&self.path))?;
                region.save(&path)?;
            }
        }
        Ok(())
    }

    /// Save all dirty regions across all dimensions.
    pub fn save_all_regions(&self) -> WorldResult<()> {
        for (&(dim, rx, rz), region) in &self.regions {
            if region.is_dirty() {
                let path = region::region_path(&self.region_dir(dim), rx, rz);
                fs::create_dir_all(path.parent().unwrap_or(&self.path))?;
                region.save(&path)?;
            }
        }
        Ok(())
    }

    /// Save the level.dat file.
    pub fn save_level(&self) -> WorldResult<()> {
        let path = self.path.join("level.dat");
        self.level.write(&path)
    }

    /// Save everything: level.dat + all dirty regions.
    pub fn save(&self) -> WorldResult<()> {
        self.save_level()?;
        self.save_all_regions()?;
        tracing::info!(name = %self.name, "saved world");
        Ok(())
    }

    /// Unload all cached regions (freeing memory).
    pub fn unload_all(&mut self) {
        self.regions.clear();
    }

    /// Get a list of region files that exist on disk for a dimension.
    pub fn list_regions(&self, dim: Dimension) -> WorldResult<Vec<(i32, i32)>> {
        let region_dir = self.region_dir(dim);
        if !region_dir.exists() {
            return Ok(Vec::new());
        }

        let mut regions = Vec::new();
        for entry in fs::read_dir(&region_dir)? {
            let entry = entry?;
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if let Some((rx, rz)) = region::parse_region_filename(&name_str) {
                regions.push((rx, rz));
            }
        }

        Ok(regions)
    }
}

impl Drop for World {
    fn drop(&mut self) {
        // Best-effort save on drop.
        if let Err(e) = self.save() {
            tracing::error!(error = %e, "failed to save world on drop");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dimension_dir_names() {
        assert_eq!(Dimension::Overworld.dir_name("world"), "world");
        assert_eq!(Dimension::Nether.dir_name("world"), "world_nether");
        assert_eq!(Dimension::TheEnd.dir_name("world"), "world_the_end");
    }

    #[test]
    fn test_create_and_open_world() {
        let dir = tempfile::tempdir().unwrap();
        let world_path = dir.path().join("test_world");

        // Create.
        let mut world = World::create(&world_path, 42).unwrap();
        assert_eq!(world.name(), "test_world");
        assert_eq!(world.level().seed, 42);

        // Verify directory structure.
        // Vanilla layout: Overworld is the world dir itself; Nether/TheEnd are siblings.
        assert!(world_path.exists());
        assert!(world_path.join("level.dat").exists());
        assert!(world_path.join("region").exists());
        let parent = world_path.parent().unwrap();
        assert!(parent.join("test_world_nether").exists());
        assert!(parent.join("test_world_nether/region").exists());
        assert!(parent.join("test_world_the_end").exists());
        assert!(parent.join("test_world_the_end/region").exists());

        // Write a chunk.
        {
            let region = world.get_region(Dimension::Overworld, 0, 0).unwrap();
            region.write_chunk(5, 5, b"test chunk data").unwrap();
        }

        // Save.
        world.save().unwrap();

        // Re-open.
        drop(world);
        let mut world2 = World::open(&world_path).unwrap();
        assert_eq!(world2.level().seed, 42);

        // Verify the chunk persisted.
        let region = world2.get_region(Dimension::Overworld, 0, 0).unwrap();
        let chunk = region.get_chunk(5, 5).unwrap();
        assert_eq!(chunk.as_deref(), Some(b"test chunk data".as_slice()));
    }

    #[test]
    fn test_list_regions_empty() {
        let dir = tempfile::tempdir().unwrap();
        let world_path = dir.path().join("world");
        World::create(&world_path, 0).unwrap();

        let world = World::open(&world_path).unwrap();
        let regions = world.list_regions(Dimension::Overworld).unwrap();
        assert!(regions.is_empty());
    }

    #[test]
    fn test_list_regions_with_files() {
        let dir = tempfile::tempdir().unwrap();
        let world_path = dir.path().join("world");
        let mut world = World::create(&world_path, 0).unwrap();

        // Create some regions.
        {
            let r = world.get_region(Dimension::Overworld, 1, 2).unwrap();
            r.write_chunk(0, 0, b"data").unwrap();
        }
        {
            let r = world.get_region(Dimension::Overworld, -3, 0).unwrap();
            r.write_chunk(0, 0, b"data").unwrap();
        }
        world.save().unwrap();

        let regions = world.list_regions(Dimension::Overworld).unwrap();
        assert_eq!(regions.len(), 2);
    }

    #[test]
    fn test_open_nonexistent_world() {
        let result = World::open(Path::new("/nonexistent/path/world"));
        assert!(result.is_err());
    }
}
