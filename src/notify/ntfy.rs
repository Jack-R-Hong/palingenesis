use std::time::Duration;

use async_trait::async_trait;
use reqwest::Client;
use tracing::debug;

use crate::config::schema::NtfyConfig;
use crate::notify::channel::NotificationChannel;
use crate::notify::error::NotifyError;
use crate::notify::events::{EventSeverity, NotificationEvent};

const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

pub struct NtfyChannel {
    topic: String,
    server: String,
    priority: Option<String>,
    client: Client,
    enabled: bool,
}

impl NtfyChannel {
    pub fn new(config: &NtfyConfig) -> Self {
        let client = Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .build()
            .unwrap_or_else(|err| {
                tracing::warn!(error = %err, "Failed to build ntfy client; using defaults");
                Client::new()
            });

        Self {
            topic: config.topic.clone(),
            server: config
                .server
                .clone()
                .unwrap_or_else(|| "https://ntfy.sh".to_string()),
            priority: config.priority.clone(),
            client,
            enabled: true,
        }
    }
}

#[async_trait]
impl NotificationChannel for NtfyChannel {
    fn name(&self) -> &'static str {
        "ntfy"
    }

    async fn send(&self, event: &NotificationEvent) -> Result<(), NotifyError> {
        let url = format!(
            "{}/{}",
            self.server.trim_end_matches('/'),
            self.topic.trim_start_matches('/')
        );
        let title = event_title(event);
        let message = format_event_message(event);
        let tags = severity_tag(event.severity());

        let mut request = self
            .client
            .post(url)
            .header("Title", title)
            .header("Tags", tags)
            .body(message);

        if let Some(priority) = &self.priority {
            request = request.header("Priority", priority);
        }

        let response = request
            .send()
            .await
            .map_err(|err| NotifyError::SendFailed {
                message: format!("ntfy request error: {err}"),
            })?;

        if !response.status().is_success() {
            return Err(NotifyError::SendFailed {
                message: format!("ntfy returned status {}", response.status()),
            });
        }

        debug!(
            channel = self.name(),
            event_type = event.event_type(),
            "ntfy notification sent"
        );
        Ok(())
    }

    fn is_enabled(&self) -> bool {
        self.enabled
    }
}

fn severity_tag(severity: EventSeverity) -> &'static str {
    match severity {
        EventSeverity::Info => "ℹ️",
        EventSeverity::Warning => "⚠️",
        EventSeverity::Error => "🔴",
    }
}

fn event_title(event: &NotificationEvent) -> &'static str {
    match event {
        NotificationEvent::SessionStopped { .. } => "Session stopped",
        NotificationEvent::ResumeAttempted { .. } => "Resume attempted",
        NotificationEvent::ResumeSucceeded { .. } => "Resume succeeded",
        NotificationEvent::ResumeFailed { .. } => "Resume failed",
        NotificationEvent::DaemonStarted { .. } => "Daemon started",
        NotificationEvent::DaemonStopped { .. } => "Daemon stopped",
    }
}

fn format_event_message(event: &NotificationEvent) -> String {
    match event {
        NotificationEvent::SessionStopped {
            timestamp,
            session_path,
            stop_reason,
            details,
        } => {
            let mut message = format!(
                "Session stopped at {}.\nSession: {}\nReason: {}",
                timestamp.to_rfc3339(),
                session_path.display(),
                stop_reason
            );
            if let Some(details) = details {
                message.push_str(&format!("\nDetails: {details}"));
            }
            message
        }
        NotificationEvent::ResumeAttempted {
            timestamp,
            session_path,
            strategy,
        } => format!(
            "Resume attempted at {}.\nSession: {}\nStrategy: {}",
            timestamp.to_rfc3339(),
            session_path.display(),
            strategy
        ),
        NotificationEvent::ResumeSucceeded {
            timestamp,
            session_path,
            strategy,
            wait_time_secs,
        } => format!(
            "Resume succeeded at {}.\nSession: {}\nStrategy: {}\nWait time: {}s",
            timestamp.to_rfc3339(),
            session_path.display(),
            strategy,
            wait_time_secs
        ),
        NotificationEvent::ResumeFailed {
            timestamp,
            session_path,
            strategy,
            error,
        } => format!(
            "Resume failed at {}.\nSession: {}\nStrategy: {}\nError: {}",
            timestamp.to_rfc3339(),
            session_path.display(),
            strategy,
            error
        ),
        NotificationEvent::DaemonStarted { timestamp, version } => format!(
            "Daemon started at {}.\nVersion: {}",
            timestamp.to_rfc3339(),
            version
        ),
        NotificationEvent::DaemonStopped { timestamp, reason } => format!(
            "Daemon stopped at {}.\nReason: {}",
            timestamp.to_rfc3339(),
            reason
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use std::path::PathBuf;

    #[test]
    fn formats_resume_failed_message() {
        let timestamp = chrono::Utc
            .with_ymd_and_hms(2025, 1, 2, 3, 4, 5)
            .single()
            .expect("valid timestamp");
        let event = NotificationEvent::ResumeFailed {
            timestamp,
            session_path: PathBuf::from("/tmp/session"),
            strategy: "same_session".to_string(),
            error: "timeout".to_string(),
        };

        let message = format_event_message(&event);

        assert!(message.contains("Resume failed at 2025-01-02T03:04:05+00:00"));
        assert!(message.contains("Session: /tmp/session"));
        assert!(message.contains("Strategy: same_session"));
        assert!(message.contains("Error: timeout"));
    }
}
