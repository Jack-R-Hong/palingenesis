# Story 1.5: PID File Management

Status: ready-for-dev

## Story

As a user,
I want palingenesis to track the daemon process via PID file,
So that I can ensure only one daemon runs and CLI commands can find it.

## Acceptance Criteria

**AC1: PID File Creation on Daemon Start**
**Given** no daemon is running
**When** I start the daemon
**Then** a PID file is created at `{runtime_dir}/palingenesis.pid`
**And** the file contains the daemon's process ID

**AC2: Prevent Multiple Daemon Instances**
**Given** a daemon is already running (PID file exists with valid process)
**When** I try to start another daemon
**Then** it fails with error "Daemon already running (PID: N)"

**AC3: Stale PID File Cleanup**
**Given** a stale PID file exists (process not running)
**When** I start the daemon
**Then** it removes the stale PID file
**And** creates a new PID file with the new process ID

**AC4: PID File Removal on Graceful Shutdown**
**Given** a running daemon
**When** it shuts down gracefully
**Then** the PID file is removed

## Tasks / Subtasks

- [ ] Create PID file module structure (AC: 1, 2, 3, 4)
  - [ ] Create `src/daemon/pid.rs` with PidFile struct
  - [ ] Update `src/daemon/mod.rs` to export pid module
- [ ] Implement PidFile struct and error types (AC: 1, 2, 3, 4)
  - [ ] Define `PidError` enum with thiserror (AlreadyRunning, Io, Parse, ProcessCheck)
  - [ ] Define `PidFile` struct holding the path
  - [ ] Implement `PidFile::new()` - uses `Paths::runtime_dir().join("palingenesis.pid")`
- [ ] Implement PID file creation (AC: 1)
  - [ ] Implement `PidFile::acquire()` - creates PID file with current process ID
  - [ ] Ensure runtime directory exists via `Paths::ensure_runtime_dir()`
  - [ ] Write PID to file with exclusive access
  - [ ] Set file permissions to 644 (owner read/write, others read)
- [ ] Implement process existence check (AC: 2, 3)
  - [ ] Implement `PidFile::is_process_running(pid)` for Linux via `/proc/{pid}`
  - [ ] Implement `PidFile::is_process_running(pid)` for Unix via `kill(pid, 0)`
  - [ ] Handle race conditions (process may exit between check and action)
- [ ] Implement stale PID detection and cleanup (AC: 2, 3)
  - [ ] Implement `PidFile::read()` - parse PID from existing file
  - [ ] Implement `PidFile::check_stale()` - check if PID file is stale
  - [ ] Implement `PidFile::remove()` - delete PID file
  - [ ] Log warning when removing stale PID file
- [ ] Implement PID file release on shutdown (AC: 4)
  - [ ] Implement `PidFile::release()` - remove PID file on graceful shutdown
  - [ ] Implement `Drop` trait for automatic cleanup on panic/unexpected exit
- [ ] Integrate with Paths module (AC: 1)
  - [ ] Use `Paths::runtime_dir()` for PID file location
  - [ ] Use `Paths::ensure_runtime_dir()` before writing PID file
- [ ] Add unit tests (AC: 1, 2, 3, 4)
  - [ ] Test PID file creation and content
  - [ ] Test stale PID detection with mock process check
  - [ ] Test already running error when valid process exists
  - [ ] Test cleanup on release
  - [ ] Test file permissions
- [ ] Add integration tests
  - [ ] Test full acquire/release lifecycle
  - [ ] Test concurrent acquisition attempts

## Dev Notes

### Architecture Requirements

**From architecture.md - Infrastructure & Deployment:**

> PID File: `/run/user/{uid}/palingenesis.pid` - Standard location, cleaned on exit.

**From architecture.md - Platform-Specific Paths:**

| Resource | Linux | macOS |
|----------|-------|-------|
| Runtime | `/run/user/{uid}/` | `/tmp/palingenesis-{uid}/` |

**From architecture.md - Project Structure:**

```
src/daemon/
    mod.rs           # Daemon orchestration
    signals.rs       # SIGTERM/SIGHUP handling
    pid.rs           # PID file management
    state.rs         # Session state machine
    shutdown.rs      # Graceful shutdown coordination
```

**Implements:** ARCH18 (PID file at `/run/user/{uid}/palingenesis.pid`)

