//! Disk cache for generated Neutron chunks.
//!
//! Purpose: re-scans (parity runs that do not touch generator code) skip
//! the ~11.5 s/chunk generation entirely. Invalidation is automatic and
//! content-based: the cache key embeds an xxh3 fingerprint over EVERYTHING
//! under crates/neutron-worldgen/src (Rust sources AND runtime-read datapack
//! JSONs), so any generator or data change silently misses and regenerates.
//!
//! Safety: if any NEUTRON_* env var known to change generation OUTPUT is set,
//! the cache refuses to engage (tracing-only flags are fine).

use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

const MAGIC: &[u8; 8] = b"NPARC01!";

/// Env vars that ALTER generated block output (see docs/PARITY.md). When one
/// is set, cached chunks would lie about parity.
pub const OUTPUT_AFFECTING_ENV: &[&str] = &[
    "NEUTRON_TMP_MASK",
    "NEUTRON_DECO_NO_MASK",
    "NEUTRON_DECO_SKIP_TREE_DRAWS",
    "NEUTRON_SCULK_ORIGIN_ORDER",
    "NEUTRON_DECO_CUSTOM_ORDER",
    "NEUTRON_SCULK_ONE_ORIGIN",
];

pub fn output_affecting_env_set() -> Option<&'static str> {
    OUTPUT_AFFECTING_ENV.iter().find(|k| std::env::var_os(k).is_some()).copied()
}

/// xxh3 over every file under `root` (path + contents), sorted by path.
/// Deterministic across runs and machines.
pub fn fingerprint_tree(root: &Path) -> u64 {
    let mut hasher = xxhash_rust::xxh3::Xxh3::new();
    let mut paths: Vec<PathBuf> = Vec::new();
    collect(root, &mut paths);
    paths.sort();
    for p in &paths {
        if let Ok(rel) = p.strip_prefix(root) {
            hasher.update(rel.to_string_lossy().as_bytes());
        }
        if let Ok(bytes) = std::fs::read(p) {
            hasher.update(&bytes);
        }
    }
    hasher.digest()
}

fn collect(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() {
            collect(&p, out);
        } else {
            out.push(p);
        }
    }
}

pub struct ChunkCache {
    dir: PathBuf,
    fingerprint: u64,
}

/// A decoded cache entry mirroring the parts of GeneratedChunk we need.
pub struct CachedChunk {
    pub blocks: Vec<u16>,
    pub biomes: Vec<u8>,
    pub heightmap: Vec<i16>,
}

impl ChunkCache {
    pub fn fingerprint(&self) -> u64 {
        self.fingerprint
    }

    /// None when caching is unavailable (env override active).
    pub fn open(dir: impl Into<PathBuf>, worldgen_src_root: &Path) -> Option<Self> {
        if let Some(var) = output_affecting_env_set() {
            eprintln!("cache: disabled ({var} alters generation output)");
            return None;
        }
        Some(ChunkCache { dir: dir.into(), fingerprint: fingerprint_tree(worldgen_src_root) })
    }

    fn path_for(&self, seed: i64, cx: i32, cz: i32) -> PathBuf {
        self.dir
            .join(format!("{:016x}", self.fingerprint))
            .join(seed.to_string())
            .join(format!("c{cx}_{cz}.bin"))
    }

    pub fn load(&self, seed: i64, cx: i32, cz: i32, cells: usize, biome_cells: usize) -> Option<CachedChunk> {
        let mut f = std::fs::File::open(self.path_for(seed, cx, cz)).ok()?;
        let mut buf = Vec::new();
        f.read_to_end(&mut buf).ok()?;
        if buf.len() < 8 + 8 + 4 + 4 + 4 + 8 || &buf[0..8] != MAGIC {
            return None;
        }
        let body = &buf[..buf.len() - 8];
        let stored = u64::from_le_bytes(buf[buf.len() - 8..].try_into().ok()?);
        if xxhash_rust::xxh3::xxh3_64(body) != stored {
            return None; // torn write / corruption: treat as miss
        }
        let mut off = 8usize;
        let rd_i64 = |b: &[u8], o: &mut usize| {
            let v = i64::from_le_bytes(b[*o..*o + 8].try_into().unwrap());
            *o += 8;
            v
        };
        let rd_i32 = |b: &[u8], o: &mut usize| {
            let v = i32::from_le_bytes(b[*o..*o + 4].try_into().unwrap());
            *o += 4;
            v
        };
        if rd_i64(body, &mut off) != seed
            || rd_i32(body, &mut off) != cx
            || rd_i32(body, &mut off) != cz
        {
            return None;
        }
        let n_blocks = rd_i32(body, &mut off) as usize;
        let n_biomes = rd_i32(body, &mut off) as usize;
        if n_blocks != cells || n_biomes != biome_cells {
            return None; // different world geometry (dimension change)
        }
        let need = n_blocks * 2 + n_biomes + 256 * 2;
        if body.len() < off + need {
            return None;
        }
        let blocks = body[off..off + n_blocks * 2]
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect();
        off += n_blocks * 2;
        let biomes = body[off..off + n_biomes].to_vec();
        off += n_biomes;
        let heightmap = body[off..off + 512]
            .chunks_exact(2)
            .map(|c| i16::from_le_bytes([c[0], c[1]]))
            .collect();
        Some(CachedChunk { blocks, biomes, heightmap })
    }

