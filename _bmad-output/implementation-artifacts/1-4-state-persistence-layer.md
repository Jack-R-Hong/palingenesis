# Story 1.4: State Persistence Layer

Status: ready-for-dev

## Story

As a daemon,
I want to persist my state to disk,
So that I can survive restarts and maintain session context.

## Acceptance Criteria

**AC1: State Initialization on Start**
**Given** the daemon starts
**When** it initializes state
**Then** it reads from `{state_dir}/state.json` if exists
**And** creates the directory and file if not exists

**AC2: State Write Format**
**Given** the daemon has state to persist
**When** it writes state
**Then** the JSON file contains: version, daemon_state, current_session, stats
**And** file permissions are set to 600

**AC3: Corrupted State Recovery**
**Given** a corrupted state file
**When** the daemon attempts to read it
**Then** it logs a warning and starts with default state
**And** backs up the corrupted file to `state.json.bak`

**AC4: Concurrent Access Protection**
**Given** concurrent access attempts
**When** multiple processes try to write state
**Then** file locking prevents corruption

## Tasks / Subtasks

- [ ] Create state module structure (AC: 1, 2, 3, 4)
  - [ ] Create `src/state/mod.rs` with module exports
  - [ ] Create `src/state/schema.rs` with state struct definitions
  - [ ] Create `src/state/store.rs` with read/write operations
- [ ] Define state schema structs (AC: 2)
  - [ ] Define `StateFile` root struct with version field
  - [ ] Define `DaemonState` enum (monitoring, paused, stopped)
  - [ ] Define `CurrentSession` struct (path, steps_completed, last_step, total_steps)
  - [ ] Define `Stats` struct (saves_count, total_resumes, last_resume)
  - [ ] Implement `Default` trait for all structs
- [ ] Implement state store operations (AC: 1, 2, 3, 4)
  - [ ] Implement `StateStore::new()` - initialize with platform paths
  - [ ] Implement `StateStore::load()` - read from JSON file
  - [ ] Implement `StateStore::save()` - write to JSON file with atomic replace
  - [ ] Implement file permission setting (mode 600 on Unix)
- [ ] Add file locking for concurrent access (AC: 4)
  - [ ] Add `fs2` crate dependency to Cargo.toml
  - [ ] Implement exclusive lock on write operations
  - [ ] Implement shared lock on read operations
  - [ ] Add lock timeout handling
- [ ] Implement corrupted state recovery (AC: 3)
  - [ ] Detect corrupted/invalid JSON on load
  - [ ] Create backup of corrupted file to `state.json.bak`
  - [ ] Log warning with corruption details
  - [ ] Initialize fresh default state
- [ ] Integrate with platform paths from Story 1.3 (AC: 1)
  - [ ] Use `Paths::state_dir()` for state file location
  - [ ] Use `Paths::ensure_state_dir()` for directory creation
- [ ] Add unit tests (AC: 1, 2, 3, 4)
  - [ ] Test state serialization/deserialization
  - [ ] Test default state initialization
  - [ ] Test corrupted file recovery
  - [ ] Test file permissions
- [ ] Add integration tests
  - [ ] Test concurrent access with file locking
  - [ ] Test state persistence across simulated restarts

## Dev Notes

### Architecture Requirements

**From architecture.md - Data Architecture:**

> State Persistence: File-based (JSON) | Single-user daemon doesn't need SQLite complexity. JSON state file at `~/.local/state/palingenesis/state.json` with file locking.

**State File Schema (from architecture.md):**

```json
{
  "version": 1,
  "daemon_state": "monitoring",
  "current_session": {
    "path": "/path/to/session.md",
    "steps_completed": [1, 2, 3],
    "last_step": 3,
    "total_steps": 12
  },
  "stats": {
    "saves_count": 42,
    "total_resumes": 127,
    "last_resume": "2026-02-05T10:30:00Z"
  }
}
```

**From architecture.md - Project Structure:**

```
src/state/
├── mod.rs        # State persistence
├── store.rs      # JSON file read/write
├── schema.rs     # State file schema
└── audit.rs      # Audit trail (JSONL) - Story 3.6
```

**Implements:** ARCH10 (State persistence via JSON file)

### Technical Implementation

**State Schema Structs:**

```rust
// src/state/schema.rs
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use chrono::{DateTime, Utc};

/// Current version of the state file schema
pub const STATE_VERSION: u32 = 1;

/// Root state file structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateFile {
    pub version: u32,
    pub daemon_state: DaemonState,
    pub current_session: Option<CurrentSession>,
    pub stats: Stats,
}

impl Default for StateFile {
    fn default() -> Self {
        Self {
            version: STATE_VERSION,
            daemon_state: DaemonState::Stopped,
            current_session: None,
            stats: Stats::default(),
        }
    }
}

/// Daemon operational states
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DaemonState {
    Monitoring,
    Paused,
    Stopped,
}

/// Current session information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CurrentSession {
    pub path: PathBuf,
    pub steps_completed: Vec<u32>,
    pub last_step: u32,
    pub total_steps: u32,
}

/// Daemon statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Stats {
    pub saves_count: u64,
    pub total_resumes: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_resume: Option<DateTime<Utc>>,
}
```

**State Store Implementation:**

