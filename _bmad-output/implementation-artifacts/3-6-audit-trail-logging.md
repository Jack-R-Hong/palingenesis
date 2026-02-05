# Story 3.6: Audit Trail Logging

Status: ready-for-dev

## Story

As a daemon,
I want to log all resume events to an audit trail,
So that I have a history of actions for debugging and metrics.

## Acceptance Criteria

**AC1: Audit Event Logging**
**Given** any resume action occurs
**When** audit logging runs
**Then** a JSON line is appended to `{state_dir}/audit.jsonl`

**AC2: Audit Entry Format**
**Given** an audit entry
**When** it is written
**Then** it includes: timestamp, event_type, session_path, stop_reason, action_taken, outcome

**AC3: File Creation**
**Given** the audit file doesn't exist
**When** the first audit entry is written
**Then** the file is created with mode 600

**AC4: File Rotation**
**Given** the audit file grows large
**When** rotation is triggered (configurable size, default 10MB)
**Then** the file is rotated to `audit.jsonl.1`
**And** a new `audit.jsonl` is created

**AC5: Concurrent Write Safety**
**Given** multiple resume events occur simultaneously
**When** audit entries are written
**Then** entries are not corrupted or interleaved

**AC6: Query Support**
**Given** audit entries exist
**When** querying the audit trail
**Then** entries can be filtered by event_type, time range, session_path

## Tasks / Subtasks

- [ ] Create AuditLogger struct (AC: 1, 2, 3)
  - [ ] Create `src/state/audit.rs`
  - [ ] Define AuditConfig with file path and rotation settings
  - [ ] Implement file creation with proper permissions
  - [ ] Implement append-only writing

- [ ] Define AuditEntry struct (AC: 2)
  - [ ] Add timestamp: DateTime<Utc>
  - [ ] Add event_type: AuditEventType enum
  - [ ] Add session_path: Option<PathBuf>
  - [ ] Add stop_reason: Option<String>
  - [ ] Add action_taken: String
  - [ ] Add outcome: AuditOutcome enum
  - [ ] Add metadata: HashMap<String, Value>

- [ ] Implement JSONL writing (AC: 1, 5)
  - [ ] Serialize entry to JSON
  - [ ] Append newline
  - [ ] Use file locking for concurrent safety
  - [ ] Flush after each write

- [ ] Implement file rotation (AC: 4)
  - [ ] Check file size before write
  - [ ] Rotate when size exceeds threshold
  - [ ] Rename current to .1, .2, etc.
  - [ ] Create new empty file
  - [ ] Limit number of rotated files

- [ ] Implement query support (AC: 6)
  - [ ] Read and parse JSONL file
  - [ ] Filter by event_type
  - [ ] Filter by time range
  - [ ] Filter by session_path
  - [ ] Return iterator over matching entries

- [ ] Add convenience methods (AC: 1, 2)
  - [ ] log_resume_started()
  - [ ] log_resume_completed()
  - [ ] log_resume_failed()
  - [ ] log_session_created()
  - [ ] log_session_backed_up()

- [ ] Add unit tests (AC: 1, 2, 3, 4, 5, 6)
  - [ ] Test entry serialization
  - [ ] Test file creation with permissions
  - [ ] Test append-only behavior
  - [ ] Test file rotation
  - [ ] Test concurrent writes
  - [ ] Test query filtering

- [ ] Add integration tests
  - [ ] Test with resume strategies
  - [ ] Test rotation under load
  - [ ] Test recovery from corrupted file

## Dev Notes

### Architecture Requirements

**From epics.md - Technical Notes:**

```
- Implements: FR13, ARCH11
- Create `src/state/audit.rs`
- Append-only, one JSON object per line
```

**From architecture.md:**

```
src/state/
    mod.rs                    # State module root
    persistence.rs            # State persistence (Story 1.4)
    audit.rs                  # Audit trail logging (THIS STORY)
```

**Implements:** FR13 (audit trail), ARCH11 (observability)

### Technical Implementation

