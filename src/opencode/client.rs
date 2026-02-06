use std::collections::HashMap;
use std::time::Duration;

use reqwest::{Client, RequestBuilder, Response, StatusCode};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::time::sleep;
use tracing::{debug, warn};

use crate::config::schema::OpenCodeConfig;

const DEFAULT_USERNAME: &str = "opencode";
const DEFAULT_MAX_RETRIES: usize = 3;
const DEFAULT_BACKOFF_DELAYS: [Duration; DEFAULT_MAX_RETRIES] = [
    Duration::from_secs(1),
    Duration::from_secs(2),
    Duration::from_secs(4),
];

#[derive(Debug, Error)]
pub enum OpenCodeApiError {
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),
    #[error("Request timed out")]
    Timeout,
    #[error("Resource not found: {0}")]
    NotFound(String),
    #[error("Unexpected status {status}: {body}")]
    HttpStatus { status: StatusCode, body: String },
    #[error("Failed to parse response: {0}")]
    ParseError(String),
}

impl OpenCodeApiError {
    fn is_retryable(&self) -> bool {
        matches!(
            self,
            OpenCodeApiError::Timeout | OpenCodeApiError::ConnectionFailed(_)
        ) || matches!(self, OpenCodeApiError::HttpStatus { status, .. } if status.is_server_error())
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Session {
    pub id: String,
    #[serde(flatten)]
    pub metadata: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CreateSessionResponse {
    #[serde(alias = "session_id")]
    pub id: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct HealthResponse {
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub healthy: Option<bool>,
    #[serde(default)]
    pub version: Option<String>,
}

#[derive(Clone, Debug)]
struct BasicAuth {
    username: String,
    password: String,
}

#[derive(Clone, Debug)]
pub struct OpenCodeClient {
    client: Client,
    base_url: String,
    auth: Option<BasicAuth>,
    backoff_delays: Vec<Duration>,
}

impl OpenCodeClient {
    pub fn new(config: &OpenCodeConfig) -> Self {
        let timeout = Duration::from_millis(config.health_check_interval);
        let client = Client::builder()
            .timeout(timeout)
            .build()
            .unwrap_or_else(|err| {
                warn!(error = %err, "Failed to build OpenCode client; using defaults");
                Client::new()
            });
        let base_url = format!("http://{}:{}", config.serve_hostname, config.serve_port);
        let auth = load_basic_auth();

        Self {
            client,
            base_url,
            auth,
            backoff_delays: DEFAULT_BACKOFF_DELAYS.to_vec(),
        }
    }

    #[cfg(test)]
    fn with_base_url(base_url: String, backoff_delays: Vec<Duration>) -> Self {
        Self {
            client: Client::builder()
                .timeout(Duration::from_millis(200))
                .build()
                .expect("build test client"),
            base_url,
            auth: None,
            backoff_delays,
        }
    }

    pub async fn health(&self) -> Result<HealthResponse, OpenCodeApiError> {
        let url = format!("{}/global/health", self.base_url);
        self.request_with_retry(|| async {
            let response = self
                .apply_auth(self.client.get(&url))
                .send()
                .await
                .map_err(map_reqwest_error)?;
            parse_json_response(response).await
        })
        .await
    }

    pub async fn list_sessions(&self) -> Result<Vec<Session>, OpenCodeApiError> {
        let url = format!("{}/session", self.base_url);
        self.request_with_retry(|| async {
            let response = self
                .apply_auth(self.client.get(&url))
                .send()
                .await
                .map_err(map_reqwest_error)?;
            parse_sessions_response(response).await
        })
        .await
    }

    pub async fn create_session(
        &self,
        prompt: &str,
    ) -> Result<CreateSessionResponse, OpenCodeApiError> {
        #[derive(Serialize)]
        struct CreateRequest<'a> {
            prompt: &'a str,
        }

        let url = format!("{}/session", self.base_url);
        self.request_with_retry(|| async {
            let response = self
                .apply_auth(self.client.post(&url))
                .json(&CreateRequest { prompt })
                .send()
                .await
                .map_err(map_reqwest_error)?;
            parse_json_response(response).await
        })
        .await
    }

    pub async fn send_message(
        &self,
        session_id: &str,
        message: &str,
    ) -> Result<(), OpenCodeApiError> {
        #[derive(Serialize)]
        struct MessageRequest<'a> {
            message: &'a str,
        }

        let url = format!("{}/session/{}/message", self.base_url, session_id);
        self.request_with_retry(|| async {
            let response = self
                .apply_auth(self.client.post(&url))
                .json(&MessageRequest { message })
                .send()
                .await
                .map_err(map_reqwest_error)?;
            match response.status() {
                StatusCode::OK | StatusCode::ACCEPTED => Ok(()),
                StatusCode::NOT_FOUND => Err(OpenCodeApiError::NotFound(session_id.to_string())),
                status => {
                    let body = response.text().await.unwrap_or_default();
                    Err(OpenCodeApiError::HttpStatus { status, body })
                }
            }
        })
        .await
    }

    fn apply_auth(&self, request: RequestBuilder) -> RequestBuilder {
        match self.auth.as_ref() {
            Some(auth) => request.basic_auth(&auth.username, Some(&auth.password)),
            None => request,
        }
    }

    async fn request_with_retry<F, Fut, T>(&self, request_fn: F) -> Result<T, OpenCodeApiError>
    where
        F: Fn() -> Fut,
        Fut: std::future::Future<Output = Result<T, OpenCodeApiError>>,
    {
        let mut last_error = match request_fn().await {
            Ok(response) => {
                debug!("OpenCode API request succeeded");
                return Ok(response);
            }
            Err(err) => err,
        };

        for (attempt, delay) in self.backoff_delays.iter().enumerate() {
            if !last_error.is_retryable() {
                return Err(last_error);
            }
            warn!(
                attempt = attempt + 1,
                delay_secs = delay.as_secs_f64(),
                error = %last_error,
                "OpenCode API request failed; retrying"
            );
            sleep(*delay).await;
            match request_fn().await {
                Ok(response) => {
                    debug!("OpenCode API request succeeded after retry");
                    return Ok(response);
                }
                Err(err) => last_error = err,
            }
        }

        Err(last_error)
    }
}

fn load_basic_auth() -> Option<BasicAuth> {
    let password = std::env::var("OPENCODE_SERVER_PASSWORD").ok()?;
    let username =
        std::env::var("OPENCODE_SERVER_USERNAME").unwrap_or_else(|_| DEFAULT_USERNAME.to_string());
    Some(BasicAuth { username, password })
}

fn map_reqwest_error(error: reqwest::Error) -> OpenCodeApiError {
    if error.is_timeout() {
        OpenCodeApiError::Timeout
    } else {
        OpenCodeApiError::ConnectionFailed(error.to_string())
    }
}

async fn parse_json_response<T: DeserializeOwned>(
    response: Response,
) -> Result<T, OpenCodeApiError> {
    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    if status.is_success() {
        serde_json::from_str::<T>(&body)
            .map_err(|err| OpenCodeApiError::ParseError(err.to_string()))
    } else if status == StatusCode::NOT_FOUND {
        Err(OpenCodeApiError::NotFound(body))
    } else {
        Err(OpenCodeApiError::HttpStatus { status, body })
    }
}

async fn parse_sessions_response(response: Response) -> Result<Vec<Session>, OpenCodeApiError> {
    #[derive(Deserialize)]
    struct SessionsWrapper {
        sessions: Vec<Session>,
    }

    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    if !status.is_success() {
        return if status == StatusCode::NOT_FOUND {
            Err(OpenCodeApiError::NotFound(body))
        } else {
            Err(OpenCodeApiError::HttpStatus { status, body })
        };
    }

    serde_json::from_str::<Vec<Session>>(&body)
        .or_else(|_| serde_json::from_str::<SessionsWrapper>(&body).map(|wrapper| wrapper.sessions))
        .map_err(|err| OpenCodeApiError::ParseError(err.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::future::IntoFuture;
    use std::net::SocketAddr;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use axum::{
        Json, Router,
        routing::{get, post},
    };
    use tokio::net::TcpListener;

    fn test_client(base_url: String) -> OpenCodeClient {
        OpenCodeClient::with_base_url(base_url, vec![Duration::from_millis(5)])
    }

    async fn spawn_server(app: Router) -> (String, tokio::task::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr: SocketAddr = listener.local_addr().expect("addr");
        let server = axum::serve(listener, app).into_future();
        let handle = tokio::spawn(async move {
            let _ = server.await;
        });
        (format!("http://{}", addr), handle)
    }

    #[tokio::test]
    async fn health_returns_status() {
        async fn handler() -> Json<HealthResponse> {
            Json(HealthResponse {
                status: Some("ok".to_string()),
                healthy: Some(true),
                version: Some("dev".to_string()),
            })
        }

        let app = Router::new().route("/global/health", get(handler));
        let (base_url, handle) = spawn_server(app).await;

        let client = test_client(base_url);
        let response = client.health().await.expect("health response");

        handle.abort();
        assert_eq!(response.status.as_deref(), Some("ok"));
        assert_eq!(response.healthy, Some(true));
    }

    #[tokio::test]
    async fn list_sessions_parses_array_response() {
        async fn handler() -> Json<Vec<Session>> {
            Json(vec![Session {
                id: "session-1".to_string(),
                metadata: HashMap::new(),
            }])
        }

        let app = Router::new().route("/session", get(handler));
        let (base_url, handle) = spawn_server(app).await;

        let client = test_client(base_url);
        let sessions = client.list_sessions().await.expect("sessions");

        handle.abort();
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].id, "session-1");
    }

    #[tokio::test]
    async fn create_session_sends_prompt() {
        #[derive(Deserialize)]
        struct CreatePayload {
            prompt: String,
        }

        async fn handler(Json(payload): Json<CreatePayload>) -> Json<CreateSessionResponse> {
            assert_eq!(payload.prompt, "hello");
            Json(CreateSessionResponse {
                id: "session-42".to_string(),
            })
        }

        let app = Router::new().route("/session", post(handler));
        let (base_url, handle) = spawn_server(app).await;

        let client = test_client(base_url);
        let response = client
            .create_session("hello")
            .await
            .expect("create session");

        handle.abort();
        assert_eq!(response.id, "session-42");
    }

    #[tokio::test]
    async fn send_message_handles_not_found() {
        async fn handler() -> StatusCode {
            StatusCode::NOT_FOUND
        }

        let app = Router::new().route("/session/missing/message", post(handler));
        let (base_url, handle) = spawn_server(app).await;

        let client = test_client(base_url);
        let err = client
            .send_message("missing", "hello")
            .await
            .expect_err("expected error");

        handle.abort();
        assert!(matches!(err, OpenCodeApiError::NotFound(_)));
    }

    #[tokio::test]
    async fn retries_on_server_error() {
        let attempts = Arc::new(AtomicUsize::new(0));
        let attempts_handler = Arc::clone(&attempts);

        async fn handler(attempts: Arc<AtomicUsize>) -> Result<Json<Vec<Session>>, StatusCode> {
            let attempt = attempts.fetch_add(1, Ordering::SeqCst);
            if attempt < 1 {
                return Err(StatusCode::INTERNAL_SERVER_ERROR);
            }
            Ok(Json(vec![Session {
                id: "session-99".to_string(),
                metadata: HashMap::new(),
            }]))
        }

        let app = Router::new().route(
            "/session",
            get(move || handler(Arc::clone(&attempts_handler))),
        );
        let (base_url, handle) = spawn_server(app).await;

        let client = test_client(base_url);
        let sessions = client.list_sessions().await.expect("sessions");

        handle.abort();
        assert_eq!(sessions.len(), 1);
        assert!(attempts.load(Ordering::SeqCst) >= 2);
    }
}
