# Story 6.1: HTTP API Server Setup

Status: done

## Story

As a daemon,
I want to run an HTTP API server,
So that external tools can monitor and control me.

## Acceptance Criteria

**AC1: Default Server Binding**
**Given** the daemon starts with HTTP API enabled
**When** initialization completes
**Then** axum server listens on `127.0.0.1:7654`

**AC2: Custom Port Configuration**
**Given** config specifies different port
**When** server starts
**Then** it uses the configured port

**AC3: Bind to All Interfaces**
**Given** config specifies bind address `0.0.0.0`
**When** server starts
**Then** it binds to all interfaces (warns about security)

**AC4: Request Logging**
**Given** the server is running
**When** any request is received
**Then** it is logged with tracing middleware

## Tasks / Subtasks

- [ ] Create HTTP server module structure (AC: 1)
  - [ ] Create `src/http/server.rs`
  - [ ] Define `HttpServer` struct with config, router, and shutdown handle
  - [ ] Implement `HttpServer::new()` constructor
  - [ ] Update `src/http/mod.rs` to export server module

- [ ] Implement server startup (AC: 1, 2)
  - [ ] Implement `HttpServer::start()` async method
  - [ ] Parse bind address and port from config
  - [ ] Create `tokio::net::TcpListener` on configured address
  - [ ] Start axum server with graceful shutdown signal
  - [ ] Log server start with bind address

- [ ] Implement address configuration (AC: 2, 3)
  - [ ] Read `http_bind` from daemon config
  - [ ] Read `http_port` from daemon config
  - [ ] Validate bind address format
  - [ ] Warn when binding to `0.0.0.0` (security risk)

- [ ] Add tracing middleware (AC: 4)
  - [ ] Configure `tower_http::trace::TraceLayer`
  - [ ] Log request method, path, status code, and latency
  - [ ] Use appropriate log levels (info for success, warn/error for failures)

- [ ] Implement graceful shutdown (AC: 1)
  - [ ] Accept shutdown signal channel in constructor
  - [ ] Wire shutdown signal to axum's `with_graceful_shutdown`
  - [ ] Log shutdown initiation and completion
  - [ ] Implement `HttpServer::shutdown()` method

- [ ] Create placeholder router (AC: 1)
  - [ ] Define base router with fallback handler
  - [ ] Return 404 JSON for unknown routes
  - [ ] Prepare router structure for future endpoint additions

- [ ] Integrate with daemon lifecycle (AC: 1)
  - [ ] Start HTTP server in daemon startup if `http_enabled`
  - [ ] Store server handle in daemon state
  - [ ] Shutdown HTTP server during daemon shutdown
  - [ ] Handle server errors gracefully

- [ ] Add unit/integration tests (AC: 1, 2, 3, 4)
  - [ ] Test server starts on default port
  - [ ] Test server uses configured port
  - [ ] Test 0.0.0.0 binding produces warning
  - [ ] Test request logging
  - [ ] Test graceful shutdown
  - [ ] Test server disabled when http_enabled=false

## Dev Notes

### Architecture Requirements

**From architecture.md - ARCH6 (HTTP API):**
> Optional HTTP server (axum) for external integrations. REST endpoints mirror IPC commands.

**From architecture.md - ARCH14 (Technology Stack):**
> axum for HTTP server, tower-http for middleware (tracing, CORS, timeout)

**Module Location:**

```
src/http/
    mod.rs                    # Module root
    server.rs                 # HTTP server setup (THIS STORY)
    handlers/                 # Endpoint handlers (future stories)
```

### Technical Implementation

**Server Structure:**

