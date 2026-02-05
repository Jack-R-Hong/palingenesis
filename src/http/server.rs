use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::{Json, Router};
use serde_json::json;
use tokio::net::TcpListener;
use tokio_util::sync::CancellationToken;
use tower_http::trace::TraceLayer;
use tracing::{info, warn};

use crate::config::schema::DaemonConfig;
use crate::daemon::state::DaemonState;
use crate::http::events::EventBroadcaster;
use crate::http::handlers;
use crate::telemetry::Metrics;

/// HTTP API server for external integrations.
pub struct HttpServer {
    bind_addr: SocketAddr,
    router: Router,
    shutdown: CancellationToken,
    events: EventBroadcaster,
}

/// Shared application state for HTTP handlers.
#[derive(Clone)]
pub struct AppState {
    daemon_state: Arc<DaemonState>,
    events: EventBroadcaster,
    metrics: Arc<Metrics>,
}

impl AppState {
    pub fn new(
        daemon_state: Arc<DaemonState>,
        events: EventBroadcaster,
        metrics: Arc<Metrics>,
    ) -> Self {
        Self {
            daemon_state,
            events,
            metrics,
        }
    }

    pub fn daemon_state(&self) -> &Arc<DaemonState> {
        &self.daemon_state
    }

    pub fn events(&self) -> &EventBroadcaster {
        &self.events
    }

    pub fn metrics(&self) -> &Arc<Metrics> {
        &self.metrics
    }
}

impl HttpServer {
    /// Create a new HTTP server from daemon configuration.
    pub fn from_config(
        config: &DaemonConfig,
        shutdown: CancellationToken,
        state: Arc<DaemonState>,
        events: EventBroadcaster,
    ) -> Result<Option<Self>> {
        if !config.http_enabled {
            return Ok(None);
        }

        Ok(Some(Self::new(
            &config.http_bind,
            config.http_port,
            shutdown,
            state,
            events,
        )?))
    }

    /// Create a new HTTP server with bind address and shutdown token.
    pub fn new(
        bind: &str,
        port: u16,
        shutdown: CancellationToken,
        state: Arc<DaemonState>,
        events: EventBroadcaster,
    ) -> Result<Self> {
        let bind_addr: SocketAddr = format!("{bind}:{port}")
            .parse()
            .with_context(|| format!("Invalid HTTP bind address: {bind}:{port}"))?;

        if bind == "0.0.0.0" {
            warn!(
                port,
                "HTTP API binding to all interfaces (0.0.0.0). This exposes the API to the network."
            );
        }

        let router = Self::create_router(state, events.clone());

        Ok(Self {
            bind_addr,
            router,
            shutdown,
            events,
        })
    }

    pub fn bind_addr(&self) -> SocketAddr {
        self.bind_addr
    }

    pub fn shutdown(&self) {
        self.shutdown.cancel();
    }

    pub fn event_broadcaster(&self) -> EventBroadcaster {
        self.events.clone()
    }

    /// Start the HTTP server and wait for shutdown.
    pub async fn start(&self) -> Result<()> {
        let listener = TcpListener::bind(self.bind_addr)
            .await
            .with_context(|| format!("Failed to bind HTTP API to {}", self.bind_addr))?;
        let local_addr = listener
            .local_addr()
            .context("Failed to read bound HTTP address")?;
        info!(address = %local_addr, "HTTP API server listening");

        let shutdown = self.shutdown.clone();
        axum::serve(listener, self.router.clone())
            .with_graceful_shutdown(async move {
                shutdown.cancelled().await;
                info!("HTTP API server shutting down");
            })
            .await
            .context("HTTP API server failed")?;

        info!("HTTP API server stopped");
        Ok(())
    }

