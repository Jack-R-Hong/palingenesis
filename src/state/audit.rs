use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tracing::{debug, info, warn};

/// Configuration for audit logging.
#[derive(Debug, Clone)]
pub struct AuditConfig {
    /// Path to audit file.
    pub audit_path: PathBuf,
    /// Maximum file size before rotation (bytes).
    pub max_size: u64,
    /// Maximum number of rotated files to keep.
    pub max_files: usize,
    /// File permissions (Unix mode).
    #[cfg(unix)]
    pub file_mode: u32,
}

impl Default for AuditConfig {
    fn default() -> Self {
        Self {
            audit_path: PathBuf::from("audit.jsonl"),
            max_size: 10 * 1024 * 1024,
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
    ResumeStarted,
    ResumeCompleted,
    ResumeFailed,
    SessionCreated,
    SessionBackedUp,
    DaemonStarted,
    DaemonStopped,
    ConfigChanged,
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
    /// When the event occurred.
    pub timestamp: DateTime<Utc>,
    /// Type of event.
    pub event_type: AuditEventType,
    /// Session file path (if applicable).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_path: Option<PathBuf>,
    /// Stop reason (if applicable).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_reason: Option<String>,
    /// Action that was taken.
    pub action_taken: String,
    /// Outcome of the action.
    pub outcome: AuditOutcome,
    /// Additional metadata.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
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
#[derive(Debug, Clone, Default)]
pub struct AuditLogger {
    config: AuditConfig,
}

impl AuditLogger {
    pub fn new(state_dir: &Path) -> Self {
        Self {
            config: AuditConfig {
                audit_path: state_dir.join("audit.jsonl"),
                ..AuditConfig::default()
            },
        }
    }

    pub fn with_config(config: AuditConfig) -> Self {
        Self { config }
    }

    /// Log an audit entry.
    pub fn log(&self, entry: &AuditEntry) -> Result<(), AuditError> {
        self.maybe_rotate()?;

        let json =
            serde_json::to_string(entry).map_err(|e| AuditError::Serialization(e.to_string()))?;

        let mut file = self.open_for_append()?;
        file.lock_exclusive()?;
        writeln!(file, "{}", json)?;
        file.flush()?;
        FileExt::unlock(&file)?;

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

        for i in (1..self.config.max_files).rev() {
            let from = self.rotated_path(i);
            let to = self.rotated_path(i + 1);
            if from.exists() {
                std::fs::rename(&from, &to)?;
            }
        }

        let first_rotated = self.rotated_path(1);
        std::fs::rename(&self.config.audit_path, &first_rotated)?;

        let oldest = self.rotated_path(self.config.max_files + 1);
        if oldest.exists() {
            std::fs::remove_file(&oldest)?;
        }

        Ok(())
    }

    fn rotated_path(&self, index: usize) -> PathBuf {
        let mut path = self.config.audit_path.clone();
        let filename = path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("audit.jsonl");
        path.set_file_name(format!("{}.{}", filename, index));
        path
    }

    /// Query audit entries with filters.
    pub fn query(&self) -> AuditQuery {
        AuditQuery::new(&self.config.audit_path)
    }

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

    pub fn log_resume_failed(&self, session_path: &Path, error: &str) -> Result<(), AuditError> {
        let entry = AuditEntry::new(AuditEventType::ResumeFailed, "Resume failed")
            .with_session(session_path.to_path_buf())
            .with_outcome(AuditOutcome::Failure)
            .with_metadata("error", error);
        self.log(&entry)
    }

    pub fn log_session_created(&self, session_path: &Path) -> Result<(), AuditError> {
        let entry = AuditEntry::new(AuditEventType::SessionCreated, "Session created")
            .with_session(session_path.to_path_buf())
            .with_outcome(AuditOutcome::Success);
        self.log(&entry)
    }

    pub fn log_session_backed_up(&self, original: &Path, backup: &Path) -> Result<(), AuditError> {
        let entry = AuditEntry::new(AuditEventType::SessionBackedUp, "Session backed up")
            .with_session(original.to_path_buf())
            .with_outcome(AuditOutcome::Success)
            .with_metadata("backup_path", backup.display().to_string());
        self.log(&entry)
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

            match serde_json::from_str::<AuditEntry>(&line) {
                Ok(entry) => {
                    if self.matches(&entry) {
                        results.push(entry);
                    }
                }
                Err(err) => {
                    warn!(error = %err, "Skipping corrupted audit entry");
                }
            }
        }

        Ok(results)
    }

    fn matches(&self, entry: &AuditEntry) -> bool {
        if let Some(types) = &self.event_types {
            if !types.contains(&entry.event_type) {
                return false;
            }
        }

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
