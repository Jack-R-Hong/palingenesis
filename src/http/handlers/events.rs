use std::convert::Infallible;
use std::time::Duration;

use axum::extract::State;
use axum::response::IntoResponse;
use axum::response::sse::{Event, KeepAlive, Sse};
use chrono::{DateTime, Utc};
use serde::Serialize;
use tokio::sync::broadcast;
use tokio_stream::StreamExt;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::wrappers::errors::BroadcastStreamRecvError;
use tracing::warn;

use crate::http::server::AppState;
use crate::notify::events::NotificationEvent;
#[cfg(test)]
use crate::telemetry::Metrics;

#[derive(Debug, Serialize)]
struct ConnectedEvent {
    status: String,
    timestamp: DateTime<Utc>,
}

/// Handles GET /api/v1/events SSE streaming requests.
pub async fn events_handler(State(state): State<AppState>) -> impl IntoResponse {
    let receiver = state.events().subscribe();
    let stream = connected_stream().chain(broadcast_stream(receiver));
    Sse::new(stream).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(30))
            .text(": heartbeat"),
    )
}

fn connected_stream() -> impl tokio_stream::Stream<Item = Result<Event, Infallible>> {
    let payload = ConnectedEvent {
        status: "connected".to_string(),
        timestamp: Utc::now(),
    };
    let event = match Event::default().event("connected").json_data(&payload) {
        Ok(event) => event,
        Err(err) => {
            warn!(error = %err, "Failed to serialize connected event");
            Event::default()
                .event("connected")
                .data("{\"status\":\"connected\"}")
        }
    };
    tokio_stream::iter([Ok(event)])
}

fn broadcast_stream(
    receiver: broadcast::Receiver<NotificationEvent>,
) -> impl tokio_stream::Stream<Item = Result<Event, Infallible>> {
    BroadcastStream::new(receiver).filter_map(|message| match message {
        Ok(event) => Some(Ok(notification_event(event))),
        Err(BroadcastStreamRecvError::Lagged(skipped)) => {
            warn!(skipped, "SSE subscriber lagged behind broadcast channel");
            None
        }
    })
}

