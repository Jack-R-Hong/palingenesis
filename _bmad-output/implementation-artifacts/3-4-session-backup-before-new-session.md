# Story 3.4: Session Backup Before New Session

Status: ready-for-dev

## Story

As a daemon,
I want to backup the session file before starting a new session,
So that I can recover if something goes wrong.

## Acceptance Criteria

**AC1: Backup File Creation**
**Given** a context exhaustion triggers new session
**When** backup runs before new session creation
**Then** the session file is copied to `session-backup-{timestamp}.md`
**And** the backup is in the same directory as the original

**AC2: Backup Preservation**
**Given** backup succeeds
**When** the new session starts
**Then** the original session file may be modified
**And** the backup remains unchanged

**AC3: Backup Failure Handling**
**Given** backup fails (disk full, permissions)
**When** the error is caught
**Then** it logs error "Failed to backup session: {reason}"
**And** proceeds with new session anyway (warn, don't block)

**AC4: Backup Pruning**
**Given** backups accumulate
**When** more than N backups exist (configurable, default 10)
**Then** oldest backups are pruned

**AC5: Timestamp Format**
**Given** a backup is created
**When** the filename is generated
**Then** it uses format `YYYYMMDD-HHMMSS`

**AC6: Backup Verification**
**Given** backup file is created
**When** verification runs
**Then** file size matches original
**And** file is readable

## Tasks / Subtasks

- [ ] Create SessionBackup struct (AC: 1, 5)
  - [ ] Create `src/resume/backup.rs`
  - [ ] Add configuration for max_backups
  - [ ] Add configuration for timestamp format
  - [ ] Implement timestamp generation

- [ ] Implement backup creation (AC: 1, 2, 5)
  - [ ] Generate backup filename with timestamp
  - [ ] Copy session file to backup location
  - [ ] Preserve file metadata if possible
  - [ ] Return backup path on success

- [ ] Implement backup verification (AC: 6)
  - [ ] Compare file sizes
  - [ ] Optionally verify content hash
  - [ ] Check file readability
  - [ ] Log verification result

- [ ] Implement error handling (AC: 3)
  - [ ] Catch I/O errors gracefully
  - [ ] Log detailed error information
  - [ ] Return Result for caller decision
  - [ ] Don't block on backup failure

- [ ] Implement backup pruning (AC: 4)
  - [ ] List existing backups in directory
  - [ ] Sort by timestamp (oldest first)
  - [ ] Delete oldest when count exceeds max
  - [ ] Log pruned backups

- [ ] Add unit tests (AC: 1, 2, 3, 4, 5, 6)
  - [ ] Test backup creation
  - [ ] Test timestamp format
  - [ ] Test backup verification
  - [ ] Test error handling (disk full simulation)
  - [ ] Test pruning logic
  - [ ] Test concurrent backup operations

- [ ] Add integration tests
  - [ ] Test full backup flow
  - [ ] Test backup + new session integration
  - [ ] Test pruning with multiple backups

## Dev Notes

### Architecture Requirements

**From epics.md - Technical Notes:**

```
- Implements: FR10
- Create backup logic in `src/resume/new_session.rs`
- Timestamp format: `YYYYMMDD-HHMMSS`
```

**Backup Flow:**

```
Context Exhaustion
       │
       ▼
┌─────────────────┐
│ Session Backup  │
├─────────────────┤
│ 1. Generate     │
│    timestamp    │
│ 2. Copy file    │
│ 3. Verify copy  │
│ 4. Prune old    │
└─────────────────┘
       │
       ▼
New Session Creation
```

**Implements:** FR10 (session backup before new session)

### Technical Implementation

**SessionBackup:**

```rust
// src/resume/backup.rs
use std::path::{Path, PathBuf};

use chrono::{Local, DateTime};
use tokio::fs;
use tracing::{info, warn, debug, error};

/// Configuration for session backup.
#[derive(Debug, Clone)]
pub struct BackupConfig {
    /// Maximum number of backups to keep
    pub max_backups: usize,
    /// Timestamp format for backup filenames
    pub timestamp_format: String,
    /// Verify backup after creation
    pub verify_backup: bool,
}

impl Default for BackupConfig {
    fn default() -> Self {
        Self {
            max_backups: 10,
            timestamp_format: "%Y%m%d-%H%M%S".to_string(),
            verify_backup: true,
        }
    }
}

/// Error types for backup operations.
#[derive(Debug, thiserror::Error)]
pub enum BackupError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("Source file not found: {path}")]
    SourceNotFound { path: PathBuf },
    
    #[error("Backup verification failed: expected {expected} bytes, got {actual}")]
    VerificationFailed { expected: u64, actual: u64 },
    
    #[error("Failed to parse backup timestamp from filename: {filename}")]
    InvalidBackupFilename { filename: String },
}

/// Handles session file backups.
pub struct SessionBackup {
    config: BackupConfig,
}

impl SessionBackup {
    pub fn new(max_backups: usize) -> Self {
        Self {
            config: BackupConfig {
                max_backups,
                ..Default::default()
            },
        }
    }
    
    pub fn with_config(config: BackupConfig) -> Self {
        Self { config }
    }
    
    /// Create a backup of the session file.
    pub async fn backup(&self, session_path: &Path) -> Result<PathBuf, BackupError> {
        // Verify source exists
        if !session_path.exists() {
            return Err(BackupError::SourceNotFound {
                path: session_path.to_path_buf(),
            });
        }
        
        // Generate backup path
        let backup_path = self.generate_backup_path(session_path);
        
        debug!(
            source = %session_path.display(),
            backup = %backup_path.display(),
            "Creating session backup"
        );
        
        // Copy file
        fs::copy(session_path, &backup_path).await?;
        
        // Verify if enabled
        if self.config.verify_backup {
            self.verify_backup(session_path, &backup_path).await?;
        }
        
        info!(backup = %backup_path.display(), "Session backup created");
        
        // Prune old backups
        if let Err(e) = self.prune_old_backups(session_path).await {
            warn!(error = %e, "Failed to prune old backups");
        }
        
        Ok(backup_path)
    }
    
    /// Generate backup filename with timestamp.
    fn generate_backup_path(&self, session_path: &Path) -> PathBuf {
        let timestamp = Local::now().format(&self.config.timestamp_format);
        let stem = session_path.file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("session");
        let extension = session_path.extension()
            .and_then(|s| s.to_str())
            .unwrap_or("md");
        
        let backup_filename = format!("{}-backup-{}.{}", stem, timestamp, extension);
        
        session_path.parent()
            .map(|p| p.join(&backup_filename))
            .unwrap_or_else(|| PathBuf::from(&backup_filename))
    }
    
    /// Verify backup matches source.
    async fn verify_backup(&self, source: &Path, backup: &Path) -> Result<(), BackupError> {
        let source_meta = fs::metadata(source).await?;
        let backup_meta = fs::metadata(backup).await?;
        
        if source_meta.len() != backup_meta.len() {
            return Err(BackupError::VerificationFailed {
                expected: source_meta.len(),
                actual: backup_meta.len(),
            });
        }
        
        debug!(
            size = source_meta.len(),
            "Backup verification passed"
        );
        
        Ok(())
    }
    
    /// Remove old backups exceeding max_backups limit.
    async fn prune_old_backups(&self, session_path: &Path) -> Result<usize, BackupError> {
        let dir = session_path.parent()
            .ok_or_else(|| BackupError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "No parent directory",
            )))?;
        
        let stem = session_path.file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("session");
        
        let pattern = format!("{}-backup-", stem);
        
        // Collect backup files
        let mut backups: Vec<(PathBuf, DateTime<Local>)> = Vec::new();
        
        let mut entries = fs::read_dir(dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if let Some(filename) = path.file_name().and_then(|s| s.to_str()) {
                if filename.starts_with(&pattern) {
                    if let Some(timestamp) = self.extract_timestamp(filename) {
                        backups.push((path, timestamp));
                    }
                }
            }
        }
        
        // Sort by timestamp (oldest first)
        backups.sort_by(|a, b| a.1.cmp(&b.1));
        
        // Remove oldest if exceeding limit
        let mut removed = 0;
        while backups.len() > self.config.max_backups {
            if let Some((path, _)) = backups.first() {
                debug!(path = %path.display(), "Pruning old backup");
                fs::remove_file(path).await?;
                backups.remove(0);
                removed += 1;
            }
        }
        
        if removed > 0 {
            info!(count = removed, "Pruned old backups");
        }
        
        Ok(removed)
    }
    
    /// Extract timestamp from backup filename.
    fn extract_timestamp(&self, filename: &str) -> Option<DateTime<Local>> {
        // Pattern: stem-backup-YYYYMMDD-HHMMSS.ext
        let parts: Vec<&str> = filename.split("-backup-").collect();
        if parts.len() != 2 {
            return None;
        }
        
        let timestamp_part = parts[1].split('.').next()?;
        
        // Parse YYYYMMDD-HHMMSS
        chrono::NaiveDateTime::parse_from_str(timestamp_part, &self.config.timestamp_format)
            .ok()
            .and_then(|naive| Local.from_local_datetime(&naive).single())
    }
    
    /// List all backups for a session file.
    pub async fn list_backups(&self, session_path: &Path) -> Result<Vec<PathBuf>, BackupError> {
        let dir = session_path.parent()
            .ok_or_else(|| BackupError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "No parent directory",
            )))?;
        
        let stem = session_path.file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("session");
        
        let pattern = format!("{}-backup-", stem);
        
        let mut backups = Vec::new();
        let mut entries = fs::read_dir(dir).await?;
        
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if let Some(filename) = path.file_name().and_then(|s| s.to_str()) {
                if filename.starts_with(&pattern) {
                    backups.push(path);
                }
            }
        }
        
        // Sort by name (which includes timestamp)
        backups.sort();
        
        Ok(backups)
    }
}

impl Default for SessionBackup {
    fn default() -> Self {
        Self::new(10)
    }
}
```

### Dependencies

Uses existing dependencies:
- `tokio` (already in Cargo.toml) - async runtime, fs operations
- `chrono` (already in Cargo.toml) - timestamp handling
- `tracing` (already in Cargo.toml) - structured logging
- `thiserror` (already in Cargo.toml) - error types

### Backup Filename Format

```
Original: session.md
Backup:   session-backup-20260205-143022.md

Original: my-workflow.md
Backup:   my-workflow-backup-20260205-143022.md
```

### Integration with NewSessionStrategy (Story 3.3)

```rust
// In NewSessionStrategy::execute
if self.config.enable_backup {
    match self.backup.backup(&ctx.session_path).await {
        Ok(backup_path) => {
            info!(backup = %backup_path.display(), "Session backed up");
        }
        Err(e) => {
            // Log error but don't fail - proceed with new session
            warn!(error = %e, "Session backup failed, proceeding anyway");
        }
    }
}
```

### Error Handling Strategy

| Error | Behavior |
|-------|----------|
| Source not found | Return error (caller decides) |
| Copy failed (disk full) | Log error, return error |
| Verification failed | Log warning, return error |
| Prune failed | Log warning, continue |

### Performance Considerations

- File copy is async to avoid blocking
- Verification is optional (configurable)
- Pruning runs after successful backup
- Large files may take time to copy

### Testing Strategy

**Unit Tests:**
- Test backup creation with temp files
- Test timestamp generation format
- Test pruning with multiple backups
- Test verification logic
- Test error handling

**Integration Tests:**
- Test backup + new session sequence
- Test concurrent backup operations
- Test with large files

### References

- [Source: _bmad-output/planning-artifacts/epics.md#Story 3.4: Session Backup Before New Session]
- [Source: _bmad-output/implementation-artifacts/3-3-new-session-resume-strategy.md]

## File List

**Files to create:**
- `src/resume/backup.rs`
- `tests/session_backup_test.rs`

**Files to modify:**
- `src/resume/mod.rs` (add backup module)
- `_bmad-output/implementation-artifacts/sprint-status.yaml`

## Change Log

- 2026-02-05: Story created and marked ready-for-dev