    pub fn store(
        &self,
        seed: i64,
        cx: i32,
        cz: i32,
        blocks: &[u16],
        biomes: &[u8],
        heightmap: &[i16],
    ) -> std::io::Result<()> {
        let p = self.path_for(seed, cx, cz);
        let dir = p.parent().expect("cache path has parent");
        std::fs::create_dir_all(dir)?;
        let mut buf = Vec::with_capacity(8 + 28 + blocks.len() * 2);
        buf.extend_from_slice(MAGIC);
        buf.extend_from_slice(&seed.to_le_bytes());
        buf.extend_from_slice(&cx.to_le_bytes());
        buf.extend_from_slice(&cz.to_le_bytes());
        buf.extend_from_slice(&(blocks.len() as i32).to_le_bytes());
        buf.extend_from_slice(&(biomes.len() as i32).to_le_bytes());
        for b in blocks {
            buf.extend_from_slice(&b.to_le_bytes());
        }
        buf.extend_from_slice(biomes);
        for h in heightmap {
            buf.extend_from_slice(&h.to_le_bytes());
        }
        let sum = xxhash_rust::xxh3::xxh3_64(&buf);
        buf.extend_from_slice(&sum.to_le_bytes());
        // Atomic-ish replace so an interrupted run never poisons the cache.
        let tmp = p.with_extension("tmp");
        std::fs::File::create(&tmp)?.write_all(&buf)?;
        std::fs::rename(&tmp, &p)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fingerprint_changes_when_source_changes() {
        let tmp = std::env::temp_dir().join(format!("npc-fp-{}", std::process::id()));
        let sub = tmp.join("src");
        std::fs::create_dir_all(&sub).unwrap();
        std::fs::write(sub.join("a.rs"), "fn a() {}").unwrap();
        let f1 = fingerprint_tree(&tmp);
        std::fs::write(sub.join("a.rs"), "fn a() { /* changed */ }").unwrap();
        let f2 = fingerprint_tree(&tmp);
        std::fs::write(sub.join("b.json"), "{}").unwrap();
        let f3 = fingerprint_tree(&tmp);
        assert_ne!(f1, f2, "content edit must change fingerprint");
        assert_ne!(f2, f3, "added data file must change fingerprint");
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn chunk_roundtrip_and_corruption_rejected() {
        let tmp = std::env::temp_dir().join(format!("npc-cache-{}", std::process::id()));
        let src = tmp.join("src");
        std::fs::create_dir_all(&src).unwrap();
        let cache = ChunkCache::open(tmp.join("cache"), &src).expect("cache opens");
        let blocks: Vec<u16> = (0..1024).map(|i| (i % 300) as u16).collect();
        let biomes: Vec<u8> = (0..64).map(|i| (i % 55) as u8).collect();
        let hm: Vec<i16> = (0..256).map(|i| (i % 320) as i16).collect();
        cache.store(424242, -3, 7, &blocks, &biomes, &hm).unwrap();
        let hit = cache.load(424242, -3, 7, 1024, 64).expect("hit");
        assert_eq!(hit.blocks, blocks);
        assert_eq!(hit.biomes, biomes);
        assert_eq!(hit.heightmap, hm);
        assert!(cache.load(424242, -3, 8, 1024, 64).is_none(), "wrong chunk misses");
        assert!(cache.load(111, -3, 7, 1024, 64).is_none(), "wrong seed misses");
        assert!(cache.load(424242, -3, 7, 512, 64).is_none(), "geometry mismatch misses");
        // Corrupt payload -> checksum miss, never garbage.
        let p = cache.path_for(424242, -3, 7);
        let mut bytes = std::fs::read(&p).unwrap();
        let mid = bytes.len() / 2;
        bytes[mid] ^= 0xFF;
        std::fs::write(&p, bytes).unwrap();
        assert!(cache.load(424242, -3, 7, 1024, 64).is_none(), "corruption rejected");
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
