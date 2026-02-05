use std::path::PathBuf;
use std::time::Duration;

use chrono::{DateTime, Utc};

use crate::monitor::classifier::StopReason;
use crate::monitor::session::Session;

/// Context provided to resume strategies.
#[derive(Debug, Clone)]
pub struct ResumeContext {
    /// Path to the session file.
    pub session_path: PathBuf,
    /// Classified stop reason.
    pub stop_reason: StopReason,
    /// Retry-After duration from rate limit response.
    pub retry_after: Option<Duration>,
    /// Parsed session metadata.
    pub session_metadata: Option<Session>,
    /// Current attempt number (1-indexed).
    pub attempt_number: u32,
    /// When the stop was detected.
    pub timestamp: DateTime<Utc>,
}

impl ResumeContext {
    pub fn new(session_path: PathBuf, stop_reason: StopReason) -> Self {
        Self {
            session_path,
            stop_reason,
            retry_after: None,
            session_metadata: None,
            attempt_number: 1,
            timestamp: Utc::now(),
        }
    }

    pub fn with_retry_after(mut self, duration: Duration) -> Self {
        self.retry_after = Some(duration);
        self
    }

    pub fn with_session(mut self, session: Session) -> Self {
        self.session_metadata = Some(session);
        self
    }

    pub fn increment_attempt(&mut self) {
        self.attempt_number = self.attempt_number.saturating_add(1);
    }
}
