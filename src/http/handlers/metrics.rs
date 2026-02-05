use axum::extract::State;
use axum::http::StatusCode;
use axum::http::header;
use axum::response::{IntoResponse, Response};
use tracing::error;

use crate::daemon::state::DaemonState;
use crate::http::server::AppState;

const METRICS_CONTENT_TYPE: &str = "application/openmetrics-text; version=1.0.0; charset=utf-8";

/// Handles GET /api/v1/metrics requests with Prometheus-compatible output.
pub async fn metrics_handler(State(state): State<AppState>) -> Response {
    let daemon_state = state.daemon_state();
    if !metrics_enabled(daemon_state) {
        return StatusCode::NOT_FOUND.into_response();
    }

    let metrics = state.metrics();
    metrics.update_from_state(daemon_state);
    match metrics.encode() {
        Ok(body) => (
            StatusCode::OK,
            [(header::CONTENT_TYPE, METRICS_CONTENT_TYPE)],
            body,
        )
            .into_response(),
        Err(err) => {
            error!(error = %err, "Failed to encode metrics");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

fn metrics_enabled(daemon_state: &DaemonState) -> bool {
    daemon_state
        .otel_config()
        .is_none_or(|otel| otel.metrics_enabled)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::Router;
    use axum::body::to_bytes;
    use axum::routing::get;
    use std::sync::Arc;
    use std::time::Instant;
    use tempfile::tempdir;
    use tower::ServiceExt;

    use crate::daemon::state::DaemonState;
    use crate::http::EventBroadcaster;
    use crate::telemetry::Metrics;
    use crate::test_utils::ENV_LOCK;

    fn test_router(state: Arc<DaemonState>) -> Router {
        Router::new()
            .route("/api/v1/metrics", get(metrics_handler))
            .with_state(AppState::new(
                state,
                EventBroadcaster::default(),
                Arc::new(Metrics::new()),
            ))
    }

    fn set_env_var(key: &str, value: impl AsRef<std::ffi::OsStr>) {
        unsafe {
            std::env::set_var(key, value);
        }
    }

    fn remove_env_var(key: &str) {
        unsafe {
            std::env::remove_var(key);
        }
    }

    #[tokio::test]
    async fn test_metrics_response_content_type() {
        let state = Arc::new(DaemonState::new());
        let response = test_router(state)
            .oneshot(
                axum::http::Request::builder()
                    .uri("/api/v1/metrics")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let content_type = response
            .headers()
            .get(header::CONTENT_TYPE)
            .expect("content type header")
            .to_str()
            .expect("content type string");
        assert_eq!(content_type, METRICS_CONTENT_TYPE);
    }

    #[tokio::test]
    async fn test_metrics_output_contains_expected_names() {
        let state = Arc::new(DaemonState::new());
        let response = test_router(state)
            .oneshot(
                axum::http::Request::builder()
                    .uri("/api/v1/metrics")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let text = String::from_utf8(body.to_vec()).expect("utf8 metrics body");
        assert!(text.contains("palingenesis_info"));
        assert!(text.contains("palingenesis_daemon_state"));
        assert!(text.contains("palingenesis_uptime_seconds"));
        assert!(text.contains("palingenesis_build_info"));
        assert!(text.contains("palingenesis_resumes_total"));
        assert!(text.contains("palingenesis_resumes_success_total"));
        assert!(text.contains("palingenesis_resumes_failure_total"));
        assert!(text.contains("palingenesis_sessions_started_total"));
        assert!(text.contains("palingenesis_rate_limits_total"));
        assert!(text.contains("palingenesis_context_exhaustions_total"));
        assert!(text.contains("palingenesis_current_session_steps_completed"));
        assert!(text.contains("palingenesis_current_session_steps_total"));
        assert!(text.contains("palingenesis_active_sessions"));
        assert!(text.contains("palingenesis_retry_attempts"));
        assert!(text.contains("palingenesis_resume_duration_seconds"));
        assert!(text.contains("palingenesis_detection_latency_seconds"));
        assert!(text.contains("palingenesis_wait_duration_seconds"));
        assert!(text.contains("palingenesis_time_saved_seconds_total"));
        assert!(text.contains("palingenesis_time_saved_per_resume_seconds"));
    }

    #[tokio::test]
    async fn test_metrics_disabled_returns_not_found() {
        let _lock = ENV_LOCK.lock().unwrap();
        let temp = tempdir().unwrap();
        let config_path = temp.path().join("config.toml");
        set_env_var("PALINGENESIS_CONFIG", &config_path);
        std::fs::write(&config_path, "[otel]\nmetrics_enabled = false\n").unwrap();

        let state = Arc::new(DaemonState::new());
        let response = test_router(state)
            .oneshot(
                axum::http::Request::builder()
                    .uri("/api/v1/metrics")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        remove_env_var("PALINGENESIS_CONFIG");
    }

    #[tokio::test]
    async fn test_metrics_endpoint_handles_burst_quickly() {
        let state = Arc::new(DaemonState::new());
        let router = test_router(state);
        let start = Instant::now();
        for _ in 0..100 {
            let response = router
                .clone()
                .oneshot(
                    axum::http::Request::builder()
                        .uri("/api/v1/metrics")
                        .body(axum::body::Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::OK);
        }
        let elapsed = start.elapsed();
        assert!(elapsed.as_secs_f64() < 5.0, "elapsed: {:?}", elapsed);
    }
}
