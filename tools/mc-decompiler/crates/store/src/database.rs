use std::path::{Path, PathBuf};

use rusqlite::{params, Connection};

use mc_decompiler_core::{ClassInfo, MinecraftVersion, VersionMetadata};

pub struct Store {
    db: Connection,
    base_path: PathBuf,
}

impl Store {
    /// Open or create a store. `path` is the root (e.g. `output`).
    /// Database lives at `path/metadata.db`, versions at `path/<version_id>/`.
    pub fn open(path: impl AsRef<Path>) -> anyhow::Result<Self> {
        let path = path.as_ref().to_path_buf();
        std::fs::create_dir_all(&path)?;
        let db_path = path.join("metadata.db");
        let db = Connection::open(db_path)?;
        db.execute_batch(
            "CREATE TABLE IF NOT EXISTS versions (
                id TEXT PRIMARY KEY,
                protocol_version INTEGER,
                jar_sha256 TEXT,
                decompiled_at TEXT,
                class_count INTEGER,
                total_lines INTEGER
            );
            CREATE TABLE IF NOT EXISTS classes (
                id TEXT PRIMARY KEY,
                version_id TEXT REFERENCES versions(id),
                source_path TEXT,
                line_count INTEGER,
                method_count INTEGER
            );",
        )?;
        Ok(Self {
            db,
            base_path: path,
        })
    }

    pub fn base_path(&self) -> &Path {
        &self.base_path
    }

    pub fn version_dir(&self, version: &str) -> PathBuf {
        self.base_path.join(version)
    }

    pub fn src_dir(&self, version: &str) -> PathBuf {
        self.version_dir(version).join("src")
    }

    pub fn classes_dir(&self, version: &str) -> PathBuf {
        self.version_dir(version).join("classes")
    }

    // --- versions ---

    pub fn add_version(&self, meta: &VersionMetadata) -> anyhow::Result<()> {
        self.db.execute(
            "INSERT INTO versions (id, protocol_version, jar_sha256, decompiled_at, class_count, total_lines)
             VALUES (?1, ?2, ?3, datetime('now'), ?4, ?5)",
            params![meta.id, meta.protocol, meta.jar_sha256, meta.class_count, meta.total_lines],
        )?;
        Ok(())
    }

    pub fn get_version(&self, id: &str) -> anyhow::Result<Option<MinecraftVersion>> {
        let mut stmt = self.db.prepare(
            "SELECT id, protocol_version, jar_sha256, decompiled_at, class_count, total_lines
             FROM versions WHERE id = ?1",
        )?;
        let mut rows = stmt.query_map(params![id], |row| {
            Ok(MinecraftVersion {
                id: row.get(0)?,
                protocol: row.get(1)?,
                jar_sha256: row.get(2)?,
                decompiled_at: row.get(3)?,
                class_count: row.get(4)?,
                total_lines: row.get(5)?,
            })
        })?;
        match rows.next() {
            Some(row) => Ok(Some(row?)),
            None => Ok(None),
        }
    }

    pub fn list_versions(&self) -> anyhow::Result<Vec<MinecraftVersion>> {
        let mut stmt = self.db.prepare(
            "SELECT id, protocol_version, jar_sha256, decompiled_at, class_count, total_lines
             FROM versions ORDER BY id",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(MinecraftVersion {
                id: row.get(0)?,
                protocol: row.get(1)?,
                jar_sha256: row.get(2)?,
                decompiled_at: row.get(3)?,
                class_count: row.get(4)?,
                total_lines: row.get(5)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn remove_version(&self, id: &str) -> anyhow::Result<()> {
        self.db
            .execute("DELETE FROM classes WHERE version_id = ?1", params![id])?;
        self.db
            .execute("DELETE FROM versions WHERE id = ?1", params![id])?;
        let dir = self.version_dir(id);
        if dir.exists() {
            std::fs::remove_dir_all(dir)?;
        }
        Ok(())
    }

    // --- classes ---

    pub fn add_class(&self, version_id: &str, class: &ClassInfo) -> anyhow::Result<()> {
        let id = format!("{version_id}:{}", class.fqn);
        self.db.execute(
            "INSERT OR REPLACE INTO classes (id, version_id, source_path, line_count, method_count)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                id,
                version_id,
                class.source_path,
                class.line_count,
                class.method_count
            ],
        )?;
        Ok(())
    }

    pub fn get_classes(&self, version_id: &str) -> anyhow::Result<Vec<ClassInfo>> {
        let mut stmt = self.db.prepare(
            "SELECT source_path, line_count, method_count
             FROM classes WHERE version_id = ?1 ORDER BY source_path",
        )?;
        let rows = stmt.query_map(params![version_id], |row| {
            let source_path: String = row.get(0)?;
            let fqn = source_path.trim_end_matches(".java").replace('/', ".");
            Ok(ClassInfo {
                fqn,
                source_path,
                line_count: row.get(1)?,
                method_count: row.get(2)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn search_classes(&self, version_id: &str, query: &str) -> anyhow::Result<Vec<ClassInfo>> {
        let mut stmt = self.db.prepare(
            "SELECT source_path, line_count, method_count
             FROM classes WHERE version_id = ?1 AND source_path LIKE ?2
             ORDER BY source_path",
        )?;
        let pattern = format!("%{query}%");
        let rows = stmt.query_map(params![version_id, pattern], |row| {
            let source_path: String = row.get(0)?;
            let fqn = source_path.trim_end_matches(".java").replace('/', ".");
            Ok(ClassInfo {
                fqn,
                source_path,
                line_count: row.get(1)?,
                method_count: row.get(2)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }
}
