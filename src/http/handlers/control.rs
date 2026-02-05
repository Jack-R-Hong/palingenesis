use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use serde::Serialize;
use uuid::Uuid;

use crate::daemon::state::DaemonState;
use crate::http::server::AppState;
use crate::ipc::socket::DaemonStateAccess;

/// Error messages returned by DaemonState methods.
/// Using constants prevents silent failures from string comparison mismatches.
mod error_messages {
    pub const ALREADY_PAUSED: &str = "Daemon already paused";
    pub const NOT_PAUSED: &str = "Daemon is not paused";
}

/// Success response payload for control endpoints (ARCH23 compliant).
///
/// Returns `{ "success": true }` for successful pause/resume operations.
#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct ControlResponse {
    success: bool,
}

impl ControlResponse {
    fn success() -> Self {
        Self { success: true }
    }
}

/// Success response payload for new-session endpoint (ARCH23 compliant).
///
/// Returns `{ "success": true, "session_id": "..." }` with a UUID identifier.
#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct ControlResponseWithId {
    success: bool,
    /// Unique session identifier (UUID v4 format).
    session_id: String,
}

impl ControlResponseWithId {
    fn success(session_id: String) -> Self {
        Self {
            success: true,
            session_id,
        }
    }
}

/// Error detail payload for control endpoint failures (ARCH23 compliant).
///
/// Contains machine-readable `code` and human-readable `message`.
#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct ErrorDetail {
    /// Machine-readable error code (e.g., "ALREADY_PAUSED", "NOT_PAUSED").
    code: String,
    /// Human-readable error message.
    message: String,
}

/// Error response envelope for control endpoints (ARCH23 compliant).
///
/// Returns `{ "success": false, "error": { "code": "...", "message": "..." } }`.
#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct ControlErrorResponse {
    success: bool,
    error: ErrorDetail,
}

impl ControlErrorResponse {
    fn new(code: &str, message: &str) -> Self {
        Self {
            success: false,
            error: ErrorDetail {
                code: code.to_string(),
                message: message.to_string(),
            },
        }
    }
}

