use std::time::Duration;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum NotifyError {
    #[error("Notification send failed: {message}")]
    SendFailed { message: String },
    #[error("Notification timed out after {duration:?}")]
    Timeout { duration: Duration },
    #[error("Notification configuration error: {message}")]
    ConfigError { message: String },
}
