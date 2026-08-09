// Copyright (c) 2026 Neutron Contributors — MIT License
//
// session.lock handling for Minecraft worlds.
//
// Vanilla creates a `session.lock` file in the world directory containing
// the PID of the running server process. On startup, if the lock file exists
// and the PID is still alive, the server refuses to open the world (another
// instance holds it). If the PID is dead, the lock is considered stale and
// can be taken over.
//
// We follow the same convention for vanilla compatibility.

use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::error::{WorldError, WorldResult};

/// Name of the session lock file.
pub const LOCK_FILE: &str = "session.lock";

/// A file-based lock that prevents multiple server instances from opening
/// the same world simultaneously.
///
/// Drop the `SessionLock` to release the lock (the file is deleted).
pub struct SessionLock {
    /// Absolute path to the lock file.
    path: PathBuf,
    /// The PID written into the lock file.
    pid: u32,
}

impl SessionLock {
    /// Attempt to acquire the session lock for the given world directory.
    ///
    /// - If no lock file exists, creates one with the current PID.
    /// - If a lock file exists and the PID is dead (stale), takes over.
    /// - If a lock file exists and the PID is alive, returns an error.
    pub fn acquire(world_dir: &Path) -> WorldResult<Self> {
        let path = world_dir.join(LOCK_FILE);
        let current_pid = std::process::id();

        if path.exists() {
            match Self::read_pid(&path) {
                Ok(Some(pid)) => {
                    if pid == current_pid {
                        // We already hold the lock (e.g. re-acquire after crash recovery).
                        tracing::debug!(
                            pid = current_pid,
                            "session.lock already held by this process"
                        );
                        return Ok(Self { path, pid });
                    }

                    if is_pid_alive(pid) {
                        return Err(WorldError::SessionLockHeld {
                            pid,
                            path,
                        });
                    }

                    // Stale lock — PID is dead. Take over.
                    tracing::info!(
                        stale_pid = pid,
                        new_pid = current_pid,
                        "taking over stale session.lock"
                    );
                }
                Ok(None) => {
                    // Empty lock file — treat as stale.
                    tracing::info!(pid = current_pid, "empty session.lock, taking lock");
                }
                Err(e) => {
                    // Corrupted lock file — treat as stale but warn.
                    tracing::warn!(
                        error = %e,
                        "session.lock is corrupted, taking lock"
                    );
                }
            }
        }

        Self::write_lock(&path, current_pid)?;

        Ok(Self {
            path,
            pid: current_pid,
        })
    }

    /// Check if a session lock exists and is held by a live process.
    ///
    /// Returns `Ok(true)` if the lock is held, `Ok(false)` if free or stale.
    pub fn is_locked(world_dir: &Path) -> WorldResult<bool> {
        let path = world_dir.join(LOCK_FILE);
        if !path.exists() {
            return Ok(false);
        }

        match Self::read_pid(&path)? {
            Some(pid) => Ok(is_pid_alive(pid)),
            None => Ok(false),
        }
    }

    /// Get the PID stored in the lock file, if any.
    pub fn held_by(world_dir: &Path) -> WorldResult<Option<u32>> {
        let path = world_dir.join(LOCK_FILE);
        if !path.exists() {
            return Ok(None);
        }
        Self::read_pid(&path)
    }

    /// Manually release the lock (delete the lock file).
    ///
    /// This is also called automatically on drop.
    pub fn release(&self) -> WorldResult<()> {
        if self.path.exists() {
            fs::remove_file(&self.path)?;
            tracing::debug!(pid = self.pid, "released session.lock");
        }
        Ok(())
    }

    /// Get the PID this lock was acquired with.
    pub fn pid(&self) -> u32 {
        self.pid
    }

    /// Get the path to the lock file.
    pub fn path(&self) -> &Path {
        &self.path
    }

    // --- Private helpers ---

    /// Read the PID from a lock file.
    fn read_pid(path: &Path) -> WorldResult<Option<u32>> {
        let mut file = File::open(path).map_err(|_| WorldError::SessionLockCorrupted {
            path: path.to_path_buf(),
        })?;

        let mut contents = String::new();
        file.read_to_string(&mut contents)
            .map_err(|_| WorldError::SessionLockCorrupted {
                path: path.to_path_buf(),
            })?;

        let contents = contents.trim();
        if contents.is_empty() {
            return Ok(None);
        }

        let pid: u32 = contents.parse().map_err(|_| WorldError::SessionLockCorrupted {
            path: path.to_path_buf(),
        })?;

        Ok(Some(pid))
    }

