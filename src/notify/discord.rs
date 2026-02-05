use std::time::Duration;

use async_trait::async_trait;
use reqwest::Client;
use serde::Serialize;
use tracing::debug;

use crate::config::schema::DiscordConfig;
use crate::notify::channel::NotificationChannel;
use crate::notify::error::NotifyError;
use crate::notify::events::{EventSeverity, NotificationEvent};

const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

pub struct DiscordChannel {
    webhook_url: String,
    client: Client,
    enabled: bool,
}

impl DiscordChannel {
    pub fn new(config: &DiscordConfig) -> Self {
        let client = Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .build()
            .unwrap_or_else(|err| {
                tracing::warn!(error = %err, "Failed to build discord client; using defaults");
                Client::new()
            });

        Self {
            webhook_url: config.webhook_url.clone(),
            client,
            enabled: true,
        }
    }
}

#[async_trait]
impl NotificationChannel for DiscordChannel {
    fn name(&self) -> &'static str {
        "discord"
    }

    async fn send(&self, event: &NotificationEvent) -> Result<(), NotifyError> {
        let payload = DiscordWebhookPayload {
            embeds: vec![DiscordEmbed {
                title: event_title(event).to_string(),
                description: format_event_message(event),
                color: severity_color(event.severity()),
                timestamp: event_timestamp(event).to_rfc3339(),
                fields: event_fields(event),
            }],
        };

        let response = self
            .client
            .post(&self.webhook_url)
            .json(&payload)
            .send()
            .await
            .map_err(|err| NotifyError::SendFailed {
                message: format!("discord request error: {err}"),
            })?;

        if !response.status().is_success() {
            return Err(NotifyError::SendFailed {
                message: format!("discord returned status {}", response.status()),
            });
        }

        debug!(
            channel = self.name(),
            event_type = event.event_type(),
            "Discord notification sent"
        );
        Ok(())
    }

    fn is_enabled(&self) -> bool {
        self.enabled
    }
}

#[derive(Debug, Serialize)]
struct DiscordWebhookPayload {
    embeds: Vec<DiscordEmbed>,
}

#[derive(Debug, Serialize)]
struct DiscordEmbed {
    title: String,
    description: String,
    color: u32,
    timestamp: String,
    fields: Vec<DiscordEmbedField>,
}

#[derive(Debug, Serialize)]
struct DiscordEmbedField {
    name: String,
    value: String,
    inline: bool,
}

fn severity_color(severity: EventSeverity) -> u32 {
    match severity {
        EventSeverity::Info => 0x00FF00,
        EventSeverity::Warning => 0xFFFF00,
        EventSeverity::Error => 0xFF0000,
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

fn event_timestamp(event: &NotificationEvent) -> chrono::DateTime<chrono::Utc> {
    match event {
        NotificationEvent::SessionStopped { timestamp, .. } => *timestamp,
        NotificationEvent::ResumeAttempted { timestamp, .. } => *timestamp,
        NotificationEvent::ResumeSucceeded { timestamp, .. } => *timestamp,
        NotificationEvent::ResumeFailed { timestamp, .. } => *timestamp,
        NotificationEvent::DaemonStarted { timestamp, .. } => *timestamp,
        NotificationEvent::DaemonStopped { timestamp, .. } => *timestamp,
    }
}

fn event_fields(event: &NotificationEvent) -> Vec<DiscordEmbedField> {
    match event {
        NotificationEvent::SessionStopped {
            session_path,
            stop_reason,
            details,
            ..
        } => {
            let mut fields = vec![
                DiscordEmbedField {
                    name: "Session".to_string(),
                    value: session_path.display().to_string(),
                    inline: true,
                },
                DiscordEmbedField {
                    name: "Reason".to_string(),
                    value: stop_reason.clone(),
                    inline: true,
                },
            ];
            if let Some(details) = details {
                fields.push(DiscordEmbedField {
                    name: "Details".to_string(),
                    value: details.clone(),
                    inline: false,
                });
            }
            fields
        }
        NotificationEvent::ResumeAttempted {
            session_path,
            strategy,
            ..
        } => vec![
            DiscordEmbedField {
                name: "Session".to_string(),
                value: session_path.display().to_string(),
                inline: true,
            },
            DiscordEmbedField {
                name: "Strategy".to_string(),
                value: strategy.clone(),
                inline: true,
            },
        ],
        NotificationEvent::ResumeSucceeded {
            session_path,
            strategy,
            wait_time_secs,
            ..
        } => vec![
            DiscordEmbedField {
                name: "Session".to_string(),
                value: session_path.display().to_string(),
                inline: true,
            },
            DiscordEmbedField {
                name: "Strategy".to_string(),
                value: strategy.clone(),
                inline: true,
            },
            DiscordEmbedField {
                name: "Wait time".to_string(),
                value: format!("{wait_time_secs}s"),
                inline: true,
            },
        ],
        NotificationEvent::ResumeFailed {
            session_path,
            strategy,
            error,
            ..
        } => vec![
            DiscordEmbedField {
                name: "Session".to_string(),
                value: session_path.display().to_string(),
                inline: true,
            },
            DiscordEmbedField {
                name: "Strategy".to_string(),
                value: strategy.clone(),
                inline: true,
            },
            DiscordEmbedField {
                name: "Error".to_string(),
                value: error.clone(),
                inline: false,
            },
        ],
        NotificationEvent::DaemonStarted { version, .. } => vec![DiscordEmbedField {
            name: "Version".to_string(),
            value: version.clone(),
            inline: true,
        }],
        NotificationEvent::DaemonStopped { reason, .. } => vec![DiscordEmbedField {
            name: "Reason".to_string(),
            value: reason.clone(),
            inline: true,
        }],
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
    fn formats_resume_succeeded_message() {
        let timestamp = chrono::Utc
            .with_ymd_and_hms(2025, 1, 2, 3, 4, 5)
            .single()
            .expect("valid timestamp");
        let event = NotificationEvent::ResumeSucceeded {
            timestamp,
            session_path: PathBuf::from("/tmp/session"),
            strategy: "same_session".to_string(),
            wait_time_secs: 120,
        };

        let message = format_event_message(&event);

        assert!(message.contains("Resume succeeded at 2025-01-02T03:04:05+00:00"));
        assert!(message.contains("Session: /tmp/session"));
        assert!(message.contains("Strategy: same_session"));
        assert!(message.contains("Wait time: 120s"));
    }
}