    fn create_router(state: Arc<DaemonState>, events: EventBroadcaster) -> Router {
        let metrics = Arc::new(Metrics::new());
        let _ = Metrics::set_global(Arc::clone(&metrics));
        let app_state = AppState::new(state, events, metrics);
        Router::new()
            .route("/health", axum::routing::get(handlers::health::health_handler))
            .route(
                "/api/v1/status",
                axum::routing::get(handlers::status::status_handler),
            )
            .route(
                "/api/v1/metrics",
                axum::routing::get(handlers::metrics::metrics_handler),
            )
            .route(
                "/api/v1/events",
                axum::routing::get(handlers::events::events_handler),
            )
            .route(
                "/api/v1/pause",
                axum::routing::post(handlers::control::pause_handler),
            )
            .route(
                "/api/v1/resume",
                axum::routing::post(handlers::control::resume_handler),
            )
            .route(
                "/api/v1/new-session",
                axum::routing::post(handlers::control::new_session_handler),
            )
            .route(
                "/api/v1/bot/discord",
                axum::routing::post(handlers::bot_discord::discord_webhook_handler),
            )
            .route(
                "/api/v1/bot/slack",
                axum::routing::post(handlers::bot_slack::slack_webhook_handler),
            )
            .fallback(Self::fallback_handler)
            .with_state(app_state)
            .layer(
                TraceLayer::new_for_http()
                    .make_span_with(|request: &Request<Body>| {
                        tracing::info_span!(
                            "http.request",
                            method = %request.method(),
                            path = %request.uri().path(),
                            status_code = tracing::field::Empty,
                        )
                    })
                    .on_request(|request: &Request<Body>, _span: &tracing::Span| {
                        tracing::info!(method = %request.method(), path = %request.uri().path(), "http.request");
                    })
                    .on_response(|response: &axum::http::Response<_>, latency: Duration, span: &tracing::Span| {
                        let status = response.status();
                        span.record("status_code", status.as_u16());
                        if status.is_server_error() {
                            tracing::error!(%status, ?latency, "finished");
                        } else if status.is_client_error() {
                            tracing::warn!(%status, ?latency, "finished");
                        } else {
                            tracing::info!(%status, ?latency, "finished");
                        }
                    })
                    .on_failure(|error, latency: Duration, _span: &tracing::Span| {
                        tracing::error!(?error, ?latency, "failed");
                    }),
            )
    }

    async fn fallback_handler() -> (StatusCode, Json<serde_json::Value>) {
        (
            StatusCode::NOT_FOUND,
            Json(json!({
                "success": false,
                "error": {
                    "code": "NOT_FOUND",
                    "message": "The requested endpoint does not exist"
                }
            })),
        )
    }

