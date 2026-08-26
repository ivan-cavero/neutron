//! Strict vanilla reference decoding: Anvil region -> block/biome grids.
//!
//! One canonical decoder for the whole repo. Every palette entry must carry a
//! `Name`; every packed index must be in range; truncated data is an error,
//! never silently skipped. Unknown block names are NOT decode errors (the ref
//! may be newer than our mapping) — they surface later as `Unmapped` gaps.

use neutron_world::nbt::ussr_nbt::owned::{List, Tag};
use neutron_world::nbt::{compound_get, read_nbt};
use neutron_world::Region;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub const WORLD_BOTTOM: i32 = -64;
pub const WORLD_TOP: i32 = 320;
pub const CHUNK_CELLS: usize = ((WORLD_TOP - WORLD_BOTTOM) * 256) as usize;
pub const QUARTS_Y: i32 = (WORLD_TOP - WORLD_BOTTOM) / 4;

/// Per-dimension generation geometry, from the vanilla 26.2 noise_settings
/// (NOT dimension_type heights — nether/end store 128-tall noise inside
/// 256-tall dimension types):
///   overworld: NoiseSettings.OVERWORLD (-64, 384)
///   nether:    NoiseSettings.NETHER (0, 128), coordinate_scale 8
///   end:       NoiseSettings.END (0, 128)
/// A future Minecraft dimension = one new entry here + a ref generated in it;
/// `discover_dimension_dirs` fails loudly if refs contain an unknown dim.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DimSpec {
    pub min_y: i32,
    pub height: i32,
}

impl DimSpec {
    pub const OVERWORLD: DimSpec = DimSpec { min_y: -64, height: 384 };
    pub const NETHER: DimSpec = DimSpec { min_y: 0, height: 128 };
    pub const END: DimSpec = DimSpec { min_y: 0, height: 128 };

    pub const KNOWN_NAMES: &'static [&'static str] = &["overworld", "the_nether", "the_end"];

    pub fn parse(name: &str) -> Option<DimSpec> {
        match name {
            "overworld" => Some(Self::OVERWORLD),
            "the_nether" | "nether" => Some(Self::NETHER),
            "the_end" | "end" => Some(Self::END),
            _ => None,
        }
    }

    #[inline]
    pub fn bottom(&self) -> i32 {
        self.min_y
    }

    #[inline]
    pub fn top(&self) -> i32 {
        self.min_y + self.height
    }

    #[inline]
    pub fn cells(&self) -> usize {
        (self.height * 256) as usize
    }

    #[inline]
    pub fn quarts_y(&self) -> i32 {
        self.height / 4
    }
}

/// Legacy aliases (overworld geometry) kept for back-compat with callers.
pub const WORLD_DIM: DimSpec = DimSpec::OVERWORLD;

/// If `region_dir` lives under a `dimensions/<ns>/<dim>/` tree, return every
/// dimension folder name found there. Unknown names = Minecraft shipped a new
/// dimension and our refs cover it while our tooling does not.
pub fn discover_dimension_dirs(region_dir: &Path) -> Option<Vec<String>> {
    let ns_dir = region_dir.parent()?.parent()?; // .../dimensions/minecraft
    let dims_root = ns_dir.parent()?; // .../dimensions
    if dims_root.file_name()?.to_str()? != "dimensions" {
        return None;
    }
    let mut out = Vec::new();
    for ns in std::fs::read_dir(dims_root).ok()? {
        let ns = ns.ok()?;
        if !ns.path().is_dir() {
            continue;
        }
        for d in std::fs::read_dir(ns.path()).ok()? {
            let d = d.ok()?;
            if d.path().is_dir() {
                if let Some(name) = d.file_name().into_string().ok() {
                    out.push(name);
                }
            }
        }
    }
    out.sort();
    out.dedup();
    Some(out)
}

#[derive(Debug)]
pub enum ParityError {
    Io(std::io::Error),
    Nbt(String),
    Decode(String),
}

