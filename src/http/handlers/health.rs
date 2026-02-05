use std::sync::Arc;
use std::time::Duration;

use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use serde::Serialize;

use crate::daemon::state::DaemonState;

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum HealthStatus {
    Ok,
    Degraded,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct HealthResponse {
    status: HealthStatus,
    uptime: String,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    issues: Vec<String>,
}

impl HealthResponse {
    fn new(status: HealthStatus, uptime: String, issues: Vec<String>) -> Self {
        Self {
            status,
            uptime,
            issues,
        }
    }
}

/// Handles GET /health requests with daemon uptime and status.
pub async fn health_handler(
    State(state): State<Arc<DaemonState>>,
) -> (StatusCode, Json<HealthResponse>) {
    let issues = collect_health_issues(&state);
    let status = if issues.is_empty() {
        HealthStatus::Ok
    } else {
        HealthStatus::Degraded
    };
    let uptime = format_uptime(state.uptime());
    let response = HealthResponse::new(status, uptime, issues);
    (StatusCode::OK, Json(response))
}

fn collect_health_issues(state: &DaemonState) -> Vec<String> {
    let mut issues = Vec::new();
    if state.is_paused() {
        issues.push("paused".to_string());
    }
    if state.daemon_config().is_none() {
        issues.push("config_unavailable".to_string());
    }
    issues
}

fn format_uptime(duration: Duration) -> String {
    let total_secs = duration.as_secs();
    let hours = total_secs / 3600;
    let minutes = (total_secs % 3600) / 60;
    let seconds = total_secs % 60;

    if hours > 0 {
        format!("{hours}h{minutes}m")
    } else if minutes > 0 {
        format!("{minutes}m")
    } else {
        format!("{seconds}s")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::to_bytes;
    use axum::routing::get;
    use axum::Router;
    use std::time::Instant;
    use tower::ServiceExt;

    use crate::ipc::socket::DaemonStateAccess;

    fn test_router(state: Arc<DaemonState>) -> Router {
        Router::new()
            .route("/health", get(health_handler))
            .with_state(state)
    }

    #[tokio::test]
    async fn test_health_response_ok() {
        let state = Arc::new(DaemonState::new());
        let response = test_router(state)
            .oneshot(
                axum::http::Request::builder()
                    .uri("/health")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(payload["status"], "ok");
        assert!(payload["uptime"].as_str().unwrap_or_default().len() > 0);
        assert!(payload.get("issues").is_none());
    }

    #[test]
    fn test_format_uptime() {
        assert_eq!(format_uptime(Duration::from_secs(2 * 3600 + 30 * 60)), "2h30m");
        assert_eq!(format_uptime(Duration::from_secs(15 * 60)), "15m");
        assert_eq!(format_uptime(Duration::from_secs(45)), "45s");
    }

    #[tokio::test]
    async fn test_health_response_time_under_100ms() {
        let state = Arc::new(DaemonState::new());
        let start = Instant::now();
        let response = test_router(state)
            .oneshot(
                axum::http::Request::builder()
                    .uri("/health")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let elapsed = start.elapsed();
        assert_eq!(response.status(), StatusCode::OK);
        assert!(elapsed < Duration::from_millis(100), "elapsed: {:?}", elapsed);
    }

    #[tokio::test]
    async fn test_health_response_degraded_includes_issues() {
        let state = Arc::new(DaemonState::new());
        state.pause().unwrap();

        let response = test_router(state)
            .oneshot(
                axum::http::Request::builder()
                    .uri("/health")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(payload["status"], "degraded");
        let issues = payload["issues"].as_array().expect("issues array");
        assert!(issues.iter().any(|issue| issue == "paused"));
    }
}
