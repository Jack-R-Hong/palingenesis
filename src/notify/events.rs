use std::path::PathBuf;

use chrono::{DateTime, Utc};
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EventSeverity {
    Info,
    Warning,
    Error,
}

/// Events emitted by the notification system.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum NotificationEvent {
    SessionStopped {
        timestamp: DateTime<Utc>,
        session_path: PathBuf,
        stop_reason: String,
        details: Option<String>,
    },
    ResumeAttempted {
        timestamp: DateTime<Utc>,
        session_path: PathBuf,
        strategy: String,
    },
    ResumeSucceeded {
        timestamp: DateTime<Utc>,
        session_path: PathBuf,
        strategy: String,
        wait_time_secs: u64,
    },
    ResumeFailed {
        timestamp: DateTime<Utc>,
        session_path: PathBuf,
        strategy: String,
        error: String,
    },
    DaemonStarted {
        timestamp: DateTime<Utc>,
        version: String,
    },
    DaemonStopped {
        timestamp: DateTime<Utc>,
        reason: String,
    },
}

impl NotificationEvent {
    pub fn timestamp(&self) -> DateTime<Utc> {
        match self {
            Self::SessionStopped { timestamp, .. } => *timestamp,
            Self::ResumeAttempted { timestamp, .. } => *timestamp,
            Self::ResumeSucceeded { timestamp, .. } => *timestamp,
            Self::ResumeFailed { timestamp, .. } => *timestamp,
            Self::DaemonStarted { timestamp, .. } => *timestamp,
            Self::DaemonStopped { timestamp, .. } => *timestamp,
        }
    }

    pub fn event_type(&self) -> &'static str {
        match self {
            Self::SessionStopped { .. } => "session_stopped",
            Self::ResumeAttempted { .. } => "resume_attempted",
            Self::ResumeSucceeded { .. } => "resume_succeeded",
            Self::ResumeFailed { .. } => "resume_failed",
            Self::DaemonStarted { .. } => "daemon_started",
            Self::DaemonStopped { .. } => "daemon_stopped",
        }
    }

    pub fn severity(&self) -> EventSeverity {
        match self {
            Self::SessionStopped { .. } => EventSeverity::Warning,
            Self::ResumeAttempted { .. } => EventSeverity::Info,
            Self::ResumeSucceeded { .. } => EventSeverity::Info,
            Self::ResumeFailed { .. } => EventSeverity::Error,
            Self::DaemonStarted { .. } => EventSeverity::Info,
            Self::DaemonStopped { .. } => EventSeverity::Warning,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use serde_json::json;

    fn timestamp() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2025, 1, 2, 3, 4, 5)
            .single()
            .expect("valid timestamp")
    }

    #[test]
    fn event_type_matches_variant() {
        let ts = timestamp();
        let session_path = PathBuf::from("/tmp/session");

        let cases = vec![
            (
                NotificationEvent::SessionStopped {
                    timestamp: ts,
                    session_path: session_path.clone(),
                    stop_reason: "rate_limit".to_string(),
                    details: None,
                },
                "session_stopped",
                EventSeverity::Warning,
            ),
            (
                NotificationEvent::ResumeAttempted {
                    timestamp: ts,
                    session_path: session_path.clone(),
                    strategy: "same_session".to_string(),
                },
                "resume_attempted",
                EventSeverity::Info,
            ),
            (
                NotificationEvent::ResumeSucceeded {
                    timestamp: ts,
                    session_path: session_path.clone(),
                    strategy: "same_session".to_string(),
                    wait_time_secs: 42,
                },
                "resume_succeeded",
                EventSeverity::Info,
            ),
            (
                NotificationEvent::ResumeFailed {
                    timestamp: ts,
                    session_path: session_path.clone(),
                    strategy: "same_session".to_string(),
                    error: "boom".to_string(),
                },
                "resume_failed",
                EventSeverity::Error,
            ),
            (
                NotificationEvent::DaemonStarted {
                    timestamp: ts,
                    version: "0.1.0".to_string(),
                },
                "daemon_started",
                EventSeverity::Info,
            ),
            (
                NotificationEvent::DaemonStopped {
                    timestamp: ts,
                    reason: "signal".to_string(),
                },
                "daemon_stopped",
                EventSeverity::Warning,
            ),
        ];

        for (event, event_type, severity) in cases {
            assert_eq!(event.event_type(), event_type);
            assert_eq!(event.severity(), severity);
        }
    }

    #[test]
    fn serializes_session_stopped() {
        let event = NotificationEvent::SessionStopped {
            timestamp: timestamp(),
            session_path: PathBuf::from("/tmp/session"),
            stop_reason: "rate_limit".to_string(),
            details: None,
        };

        let value = serde_json::to_value(&event).expect("serialize event");
        let expected = json!({
            "event": "session_stopped",
            "timestamp": "2025-01-02T03:04:05Z",
            "session_path": "/tmp/session",
            "stop_reason": "rate_limit",
            "details": null
        });

        assert_eq!(value, expected);
    }

    #[test]
    fn serializes_resume_succeeded() {
        let event = NotificationEvent::ResumeSucceeded {
            timestamp: timestamp(),
            session_path: PathBuf::from("/tmp/session"),
            strategy: "same_session".to_string(),
            wait_time_secs: 120,
        };

        let value = serde_json::to_value(&event).expect("serialize event");
        let expected = json!({
            "event": "resume_succeeded",
            "timestamp": "2025-01-02T03:04:05Z",
            "session_path": "/tmp/session",
            "strategy": "same_session",
            "wait_time_secs": 120
        });

        assert_eq!(value, expected);
    }
}
