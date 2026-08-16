use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MinecraftVersion {
    pub id: String,
    pub protocol: u32,
    pub jar_sha256: String,
    pub decompiled_at: Option<String>,
    pub class_count: u32,
    pub total_lines: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionMetadata {
    pub id: String,
    pub protocol: u32,
    pub jar_sha256: String,
    pub class_count: u32,
    pub total_lines: u32,
}

impl From<VersionMetadata> for MinecraftVersion {
    fn from(meta: VersionMetadata) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        Self {
            id: meta.id,
            protocol: meta.protocol,
            jar_sha256: meta.jar_sha256,
            decompiled_at: Some(format!("{now}")),
            class_count: meta.class_count,
            total_lines: meta.total_lines,
        }
    }
}
