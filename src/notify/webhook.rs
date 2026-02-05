use std::collections::HashMap;
use std::time::Duration;

use async_trait::async_trait;
use reqwest::Client;
use reqwest::header::{HeaderName, HeaderValue};
use tokio::time::sleep;
use tracing::{debug, warn};

use crate::config::schema::WebhookConfig;
use crate::notify::channel::NotificationChannel;
use crate::notify::error::NotifyError;
use crate::notify::events::NotificationEvent;

const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);
const MAX_RETRIES: usize = 3;
const BACKOFF_DELAYS: [Duration; MAX_RETRIES] = [
    Duration::from_secs(1),
    Duration::from_secs(2),
    Duration::from_secs(4),
];

pub struct WebhookChannel {
    url: String,
    headers: Option<HashMap<String, String>>,
    client: Client,
    enabled: bool,
}

impl WebhookChannel {
    pub fn new(config: &WebhookConfig) -> Self {
        let client = Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .build()
            .unwrap_or_else(|err| {
                warn!(error = %err, "Failed to build webhook client; using defaults");
                Client::new()
            });

        Self {
            url: config.url.clone(),
            headers: config.headers.clone(),
            client,
            enabled: true,
        }
    }
}

#[async_trait]
impl NotificationChannel for WebhookChannel {
    fn name(&self) -> &'static str {
        "webhook"
    }

    async fn send(&self, event: &NotificationEvent) -> Result<(), NotifyError> {
        let message = format_event_message(event);
        let mut last_error = match send_once(self, event).await {
            Ok(()) => {
                debug!(
                    channel = self.name(),
                    event_type = event.event_type(),
                    "Webhook notification sent"
                );
                return Ok(());
            }
            Err(err) => err,
        };

        for (attempt, delay) in BACKOFF_DELAYS.iter().enumerate() {
            warn!(
                channel = self.name(),
                event_type = event.event_type(),
                attempt = attempt + 1,
                delay_secs = delay.as_secs(),
                message = %message,
                "Webhook send failed; retrying"
            );
            sleep(*delay).await;
            match send_once(self, event).await {
                Ok(()) => {
                    debug!(
                        channel = self.name(),
                        event_type = event.event_type(),
                        "Webhook notification sent"
                    );
                    return Ok(());
                }
                Err(err) => {
                    last_error = err;
                }
            }
        }

        Err(NotifyError::SendFailed {
            message: last_error,
        })
    }

    fn is_enabled(&self) -> bool {
        self.enabled
    }
}

fn apply_headers(
    request: reqwest::RequestBuilder,
    headers: Option<&HashMap<String, String>>,
) -> reqwest::RequestBuilder {
    let Some(headers) = headers else {
        return request;
    };

    let mut request = request;
    for (key, value) in headers {
        let name = HeaderName::from_bytes(key.as_bytes());
        let value = HeaderValue::from_str(value);
        match (name, value) {
            (Ok(name), Ok(value)) => {
                request = request.header(name, value);
            }
            _ => {
                warn!(header = %key, "Invalid webhook header; skipping");
            }
        }
    }
    request
}

async fn send_once(channel: &WebhookChannel, event: &NotificationEvent) -> Result<(), String> {
    let request = channel.client.post(&channel.url).json(event);
    let request = apply_headers(request, channel.headers.as_ref());

    match request.send().await {
        Ok(response) => {
            if response.status().is_success() {
                Ok(())
            } else {
                Err(format!("Unexpected status: {}", response.status()))
            }
        }
        Err(err) => Err(format!("Request error: {err}")),
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
    fn formats_session_stopped_message() {
        let timestamp = chrono::Utc
            .with_ymd_and_hms(2025, 1, 2, 3, 4, 5)
            .single()
            .expect("valid timestamp");
        let event = NotificationEvent::SessionStopped {
            timestamp,
            session_path: PathBuf::from("/tmp/session"),
            stop_reason: "rate_limit".to_string(),
            details: Some("Retry later".to_string()),
        };

        let message = format_event_message(&event);

        assert!(message.contains("Session stopped at 2025-01-02T03:04:05+00:00"));
        assert!(message.contains("Session: /tmp/session"));
        assert!(message.contains("Reason: rate_limit"));
        assert!(message.contains("Details: Retry later"));
    }
}
