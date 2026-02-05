use std::path::{Path, PathBuf};

use async_trait::async_trait;
use chrono::{DateTime, Local, NaiveDateTime, TimeZone};
use thiserror::Error;
use tokio::fs;
use tokio::io::AsyncReadExt;
use tracing::{debug, info, warn};

/// Configuration for session backup.
#[derive(Debug, Clone)]
pub struct BackupConfig {
    /// Maximum number of backups to keep.
    pub max_backups: usize,
    /// Timestamp format for backup filenames.
    pub timestamp_format: String,
    /// Verify backup after creation.
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
#[derive(Debug, Error)]
pub enum BackupError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Source file not found: {path}")]
    SourceNotFound { path: PathBuf },

    #[error("Backup verification failed: expected {expected} bytes, got {actual}")]
    VerificationFailed { expected: u64, actual: u64 },

    #[error("Backup file not readable: {path}")]
    Unreadable { path: PathBuf },

    #[error("Failed to parse backup timestamp from filename: {filename}")]
    InvalidBackupFilename { filename: String },
}

#[async_trait]
pub trait BackupHandler: Send + Sync {
    async fn backup(&self, session_path: &Path) -> Result<PathBuf, BackupError>;
}

/// Handles session file backups.
#[derive(Debug, Clone)]
pub struct SessionBackup {
    config: BackupConfig,
}

impl SessionBackup {
    pub fn new(max_backups: usize) -> Self {
        Self {
            config: BackupConfig {
                max_backups,
                ..BackupConfig::default()
            },
        }
    }

    pub fn with_config(config: BackupConfig) -> Self {
        Self { config }
    }

    /// Create a backup of the session file.
    pub async fn create_backup(&self, session_path: &Path) -> Result<PathBuf, BackupError> {
        if !session_path.exists() {
            return Err(BackupError::SourceNotFound {
                path: session_path.to_path_buf(),
            });
        }

        let backup_path = self.generate_backup_path(session_path);

        debug!(
            source = %session_path.display(),
            backup = %backup_path.display(),
            "Creating session backup"
        );

        fs::copy(session_path, &backup_path).await?;
        self.copy_metadata(session_path, &backup_path).await;

        if self.config.verify_backup {
            self.verify_backup(session_path, &backup_path).await?;
        }

        info!(backup = %backup_path.display(), "Session backup created");

        if let Err(err) = self.prune_old_backups(session_path).await {
            warn!(error = %err, "Failed to prune old backups");
        }

        Ok(backup_path)
    }

    fn generate_backup_path(&self, session_path: &Path) -> PathBuf {
        let timestamp = Local::now()
            .format(&self.config.timestamp_format)
            .to_string();
        let stem = session_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("session");
        let extension = session_path.extension().and_then(|s| s.to_str());
        let backup_filename = match extension {
            Some(ext) => format!("{}-backup-{}.{}", stem, timestamp, ext),
            None => format!("{}-backup-{}", stem, timestamp),
        };

        session_path
            .parent()
            .map(|p| p.join(&backup_filename))
            .unwrap_or_else(|| PathBuf::from(&backup_filename))
    }

    async fn copy_metadata(&self, source: &Path, backup: &Path) {
        let permissions = match fs::metadata(source).await {
            Ok(meta) => meta.permissions(),
            Err(err) => {
                warn!(error = %err, "Failed to read source metadata");
                return;
            }
        };

        if let Err(err) = fs::set_permissions(backup, permissions).await {
            warn!(error = %err, "Failed to set backup permissions");
        }
    }

    pub(crate) async fn verify_backup(
        &self,
        source: &Path,
        backup: &Path,
    ) -> Result<(), BackupError> {
        let source_meta = fs::metadata(source).await?;
        let backup_meta = fs::metadata(backup).await?;

        if source_meta.len() != backup_meta.len() {
            return Err(BackupError::VerificationFailed {
                expected: source_meta.len(),
                actual: backup_meta.len(),
            });
        }

        let mut file = fs::File::open(backup).await?;
        let mut buffer = [0u8; 1];
        let _ = file.read(&mut buffer).await?;

        debug!(size = source_meta.len(), "Backup verification passed");

        Ok(())
    }

