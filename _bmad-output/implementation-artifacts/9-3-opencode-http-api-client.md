# Story 9.3: OpenCode HTTP API Client

Status: ready-for-dev

## Story

As a daemon,
I want to interact with OpenCode via its HTTP API,
So that I can manage sessions, check health, and control OpenCode programmatically.

## Acceptance Criteria

**AC1: Health Check Endpoint**
**Given** OpenCode is running on configured port
**When** daemon calls `/global/health`
**Then** it receives health status response
**And** considers OpenCode healthy if status is 200 OK

**AC2: Session List Endpoint**
**Given** OpenCode API is available
**When** daemon calls `/session/list`
**Then** it receives list of active sessions
**And** parses session IDs and metadata

**AC3: Session Create Endpoint**
**Given** daemon needs to start new session
**When** it calls `/session/new` with prompt
**Then** a new OpenCode session is created
**And** response contains session ID

**AC4: Session Continue Endpoint**
**Given** an existing session ID
**When** daemon calls `/session/{id}/send` with message
**Then** the message is sent to the session
**And** response contains acknowledgment

**AC5: Connection Retry**
**Given** OpenCode API is temporarily unavailable
**When** daemon makes API request
**Then** it retries with exponential backoff
**And** gives up after max_retries (configurable)

**AC6: Timeout Handling**
**Given** API request is made
**When** response takes longer than timeout
**Then** request is aborted after `health_timeout_ms`
**And** error is logged

## Tasks / Subtasks

- [ ] Create HTTP client module (AC: 1, 5, 6)
  - [ ] Create `src/opencode/client.rs` module
  - [ ] Define `OpenCodeClient` struct with reqwest client
  - [ ] Configure client with timeout from config
  - [ ] Implement retry logic with backoff

- [ ] Implement health check (AC: 1, 6)
  - [ ] Add `health_check()` async method
  - [ ] GET `http://{hostname}:{port}/global/health`
  - [ ] Parse response and return health status
  - [ ] Handle connection refused, timeout errors

- [ ] Implement session list (AC: 2)
  - [ ] Add `list_sessions()` async method
  - [ ] GET `http://{hostname}:{port}/session/list`
  - [ ] Define `Session` struct for response parsing
  - [ ] Return `Vec<Session>` with IDs and metadata

- [ ] Implement session creation (AC: 3)
  - [ ] Add `create_session(prompt: &str)` async method
  - [ ] POST `http://{hostname}:{port}/session/new`
  - [ ] Send JSON body with initial prompt
  - [ ] Parse response for session ID

- [ ] Implement session message sending (AC: 4)
  - [ ] Add `send_message(session_id: &str, message: &str)` async method
  - [ ] POST `http://{hostname}:{port}/session/{id}/send`
  - [ ] Handle session not found error (404)
  - [ ] Return success/failure status

- [ ] Implement config endpoint access (AC: optional)
  - [ ] Add `get_config()` async method
  - [ ] GET `http://{hostname}:{port}/config`
  - [ ] Parse OpenCode configuration response

- [ ] Add error types (AC: 1, 2, 3, 4, 5, 6)
  - [ ] Define `OpenCodeApiError` enum
  - [ ] Variants: ConnectionFailed, Timeout, NotFound, ServerError, ParseError
  - [ ] Implement Display and Error traits

- [ ] Add unit/integration tests (AC: 1, 2, 3, 4, 5, 6)
  - [ ] Test health check with mock server
  - [ ] Test session list parsing
  - [ ] Test session creation flow
  - [ ] Test retry behavior on failure
  - [ ] Test timeout handling

## Dev Notes

### Architecture Requirements

**From architecture.md - FR47 (OpenCode HTTP API):**
> Daemon manages sessions via OpenCode HTTP API (`/session/*` endpoints)

**From architecture.md - Integration Points:**
> OpenCode Server API | `opencode/client.rs` | HTTP (REST API on port 4096)

**From architecture.md - Module Location:**
```
src/opencode/
    mod.rs                    # OpenCode integration root
    process.rs                # Process monitoring (Story 9.1)
    restart.rs                # Restart logic (Story 9.2)
    client.rs                 # HTTP client (THIS STORY)
    session.rs                # Session management (uses client)
```

### Technical Implementation

**OpenCode Client:**

