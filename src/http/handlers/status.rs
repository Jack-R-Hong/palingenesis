use std::sync::Arc;

use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use serde::Serialize;

use crate::config::schema::{DaemonConfig, MonitoringConfig};
use crate::daemon::state::DaemonState;
use crate::ipc::protocol::DaemonStatus;
use crate::ipc::socket::DaemonStateAccess;

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct StatusEnvelope {
    success: bool,
    data: StatusResponse,
}

impl StatusEnvelope {
    fn new(data: StatusResponse) -> Self {
        Self {
            success: true,
            data,
        }
    }
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct StatusResponse {
    state: String,
    current_session: Option<String>,
    stats: StatsResponse,
    config_summary: ConfigSummary,
}

impl StatusResponse {
    fn from_status(status: DaemonStatus, config_summary: ConfigSummary) -> Self {
        let stats = StatsResponse::from_status(&status);
        Self {
            state: status.state,
            current_session: status.current_session,
            stats,
            config_summary,
        }
    }
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct StatsResponse {
    uptime_secs: u64,
    saves_count: u64,
    total_resumes: u64,
}

impl StatsResponse {
    fn from_status(status: &DaemonStatus) -> Self {
        Self {
            uptime_secs: status.uptime_secs,
            saves_count: status.saves_count,
            total_resumes: status.total_resumes,
        }
    }
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct ConfigSummary {
    http_enabled: bool,
    http_port: u16,
    auto_detect: bool,
    assistants: Vec<String>,
}

impl ConfigSummary {
    fn from_configs(daemon: &DaemonConfig, monitoring: &MonitoringConfig) -> Self {
        Self {
            http_enabled: daemon.http_enabled,
            http_port: daemon.http_port,
            auto_detect: monitoring.auto_detect,
            assistants: monitoring.assistants.clone(),
        }
    }
}

/// Handles GET /api/v1/status requests with daemon status payload.
pub async fn status_handler(
    State(state): State<Arc<DaemonState>>,
) -> (StatusCode, Json<StatusEnvelope>) {
    let status = state.get_status();
    let config_summary = build_config_summary(&state);
    let response = StatusEnvelope::new(StatusResponse::from_status(status, config_summary));
    (StatusCode::OK, Json(response))
}

fn build_config_summary(state: &DaemonState) -> ConfigSummary {
    let daemon_config = state.daemon_config().unwrap_or_default();
    let monitoring_config = state.monitoring_config().unwrap_or_default();
    ConfigSummary::from_configs(&daemon_config, &monitoring_config)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::to_bytes;
    use axum::routing::get;
    use axum::Router;
    use tower::ServiceExt;

    fn test_router(state: Arc<DaemonState>) -> Router {
        Router::new()
            .route("/api/v1/status", get(status_handler))
            .with_state(state)
    }

    #[tokio::test]
    async fn test_status_response_ok_structure() {
        let state = Arc::new(DaemonState::new());
        let response = test_router(state)
            .oneshot(
                axum::http::Request::builder()
                    .uri("/api/v1/status")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();

        assert_eq!(payload["success"], true);
        assert!(payload["data"]["state"].as_str().is_some());
        assert!(payload["data"]["current_session"].is_null());
        assert!(payload["data"]["stats"]["uptime_secs"].as_u64().is_some());
        assert!(payload["data"]["stats"]["saves_count"].as_u64().is_some());
        assert!(payload["data"]["stats"]["total_resumes"].as_u64().is_some());
        assert!(payload["data"]["config_summary"]["http_enabled"].as_bool().is_some());
        assert!(payload["data"]["config_summary"]["http_port"].as_u64().is_some());
        assert!(payload["data"]["config_summary"]["auto_detect"].as_bool().is_some());
        assert!(payload["data"]["config_summary"]["assistants"].as_array().is_some());
    }

    #[tokio::test]
    async fn test_status_response_paused_state() {
        let state = Arc::new(DaemonState::new());
        state.pause().unwrap();

        let response = test_router(state)
            .oneshot(
                axum::http::Request::builder()
                    .uri("/api/v1/status")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(payload["data"]["state"], "paused");
    }

    #[tokio::test]
    async fn test_status_response_matches_daemon_status() {
        let state = Arc::new(DaemonState::new());
        let snapshot = state.get_status();
        let response = test_router(Arc::clone(&state))
            .oneshot(
                axum::http::Request::builder()
                    .uri("/api/v1/status")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();

        assert_eq!(payload["data"]["state"], snapshot.state);
        if let Some(session) = snapshot.current_session {
            assert_eq!(payload["data"]["current_session"], session);
        } else {
            assert!(payload["data"]["current_session"].is_null());
        }
        assert_eq!(payload["data"]["stats"]["uptime_secs"], snapshot.uptime_secs);
        assert_eq!(payload["data"]["stats"]["saves_count"], snapshot.saves_count);
        assert_eq!(payload["data"]["stats"]["total_resumes"], snapshot.total_resumes);
    }
}