impl std::fmt::Display for ParityError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParityError::Io(e) => write!(f, "io error: {e}"),
            ParityError::Nbt(s) => write!(f, "nbt error: {s}"),
            ParityError::Decode(s) => write!(f, "decode error: {s}"),
        }
    }
}

impl From<std::io::Error> for ParityError {
    fn from(e: std::io::Error) -> Self {
        ParityError::Io(e)
    }
}

/// Flat per-chunk block grid of `minecraft:*` names.
/// Index: `(y - dim.min_y) * 256 + z * 16 + x`.
#[derive(Clone)]
pub struct BlockGrid {
    pub names: Vec<String>,
    pub dim: DimSpec,
}

impl BlockGrid {
    pub fn get(&self, x: u32, y: i32, z: u32) -> &str {
        let i = ((y - self.dim.min_y) * 256 + (z as i32) * 16 + x as i32) as usize;
        self.names.get(i).map(|s| s.as_str()).unwrap_or("minecraft:air")
    }
}

/// Flat per-chunk biome quart grid. Index: `(qy * 4 + qz) * 4 + qx`
/// with qy relative to dim.min_y/4.
#[derive(Clone)]
pub struct BiomeGrid {
    pub names: Vec<String>,
    pub dim: DimSpec,
}

impl BiomeGrid {
    pub fn get(&self, qx: u32, qy: i32, qz: u32) -> &str {
        let i = (((qy as usize) * 4 + qz as usize) * 4 + qx as usize) as usize;
        self.names.get(i).map(|s| s.as_str()).unwrap_or("?")
    }
}

#[derive(Clone)]
pub struct RefChunk {
    pub status: String,
    pub blocks: BlockGrid,
    pub biomes: Option<BiomeGrid>,
    /// Sorted distinct structure-start ids in this chunk
    /// (`structures.starts` keys with a non-invalid id). Feeds the
    /// structure-inventory tripwire: a NEW vanilla structure type shows up
    /// here even when its blocks happen to match.
    pub structure_starts: Vec<String>,
    /// Block-entity type counts in this chunk (chests, spawners, …).
    /// Structures fill chests during postProcess with seed-derived loot —
    /// deterministic per world seed but stored as BLOCK ENTITIES (tile
    /// entities), invisible to a blocks-only diff. This inventory is how a
    /// new entity kind gets noticed at all.
    pub block_entities: std::collections::BTreeMap<String, u64>,
}

/// Overworld structure types known to Neutron's worldgen (26.2). Extend this
/// list as ports land; anything else found in refs triggers the tripwire.
pub const KNOWN_STRUCTURE_TYPES: &[&str] = &["minecraft:mineshaft"];

pub struct Discovery {
    /// Full-status chunks, sorted lexicographically by (cx, cz).
    pub full: Vec<(i32, i32)>,
    /// Non-full chunks present on disk (protos): reported, never compared.
    pub protos: Vec<((i32, i32), String)>,
}

/// Cached set of open region files for one reference directory.
pub struct RegionSet {
    dir: PathBuf,
    regions: HashMap<(i32, i32), Region>,
}