**AuditLogger:**

```rust
// src/state/audit.rs
use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tracing::{debug, info, warn};

/// Configuration for audit logging.
#[derive(Debug, Clone)]
pub struct AuditConfig {
    /// Path to audit file
    pub audit_path: PathBuf,
    /// Maximum file size before rotation (bytes)
    pub max_size: u64,
    /// Maximum number of rotated files to keep
    pub max_files: usize,
    /// File permissions (Unix mode)
    #[cfg(unix)]
    pub file_mode: u32,
}

impl Default for AuditConfig {
    fn default() -> Self {
        Self {
            audit_path: PathBuf::from("audit.jsonl"),
            max_size: 10 * 1024 * 1024, // 10MB
            max_files: 5,
            #[cfg(unix)]
            file_mode: 0o600,
        }
    }
}

/// Types of audit events.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum AuditEventType {
    /// Resume operation started
    ResumeStarted,
    /// Resume operation completed successfully
    ResumeCompleted,
    /// Resume operation failed
    ResumeFailed,
    /// New session created
    SessionCreated,
    /// Session backed up
    SessionBackedUp,
    /// Daemon started
    DaemonStarted,
    /// Daemon stopped
    DaemonStopped,
    /// Configuration changed
    ConfigChanged,
    /// Error occurred
    Error,
}

/// Outcome of an audited action.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum AuditOutcome {
    Success,
    Failure,
    Skipped,
    Pending,
}

/// A single audit trail entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    /// When the event occurred
    pub timestamp: DateTime<Utc>,
    /// Type of event
    pub event_type: AuditEventType,
    /// Session file path (if applicable)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_path: Option<PathBuf>,
    /// Stop reason (if applicable)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_reason: Option<String>,
    /// Action that was taken
    pub action_taken: String,
    /// Outcome of the action
    pub outcome: AuditOutcome,
    /// Additional metadata
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub metadata: HashMap<String, Value>,
}

impl AuditEntry {
    pub fn new(event_type: AuditEventType, action: impl Into<String>) -> Self {
        Self {
            timestamp: Utc::now(),
            event_type,
            session_path: None,
            stop_reason: None,
            action_taken: action.into(),
            outcome: AuditOutcome::Pending,
            metadata: HashMap::new(),
        }
    }
    
    pub fn with_session(mut self, path: PathBuf) -> Self {
        self.session_path = Some(path);
        self
    }
    
    pub fn with_stop_reason(mut self, reason: impl Into<String>) -> Self {
        self.stop_reason = Some(reason.into());
        self
    }
    
    pub fn with_outcome(mut self, outcome: AuditOutcome) -> Self {
        self.outcome = outcome;
        self
    }
    
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<Value>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }
}

/// Audit trail logger.
pub struct AuditLogger {
    config: AuditConfig,
}

impl AuditLogger {
    pub fn new(state_dir: &Path) -> Self {
        Self {
            config: AuditConfig {
                audit_path: state_dir.join("audit.jsonl"),
                ..Default::default()
            },
        }
    }
    
    pub fn with_config(config: AuditConfig) -> Self {
        Self { config }
    }
    
    /// Log an audit entry.
    pub fn log(&self, entry: &AuditEntry) -> Result<(), AuditError> {
        // Check if rotation needed
        self.maybe_rotate()?;
        
        // Serialize entry
        let json = serde_json::to_string(entry)
            .map_err(|e| AuditError::Serialization(e.to_string()))?;
        
        // Append to file
        let mut file = self.open_for_append()?;
        writeln!(file, "{}", json)?;
        file.flush()?;
        
        debug!(
            event_type = ?entry.event_type,
            outcome = ?entry.outcome,
            "Audit entry logged"
        );
        
        Ok(())
    }
    
    /// Open audit file for appending, creating if needed.
    fn open_for_append(&self) -> Result<File, AuditError> {
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.config.audit_path)?;
        
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let permissions = std::fs::Permissions::from_mode(self.config.file_mode);
            std::fs::set_permissions(&self.config.audit_path, permissions)?;
        }
        
        Ok(file)
    }
    
    /// Rotate file if it exceeds max size.
    fn maybe_rotate(&self) -> Result<(), AuditError> {
        if !self.config.audit_path.exists() {
            return Ok(());
        }
        
        let metadata = std::fs::metadata(&self.config.audit_path)?;
        if metadata.len() < self.config.max_size {
            return Ok(());
        }
        
        info!(
            size = metadata.len(),
            max = self.config.max_size,
            "Rotating audit file"
        );
        
        // Rotate existing files
        for i in (1..self.config.max_files).rev() {
            let from = self.rotated_path(i);
            let to = self.rotated_path(i + 1);
            if from.exists() {
                std::fs::rename(&from, &to)?;
            }
        }
        
        // Rename current to .1
        let first_rotated = self.rotated_path(1);
        std::fs::rename(&self.config.audit_path, &first_rotated)?;
        
        // Delete oldest if exceeds max_files
        let oldest = self.rotated_path(self.config.max_files);
        if oldest.exists() {
            std::fs::remove_file(&oldest)?;
        }
        
        Ok(())
    }
    
    /// Get path for rotated file.
    fn rotated_path(&self, index: usize) -> PathBuf {
        let mut path = self.config.audit_path.clone();
        let filename = path.file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("audit.jsonl");
        path.set_file_name(format!("{}.{}", filename, index));
        path
    }
    
    /// Query audit entries with filters.
    pub fn query(&self) -> AuditQuery {
        AuditQuery::new(&self.config.audit_path)
    }
    
    // Convenience methods
    
    pub fn log_resume_started(
        &self,
        session_path: &Path,
        stop_reason: &str,
    ) -> Result<(), AuditError> {
        let entry = AuditEntry::new(AuditEventType::ResumeStarted, "Starting resume")
            .with_session(session_path.to_path_buf())
            .with_stop_reason(stop_reason)
            .with_outcome(AuditOutcome::Pending);
        self.log(&entry)
    }
    
    pub fn log_resume_completed(
        &self,
        session_path: &Path,
        action: &str,
    ) -> Result<(), AuditError> {
        let entry = AuditEntry::new(AuditEventType::ResumeCompleted, action)
            .with_session(session_path.to_path_buf())
            .with_outcome(AuditOutcome::Success);
        self.log(&entry)
    }
    
    pub fn log_resume_failed(
        &self,
        session_path: &Path,
        error: &str,
    ) -> Result<(), AuditError> {
        let entry = AuditEntry::new(AuditEventType::ResumeFailed, "Resume failed")
            .with_session(session_path.to_path_buf())
            .with_outcome(AuditOutcome::Failure)
            .with_metadata("error", error);
        self.log(&entry)
    }
    
    pub fn log_session_backed_up(
        &self,
        original: &Path,
        backup: &Path,
    ) -> Result<(), AuditError> {
        let entry = AuditEntry::new(AuditEventType::SessionBackedUp, "Session backed up")
            .with_session(original.to_path_buf())
            .with_outcome(AuditOutcome::Success)
            .with_metadata("backup_path", backup.display().to_string());
        self.log(&entry)
    }
}

impl Default for AuditLogger {
    fn default() -> Self {
        Self {
            config: AuditConfig::default(),
        }
    }
}

/// Query builder for audit entries.
pub struct AuditQuery {
    path: PathBuf,
    event_types: Option<Vec<AuditEventType>>,
    start_time: Option<DateTime<Utc>>,
    end_time: Option<DateTime<Utc>>,
    session_path: Option<PathBuf>,
}

impl AuditQuery {
    fn new(path: &Path) -> Self {
        Self {
            path: path.to_path_buf(),
            event_types: None,
            start_time: None,
            end_time: None,
            session_path: None,
        }
    }
    
    pub fn event_types(mut self, types: Vec<AuditEventType>) -> Self {
        self.event_types = Some(types);
        self
    }
    
    pub fn after(mut self, time: DateTime<Utc>) -> Self {
        self.start_time = Some(time);
        self
    }
    
    pub fn before(mut self, time: DateTime<Utc>) -> Self {
        self.end_time = Some(time);
        self
    }
    
    pub fn for_session(mut self, path: PathBuf) -> Self {
        self.session_path = Some(path);
        self
    }
    
    /// Execute query and return matching entries.
    pub fn execute(&self) -> Result<Vec<AuditEntry>, AuditError> {
        if !self.path.exists() {
            return Ok(Vec::new());
        }
        
        let file = File::open(&self.path)?;
        let reader = BufReader::new(file);
        
        let mut results = Vec::new();
        
        for line in reader.lines() {
            let line = line?;
            if line.is_empty() {
                continue;
            }
            
            let entry: AuditEntry = serde_json::from_str(&line)
                .map_err(|e| AuditError::Deserialization(e.to_string()))?;
            
            if self.matches(&entry) {
                results.push(entry);
            }
        }
        
        Ok(results)
    }
    
    fn matches(&self, entry: &AuditEntry) -> bool {
        // Check event type filter
        if let Some(types) = &self.event_types {
            if !types.contains(&entry.event_type) {
                return false;
            }
        }
        
        // Check time range
        if let Some(start) = self.start_time {
            if entry.timestamp < start {
                return false;
            }
        }
        if let Some(end) = self.end_time {
            if entry.timestamp > end {
                return false;
            }
        }
        
        // Check session path
        if let Some(path) = &self.session_path {
            if entry.session_path.as_ref() != Some(path) {
                return false;
            }
        }
        
        true
    }
}

#[derive(Debug, thiserror::Error)]
pub enum AuditError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("Serialization error: {0}")]
    Serialization(String),
    
    #[error("Deserialization error: {0}")]
    Deserialization(String),
}
```