### Technical Implementation

**PidFile Struct:**

```rust
// src/daemon/pid.rs
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::PathBuf;
use std::process;
use tracing::{info, warn};

use crate::config::Paths;

#[derive(Debug, thiserror::Error)]
pub enum PidError {
    #[error("Daemon already running (PID: {pid})")]
    AlreadyRunning { pid: u32 },
    
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("Failed to parse PID from file: {0}")]
    Parse(String),
    
    #[error("Failed to check process existence: {0}")]
    ProcessCheck(String),
}

pub struct PidFile {
    path: PathBuf,
    acquired: bool,
}

impl PidFile {
    /// Create a new PidFile instance pointing to the standard location.
    pub fn new() -> Self {
        Self {
            path: Paths::runtime_dir().join("palingenesis.pid"),
            acquired: false,
        }
    }

    /// Acquire the PID file lock.
    /// Returns error if another daemon is already running.
    pub fn acquire(&mut self) -> Result<(), PidError> {
        // Check for existing PID file
        if self.path.exists() {
            match self.read() {
                Ok(existing_pid) => {
                    if Self::is_process_running(existing_pid)? {
                        return Err(PidError::AlreadyRunning { pid: existing_pid });
                    }
                    // Stale PID file - process not running
                    warn!(
                        pid = existing_pid,
                        path = %self.path.display(),
                        "Removing stale PID file"
                    );
                    self.remove()?;
                }
                Err(e) => {
                    warn!(error = %e, "Failed to read PID file, removing");
                    self.remove()?;
                }
            }
        }

        // Ensure runtime directory exists
        Paths::ensure_runtime_dir()?;

        // Write current PID
        let pid = process::id();
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)  // Fail if file exists (race condition protection)
            .open(&self.path)?;
        
        file.write_all(pid.to_string().as_bytes())?;
        file.sync_all()?;

        // Set permissions to 644
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&self.path, fs::Permissions::from_mode(0o644))?;
        }

        self.acquired = true;
        info!(pid = pid, path = %self.path.display(), "PID file created");
        Ok(())
    }

    /// Read PID from existing file.
    pub fn read(&self) -> Result<u32, PidError> {
        let mut file = File::open(&self.path)?;
        let mut contents = String::new();
        file.read_to_string(&mut contents)?;
        
        contents
            .trim()
            .parse()
            .map_err(|_| PidError::Parse(contents.clone()))
    }

    /// Check if a process with the given PID is running.
    #[cfg(target_os = "linux")]
    pub fn is_process_running(pid: u32) -> Result<bool, PidError> {
        let proc_path = PathBuf::from(format!("/proc/{}", pid));
        Ok(proc_path.exists())
    }

    #[cfg(unix)]
    #[cfg(not(target_os = "linux"))]
    pub fn is_process_running(pid: u32) -> Result<bool, PidError> {
        use nix::sys::signal::{kill, Signal};
        use nix::unistd::Pid;
        
        match kill(Pid::from_raw(pid as i32), None) {
            Ok(_) => Ok(true),
            Err(nix::errno::Errno::ESRCH) => Ok(false),  // No such process
            Err(nix::errno::Errno::EPERM) => Ok(true),   // Process exists but no permission
            Err(e) => Err(PidError::ProcessCheck(e.to_string())),
        }
    }

    /// Remove the PID file.
    pub fn remove(&self) -> Result<(), PidError> {
        if self.path.exists() {
            fs::remove_file(&self.path)?;
            info!(path = %self.path.display(), "PID file removed");
        }
        Ok(())
    }

    /// Release the PID file (call on graceful shutdown).
    pub fn release(&mut self) -> Result<(), PidError> {
        if self.acquired {
            self.remove()?;
            self.acquired = false;
        }
        Ok(())
    }

    /// Returns the PID file path.
    pub fn path(&self) -> &PathBuf {
        &self.path
    }
}

impl Drop for PidFile {
    fn drop(&mut self) {
        if self.acquired {
            if let Err(e) = self.release() {
                eprintln!("Warning: Failed to clean up PID file: {}", e);
            }
        }
    }
}

impl Default for PidFile {
    fn default() -> Self {
        Self::new()
    }
}
```

### Dependencies