    /// Write a lock file with the given PID.
    fn write_lock(path: &Path, pid: u32) -> WorldResult<()> {
        let mut file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(path)
            .map_err(|e| WorldError::Io(e))?;

        write!(file, "{}", pid).map_err(|e| WorldError::Io(e))?;
        file.flush().map_err(|e| WorldError::Io(e))?;

        tracing::debug!(pid, "acquired session.lock at {}", path.display());
        Ok(())
    }
}

impl Drop for SessionLock {
    fn drop(&mut self) {
        // Best-effort release. If the file is already gone, that's fine.
        let _ = self.release();
    }
}

/// Check if a process with the given PID is still running.
///
/// Uses platform commands to check process existence without unsafe code.
fn is_pid_alive(pid: u32) -> bool {
    if pid == 0 {
        return false; // PID 0 is never a real server.
    }

    #[cfg(unix)]
    {
        // `kill -0 <pid>` succeeds if the process exists (no signal sent).
        Command::new("kill")
            .args(["-0", &pid.to_string()])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    #[cfg(windows)]
    {
        // `tasklist` with PID filter: if output contains the PID, it's alive.
        Command::new("tasklist")
            .args(["/FI", &format!("PID eq {pid}"), "/NH"])
            .output()
            .map(|o| {
                let stdout = String::from_utf8_lossy(&o.stdout);
                stdout.contains(&pid.to_string())
            })
            .unwrap_or(false)
    }

    #[cfg(not(any(unix, windows)))]
    {
        // Unknown platform — assume PID might be alive for safety.
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_acquire_and_release() {
        let dir = tempfile::tempdir().unwrap();
        let lock = SessionLock::acquire(dir.path()).unwrap();
        assert_eq!(lock.pid(), std::process::id());
        assert!(dir.path().join(LOCK_FILE).exists());

        lock.release().unwrap();
        assert!(!dir.path().join(LOCK_FILE).exists());
    }

    #[test]
    fn test_drop_releases_lock() {
        let dir = tempfile::tempdir().unwrap();
        {
            let _lock = SessionLock::acquire(dir.path()).unwrap();
            assert!(dir.path().join(LOCK_FILE).exists());
        }
        // Lock should be released after drop.
        assert!(!dir.path().join(LOCK_FILE).exists());
    }

    #[test]
    fn test_acquire_stale_lock() {
        let dir = tempfile::tempdir().unwrap();
        // Write a lock file with a PID that's definitely dead (e.g., 1 on most systems,
        // or a very high number that no real process would have).
        let dead_pid = 999_999_999;
        let lock_path = dir.path().join(LOCK_FILE);
        fs::write(&lock_path, format!("{}", dead_pid)).unwrap();

        // Should succeed — the dead PID is stale.
        let lock = SessionLock::acquire(dir.path()).unwrap();
        assert_eq!(lock.pid(), std::process::id());
    }

    #[test]
    fn test_is_locked_false_when_no_file() {
        let dir = tempfile::tempdir().unwrap();
        assert!(!SessionLock::is_locked(dir.path()).unwrap());
    }

    #[test]
    fn test_is_locked_true_when_held() {
        let dir = tempfile::tempdir().unwrap();
        let _lock = SessionLock::acquire(dir.path()).unwrap();
        assert!(SessionLock::is_locked(dir.path()).unwrap());
    }

    #[test]
    fn test_held_by_returns_pid() {
        let dir = tempfile::tempdir().unwrap();
        let lock = SessionLock::acquire(dir.path()).unwrap();
        let holder = SessionLock::held_by(dir.path()).unwrap();
        assert_eq!(holder, Some(lock.pid()));
    }

    #[test]
    fn test_reacquire_same_process() {
        let dir = tempfile::tempdir().unwrap();
        let lock1 = SessionLock::acquire(dir.path()).unwrap();
        // Acquiring again from the same process should succeed.
        let lock2 = SessionLock::acquire(dir.path()).unwrap();
        assert_eq!(lock1.pid(), lock2.pid());
    }
}
