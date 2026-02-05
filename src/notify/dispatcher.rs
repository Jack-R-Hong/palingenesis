use tracing::error;

use crate::notify::channel::NotificationChannel;
use crate::notify::error::NotifyError;
use crate::notify::events::NotificationEvent;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DispatchSummary {
    pub total: usize,
    pub successes: usize,
    pub failures: usize,
    pub failed_channels: Vec<String>,
}

impl DispatchSummary {
    fn new(total: usize, failures: Vec<String>) -> Self {
        let failures_count = failures.len();
        Self {
            total,
            successes: total.saturating_sub(failures_count),
            failures: failures_count,
            failed_channels: failures,
        }
    }
}

pub struct Dispatcher {
    channels: Vec<Box<dyn NotificationChannel>>,
}

impl Dispatcher {
    pub fn new(channels: Vec<Box<dyn NotificationChannel>>) -> Self {
        Self { channels }
    }

    pub async fn dispatch(&self, event: NotificationEvent) -> DispatchSummary {
        let enabled: Vec<&dyn NotificationChannel> = self
            .channels
            .iter()
            .map(|channel| channel.as_ref())
            .filter(|channel| channel.is_enabled())
            .collect();

        let mut failures = Vec::new();
        let total = enabled.len();

        for chunk in enabled.chunks(4) {
            let mut outcomes = Vec::new();
            match chunk.len() {
                0 => {}
                1 => {
                    outcomes.push(send_one(chunk[0], &event).await);
                }
                2 => {
                    let fut1 = send_one(chunk[0], &event);
                    let fut2 = send_one(chunk[1], &event);
                    let (res1, res2) = tokio::join!(fut1, fut2);
                    outcomes.extend([res1, res2]);
                }
                3 => {
                    let fut1 = send_one(chunk[0], &event);
                    let fut2 = send_one(chunk[1], &event);
                    let fut3 = send_one(chunk[2], &event);
                    let (res1, res2, res3) = tokio::join!(fut1, fut2, fut3);
                    outcomes.extend([res1, res2, res3]);
                }
                _ => {
                    let fut1 = send_one(chunk[0], &event);
                    let fut2 = send_one(chunk[1], &event);
                    let fut3 = send_one(chunk[2], &event);
                    let fut4 = send_one(chunk[3], &event);
                    let (res1, res2, res3, res4) = tokio::join!(fut1, fut2, fut3, fut4);
                    outcomes.extend([res1, res2, res3, res4]);
                }
            }

            for outcome in outcomes {
                if let Err(err) = outcome.result {
                    error!(
                        channel = outcome.name,
                        event_type = event.event_type(),
                        error = %err,
                        "Notification channel send failed"
                    );
                    failures.push(outcome.name.to_string());
                }
            }
        }

        DispatchSummary::new(total, failures)
    }
}

struct ChannelOutcome {
    name: &'static str,
    result: Result<(), NotifyError>,
}

async fn send_one(channel: &dyn NotificationChannel, event: &NotificationEvent) -> ChannelOutcome {
    let name = channel.name();
    let result = channel.send(event).await;
    ChannelOutcome { name, result }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::notify::events::EventSeverity;
    use async_trait::async_trait;
    use chrono::TimeZone;
    use std::path::PathBuf;

    struct MockChannel {
        name: &'static str,
        enabled: bool,
        fail: bool,
    }

    #[async_trait]
    impl NotificationChannel for MockChannel {
        fn name(&self) -> &'static str {
            self.name
        }

        async fn send(&self, event: &NotificationEvent) -> Result<(), NotifyError> {
            let _ = event.severity();
            if self.fail {
                return Err(NotifyError::SendFailed {
                    message: format!("{} failed", self.name),
                });
            }
            Ok(())
        }

        fn is_enabled(&self) -> bool {
            self.enabled
        }
    }

    fn sample_event() -> NotificationEvent {
        let timestamp = chrono::Utc
            .with_ymd_and_hms(2025, 1, 2, 3, 4, 5)
            .single()
            .expect("valid timestamp");
        NotificationEvent::ResumeAttempted {
            timestamp,
            session_path: PathBuf::from("/tmp/session"),
            strategy: "same_session".to_string(),
        }
    }

    #[tokio::test]
    async fn dispatch_collects_successes_and_failures() {
        let dispatcher = Dispatcher::new(vec![
            Box::new(MockChannel {
                name: "ok",
                enabled: true,
                fail: false,
            }),
            Box::new(MockChannel {
                name: "disabled",
                enabled: false,
                fail: false,
            }),
            Box::new(MockChannel {
                name: "fail",
                enabled: true,
                fail: true,
            }),
        ]);

        let summary = dispatcher.dispatch(sample_event()).await;

        assert_eq!(summary.total, 2);
        assert_eq!(summary.successes, 1);
        assert_eq!(summary.failures, 1);
        assert_eq!(summary.failed_channels, vec!["fail".to_string()]);
    }

    #[tokio::test]
    async fn dispatch_handles_no_enabled_channels() {
        let dispatcher = Dispatcher::new(vec![Box::new(MockChannel {
            name: "disabled",
            enabled: false,
            fail: false,
        })]);

        let summary = dispatcher.dispatch(sample_event()).await;

        assert_eq!(summary.total, 0);
        assert_eq!(summary.successes, 0);
        assert_eq!(summary.failures, 0);
        assert!(summary.failed_channels.is_empty());
    }

    #[tokio::test]
    async fn dispatch_sends_in_parallel_batches() {
        let dispatcher = Dispatcher::new(vec![
            Box::new(MockChannel {
                name: "one",
                enabled: true,
                fail: false,
            }),
            Box::new(MockChannel {
                name: "two",
                enabled: true,
                fail: false,
            }),
            Box::new(MockChannel {
                name: "three",
                enabled: true,
                fail: false,
            }),
            Box::new(MockChannel {
                name: "four",
                enabled: true,
                fail: false,
            }),
        ]);

        let summary = dispatcher.dispatch(sample_event()).await;

        assert_eq!(summary.total, 4);
        assert_eq!(summary.successes, 4);
        assert_eq!(summary.failures, 0);
        assert_eq!(EventSeverity::Info, sample_event().severity());
    }
}
