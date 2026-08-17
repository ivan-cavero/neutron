//! Server lifecycle management: start, wait, stop for each server type.

use crate::types::ServerType;
use eyre::{Result, WrapErr};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

/// A running server process.
pub struct ServerProcess {
    child: Child,
    server_type: ServerType,
    log_path: PathBuf,
    start_time: Instant,
}

/// Start a server process.
pub fn start(
    server_type: ServerType,
    version: &str,
    server_dir: &Path,
    run_id: &str,
    max_players: usize,
    seed: &str,
    log_dir: &Path,
) -> Result<ServerProcess> {
    // Resolve to absolute paths for cross-platform compatibility
    let server_dir = if server_dir.is_absolute() {
        server_dir.to_path_buf()
    } else {
        std::env::current_dir()?.join(server_dir)
    };
    let log_dir = if log_dir.is_absolute() {
        log_dir.to_path_buf()
    } else {
        std::env::current_dir()?.join(log_dir)
    };

    crate::config::ensure_dirs(&server_dir, &log_dir)?;

    // Generate server config
    match server_type {
        ServerType::Vanilla => {
            crate::config::write_server_properties(&server_dir, max_players, seed, run_id)?;
        }
        ServerType::Paper | ServerType::Folia => {
            crate::config::write_server_properties(&server_dir, max_players, seed, run_id)?;
            // Enable spark HTTP for TPS measurement
            crate::config::write_paper_global(&server_dir)?;
        }
        ServerType::Pumpkin => {
            crate::config::write_pumpkin_config(&server_dir, max_players, seed, run_id)?;
        }
    }

    let log_path = log_dir.join(format!("{}.log", run_id));
    let log_file = fs::File::create(&log_path)
        .wrap_err_with(|| format!("creating log file: {}", log_path.display()))?;

    // Resolve benchmarks workspace root (where servers/ lives)
    let bench_dir = crate::ws_root();

    let child = match server_type {
        ServerType::Vanilla | ServerType::Paper | ServerType::Folia => {
            let jar_name = "server.jar";
            let jar_path = server_dir.join(jar_name);
            if !jar_path.exists() {
                let src = resolve_managed(&bench_dir, server_type, version)?;
                fs::copy(&src, &jar_path)?;
            }

            java_command()
                .args([
                    "-Xms2G",
                    "-Xmx2G",
                    "-XX:+AlwaysPreTouch",
                    "-jar",
                    jar_path.to_str().unwrap(),
                    "nogui",
                ])
                .current_dir(server_dir)
                .stdout(Stdio::from(log_file.try_clone()?))
                .stderr(Stdio::from(log_file))
                .spawn()
                .wrap_err("failed to start Java server")?
        }
        ServerType::Pumpkin => {
            let exe_name = if cfg!(target_os = "windows") {
                "pumpkin.exe"
            } else {
                "pumpkin"
            };
            let exe_path = server_dir.join(exe_name);
            if !exe_path.exists() {
                let src = resolve_managed(&bench_dir, server_type, version)?;
                fs::copy(&src, &exe_path)?;
            }

            Command::new(&exe_path)
                .current_dir(server_dir)
                .stdout(Stdio::from(log_file.try_clone()?))
                .stderr(Stdio::from(log_file))
                .spawn()
                .wrap_err("failed to start Pumpkin server")?
        }
    };

    Ok(ServerProcess {
        child,
        server_type,
        log_path,
        start_time: Instant::now(),
    })
}

impl ServerProcess {
    /// Wait for the server to be ready (Done line in log).
    pub fn wait_ready(&mut self, timeout: Duration) -> Result<Duration> {
        let deadline = Instant::now() + timeout;
        let start = Instant::now();

        loop {
            if Instant::now() > deadline {
                eyre::bail!(
                    "Server did not start within {:?} (no 'Done' line in {})",
                    timeout,
                    self.log_path.display()
                );
            }

            if let Ok(content) = fs::read_to_string(&self.log_path) {
                // Java servers: "Done (Xs)!"
                // Pumpkin: "Server is now running"
                if (content.contains("Done ") && content.contains("s)!"))
                    || content.contains("Server is now running")
                {
                    let startup = start.elapsed();
                    return Ok(startup);
                }
            }

            if let Ok(Some(status)) = self.child.try_wait() {
                eyre::bail!("Server process exited prematurely with status: {}", status);
            }

            std::thread::sleep(Duration::from_millis(500));
        }
    }