fn notification_event(event: NotificationEvent) -> Event {
    match Event::default().event(event.event_type()).json_data(&event) {
        Ok(event) => event,
        Err(err) => {
            warn!(error = %err, event_type = event.event_type(), "Failed to serialize SSE event");
            Event::default()
                .event(event.event_type())
                .data("{\"error\":\"serialization_failed\"}")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::Router;
    use axum::body::Body;
    use axum::http::Request;
    use axum::http::header::CONTENT_TYPE;
    use axum::routing::get;
    use http_body_util::BodyExt;
    use std::path::PathBuf;
    use std::sync::Arc;
    use std::time::Duration;
    use tokio::time::{advance, timeout};
    use tower::ServiceExt;

    use crate::daemon::state::DaemonState;
    use crate::http::{AppState, EventBroadcaster};

    fn test_router(state: AppState) -> Router {
        Router::new()
            .route("/api/v1/events", get(events_handler))
            .with_state(state)
    }

    async fn read_frame_text(body: &mut Body) -> String {
        let frame = timeout(Duration::from_secs(2), body.frame())
            .await
            .expect("frame timeout")
            .expect("frame exists")
            .expect("body ended");
        let data = frame.into_data().expect("data frame");
        String::from_utf8(data.to_vec()).expect("utf8 body")
    }

    fn sample_event() -> NotificationEvent {
        NotificationEvent::SessionStopped {
            timestamp: Utc::now(),
            session_path: PathBuf::from("/tmp/session"),
            stop_reason: "rate_limit".to_string(),
            details: None,
        }
    }

    #[tokio::test]
    async fn test_sse_content_type() {
        let state = AppState::new(
            Arc::new(DaemonState::new()),
            EventBroadcaster::default(),
            Arc::new(Metrics::new()),
        );
        let response = test_router(state)
            .oneshot(
                Request::builder()
                    .uri("/api/v1/events")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        let content_type = response
            .headers()
            .get(CONTENT_TYPE)
            .expect("content type header")
            .to_str()
            .expect("content type string");
        assert!(content_type.starts_with("text/event-stream"));
    }

    #[tokio::test]
    async fn test_initial_connected_event_sent() {
        let state = AppState::new(
            Arc::new(DaemonState::new()),
            EventBroadcaster::default(),
            Arc::new(Metrics::new()),
        );
        let response = test_router(state)
            .oneshot(
                Request::builder()
                    .uri("/api/v1/events")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        let mut body = response.into_body();
        let text = read_frame_text(&mut body).await;
        assert!(text.contains("event: connected"));
        assert!(text.contains("\"status\":\"connected\""));
    }

    #[tokio::test]
    async fn test_notification_event_formatting() {
        let broadcaster = EventBroadcaster::default();
        let state = AppState::new(
            Arc::new(DaemonState::new()),
            broadcaster.clone(),
            Arc::new(Metrics::new()),
        );
        let response = test_router(state)
            .oneshot(
                Request::builder()
                    .uri("/api/v1/events")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        let mut body = response.into_body();
        let _ = read_frame_text(&mut body).await;

        broadcaster.send(sample_event()).expect("send event");
        let text = read_frame_text(&mut body).await;
        assert!(text.contains("event: session_stopped"));
        assert!(text.contains("\"event\":\"session_stopped\""));
    }

    #[tokio::test]
    async fn test_multiple_clients_receive_same_event() {
        let broadcaster = EventBroadcaster::default();
        let state = AppState::new(
            Arc::new(DaemonState::new()),
            broadcaster.clone(),
            Arc::new(Metrics::new()),
        );
        let router = test_router(state);

        let response_one = router
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/events")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let response_two = router
            .oneshot(
                Request::builder()
                    .uri("/api/v1/events")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        let mut body_one = response_one.into_body();
        let mut body_two = response_two.into_body();
        let _ = read_frame_text(&mut body_one).await;
        let _ = read_frame_text(&mut body_two).await;

        broadcaster.send(sample_event()).expect("send event");

        let text_one = read_frame_text(&mut body_one).await;
        let text_two = read_frame_text(&mut body_two).await;
        assert!(text_one.contains("event: session_stopped"));
        assert!(text_two.contains("event: session_stopped"));
    }

    #[tokio::test]
    async fn test_client_disconnect_does_not_affect_others() {
        let broadcaster = EventBroadcaster::default();
        let state = AppState::new(
            Arc::new(DaemonState::new()),
            broadcaster.clone(),
            Arc::new(Metrics::new()),
        );
        let router = test_router(state);

        let response_one = router
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/events")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let response_two = router
            .oneshot(
                Request::builder()
                    .uri("/api/v1/events")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        let mut body_one = response_one.into_body();
        let mut body_two = response_two.into_body();
        let _ = read_frame_text(&mut body_one).await;
        let _ = read_frame_text(&mut body_two).await;

        drop(body_two);
        broadcaster.send(sample_event()).expect("send event");

        let text_one = read_frame_text(&mut body_one).await;
        assert!(text_one.contains("event: session_stopped"));
    }

    #[tokio::test]
    async fn test_keep_alive_heartbeat_sent_after_idle() {
        tokio::time::pause();
        let broadcaster = EventBroadcaster::default();
        let state = AppState::new(
            Arc::new(DaemonState::new()),
            broadcaster.clone(),
            Arc::new(Metrics::new()),
        );
        let response = test_router(state)
            .oneshot(
                Request::builder()
                    .uri("/api/v1/events")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        let mut body = response.into_body();
        let _ = read_frame_text(&mut body).await;

        let next_frame = tokio::spawn(async move { body.frame().await });
        advance(Duration::from_secs(31)).await;

        let frame = next_frame
            .await
            .expect("heartbeat task")
            .expect("heartbeat frame")
            .expect("heartbeat data");
        let data = frame.into_data().expect("data frame");
        let text = String::from_utf8(data.to_vec()).expect("utf8 body");
        assert!(text.contains(": heartbeat"));
    }
}