impl RegionSet {
    pub fn open(dir: impl Into<PathBuf>) -> Result<Self, ParityError> {
        let dir = dir.into();
        if !dir.is_dir() {
            return Err(ParityError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("region dir not found: {}", dir.display()),
            )));
        }
        Ok(RegionSet { dir, regions: HashMap::new() })
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    fn region(&mut self, rx: i32, rz: i32) -> Result<&mut Region, ParityError> {
        if !self.regions.contains_key(&(rx, rz)) {
            let path = self.dir.join(format!("r.{rx}.{rz}.mca"));
            let region = Region::open(&path)
                .map_err(|e| ParityError::Decode(format!("open {path:?}: {e}")))?
                .with_coords(rx, rz);
            self.regions.insert((rx, rz), region);
        }
        Ok(self.regions.get_mut(&(rx, rz)).expect("just inserted"))
    }

    /// All full-status chunks across every `r.*.mca` in the dir, sorted.
    /// Proto/stub chunks are listed with their Status so callers can report
    /// coverage honestly instead of skipping silently.
    pub fn discover(&mut self) -> Result<Discovery, ParityError> {
        let mut rcoords: Vec<(i32, i32)> = std::fs::read_dir(&self.dir)?
            .filter_map(|e| {
                let name = e.ok()?.file_name().into_string().ok()?;
                let rest = name.strip_prefix("r.")?.strip_suffix(".mca")?;
                let mut it = rest.split('.');
                let rx = it.next()?.parse().ok()?;
                let rz = it.next()?.parse().ok()?;
                Some((rx, rz))
            })
            .collect();
        rcoords.sort();
        let mut full = Vec::new();
        let mut protos = Vec::new();
        for &(rx, rz) in &rcoords {
            for lz in 0..32u32 {
                for lx in 0..32u32 {
                    let data = match self.region(rx, rz)?.get_chunk(lx as i32, lz as i32) {
                        Ok(Some(d)) => d,
                        _ => continue,
                    };
                    let nbt = match read_nbt(&data) {
                        Ok(n) => n,
                        Err(_) => continue,
                    };
                    let status = match compound_get(&nbt.compound, "Status") {
                        Some(Tag::String(s)) => s.to_string(),
                        _ => "<none>".to_string(),
                    };
                    let coords = (rx * 32 + lx as i32, rz * 32 + lz as i32);
                    if status.ends_with("full") {
                        full.push(coords);
                    } else {
                        protos.push((coords, status));
                    }                }
            }
        }
        full.sort();
        Ok(Discovery { full, protos })
    }

    pub fn load_chunk(&mut self, cx: i32, cz: i32, dim: DimSpec) -> Result<Option<RefChunk>, ParityError> {
        let (rx, rz) = (cx >> 5, cz >> 5);
        let data = match self
            .region(rx, rz)?
            .get_chunk(cx & 31, cz & 31)
            .map_err(|e| ParityError::Decode(format!("chunk {cx},{cz}: {e}")))? {
            Some(d) => d,
            None => return Ok(None),
        };
        let nbt = read_nbt(&data).map_err(|e| ParityError::Nbt(format!("{e}")))?;
        let status = match compound_get(&nbt.compound, "Status") {
            Some(Tag::String(s)) => s.to_string(),
            _ => return Ok(None),
        };
        if !status.ends_with("full") {
            return Ok(None); // proto chunk: not a measurement target
        }
        let sections = match compound_get(&nbt.compound, "sections") {
            Some(Tag::List(List::Compound(l))) => l,
            _ => return Err(ParityError::Decode(format!("chunk {cx},{cz}: no sections list"))),
        };
        let blocks = decode_block_sections(sections, cx, cz, dim)?;
        let biomes = decode_biome_sections(sections, cx, cz, dim)?;
        let structure_starts = decode_structure_starts(&nbt.compound);
        let block_entities = decode_block_entities(&nbt.compound);
        Ok(Some(RefChunk { status, blocks, biomes, structure_starts, block_entities }))
    }
}

fn decode_block_entities(
    compound: &neutron_world::nbt::ussr_nbt::owned::Compound,
) -> std::collections::BTreeMap<String, u64> {
    let mut out = std::collections::BTreeMap::new();
    // Pre-1.18 name was "TileEntities"; modern chunks use "block_entities".
    let Some(Tag::List(List::Compound(list))) =
        compound_get(compound, "block_entities").or_else(|| compound_get(compound, "TileEntities"))
    else {
        return out;
    };
    for be in list {
        if let Some(Tag::String(id)) = compound_get(be, "id") {
            *out.entry(id.to_string()).or_insert(0) += 1;
        }
    }
    out
}