    /// Stop the server process.
    pub fn stop(&mut self) -> Result<()> {
        #[cfg(target_os = "windows")]
        {
            let _ = Command::new("taskkill")
                .args(["/F", "/T", "/PID", &self.child.id().to_string()])
                .output();
        }

        #[cfg(not(target_os = "windows"))]
        {
            unsafe {
                libc::kill(self.child.id() as i32, libc::SIGTERM);
            }
            std::thread::sleep(Duration::from_secs(2));
            let _ = self.child.kill();
        }

        Ok(())
    }

    /// Get the server type.
    pub fn server_type(&self) -> ServerType {
        self.server_type
    }

    /// Get the server process ID.
    pub fn pid(&self) -> sysinfo::Pid {
        sysinfo::Pid::from_u32(self.child.id())
    }
}

impl Drop for ServerProcess {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

/// Resolve the managed binary for `<type>/<version>` (multi-version layout
/// `servers/<type>/<version>/<binary>`), falling back to the legacy single-jar
/// layout `servers/<type>/<binary>`. Errors name the missing file and the
/// download command.
fn resolve_managed(bench_dir: &Path, server_type: ServerType, version: &str) -> Result<PathBuf> {
    let label = server_type.label();
    let bin = server_type.binary_name();
    let versioned = bench_dir.join("servers").join(label).join(version).join(bin);
    let legacy = bench_dir.join("servers").join(label).join(bin);
    if versioned.exists() {
        Ok(versioned)
    } else if legacy.exists() {
        Ok(legacy)
    } else {
        eyre::bail!(
            "Server binary not found: {} (or legacy {}).
  Download it with: neutron-bench servers download {} {}",
            versioned.display(),
            legacy.display(),
            label,
            version
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp_bench() -> PathBuf {
        std::env::temp_dir().join(format!("nb-resolve-{}", std::process::id()))
    }

    #[test]
    fn resolve_prefers_versioned_over_legacy() {
        let bench = tmp_bench();
        let versioned = bench.join("servers/vanilla/26.2/server.jar");
        let legacy = bench.join("servers/vanilla/server.jar");
        std::fs::create_dir_all(versioned.parent().unwrap()).unwrap();
        std::fs::create_dir_all(legacy.parent().unwrap()).unwrap();
        std::fs::write(&versioned, b"v").unwrap();
        std::fs::write(&legacy, b"l").unwrap();

        assert_eq!(
            resolve_managed(&bench, ServerType::Vanilla, "26.2").unwrap(),
            versioned
        );

        // Legacy single-jar layout still works when the versioned jar is absent.
        std::fs::remove_file(&versioned).unwrap();
        assert_eq!(
            resolve_managed(&bench, ServerType::Vanilla, "26.2").unwrap(),
            legacy
        );

        // Missing jar -> actionable error naming the file and the download command.
        std::fs::remove_file(&legacy).unwrap();
        let err = resolve_managed(&bench, ServerType::Vanilla, "99.9")
            .unwrap_err()
            .to_string();
        assert!(
            err.contains("servers/vanilla/99.9/server.jar")
                || err.contains("servers\\vanilla\\99.9\\server.jar")
        );
        assert!(err.contains("neutron-bench servers download vanilla 99.9"));

        let _ = std::fs::remove_dir_all(&bench);
    }
}

/// `$JAVA_HOME/bin/java` when set and present (Java 25 is required to run 26.x
/// jars), falling back to `java` on PATH.
fn java_command() -> Command {
    if let Some(home) = std::env::var_os("JAVA_HOME") {
        let java = std::path::PathBuf::from(home).join("bin").join(if cfg!(target_os = "windows") {
            "java.exe"
        } else {
            "java"
        });
        if java.exists() {
            return Command::new(java);
        }
    }
    Command::new("java")
}