```rust
// src/state/store.rs
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::PathBuf;
use fs2::FileExt;
use tracing::{info, warn};

use crate::config::Paths;
use super::schema::StateFile;

pub struct StateStore {
    path: PathBuf,
}

impl StateStore {
    pub fn new() -> Self {
        let path = Paths::state_dir().join("state.json");
        Self { path }
    }

    /// Load state from file, returning default if not exists or corrupted
    pub fn load(&self) -> StateFile {
        match self.load_inner() {
            Ok(state) => state,
            Err(e) => {
                warn!("Failed to load state: {}, using defaults", e);
                StateFile::default()
            }
        }
    }

    fn load_inner(&self) -> Result<StateFile, StateError> {
        if !self.path.exists() {
            return Ok(StateFile::default());
        }

        let mut file = File::open(&self.path)?;
        file.lock_shared()?;

        let mut contents = String::new();
        file.read_to_string(&mut contents)?;

        match serde_json::from_str(&contents) {
            Ok(state) => Ok(state),
            Err(e) => {
                self.backup_corrupted()?;
                Err(StateError::Corrupted(e.to_string()))
            }
        }
    }

    /// Save state to file with atomic write
    pub fn save(&self, state: &StateFile) -> Result<(), StateError> {
        Paths::ensure_state_dir()?;

        let temp_path = self.path.with_extension("json.tmp");
        
        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&temp_path)?;

        file.lock_exclusive()?;

        let contents = serde_json::to_string_pretty(state)?;
        file.write_all(contents.as_bytes())?;
        file.sync_all()?;

        // Set permissions to 600 (owner read/write only)
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&temp_path, fs::Permissions::from_mode(0o600))?;
        }

        // Atomic rename
        fs::rename(&temp_path, &self.path)?;

        info!(path = %self.path.display(), "State persisted");
        Ok(())
    }

    fn backup_corrupted(&self) -> Result<(), StateError> {
        let backup_path = self.path.with_extension("json.bak");
        warn!(
            original = %self.path.display(),
            backup = %backup_path.display(),
            "Backing up corrupted state file"
        );
        fs::copy(&self.path, &backup_path)?;
        Ok(())
    }
}
```

### Dependencies to Add

```toml
# Cargo.toml additions
[dependencies]
fs2 = "0.4"           # File locking (cross-platform)
chrono = { version = "0.4", features = ["serde"] }  # DateTime for last_resume
```

Note: `serde_json` is likely already available via serde features, verify in Cargo.toml.

### Error Handling Pattern

```rust
// src/state/error.rs or inline in store.rs
#[derive(Debug, thiserror::Error)]
pub enum StateError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    
    #[error("State file corrupted: {0}")]
    Corrupted(String),
    
    #[error("Lock acquisition timeout")]
    LockTimeout,
}
```

### Testing Strategy

**Unit Tests:**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_default_state() {
        let state = StateFile::default();
        assert_eq!(state.version, STATE_VERSION);
        assert_eq!(state.daemon_state, DaemonState::Stopped);
        assert!(state.current_session.is_none());
    }

    #[test]
    fn test_state_serialization() {
        let state = StateFile::default();
        let json = serde_json::to_string(&state).unwrap();
        let parsed: StateFile = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.version, state.version);
    }

    #[test]
    fn test_corrupted_recovery() {
        let temp = tempdir().unwrap();
        let state_path = temp.path().join("state.json");
        
        // Write corrupted JSON
        fs::write(&state_path, "{ invalid json }").unwrap();
        
        // Store should recover with default
        let store = StateStore { path: state_path.clone() };
        let state = store.load();
        
        assert_eq!(state.version, STATE_VERSION);
        assert!(temp.path().join("state.json.bak").exists());
    }
}
```

**Integration Tests:**

```rust
// tests/state_test.rs
use std::thread;
use palingenesis::state::{StateStore, StateFile, DaemonState};

#[test]
fn test_concurrent_write() {
    let temp = tempfile::tempdir().unwrap();
    // Test that concurrent writes are serialized by file locks
    // ...
}
```

### Previous Story Learnings

From Story 1-3:
1. **Platform paths**: Use `Paths::state_dir()` from config module
2. **Directory creation**: Use `Paths::ensure_state_dir()` before writing
3. **Error handling**: Use `thiserror` for domain errors
4. **Module exports**: Re-export public API through mod.rs

### Project Structure Notes

- This story creates `src/state/` module which aligns with architecture spec
- `src/state/audit.rs` (audit trail) will be implemented in Story 3.6
- State store will be used by daemon core (Story 1.8) for persistence

### References

- [Source: _bmad-output/planning-artifacts/architecture.md#Data Architecture]
- [Source: _bmad-output/planning-artifacts/architecture.md#Project Structure & Boundaries]
- [Source: _bmad-output/planning-artifacts/epics.md#Story 1.4: State Persistence Layer]

## Dev Agent Record

### Agent Model Used

{{agent_model_name_version}}

### Debug Log References

### Completion Notes List

### File List

**Files to create:**
- `src/state/mod.rs`
- `src/state/schema.rs`
- `src/state/store.rs`
- `tests/state_test.rs`

**Files to modify:**
- `Cargo.toml` - Add fs2, chrono, serde_json dependencies
- `src/lib.rs` - Add state module declaration
- `_bmad-output/implementation-artifacts/sprint-status.yaml` - Update story status