```rust
// src/opencode/client.rs
use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Error types for OpenCode API interactions
#[derive(Debug, thiserror::Error)]
pub enum OpenCodeApiError {
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),
    
    #[error("Request timeout")]
    Timeout,
    
    #[error("Session not found: {0}")]
    NotFound(String),
    
    #[error("Server error: {0}")]
    ServerError(String),
    
    #[error("Failed to parse response: {0}")]
    ParseError(String),
}

/// OpenCode session information
#[derive(Debug, Clone, Deserialize)]
pub struct Session {
    pub id: String,
    pub created_at: String,
    pub status: String,
    #[serde(default)]
    pub messages_count: u32,
}

/// Response from session creation
#[derive(Debug, Deserialize)]
pub struct CreateSessionResponse {
    pub session_id: String,
}

/// Health check response
#[derive(Debug, Deserialize)]
pub struct HealthResponse {
    pub status: String,
    #[serde(default)]
    pub version: Option<String>,
}

/// HTTP client for OpenCode API
pub struct OpenCodeClient {
    client: Client,
    base_url: String,
    max_retries: u32,
}

impl OpenCodeClient {
    pub fn new(config: &OpenCodeConfig) -> anyhow::Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_millis(config.health_timeout_ms))
            .build()?;
        
        let base_url = format!(
            "http://{}:{}",
            config.serve_hostname,
            config.serve_port
        );
        
        Ok(Self {
            client,
            base_url,
            max_retries: config.max_api_retries.unwrap_or(3),
        })
    }

    /// Check OpenCode health status
    pub async fn health_check(&self) -> Result<HealthResponse, OpenCodeApiError> {
        let url = format!("{}/global/health", self.base_url);
        
        self.request_with_retry(|| async {
            let resp = self.client
                .get(&url)
                .send()
                .await
                .map_err(|e| {
                    if e.is_timeout() {
                        OpenCodeApiError::Timeout
                    } else {
                        OpenCodeApiError::ConnectionFailed(e.to_string())
                    }
                })?;
            
            if resp.status().is_success() {
                resp.json::<HealthResponse>()
                    .await
                    .map_err(|e| OpenCodeApiError::ParseError(e.to_string()))
            } else {
                Err(OpenCodeApiError::ServerError(
                    format!("Status: {}", resp.status())
                ))
            }
        }).await
    }

    /// List all active sessions
    pub async fn list_sessions(&self) -> Result<Vec<Session>, OpenCodeApiError> {
        let url = format!("{}/session/list", self.base_url);
        
        let resp = self.client
            .get(&url)
            .send()
            .await
            .map_err(|e| OpenCodeApiError::ConnectionFailed(e.to_string()))?;
        
        if resp.status().is_success() {
            resp.json::<Vec<Session>>()
                .await
                .map_err(|e| OpenCodeApiError::ParseError(e.to_string()))
        } else {
            Err(OpenCodeApiError::ServerError(
                format!("Status: {}", resp.status())
            ))
        }
    }

    /// Create a new session with initial prompt
    pub async fn create_session(&self, prompt: &str) -> Result<CreateSessionResponse, OpenCodeApiError> {
        let url = format!("{}/session/new", self.base_url);
        
        #[derive(Serialize)]
        struct CreateRequest<'a> {
            prompt: &'a str,
        }
        
        let resp = self.client
            .post(&url)
            .json(&CreateRequest { prompt })
            .send()
            .await
            .map_err(|e| OpenCodeApiError::ConnectionFailed(e.to_string()))?;
        
        match resp.status() {
            StatusCode::OK | StatusCode::CREATED => {
                resp.json::<CreateSessionResponse>()
                    .await
                    .map_err(|e| OpenCodeApiError::ParseError(e.to_string()))
            }
            status => Err(OpenCodeApiError::ServerError(
                format!("Status: {}", status)
            ))
        }
    }

    /// Send a message to an existing session
    pub async fn send_message(
        &self,
        session_id: &str,
        message: &str,
    ) -> Result<(), OpenCodeApiError> {
        let url = format!("{}/session/{}/send", self.base_url, session_id);
        
        #[derive(Serialize)]
        struct SendRequest<'a> {
            message: &'a str,
        }
        
        let resp = self.client
            .post(&url)
            .json(&SendRequest { message })
            .send()
            .await
            .map_err(|e| OpenCodeApiError::ConnectionFailed(e.to_string()))?;
        
        match resp.status() {
            StatusCode::OK | StatusCode::ACCEPTED => Ok(()),
            StatusCode::NOT_FOUND => Err(OpenCodeApiError::NotFound(session_id.to_string())),
            status => Err(OpenCodeApiError::ServerError(
                format!("Status: {}", status)
            ))
        }
    }

    /// Get OpenCode configuration
    pub async fn get_config(&self) -> Result<serde_json::Value, OpenCodeApiError> {
        let url = format!("{}/config", self.base_url);
        
        let resp = self.client
            .get(&url)
            .send()
            .await
            .map_err(|e| OpenCodeApiError::ConnectionFailed(e.to_string()))?;
        
        if resp.status().is_success() {
            resp.json::<serde_json::Value>()
                .await
                .map_err(|e| OpenCodeApiError::ParseError(e.to_string()))
        } else {
            Err(OpenCodeApiError::ServerError(
                format!("Status: {}", resp.status())
            ))
        }
    }

    /// Execute request with retry logic
    async fn request_with_retry<F, Fut, T>(
        &self,
        request_fn: F,
    ) -> Result<T, OpenCodeApiError>
    where
        F: Fn() -> Fut,
        Fut: std::future::Future<Output = Result<T, OpenCodeApiError>>,
    {
        let mut last_error = None;
        let mut delay = Duration::from_millis(100);
        
        for attempt in 0..self.max_retries {
            match request_fn().await {
                Ok(result) => return Ok(result),
                Err(e) => {
                    tracing::warn!(
                        attempt = attempt + 1,
                        max_retries = self.max_retries,
                        error = %e,
                        "API request failed, retrying"
                    );
                    last_error = Some(e);
                    
                    if attempt + 1 < self.max_retries {
                        tokio::time::sleep(delay).await;
                        delay *= 2; // Exponential backoff
                    }
                }
            }
        }
        
        Err(last_error.unwrap_or(OpenCodeApiError::ConnectionFailed(
            "Max retries exceeded".to_string()
        )))
    }
}
```