    pub(crate) async fn prune_old_backups(
        &self,
        session_path: &Path,
    ) -> Result<usize, BackupError> {
        let dir = session_path.parent().ok_or_else(|| {
            BackupError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "No parent directory",
            ))
        })?;

        let stem = session_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("session");
        let pattern = format!("{}-backup-", stem);

        let mut backups: Vec<(PathBuf, DateTime<Local>)> = Vec::new();
        let mut entries = fs::read_dir(dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if let Some(filename) = path.file_name().and_then(|s| s.to_str()) {
                if filename.starts_with(&pattern) {
                    match self.extract_timestamp(filename) {
                        Ok(timestamp) => backups.push((path, timestamp)),
                        Err(err) => warn!(error = %err, "Skipping backup with invalid timestamp"),
                    }
                }
            }
        }

        backups.sort_by(|a, b| a.1.cmp(&b.1));

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

    fn extract_timestamp(&self, filename: &str) -> Result<DateTime<Local>, BackupError> {
        let parts: Vec<&str> = filename.split("-backup-").collect();
        if parts.len() != 2 {
            return Err(BackupError::InvalidBackupFilename {
                filename: filename.to_string(),
            });
        }

        let timestamp_part =
            parts[1]
                .split('.')
                .next()
                .ok_or_else(|| BackupError::InvalidBackupFilename {
                    filename: filename.to_string(),
                })?;

        let naive = NaiveDateTime::parse_from_str(timestamp_part, &self.config.timestamp_format)
            .map_err(|_| BackupError::InvalidBackupFilename {
                filename: filename.to_string(),
            })?;

        Local.from_local_datetime(&naive).single().ok_or_else(|| {
            BackupError::InvalidBackupFilename {
                filename: filename.to_string(),
            }
        })
    }
}

#[async_trait]
impl BackupHandler for SessionBackup {
    async fn backup(&self, session_path: &Path) -> Result<PathBuf, BackupError> {
        self.create_backup(session_path).await
    }
}

impl Default for SessionBackup {
    fn default() -> Self {
        Self::new(10)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn verify_backup_detects_size_mismatch() {
        let temp = tempfile::tempdir().expect("tempdir");
        let source = temp.path().join("session.md");
        let backup = temp.path().join("session-backup-20260205-143022.md");

        fs::write(&source, b"hello").await.expect("source write");
        fs::write(&backup, b"hello world")
            .await
            .expect("backup write");

        let backupper = SessionBackup::default();
        let err = backupper
            .verify_backup(&source, &backup)
            .await
            .expect_err("expected verification failure");

        assert!(matches!(err, BackupError::VerificationFailed { .. }));
    }

    #[tokio::test]
    async fn prune_removes_oldest_backups() {
        let temp = tempfile::tempdir().expect("tempdir");
        let session = temp.path().join("session.md");
        fs::write(&session, b"session")
            .await
            .expect("session write");

        let timestamps = ["20240101-000000", "20240102-000000", "20240103-000000"];

        for ts in timestamps {
            let backup = temp.path().join(format!("session-backup-{}.md", ts));
            fs::write(&backup, b"backup").await.expect("backup write");
        }

        let backupper = SessionBackup::with_config(BackupConfig {
            max_backups: 2,
            ..BackupConfig::default()
        });

        let removed = backupper
            .prune_old_backups(&session)
            .await
            .expect("prune backups");

        assert_eq!(removed, 1);
        assert!(
            !temp
                .path()
                .join("session-backup-20240101-000000.md")
                .exists()
        );
        assert!(
            temp.path()
                .join("session-backup-20240102-000000.md")
                .exists()
        );
    }

    #[test]
    fn extract_timestamp_parses_format() {
        let backupper = SessionBackup::default();
        let timestamp = backupper
            .extract_timestamp("session-backup-20260205-143022.md")
            .expect("timestamp parse");
        assert_eq!(
            timestamp.format("%Y%m%d-%H%M%S").to_string(),
            "20260205-143022"
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn backup_fails_when_directory_unwritable() {
        use std::fs::Permissions;
        use std::os::unix::fs::PermissionsExt;

        let temp = tempfile::tempdir().expect("tempdir");
        let session = temp.path().join("session.md");
        fs::write(&session, b"session")
            .await
            .expect("session write");

        let permissions = Permissions::from_mode(0o500);
        std::fs::set_permissions(temp.path(), permissions).expect("set permissions");

        let backupper = SessionBackup::default();
        let result = backupper.create_backup(&session).await;

        assert!(matches!(result, Err(BackupError::Io(_))));

        let reset = Permissions::from_mode(0o700);
        std::fs::set_permissions(temp.path(), reset).expect("reset permissions");
    }
}