fn decode_structure_starts(compound: &neutron_world::nbt::ussr_nbt::owned::Compound) -> Vec<String> {
    let Some(Tag::Compound(structures)) = compound_get(compound, "structures") else {
        return Vec::new();
    };
    let Some(Tag::Compound(starts)) = compound_get(structures, "starts") else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for (name, start_tag) in &starts.tags {
        let Tag::Compound(start) = start_tag else {
            continue;
        };
        // Placeholder entries carry id "minecraft:invalid"; real starts name
        // their structure type.
        let id = match compound_get(start, "id") {
            Some(Tag::String(s)) => s.to_string(),
            _ => continue,
        };
        if id == "minecraft:invalid" || id.is_empty() {
            continue;
        }
        out.push(name.to_string());
    }
    out.sort();
    out.dedup();
    out
}

fn section_y(sec: &neutron_world::nbt::ussr_nbt::owned::Compound, cx: i32, cz: i32) -> Result<i32, ParityError> {
    match compound_get(sec, "Y") {
        Some(Tag::Byte(y)) => Ok(*y as i8 as i32),
        Some(Tag::Int(y)) => Ok(*y),
        _ => Err(ParityError::Decode(format!("chunk {cx},{cz}: section missing Y"))),
    }
}

fn ceil_bits(n: usize) -> u32 {
    if n <= 1 { 0 } else { ((n - 1).ilog2()) + 1 }
}

fn cell_of(i: u32, y_sec: i32) -> (u32, i32, u32) {
    ((i & 15), y_sec * 16 + (i >> 8) as i32, ((i >> 4) & 15))
}

/// Vanilla non-straddling bit packing for block states: 4-bit minimum.
fn unpack_into(
    longs: &[i64],
    bits: u32,
    count: u32,
    cx: i32,
    cz: i32,
    what: &str,
) -> Result<Vec<u32>, ParityError> {
    let epl = (64 / bits) as usize;
    let need = count.div_ceil(epl as u32) as usize;
    if longs.len() < need {
        return Err(ParityError::Decode(format!(
            "chunk {cx},{cz} {what}: data too short ({} longs, need {need})",
            longs.len()
        )));
    }
    let mask = (1u64 << bits) - 1;
    let mut out = vec![0u32; count as usize];
    for (i, slot) in out.iter_mut().enumerate() {
        let li = i / epl;
        let bo = ((i % epl) * bits as usize) as u32;
        *slot = (((longs[li] as u64) >> bo) & mask) as u32;
    }
    Ok(out)
}

fn decode_block_sections(
    sections: &[neutron_world::nbt::ussr_nbt::owned::Compound],
    cx: i32,
    cz: i32,
    dim: DimSpec,
) -> Result<BlockGrid, ParityError> {
    let mut names = vec!["minecraft:air".to_string(); dim.cells()];
    let grid_index = |x: u32, y: i32, z: u32| -> usize {
        ((y - dim.min_y) * 256 + z as i32 * 16 + x as i32) as usize
    };
    for sec in sections {
        let y_sec = section_y(sec, cx, cz)?;
        let Some(Tag::Compound(bs)) = compound_get(sec, "block_states") else {
            continue;
        };
        let Some(Tag::List(List::Compound(palette))) = compound_get(bs, "palette") else {
            continue;
        };
        let mut pal: Vec<String> = Vec::with_capacity(palette.len());
        for (pi, pc) in palette.iter().enumerate() {
            match compound_get(pc, "Name") {
                Some(Tag::String(s)) => pal.push(s.to_string()),
                _ => {
                    return Err(ParityError::Decode(format!(
                        "chunk {cx},{cz} section Y={y_sec}: palette[{pi}] has no Name"
                    )))
                }
            }
        }
        if pal.is_empty() {
            continue;
        }
        if pal.len() == 1 {
            for i in 0..4096u32 {
                let (x, y, z) = cell_of(i, y_sec);
                names[grid_index(x, y, z)] = pal[0].clone();
            }
            continue;
        }
        let bits = ceil_bits(pal.len()).max(4);
        let Some(Tag::LongArray(data)) = compound_get(bs, "data") else {
            return Err(ParityError::Decode(format!(
                "chunk {cx},{cz} section Y={y_sec}: {}-entry palette without data",
                pal.len()
            )));
        };
        let idxs = unpack_into(&data.to_vec(), bits, 4096, cx, cz, "block_states")?;
        for (i, idxp) in idxs.into_iter().enumerate() {
            let name = pal.get(idxp as usize).cloned().ok_or_else(|| {
                ParityError::Decode(format!(
                    "chunk {cx},{cz} section Y={y_sec}: palette index {idxp} out of range"
                ))
            })?;
            let (x, y, z) = cell_of(i as u32, y_sec);
            names[grid_index(x, y, z)] = name;
        }
    }
    Ok(BlockGrid { names, dim })
}

