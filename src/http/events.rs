use tokio::sync::broadcast;

use crate::notify::events::NotificationEvent;

const DEFAULT_CAPACITY: usize = 1024;

/// Broadcasts daemon events to multiple SSE subscribers.
#[derive(Clone, Debug)]
pub struct EventBroadcaster {
    sender: broadcast::Sender<NotificationEvent>,
}

impl EventBroadcaster {
    /// Create a new broadcaster with the provided channel capacity.
    pub fn new(capacity: usize) -> Self {
        let capacity = capacity.max(1);
        let (sender, _) = broadcast::channel(capacity);
        Self { sender }
    }

    /// Subscribe to notification events.
    pub fn subscribe(&self) -> broadcast::Receiver<NotificationEvent> {
        self.sender.subscribe()
    }

    /// Send a notification event to all subscribers.
    pub fn send(
        &self,
        event: NotificationEvent,
    ) -> Result<usize, broadcast::error::SendError<NotificationEvent>> {
        self.sender.send(event)
    }
}

impl Default for EventBroadcaster {
    fn default() -> Self {
        Self::new(DEFAULT_CAPACITY)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use std::path::PathBuf;

    fn sample_event() -> NotificationEvent {
        let timestamp = chrono::Utc
            .with_ymd_and_hms(2025, 2, 3, 4, 5, 6)
            .single()
            .expect("valid timestamp");
        NotificationEvent::SessionStopped {
            timestamp,
            session_path: PathBuf::from("/tmp/session"),
            stop_reason: "rate_limit".to_string(),
            details: None,
        }
    }

    #[test]
    fn test_new_creates_broadcaster() {
        let broadcaster = EventBroadcaster::new(8);
        let _receiver = broadcaster.subscribe();
    }

    #[tokio::test]
    async fn test_send_delivers_to_all_subscribers() {
        let broadcaster = EventBroadcaster::new(8);
        let mut receiver_one = broadcaster.subscribe();
        let mut receiver_two = broadcaster.subscribe();

        broadcaster.send(sample_event()).expect("send event");

        let event_one = receiver_one.recv().await.expect("recv event one");
        let event_two = receiver_two.recv().await.expect("recv event two");
        assert_eq!(event_one, event_two);
    }

    #[tokio::test]
    async fn test_lagged_subscribers_do_not_block_sender() {
        let broadcaster = EventBroadcaster::new(1);
        let _receiver = broadcaster.subscribe();

        for _ in 0..3 {
            broadcaster.send(sample_event()).expect("send event");
        }
    }
}
