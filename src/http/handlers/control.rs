use std::sync::Arc;

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use serde::Serialize;

use crate::daemon::state::DaemonState;
use crate::ipc::socket::DaemonStateAccess;

/// Basic control response payload for success-only endpoints.
#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct ControlResponse {
    success: bool,
}

impl ControlResponse {
    fn success() -> Self {
        Self { success: true }
    }
}

/// Control response payload that includes a session identifier.
#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct ControlResponseWithId {
    success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    session_id: Option<String>,
}

impl ControlResponseWithId {
    fn success(session_id: Option<String>) -> Self {
        Self {
            success: true,
            session_id,
        }
    }
}

/// Error detail payload for control endpoint failures.
#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct ErrorDetail {
    code: String,
    message: String,
}

/// Error response envelope for control endpoints.
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

/// Handles POST /api/v1/pause requests to pause daemon monitoring.
pub async fn pause_handler(
    State(state): State<Arc<DaemonState>>,
) -> impl IntoResponse {
    match state.pause() {
        Ok(()) => (StatusCode::OK, Json(ControlResponse::success())).into_response(),
        Err(message) if message == "Daemon already paused" => {
            error_response("ALREADY_PAUSED", &message, StatusCode::BAD_REQUEST).into_response()
        }
        Err(message) => {
            error_response("PAUSE_ERROR", &message, StatusCode::INTERNAL_SERVER_ERROR)
                .into_response()
        }
    }
}

/// Handles POST /api/v1/resume requests to resume daemon monitoring.
pub async fn resume_handler(
    State(state): State<Arc<DaemonState>>,
) -> impl IntoResponse {
    match state.resume() {
        Ok(()) => (StatusCode::OK, Json(ControlResponse::success())).into_response(),
        Err(message) if message == "Daemon is not paused" => {
            error_response("NOT_PAUSED", &message, StatusCode::BAD_REQUEST).into_response()
        }
        Err(message) => {
            error_response("RESUME_ERROR", &message, StatusCode::INTERNAL_SERVER_ERROR)
                .into_response()
        }
    }
}

/// Handles POST /api/v1/new-session requests to start a new session.
pub async fn new_session_handler(
    State(state): State<Arc<DaemonState>>,
) -> impl IntoResponse {
    match state.new_session() {
        Ok(()) => {
            let status = state.get_status();
            let session_id = status
                .current_session
                .unwrap_or_else(|| status.saves_count.to_string());
            let response = ControlResponseWithId::success(Some(session_id));
            (StatusCode::OK, Json(response)).into_response()
        }
        Err(message) => {
            error_response("SESSION_ERROR", &message, StatusCode::INTERNAL_SERVER_ERROR)
                .into_response()
        }
    }
}

#[cfg(test)]
mod tests {
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
            .with_state(state)
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