### Dependencies

Uses existing dependencies:
- `serde` + `serde_json` (already in Cargo.toml) - JSON serialization
- `chrono` (already in Cargo.toml) - timestamps
- `tracing` (already in Cargo.toml) - structured logging
- `thiserror` (already in Cargo.toml) - error types

### JSONL Format

Each line is a complete JSON object:

```jsonl
{"timestamp":"2026-02-05T14:30:22.123Z","event_type":"resume_started","session_path":"/path/to/session.md","stop_reason":"rate_limit","action_taken":"Starting resume","outcome":"pending"}
{"timestamp":"2026-02-05T14:30:52.456Z","event_type":"resume_completed","session_path":"/path/to/session.md","action_taken":"Resumed same session","outcome":"success"}
```

### File Rotation

```
audit.jsonl      <- Current file (active)
audit.jsonl.1    <- Previous rotation
audit.jsonl.2    <- Older rotation
audit.jsonl.3    <- Oldest (deleted when max_files exceeded)
```

### Concurrent Safety

Uses append mode (`O_APPEND`) which provides atomic writes for small writes on most filesystems. For additional safety, could add file locking:

```rust
use fs2::FileExt;

let file = self.open_for_append()?;
file.lock_exclusive()?;
writeln!(file, "{}", json)?;
file.unlock()?;
```

### Testing Strategy

**Unit Tests:**
- Test entry serialization/deserialization
- Test file creation with permissions
- Test rotation logic
- Test query filtering
- Mock file system for concurrent tests

**Integration Tests:**
- Test with resume strategies
- Test rotation under load
- Test query performance

### References

- [Source: _bmad-output/planning-artifacts/epics.md#Story 3.6: Audit Trail Logging]
- [Source: _bmad-output/planning-artifacts/architecture.md#Observability]

## File List

**Files to create:**
- `src/state/audit.rs`
- `tests/audit_test.rs`

**Files to modify:**
- `src/state/mod.rs` (add audit module)
- `_bmad-output/implementation-artifacts/sprint-status.yaml`

## Change Log

- 2026-02-05: Story created and marked ready-for-dev
