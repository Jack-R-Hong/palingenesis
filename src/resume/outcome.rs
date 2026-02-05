use std::path::PathBuf;
use std::time::Duration;

/// Outcome of a resume strategy execution.
#[derive(Debug, Clone)]
pub enum ResumeOutcome {
    /// Resume succeeded.
    Success {
        /// Session that was resumed/created.
        session_path: PathBuf,
        /// Description of action taken.
        action: String,
    },
    /// Resume failed.
    Failure {
        /// Error message.
        message: String,
        /// Whether retry is possible.
        retryable: bool,
    },
    /// Resume skipped intentionally.
    Skipped {
        /// Reason for skipping.
        reason: String,
    },
    /// Resume delayed for later.
    Delayed {
        /// When to retry.
        next_attempt: Duration,
        /// Reason for delay.
        reason: String,
    },
}

impl ResumeOutcome {
    pub fn success(session_path: PathBuf, action: impl Into<String>) -> Self {
        Self::Success {
            session_path,
            action: action.into(),
        }
    }

    pub fn failure(message: impl Into<String>, retryable: bool) -> Self {
        Self::Failure {
            message: message.into(),
            retryable,
        }
    }

    pub fn skipped(reason: impl Into<String>) -> Self {
        Self::Skipped {
            reason: reason.into(),
        }
    }

    pub fn delayed(next_attempt: Duration, reason: impl Into<String>) -> Self {
        Self::Delayed {
            next_attempt,
            reason: reason.into(),
        }
    }

    pub fn is_success(&self) -> bool {
        matches!(self, Self::Success { .. })
    }

    pub fn should_retry(&self) -> bool {
        matches!(
            self,
            Self::Failure {
                retryable: true,
                ..
            } | Self::Delayed { .. }
        )
    }
}
