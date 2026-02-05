use std::time::Duration;

use async_trait::async_trait;
use reqwest::Client;
use serde::Serialize;
use tracing::debug;

use crate::config::schema::SlackConfig;
use crate::notify::channel::NotificationChannel;
use crate::notify::error::NotifyError;
use crate::notify::events::{EventSeverity, NotificationEvent};

const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

pub struct SlackChannel {
    webhook_url: String,
    client: Client,
    enabled: bool,
}

impl SlackChannel {
    pub fn new(config: &SlackConfig) -> Self {
        let client = Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .build()
            .unwrap_or_else(|err| {
                tracing::warn!(error = %err, "Failed to build slack client; using defaults");
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
impl NotificationChannel for SlackChannel {
    fn name(&self) -> &'static str {
        "slack"
    }

    async fn send(&self, event: &NotificationEvent) -> Result<(), NotifyError> {
        let message = format_event_message(event);
        let title = format!(
            "{} {}",
            severity_emoji(event.severity()),
            event_title(event)
        );
        let payload = SlackWebhookPayload {
            blocks: vec![
                SlackBlock::Header {
                    text: SlackText {
                        text_type: "plain_text",
                        text: title,
                    },
                },
                SlackBlock::Section {
                    fields: event_fields(event),
                },
            ],
        };

        let response = self
            .client
            .post(&self.webhook_url)
            .json(&payload)
            .send()
            .await
            .map_err(|err| NotifyError::SendFailed {
                message: format!("slack request error: {err}"),
            })?;

        if !response.status().is_success() {
            return Err(NotifyError::SendFailed {
                message: format!("slack returned status {}", response.status()),
            });
        }

        debug!(
            channel = self.name(),
            event_type = event.event_type(),
            message = %message,
            "Slack notification sent"
        );
        Ok(())
    }

    fn is_enabled(&self) -> bool {
        self.enabled
    }
}

#[derive(Debug, Serialize)]
struct SlackWebhookPayload {
    blocks: Vec<SlackBlock>,
}

#[derive(Debug, Serialize)]
#[serde(tag = "type")]
enum SlackBlock {
    #[serde(rename = "header")]
    Header { text: SlackText },
    #[serde(rename = "section")]
    Section { fields: Vec<SlackText> },
}

#[derive(Debug, Serialize)]
struct SlackText {
    #[serde(rename = "type")]
    text_type: &'static str,
    text: String,
}

fn severity_emoji(severity: EventSeverity) -> &'static str {
    match severity {
        EventSeverity::Info => "ℹ️",
        EventSeverity::Warning => "⚠️",
        EventSeverity::Error => "❌",
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

fn event_fields(event: &NotificationEvent) -> Vec<SlackText> {
    match event {
        NotificationEvent::SessionStopped {
            session_path,
            stop_reason,
            details,
            ..
        } => {
            let mut fields = vec![
                SlackText {
                    text_type: "mrkdwn",
                    text: format!("*Session:*\n{}", session_path.display()),
                },
                SlackText {
                    text_type: "mrkdwn",
                    text: format!("*Reason:*\n{stop_reason}"),
                },
            ];
            if let Some(details) = details {
                fields.push(SlackText {
                    text_type: "mrkdwn",
                    text: format!("*Details:*\n{details}"),
                });
            }
            fields
        }
        NotificationEvent::ResumeAttempted {
            session_path,
            strategy,
            ..
        } => vec![
            SlackText {
                text_type: "mrkdwn",
                text: format!("*Session:*\n{}", session_path.display()),
            },
            SlackText {
                text_type: "mrkdwn",
                text: format!("*Strategy:*\n{strategy}"),
            },
        ],
        NotificationEvent::ResumeSucceeded {
            session_path,
            strategy,
            wait_time_secs,
            ..
        } => vec![
            SlackText {
                text_type: "mrkdwn",
                text: format!("*Session:*\n{}", session_path.display()),
            },
            SlackText {
                text_type: "mrkdwn",
                text: format!("*Strategy:*\n{strategy}"),
            },
            SlackText {
                text_type: "mrkdwn",
                text: format!("*Wait time:*\n{wait_time_secs}s"),
            },
        ],
        NotificationEvent::ResumeFailed {
            session_path,
            strategy,
            error,
            ..
        } => vec![
            SlackText {
                text_type: "mrkdwn",
                text: format!("*Session:*\n{}", session_path.display()),
            },
            SlackText {
                text_type: "mrkdwn",
                text: format!("*Strategy:*\n{strategy}"),
            },
            SlackText {
                text_type: "mrkdwn",
                text: format!("*Error:*\n{error}"),
            },
        ],
        NotificationEvent::DaemonStarted { version, .. } => vec![SlackText {
            text_type: "mrkdwn",
            text: format!("*Version:*\n{version}"),
        }],
        NotificationEvent::DaemonStopped { reason, .. } => vec![SlackText {
            text_type: "mrkdwn",
            text: format!("*Reason:*\n{reason}"),
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
    fn formats_daemon_started_message() {
        let timestamp = chrono::Utc
            .with_ymd_and_hms(2025, 1, 2, 3, 4, 5)
            .single()
            .expect("valid timestamp");
        let event = NotificationEvent::DaemonStarted {
            timestamp,
            version: "0.1.0".to_string(),
        };

        let message = format_event_message(&event);

        assert!(message.contains("Daemon started at 2025-01-02T03:04:05+00:00"));
        assert!(message.contains("Version: 0.1.0"));

        let fields = event_fields(&NotificationEvent::ResumeAttempted {
            timestamp,
            session_path: PathBuf::from("/tmp/session"),
            strategy: "same_session".to_string(),
        });
        assert_eq!(fields.len(), 2);
    }
}
