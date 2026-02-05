use std::path::PathBuf;
use std::time::Duration;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum ResumeError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Session not found: {path}")]
    SessionNotFound { path: PathBuf },

    #[error("Command execution failed: {command}")]
    CommandFailed { command: String, stderr: String },

    #[error("Operation timed out after {duration:?}")]
    Timeout { duration: Duration },

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Retry limit exceeded after {attempts} attempts")]
    RetryExceeded { attempts: u32 },
}