**Integration with Process Monitor:**

```rust
// src/opencode/process.rs (additions)
impl OpenCodeMonitor {
    /// Check if OpenCode is healthy via HTTP API
    pub async fn check_health_via_api(&self) -> bool {
        match &self.api_client {
            Some(client) => {
                match client.health_check().await {
                    Ok(health) => health.status == "ok",
                    Err(e) => {
                        tracing::debug!(error = %e, "Health check failed");
                        false
                    }
                }
            }
            None => false,
        }
    }
}
```

### OpenCode API Reference

**Known Endpoints (based on context):**
- `GET /global/health` - Health check
- `GET /session/list` - List sessions
- `POST /session/new` - Create session
- `POST /session/{id}/send` - Send message to session
- `GET /config` - Get configuration

**Default Configuration:**
- Port: 4096
- Hostname: localhost

### Dependencies

Already available:
- `reqwest` - HTTP client
- `serde` / `serde_json` - JSON serialization
- `thiserror` - Error types
- `tracing` - Logging

### Testing Strategy

**Unit Tests:**
- Test error type conversions
- Test retry logic with mock failures
- Test URL construction

**Integration Tests:**
- Use `wiremock` or `mockito` for HTTP mocking
- Test each endpoint with expected responses
- Test error handling for various status codes

**Manual Testing:**
1. Start `opencode serve`
2. Start daemon with OpenCode integration enabled
3. Check daemon logs for successful health checks
4. Verify session listing works

### References

- [Source: _bmad-output/planning-artifacts/architecture.md - Module: src/opencode/]
- [Source: _bmad-output/planning-artifacts/epics.md#Story 9.3]
- [Source: _bmad-output/planning-artifacts/prd.md - FR47]
- [Depends: Story 9.1 - OpenCode Process Detection]
- [Existing: src/http/client.rs - HTTP client patterns if any]

## File List

**Files to create:**
- `src/opencode/client.rs`
- `src/opencode/session.rs` (optional, for higher-level session management)

**Files to modify:**
- `src/opencode/mod.rs` (add client module)
- `src/opencode/process.rs` (integrate API health check)
- `_bmad-output/implementation-artifacts/sprint-status.yaml`

## Change Log

- 2026-02-06: Story created and marked ready-for-dev
