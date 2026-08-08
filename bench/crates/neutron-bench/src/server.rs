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

    // Resolve bench directory (where servers/ lives)
    let bench_dir = std::env::current_dir()
        .wrap_err("getting current directory")?
        .join("bench");

    let child = match server_type {
        ServerType::Vanilla | ServerType::Paper | ServerType::Folia => {
            let jar_name = "server.jar";
            let jar_path = server_dir.join(jar_name);
            if !jar_path.exists() {
                // Try bench/servers/<type>/server.jar
                let bench_jar = bench_dir.join("servers").join(server_type.label()).join(jar_name);
                if bench_jar.exists() {
                    fs::copy(&bench_jar, &jar_path)?;
                } else {
                    eyre::bail!(
                        "Server jar not found: {} or {}",
                        jar_path.display(),
                        bench_jar.display()
                    );
                }
            }

            Command::new("java")
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
                let bench_exe = bench_dir.join("servers").join("pumpkin").join(exe_name);
                if bench_exe.exists() {
                    fs::copy(&bench_exe, &exe_path)?;
                } else {
                    eyre::bail!("Pumpkin binary not found: {}", bench_exe.display());
                }
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