fn decode_biome_sections(
    sections: &[neutron_world::nbt::ussr_nbt::owned::Compound],
    cx: i32,
    cz: i32,
    dim: DimSpec,
) -> Result<Option<BiomeGrid>, ParityError> {
    let total = (dim.quarts_y() * 16) as usize;
    let mut names: Vec<String> = vec![String::new(); total];
    let mut seen = false;
    for sec in sections {
        let y_sec = section_y(sec, cx, cz)?;
        let Some(Tag::Compound(bs)) = compound_get(sec, "biomes") else {
            continue;
        };
        // Post-processed refs store biome palettes either as List[String]
        // or List[Compound{Name}] — accept both, unlike block palettes
        // which are always Compound{Name}.
        let pal: Vec<String> = match compound_get(bs, "palette") {
            Some(Tag::List(List::String(v))) => v.iter().map(|s| s.to_string()).collect(),
            Some(Tag::List(List::Compound(p))) => p
                .iter()
                .filter_map(|pc| match compound_get(pc, "Name") {
                    Some(Tag::String(s)) => Some(s.to_string()),
                    _ => None,
                })
                .collect(),
            _ => continue,
        };
        if pal.is_empty() {
            continue;
        }
        seen = true;
        let base_qy = y_sec * 4;
        let fill = |names: &mut Vec<String>, v: usize| {
            for sy in 0i32..4 {
                for bz in 0u32..4 {
                    for bx in 0u32..4 {
                        let qi = (((base_qy + sy - dim.min_y / 4) * 4 + bz as i32) * 4
                            + bx as i32) as usize;
                        if let Some(slot) = names.get_mut(qi) {
                            *slot = pal[v].clone();
                        }
                    }
                }
            }
        };
        match compound_get(bs, "data") {
            None => {
                fill(&mut names, 0);
            }
            Some(Tag::LongArray(data)) => {
                // This ref packs biome quarts with NO 4-bit minimum
                // (2 biomes -> 1 bit -> exactly 1 long per half-section).
                let bits = ceil_bits(pal.len());
                if bits == 0 {
                    fill(&mut names, 0);
                    continue;
                }
                let epl = (64 / bits) as usize;
                let longs = data.to_vec();
                for sy in 0i32..4 {
                    for bz in 0i32..4 {
                        for bx in 0i32..4 {
                            let idx = (sy * 16 + bz * 4 + bx) as usize;
                            let li = idx / epl;
                            let sh = ((idx % epl) * bits as usize) as u32;
                            if li >= longs.len() {
                                return Err(ParityError::Decode(format!(
                                    "chunk {cx},{cz} section Y={y_sec}: biome data too short"
                                )));
                            }
                            let v = (((longs[li] as u64) >> sh) & ((1u64 << bits) - 1)) as usize;
                            let name = pal.get(v).cloned().ok_or_else(|| {
                                ParityError::Decode(format!(
                                    "chunk {cx},{cz} section Y={y_sec}: biome index {v} out of range"
                                ))
                            })?;
                            let qi = (((base_qy + sy - dim.min_y / 4) * 4 + bz) * 4 + bx)
                                as usize;
                            if let Some(slot) = names.get_mut(qi) {
                                *slot = name;
                            }
                        }
                    }
                }
            }
            _ => continue,
        }
    }
    Ok(if seen { Some(BiomeGrid { names, dim }) } else { None })
}