No new dependencies required. Uses:
- `nix` (already in Cargo.toml for signal handling on Unix)
- `tracing` (already in Cargo.toml)
- `thiserror` (already in Cargo.toml)

### Error Handling Pattern

Uses `thiserror` following project conventions from architecture.md:
- `PidError::AlreadyRunning` - Another daemon instance is running
- `PidError::Io` - File system operations failed
- `PidError::Parse` - PID file contains invalid data
- `PidError::ProcessCheck` - Failed to verify process existence

### Testing Strategy

**Unit Tests:**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn test_pid_file_creation() {
        let _lock = ENV_LOCK.lock().unwrap();
        let temp = tempdir().unwrap();
        std::env::set_var("PALINGENESIS_RUNTIME", temp.path());

        let mut pid_file = PidFile::new();
        pid_file.acquire().unwrap();

        let pid_path = temp.path().join("palingenesis.pid");
        assert!(pid_path.exists());

        let contents = fs::read_to_string(&pid_path).unwrap();
        assert_eq!(contents.trim().parse::<u32>().unwrap(), process::id());

        std::env::remove_var("PALINGENESIS_RUNTIME");
    }

    #[test]
    fn test_stale_pid_detection() {
        let _lock = ENV_LOCK.lock().unwrap();
        let temp = tempdir().unwrap();
        std::env::set_var("PALINGENESIS_RUNTIME", temp.path());

        // Write a stale PID (very high number unlikely to be running)
        let pid_path = temp.path().join("palingenesis.pid");
        fs::create_dir_all(temp.path()).unwrap();
        fs::write(&pid_path, "999999999").unwrap();

        let mut pid_file = PidFile::new();
        // Should succeed because the PID is stale
        pid_file.acquire().unwrap();

        std::env::remove_var("PALINGENESIS_RUNTIME");
    }

    #[test]
    fn test_cleanup_on_release() {
        let _lock = ENV_LOCK.lock().unwrap();
        let temp = tempdir().unwrap();
        std::env::set_var("PALINGENESIS_RUNTIME", temp.path());

        let pid_path = temp.path().join("palingenesis.pid");

        let mut pid_file = PidFile::new();
        pid_file.acquire().unwrap();
        assert!(pid_path.exists());

        pid_file.release().unwrap();
        assert!(!pid_path.exists());

        std::env::remove_var("PALINGENESIS_RUNTIME");
    }
}
```

### Previous Story Learnings

From Story 1-3 (Platform-Specific Path Resolution):
1. **Runtime directory**: Use `Paths::runtime_dir()` for PID file location
2. **Directory creation**: Use `Paths::ensure_runtime_dir()` before writing
3. **Platform differences**: Linux uses `/run/user/{uid}/`, macOS uses `/tmp/palingenesis-{uid}/`
4. **Environment override**: Respect `PALINGENESIS_RUNTIME` env var

From Story 1-4 (State Persistence Layer):
1. **Error handling**: Use `thiserror` for domain errors
2. **File permissions**: Set appropriate Unix permissions (644 for PID, 600 for sensitive data)
3. **Atomic operations**: Consider race conditions in multi-process scenarios
4. **Cleanup on Drop**: Implement `Drop` trait for automatic resource cleanup

### Project Structure Notes

- This story creates `src/daemon/pid.rs` which aligns with architecture spec
- PidFile will be used by:
  - Story 1-8 (Daemon Start Command) - acquire PID file
  - Story 1-9 (Daemon Stop Command) - read PID to send signal
  - Story 1-13 (Graceful Shutdown) - release PID file

### References

- [Source: _bmad-output/planning-artifacts/architecture.md#Infrastructure & Deployment]
- [Source: _bmad-output/planning-artifacts/architecture.md#Platform-Specific Paths]
- [Source: _bmad-output/planning-artifacts/architecture.md#Project Structure & Boundaries]
- [Source: _bmad-output/planning-artifacts/epics.md#Story 1.5: PID File Management]

## Dev Agent Record

### Agent Model Used

{{agent_model_name_version}}

### Debug Log References

### Completion Notes List

### File List

**Files to create:**
- `src/daemon/pid.rs`

**Files to modify:**
- `src/daemon/mod.rs` - Export pid module
- `_bmad-output/implementation-artifacts/sprint-status.yaml` - Update story status