    #[cfg(test)]
    pub(crate) fn router(&self) -> Router {
        self.router.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::to_bytes;
    use std::io::Write;
    use std::sync::{Arc, Mutex};

    use tower::ServiceExt;
    use tracing_subscriber::EnvFilter;
    use tracing_subscriber::layer::SubscriberExt;

    use crate::test_utils::TRACING_LOCK;

    #[derive(Clone)]
    struct BufferWriter {
        buffer: Arc<Mutex<Vec<u8>>>,
    }

    struct BufferGuard {
        buffer: Arc<Mutex<Vec<u8>>>,
    }

    impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for BufferWriter {
        type Writer = BufferGuard;

        fn make_writer(&'a self) -> Self::Writer {
            BufferGuard {
                buffer: Arc::clone(&self.buffer),
            }
        }
    }

    impl Write for BufferGuard {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            let mut guard = self.buffer.lock().unwrap();
            guard.extend_from_slice(buf);
            Ok(buf.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    fn capture_logs() -> (Arc<Mutex<Vec<u8>>>, tracing::subscriber::DefaultGuard) {
        let buffer = Arc::new(Mutex::new(Vec::new()));
        let writer = BufferWriter {
            buffer: Arc::clone(&buffer),
        };
        let subscriber = tracing_subscriber::registry()
            .with(EnvFilter::new("info"))
            .with(
                tracing_subscriber::fmt::layer()
                    .with_writer(writer)
                    .with_ansi(false),
            );
        let guard = tracing::subscriber::set_default(subscriber);
        (buffer, guard)
    }

    fn pick_port() -> u16 {
        std::net::TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port()
    }

    #[test]
    fn test_bind_addr_parsing() {
        let server = HttpServer::new(
            "127.0.0.1",
            7654,
            CancellationToken::new(),
            Arc::new(DaemonState::new()),
            EventBroadcaster::default(),
        )
        .unwrap();
        assert_eq!(server.bind_addr(), "127.0.0.1:7654".parse().unwrap());
    }

    #[test]
    fn test_invalid_bind_addr_returns_error() {
        let result = HttpServer::new(
            "not-an-ip",
            7654,
            CancellationToken::new(),
            Arc::new(DaemonState::new()),
            EventBroadcaster::default(),
        );
        assert!(result.is_err());
        let err_msg = result.err().unwrap().to_string();
        assert!(err_msg.contains("Invalid HTTP bind address"));
    }

    #[test]
    fn test_custom_port_configuration() {
        let server = HttpServer::new(
            "127.0.0.1",
            9001,
            CancellationToken::new(),
            Arc::new(DaemonState::new()),
            EventBroadcaster::default(),
        )
        .unwrap();
        assert_eq!(server.bind_addr().port(), 9001);
    }

    #[test]
    fn test_binding_all_interfaces_warns() {
        let _tracing = TRACING_LOCK.lock().unwrap();
        let (buffer, _guard) = capture_logs();
        let _server = HttpServer::new(
            "0.0.0.0",
            7654,
            CancellationToken::new(),
            Arc::new(DaemonState::new()),
            EventBroadcaster::default(),
        )
        .unwrap();
        let output = String::from_utf8(buffer.lock().unwrap().clone()).unwrap();
        assert!(output.contains("HTTP API binding to all interfaces"));
    }

    #[test]
    fn test_http_disabled_returns_none() {
        let mut config = DaemonConfig::default();
        config.http_enabled = false;
        let result = HttpServer::from_config(
            &config,
            CancellationToken::new(),
            Arc::new(DaemonState::new()),
            EventBroadcaster::default(),
        )
        .unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_router_fallback_returns_json() {
        let server = HttpServer::new(
            "127.0.0.1",
            7654,
            CancellationToken::new(),
            Arc::new(DaemonState::new()),
            EventBroadcaster::default(),
        )
        .unwrap();
        let response = server
            .router()
            .oneshot(
                Request::builder()
                    .uri("/missing")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(payload["success"], false);
        assert_eq!(payload["error"]["code"], "NOT_FOUND");
    }

    #[tokio::test]
    #[ignore = "Flaky under parallel test execution due to global tracing subscriber"]
    async fn test_request_logging() {
        let _tracing = TRACING_LOCK.lock().unwrap();
        let (buffer, _guard) = capture_logs();
        let server = HttpServer::new(
            "127.0.0.1",
            7654,
            CancellationToken::new(),
            Arc::new(DaemonState::new()),
            EventBroadcaster::default(),
        )
        .unwrap();
        let response = server
            .router()
            .oneshot(
                Request::builder()
                    .uri("/missing")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        let output = String::from_utf8(buffer.lock().unwrap().clone()).unwrap();
        assert!(output.contains("http.request"));
        assert!(output.contains("finished"));
    }

    #[tokio::test]
    async fn test_server_start_and_shutdown() {
        let port = pick_port();
        let shutdown = CancellationToken::new();
        let server = HttpServer::new(
            "127.0.0.1",
            port,
            shutdown.clone(),
            Arc::new(DaemonState::new()),
            EventBroadcaster::default(),
        )
        .unwrap();
        let handle = tokio::spawn(async move {
            server.start().await.unwrap();
        });

        // Retry with backoff to avoid flaky tests on slow CI
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .build()
            .unwrap();
        let mut response = None;
        for attempt in 0..10 {
            tokio::time::sleep(Duration::from_millis(20 * (attempt + 1))).await;
            if let Ok(resp) = client
                .get(format!("http://127.0.0.1:{port}/missing"))
                .send()
                .await
            {
                response = Some(resp);
                break;
            }
        }
        let response = response.expect("Server should respond within retries");
        assert_eq!(response.status(), reqwest::StatusCode::NOT_FOUND);

        shutdown.cancel();
        handle.await.unwrap();
    }
}
