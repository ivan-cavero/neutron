use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassInfo {
    pub fqn: String,
    pub source_path: String,
    pub line_count: u32,
    pub method_count: u32,
}

impl ClassInfo {
    /// Parse a Java source file and extract class metadata.
    ///
    /// # Errors
    /// Returns an error if the file cannot be read.
    pub fn from_path(src_dir: &std::path::Path, relative_path: &str) -> anyhow::Result<Self> {
        let full_path = src_dir.join(relative_path);
        let content = std::fs::read_to_string(&full_path)?;
        let line_count = u32::try_from(content.lines().count()).unwrap_or(u32::MAX);
        let method_count = count_methods(&content);
        let fqn = path_to_fqn(relative_path);
        Ok(Self {
            fqn,
            source_path: relative_path.to_string(),
            line_count,
            method_count,
        })
    }
}

fn path_to_fqn(path: &str) -> String {
    path.trim_end_matches(".java").replace('/', ".")
}

fn count_methods(content: &str) -> u32 {
    let mut count: u32 = 0;
    let mut brace_depth: i32 = 0;
    let mut in_class = false;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.contains("class ") || trimmed.contains("interface ") {
            in_class = true;
        }
        for ch in trimmed.chars() {
            match ch {
                '{' => brace_depth += 1,
                '}' => {
                    brace_depth -= 1;
                    if brace_depth == 0 {
                        in_class = false;
                    }
                }
                _ => {}
            }
        }
        if in_class
            && brace_depth >= 1
            && (trimmed.starts_with("public ")
                || trimmed.starts_with("private ")
                || trimmed.starts_with("protected ")
                || trimmed.starts_with("static "))
            && trimmed.contains('(')
            && !trimmed.contains("class ")
        {
            count = count.saturating_add(1);
        }
    }
    count
}
