use thiserror::Error;

#[derive(Debug, Error)]
pub enum DecompilerError {
    #[error("Java not found at {path}")]
    JavaNotFound { path: String },

    #[error("Vineflower failed: {stderr}")]
    VineflowerFailed { stderr: String },

    #[error("Version {0} already decompiled")]
    VersionAlreadyDecompiled(String),

    #[error("JAR not found: {0}")]
    JarNotFound(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}