fn error_response(
    code: &str,
    message: &str,
    status: StatusCode,
) -> (StatusCode, Json<ControlErrorResponse>) {
    (status, Json(ControlErrorResponse::new(code, message)))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ControlError {
    pub code: String,
    pub message: String,
    pub status: StatusCode,
}

impl ControlError {
    fn new(code: &str, message: &str, status: StatusCode) -> Self {
        Self {
            code: code.to_string(),
            message: message.to_string(),
            status,
        }
    }
}

pub fn pause_daemon(daemon_state: &DaemonState) -> Result<(), ControlError> {
    match daemon_state.pause() {
        Ok(()) => Ok(()),
        Err(message) if message == error_messages::ALREADY_PAUSED => Err(ControlError::new(
            "ALREADY_PAUSED",
            &message,
            StatusCode::BAD_REQUEST,
        )),
        Err(message) => Err(ControlError::new(
            "PAUSE_ERROR",
            &message,
            StatusCode::INTERNAL_SERVER_ERROR,
        )),
    }
}

pub fn resume_daemon(daemon_state: &DaemonState) -> Result<(), ControlError> {
    match daemon_state.resume() {
        Ok(()) => Ok(()),
        Err(message) if message == error_messages::NOT_PAUSED => Err(ControlError::new(
            "NOT_PAUSED",
            &message,
            StatusCode::BAD_REQUEST,
        )),
        Err(message) => Err(ControlError::new(
            "RESUME_ERROR",
            &message,
            StatusCode::INTERNAL_SERVER_ERROR,
        )),
    }
}

pub fn new_session_daemon(daemon_state: &DaemonState) -> Result<String, ControlError> {
    match daemon_state.new_session() {
        Ok(()) => Ok(Uuid::new_v4().to_string()),
        Err(message) => Err(ControlError::new(
            "SESSION_ERROR",
            &message,
            StatusCode::INTERNAL_SERVER_ERROR,
        )),
    }
}

/// Handles POST /api/v1/pause requests to pause daemon monitoring.
pub async fn pause_handler(
    State(state): State<AppState>,
) -> impl IntoResponse {
    let daemon_state = state.daemon_state();
    match pause_daemon(daemon_state) {
        Ok(()) => (StatusCode::OK, Json(ControlResponse::success())).into_response(),
        Err(err) => error_response(&err.code, &err.message, err.status).into_response(),
    }
}

/// Handles POST /api/v1/resume requests to resume daemon monitoring.
pub async fn resume_handler(
    State(state): State<AppState>,
) -> impl IntoResponse {
    let daemon_state = state.daemon_state();
    match resume_daemon(daemon_state) {
        Ok(()) => (StatusCode::OK, Json(ControlResponse::success())).into_response(),
        Err(err) => error_response(&err.code, &err.message, err.status).into_response(),
    }
}

/// Handles POST /api/v1/new-session requests to start a new session.
pub async fn new_session_handler(
    State(state): State<AppState>,
) -> impl IntoResponse {
    let daemon_state = state.daemon_state();
    match new_session_daemon(daemon_state) {
        Ok(session_id) => {
            let response = ControlResponseWithId::success(session_id);
            (StatusCode::OK, Json(response)).into_response()
        }
        Err(err) => error_response(&err.code, &err.message, err.status).into_response(),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use axum::body::to_bytes;
    use axum::routing::post;
    use axum::Router;
    use tower::ServiceExt;

    fn test_router(state: Arc<DaemonState>) -> Router {
        Router::new()
            .route("/api/v1/pause", post(pause_handler))
            .route("/api/v1/resume", post(resume_handler))
            .route("/api/v1/new-session", post(new_session_handler))
            .with_state(AppState::new(state, crate::http::EventBroadcaster::default()))
    }

    async fn read_json(response: axum::http::Response<axum::body::Body>) -> serde_json::Value {
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        serde_json::from_slice(&body).unwrap()
    }

    #[tokio::test]
    async fn test_pause_success() {
        let state = Arc::new(DaemonState::new());
        let response = test_router(state)
            .oneshot(
                axum::http::Request::builder()
                    .method("POST")
                    .uri("/api/v1/pause")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let payload = read_json(response).await;
        assert_eq!(payload["success"], true);
    }

    #[tokio::test]
    async fn test_pause_already_paused_returns_error() {
        let state = Arc::new(DaemonState::new());
        state.pause().unwrap();

        let response = test_router(state)
            .oneshot(
                axum::http::Request::builder()
                    .method("POST")
                    .uri("/api/v1/pause")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let payload = read_json(response).await;
        assert_eq!(payload["success"], false);
        assert_eq!(payload["error"]["code"], "ALREADY_PAUSED");
        assert_eq!(payload["error"]["message"], "Daemon already paused");
    }

    #[tokio::test]
    async fn test_resume_success() {
        let state = Arc::new(DaemonState::new());
        state.pause().unwrap();

        let response = test_router(state)
            .oneshot(
                axum::http::Request::builder()
                    .method("POST")
                    .uri("/api/v1/resume")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let payload = read_json(response).await;
        assert_eq!(payload["success"], true);
    }

    #[tokio::test]
    async fn test_resume_not_paused_returns_error() {
        let state = Arc::new(DaemonState::new());

        let response = test_router(state)
            .oneshot(
                axum::http::Request::builder()
                    .method("POST")
                    .uri("/api/v1/resume")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let payload = read_json(response).await;
        assert_eq!(payload["success"], false);
        assert_eq!(payload["error"]["code"], "NOT_PAUSED");
        assert_eq!(payload["error"]["message"], "Daemon is not paused");
    }

    #[tokio::test]
    async fn test_new_session_success_includes_session_id() {
        let state = Arc::new(DaemonState::new());

        let response = test_router(state)
            .oneshot(
                axum::http::Request::builder()
                    .method("POST")
                    .uri("/api/v1/new-session")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let payload = read_json(response).await;
        assert_eq!(payload["success"], true);
        assert!(
            payload["session_id"].as_str().is_some_and(|id| !id.is_empty()),
            "session_id should be a non-empty string"
        );
    }

    #[tokio::test]
    async fn test_error_response_format_matches_spec() {
        let state = Arc::new(DaemonState::new());
        state.pause().unwrap();

        let response = test_router(state)
            .oneshot(
                axum::http::Request::builder()
                    .method("POST")
                    .uri("/api/v1/pause")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        let payload = read_json(response).await;
        assert_eq!(payload["success"], false);
        assert!(payload.get("error").is_some());
        assert!(payload["error"].get("code").is_some());
        assert!(payload["error"].get("message").is_some());
    }
}