```rust
// src/http/server.rs
use std::net::SocketAddr;
use axum::{Router, routing::get, Json};
use tokio::net::TcpListener;
use tokio::sync::watch;
use tower_http::trace::TraceLayer;
use tracing::{info, warn};

/// HTTP API server for external integrations
pub struct HttpServer {
    bind_addr: SocketAddr,
    router: Router,
    shutdown_rx: watch::Receiver<bool>,
}

impl HttpServer {
    /// Create new HTTP server with config
    pub fn new(
        bind: &str,
        port: u16,
        shutdown_rx: watch::Receiver<bool>,
    ) -> anyhow::Result<Self> {
        let bind_addr: SocketAddr = format!("{}:{}", bind, port).parse()?;
        
        // Warn about security implications
        if bind == "0.0.0.0" {
            warn!(
                "HTTP API binding to all interfaces (0.0.0.0:{}). \
                 This exposes the API to the network!",
                port
            );
        }
        
        let router = Self::create_router();
        
        Ok(Self {
            bind_addr,
            router,
            shutdown_rx,
        })
    }
    
    /// Start the HTTP server
    pub async fn start(&self) -> anyhow::Result<()> {
        let listener = TcpListener::bind(self.bind_addr).await?;
        info!("HTTP API server listening on {}", self.bind_addr);
        
        let shutdown_rx = self.shutdown_rx.clone();
        
        axum::serve(listener, self.router.clone())
            .with_graceful_shutdown(async move {
                let mut rx = shutdown_rx;
                while !*rx.borrow() {
                    if rx.changed().await.is_err() {
                        break;
                    }
                }
                info!("HTTP server shutting down gracefully");
            })
            .await?;
        
        Ok(())
    }
    
    /// Create the router with middleware
    fn create_router() -> Router {
        Router::new()
            // Placeholder: endpoints added in future stories
            .fallback(Self::fallback_handler)
            .layer(
                TraceLayer::new_for_http()
                    .make_span_with(|request: &axum::http::Request<_>| {
                        tracing::info_span!(
                            "http_request",
                            method = %request.method(),
                            uri = %request.uri(),
                        )
                    })
            )
    }
    
    /// Handler for unknown routes
    async fn fallback_handler() -> (axum::http::StatusCode, Json<serde_json::Value>) {
        (
            axum::http::StatusCode::NOT_FOUND,
            Json(serde_json::json!({
                "error": "not_found",
                "message": "The requested endpoint does not exist"
            }))
        )
    }
}
```

**Daemon Integration:**

```rust
// src/daemon/core.rs (modifications)
use crate::http::HttpServer;

impl Daemon {
    pub async fn start(&mut self) -> anyhow::Result<()> {
        // ... existing startup code ...
        
        // Start HTTP server if enabled
        let config = self.config.read().await;
        if config.daemon.http_enabled {
            let http_server = HttpServer::new(
                &config.daemon.http_bind,
                config.daemon.http_port,
                self.shutdown_rx.clone(),
            )?;
            
            // Spawn HTTP server task
            let http_handle = tokio::spawn(async move {
                if let Err(e) = http_server.start().await {
                    tracing::error!("HTTP server error: {}", e);
                }
            });
            
            self.http_handle = Some(http_handle);
        }
        
        Ok(())
    }
}
```

**Configuration (already exists in schema.rs):**

```rust
// src/config/schema.rs - already implemented
pub struct DaemonConfig {
    /// Enable HTTP API server
    pub http_enabled: bool,  // default: false
    /// HTTP API port
    pub http_port: u16,      // default: 7654
    /// HTTP API bind address
    pub http_bind: String,   // default: "127.0.0.1"
}
```

### Request Logging Format

Using tower-http TraceLayer, requests will be logged as:

```
INFO http_request{method=GET uri=/api/v1/status}: started
INFO http_request{method=GET uri=/api/v1/status}: finished latency=5ms status=200
```

### Security Considerations

1. **Default binding to localhost only** - prevents accidental network exposure
2. **Explicit warning for 0.0.0.0** - user must acknowledge security risk
3. **No authentication in MVP** - rely on localhost binding for security
4. **Future: Add API key authentication** (out of scope for this story)

### Dependencies

Already in Cargo.toml:
- `axum = "0.8.8"`
- `tower = "0.5.3"`
- `tower-http = { version = "0.6.8", features = ["trace", "timeout", "cors"] }`

### Testing Strategy

**Unit Tests:**
- Test address parsing and validation
- Test 0.0.0.0 warning is produced
- Test router fallback returns 404 JSON

**Integration Tests:**
- Start server, make request, verify response
- Test graceful shutdown
- Test server respects http_enabled=false

**Test Example:**

```rust
#[tokio::test]
async fn test_server_starts_on_default_port() {
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let server = HttpServer::new("127.0.0.1", 7654, shutdown_rx).unwrap();
    
    let handle = tokio::spawn(async move {
        server.start().await.unwrap();
    });
    
    // Give server time to start
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    // Make request
    let client = reqwest::Client::new();
    let resp = client.get("http://127.0.0.1:7654/nonexistent")
        .send()
        .await
        .unwrap();
    
    assert_eq!(resp.status(), 404);
    
    // Shutdown
    shutdown_tx.send(true).unwrap();
    handle.await.unwrap();
}
```

### References

- [Source: _bmad-output/planning-artifacts/architecture.md#ARCH6]
- [Source: _bmad-output/planning-artifacts/architecture.md#ARCH14]
- [Source: _bmad-output/planning-artifacts/epics.md#Story 6.1: HTTP API Server Setup]
- [Existing: src/ipc/protocol.rs - Command/Response pattern reference]
- [Existing: src/config/schema.rs - http_* config fields]

## File List

**Files to create:**
- `src/http/server.rs`

**Files to modify:**
- `src/http/mod.rs`
- `src/daemon/core.rs` (or equivalent daemon entry point)
- `_bmad-output/implementation-artifacts/sprint-status.yaml`

## Change Log

- 2026-02-06: Story created and marked ready-for-dev
