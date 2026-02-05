# Story 1.6: Unix Socket IPC Server

Status: done

## Story

As a daemon,
I want to listen on a Unix socket for CLI commands,
So that CLI tools can communicate with me efficiently.

## Acceptance Criteria

**AC1: Unix Socket Creation on Daemon Start**
**Given** the daemon starts
**When** it initializes IPC
**Then** it creates a Unix socket at `{runtime_dir}/palingenesis.sock`
**And** the socket accepts connections

**AC2: STATUS Command Handling**
**Given** a CLI client connects to the socket
**When** it sends `STATUS\n`
**Then** the daemon responds with JSON status
**And** the connection closes cleanly

**AC3: PAUSE Command Handling**
**Given** a CLI client sends `PAUSE\n`
**When** the daemon receives the command
**Then** it transitions to paused state
**And** responds with `OK\n`

**AC4: RESUME Command Handling**
**Given** a CLI client sends `RESUME\n`
**When** the daemon receives the command
**Then** it transitions to monitoring state
**And** responds with `OK\n`

**AC5: RELOAD Command Handling**
**Given** a CLI client sends `RELOAD\n`
**When** the daemon receives the command
**Then** it reloads configuration
**And** responds with `OK\n` or `ERR: {message}\n`

**AC6: Stale Socket Cleanup**
**Given** the socket path already exists
**When** the daemon starts
**Then** it removes the stale socket file
**And** creates a new socket

**AC7: Socket Cleanup on Shutdown**
**Given** the daemon shuts down
**When** cleanup runs
**Then** the socket file is removed

## Tasks / Subtasks

- [x] Create IPC module structure (AC: 1, 6, 7)
  - [x] Create `src/ipc/socket.rs` with IpcServer struct
  - [x] Create `src/ipc/protocol.rs` with Command and Response types
  - [x] Update `src/ipc/mod.rs` to export modules
- [x] Implement IPC protocol types (AC: 2, 3, 4, 5)
  - [x] Define `IpcCommand` enum (Status, Pause, Resume, Reload)
  - [x] Define `IpcResponse` enum (Ok, Error, Status)
  - [x] Implement parsing from text commands (`STATUS\n`, etc.)
  - [x] Implement response serialization
- [x] Implement IpcServer struct (AC: 1, 6)
  - [x] Define `IpcError` enum with thiserror (Io, Protocol, AlreadyBound)
  - [x] Implement `IpcServer::new()` - uses `Paths::runtime_dir().join("palingenesis.sock")`
  - [x] Implement stale socket detection and removal
  - [x] Implement `IpcServer::bind()` - creates and binds UnixListener
- [x] Implement connection handling (AC: 2, 3, 4, 5)
  - [x] Implement `IpcServer::run()` - accepts connections in loop
  - [x] Implement `handle_connection()` - read command, process, respond
  - [x] Implement command dispatch to daemon state
  - [x] Handle connection timeouts (5s)
- [x] Implement graceful shutdown integration (AC: 7)
  - [x] Accept CancellationToken for graceful shutdown
  - [x] Remove socket file on shutdown
  - [x] Implement `Drop` trait for automatic cleanup
- [x] Integrate with daemon state (AC: 2, 3, 4, 5)
  - [x] Accept shared daemon state (Arc<RwLock<DaemonState>>)
  - [x] STATUS returns JSON serialized state
  - [x] PAUSE/RESUME modify daemon state
  - [x] RELOAD triggers config reload
- [x] Add unit tests (AC: 1, 2, 3, 4, 5, 6, 7)
  - [x] Test socket creation and binding
  - [x] Test stale socket cleanup
  - [x] Test command parsing
  - [x] Test response serialization
  - [x] Test STATUS command handling
  - [x] Test PAUSE/RESUME command handling
  - [x] Test connection timeout
  - [x] Test cleanup on drop
- [x] Add integration tests
  - [x] Test full IPC server lifecycle
  - [x] Test concurrent client connections
  - [x] Test graceful shutdown with CancellationToken

## Dev Notes

### Architecture Requirements

**From architecture.md - API & Communication:**

> | Decision | Choice | Rationale |
> |----------|--------|-----------|
> | **CLI-Daemon IPC** | Hybrid (Unix socket + HTTP) | Unix socket for fast CLI commands, HTTP API for external integrations. |
> | **Socket Path** | `/run/user/{uid}/palingenesis.sock` | XDG runtime dir, secure, no port conflicts. |

**From architecture.md - Unix Socket Commands:**

```
STATUS  -> JSON status response
PAUSE   -> OK / ERR
RESUME  -> OK / ERR
RELOAD  -> OK / ERR (config reload)
```

**From architecture.md - Project Structure:**

```
src/ipc/
    mod.rs                    # IPC module root
    socket.rs                 # Unix socket server
    protocol.rs               # Command/response protocol
    client.rs                 # CLI -> daemon client (Story 1.7)
```

**Implements:** ARCH13, ARCH15

### Technical Implementation

**IPC Protocol Types:**

```rust
// src/ipc/protocol.rs
use serde::{Deserialize, Serialize};

/// Commands that can be sent to the daemon via Unix socket.
#[derive(Debug, Clone, PartialEq)]
pub enum IpcCommand {
    /// Request current daemon status
    Status,
    /// Pause session monitoring
    Pause,
    /// Resume session monitoring
    Resume,
    /// Reload configuration file
    Reload,
}

impl IpcCommand {
    /// Parse command from text line (without newline).
    pub fn parse(line: &str) -> Option<Self> {
        match line.trim().to_uppercase().as_str() {
            "STATUS" => Some(Self::Status),
            "PAUSE" => Some(Self::Pause),
            "RESUME" => Some(Self::Resume),
            "RELOAD" => Some(Self::Reload),
            _ => None,
        }
    }
}

/// Response types from the daemon.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum IpcResponse {
    /// Success response
    Ok,
    /// Error response with message
    Error { message: String },
    /// Status response with JSON data
    Status(DaemonStatus),
}

/// Daemon status for STATUS command response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaemonStatus {
    pub state: String,
    pub uptime_secs: u64,
    pub current_session: Option<String>,
    pub saves_count: u64,
    pub total_resumes: u64,
}

impl IpcResponse {
    /// Serialize response to text format.
    pub fn to_text(&self) -> String {
        match self {
            Self::Ok => "OK\n".to_string(),
            Self::Error { message } => format!("ERR: {}\n", message),
            Self::Status(status) => {
                // JSON followed by newline
                serde_json::to_string(status).unwrap_or_default() + "\n"
            }
        }
    }
}
```

**IpcServer Implementation:**

```rust
// src/ipc/socket.rs
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, warn};

use crate::config::Paths;
use crate::ipc::protocol::{DaemonStatus, IpcCommand, IpcResponse};

const CONNECTION_TIMEOUT_SECS: u64 = 5;

#[derive(Debug, thiserror::Error)]
pub enum IpcError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Protocol error: {0}")]
    Protocol(String),

    #[error("Socket already bound")]
    AlreadyBound,

    #[error("Socket path does not exist")]
    NotBound,
}

/// Shared state that the IPC server can access.
pub trait DaemonStateAccess: Send + Sync {
    fn get_status(&self) -> DaemonStatus;
    fn pause(&self) -> Result<(), String>;
    fn resume(&self) -> Result<(), String>;
    fn reload_config(&self) -> Result<(), String>;
}

pub struct IpcServer {
    path: PathBuf,
    listener: Option<UnixListener>,
}

impl IpcServer {
    /// Create a new IpcServer instance pointing to the standard location.
    pub fn new() -> Self {
        Self {
            path: Paths::runtime_dir().join("palingenesis.sock"),
            listener: None,
        }
    }

    /// Create with custom path (for testing).
    pub fn with_path(path: PathBuf) -> Self {
        Self {
            path,
            listener: None,
        }
    }

    /// Returns the socket path.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Bind and start listening on the Unix socket.
    pub async fn bind(&mut self) -> Result<(), IpcError> {
        if self.listener.is_some() {
            return Err(IpcError::AlreadyBound);
        }

        // Remove stale socket if exists
        if self.path.exists() {
            warn!(path = %self.path.display(), "Removing stale socket file");
            std::fs::remove_file(&self.path)?;
        }

        // Ensure runtime directory exists
        Paths::ensure_runtime_dir()?;

        // Bind to socket
        let listener = UnixListener::bind(&self.path)?;
        info!(path = %self.path.display(), "IPC socket bound");

        // Set socket permissions to 600 (owner only)
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&self.path, std::fs::Permissions::from_mode(0o600))?;
        }

        self.listener = Some(listener);
        Ok(())
    }

    /// Run the IPC server, accepting connections until cancellation.
    pub async fn run<S: DaemonStateAccess + 'static>(
        &self,
        state: Arc<S>,
        cancel: CancellationToken,
    ) -> Result<(), IpcError> {
        let listener = self.listener.as_ref().ok_or(IpcError::NotBound)?;

        loop {
            tokio::select! {
                _ = cancel.cancelled() => {
                    info!("IPC server shutting down");
                    break;
                }
                result = listener.accept() => {
                    match result {
                        Ok((stream, _addr)) => {
                            let state = Arc::clone(&state);
                            tokio::spawn(async move {
                                if let Err(e) = handle_connection(stream, state).await {
                                    debug!(error = %e, "Connection handling error");
                                }
                            });
                        }
                        Err(e) => {
                            error!(error = %e, "Failed to accept connection");
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Remove the socket file (call on shutdown).
    pub fn cleanup(&self) -> Result<(), IpcError> {
        if self.path.exists() {
            std::fs::remove_file(&self.path)?;
            info!(path = %self.path.display(), "IPC socket removed");
        }
        Ok(())
    }
}

impl Drop for IpcServer {
    fn drop(&mut self) {
        if let Err(e) = self.cleanup() {
            eprintln!("Warning: Failed to clean up IPC socket: {}", e);
        }
    }
}

impl Default for IpcServer {
    fn default() -> Self {
        Self::new()
    }
}

/// Handle a single client connection.
async fn handle_connection<S: DaemonStateAccess>(
    stream: UnixStream,
    state: Arc<S>,
) -> Result<(), IpcError> {
    let (reader, mut writer) = stream.into_split();
    let mut reader = BufReader::new(reader);
    let mut line = String::new();

    // Read command with timeout
    let read_result = tokio::time::timeout(
        std::time::Duration::from_secs(CONNECTION_TIMEOUT_SECS),
        reader.read_line(&mut line),
    )
    .await;

    let response = match read_result {
        Ok(Ok(0)) => {
            // EOF - client disconnected
            return Ok(());
        }
        Ok(Ok(_)) => {
            // Parse and handle command
            match IpcCommand::parse(&line) {
                Some(cmd) => handle_command(cmd, &*state),
                None => IpcResponse::Error {
                    message: format!("Unknown command: {}", line.trim()),
                },
            }
        }
        Ok(Err(e)) => {
            return Err(IpcError::Io(e));
        }
        Err(_) => IpcResponse::Error {
            message: "Connection timeout".to_string(),
        },
    };

    // Send response
    writer.write_all(response.to_text().as_bytes()).await?;
    writer.flush().await?;

    Ok(())
}

/// Dispatch command to appropriate handler.
fn handle_command<S: DaemonStateAccess>(cmd: IpcCommand, state: &S) -> IpcResponse {
    match cmd {
        IpcCommand::Status => IpcResponse::Status(state.get_status()),
        IpcCommand::Pause => match state.pause() {
            Ok(()) => IpcResponse::Ok,
            Err(msg) => IpcResponse::Error { message: msg },
        },
        IpcCommand::Resume => match state.resume() {
            Ok(()) => IpcResponse::Ok,
            Err(msg) => IpcResponse::Error { message: msg },
        },
        IpcCommand::Reload => match state.reload_config() {
            Ok(()) => IpcResponse::Ok,
            Err(msg) => IpcResponse::Error { message: msg },
        },
    }
}
```

### Dependencies

Uses existing dependencies:
- `tokio` (already in Cargo.toml) - async runtime, UnixListener
- `tokio-util` (add for CancellationToken)
- `tracing` (already in Cargo.toml)
- `thiserror` (already in Cargo.toml)
- `serde` (already in Cargo.toml)
- `serde_json` (already in Cargo.toml)

**Add to Cargo.toml:**

```toml
tokio-util = { version = "0.7", features = ["sync"] }
```

### Error Handling Pattern

Uses `thiserror` following project conventions from architecture.md:
- `IpcError::Io` - Socket/file system operations failed
- `IpcError::Protocol` - Invalid command or response format
- `IpcError::AlreadyBound` - Socket already listening
- `IpcError::NotBound` - Attempted to run without binding first

### Testing Strategy

**Unit Tests:**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    struct MockState {
        paused: std::sync::atomic::AtomicBool,
    }

    impl DaemonStateAccess for MockState {
        fn get_status(&self) -> DaemonStatus {
            DaemonStatus {
                state: if self.paused.load(std::sync::atomic::Ordering::SeqCst) {
                    "paused".to_string()
                } else {
                    "monitoring".to_string()
                },
                uptime_secs: 3600,
                current_session: Some("/path/to/session.md".to_string()),
                saves_count: 42,
                total_resumes: 10,
            }
        }

        fn pause(&self) -> Result<(), String> {
            self.paused.store(true, std::sync::atomic::Ordering::SeqCst);
            Ok(())
        }

        fn resume(&self) -> Result<(), String> {
            self.paused.store(false, std::sync::atomic::Ordering::SeqCst);
            Ok(())
        }

        fn reload_config(&self) -> Result<(), String> {
            Ok(())
        }
    }

    #[test]
    fn test_command_parsing() {
        assert_eq!(IpcCommand::parse("STATUS"), Some(IpcCommand::Status));
        assert_eq!(IpcCommand::parse("status\n"), Some(IpcCommand::Status));
        assert_eq!(IpcCommand::parse("PAUSE"), Some(IpcCommand::Pause));
        assert_eq!(IpcCommand::parse("RESUME"), Some(IpcCommand::Resume));
        assert_eq!(IpcCommand::parse("RELOAD"), Some(IpcCommand::Reload));
        assert_eq!(IpcCommand::parse("UNKNOWN"), None);
    }

    #[test]
    fn test_response_serialization() {
        assert_eq!(IpcResponse::Ok.to_text(), "OK\n");
        assert_eq!(
            IpcResponse::Error { message: "test".to_string() }.to_text(),
            "ERR: test\n"
        );
    }

    #[tokio::test]
    async fn test_socket_bind_and_cleanup() {
        let temp = tempdir().unwrap();
        let sock_path = temp.path().join("test.sock");

        let mut server = IpcServer::with_path(sock_path.clone());
        server.bind().await.unwrap();

        assert!(sock_path.exists());

        server.cleanup().unwrap();
        assert!(!sock_path.exists());
    }

    #[tokio::test]
    async fn test_stale_socket_removal() {
        let temp = tempdir().unwrap();
        let sock_path = temp.path().join("test.sock");

        // Create stale socket file
        std::fs::write(&sock_path, "stale").unwrap();

        let mut server = IpcServer::with_path(sock_path.clone());
        server.bind().await.unwrap();

        // Should have replaced stale file with actual socket
        assert!(sock_path.exists());
    }
}
```

**Integration Tests:**

```rust
// tests/ipc_test.rs
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;

#[tokio::test]
async fn test_ipc_status_command() {
    // Setup server with mock state
    // Connect client
    // Send STATUS command
    // Verify JSON response
}

#[tokio::test]
async fn test_ipc_concurrent_connections() {
    // Multiple clients connecting simultaneously
}
```

### Previous Story Learnings

From Story 1-3 (Platform-Specific Path Resolution):
1. **Runtime directory**: Use `Paths::runtime_dir()` for socket location
2. **Directory creation**: Use `Paths::ensure_runtime_dir()` before binding
3. **Platform differences**: Linux uses `/run/user/{uid}/`, macOS uses `/tmp/palingenesis-{uid}/`

From Story 1-4 (State Persistence Layer):
1. **Error handling**: Use `thiserror` for domain errors
2. **File permissions**: Set socket to 600 (owner only)
3. **Cleanup on Drop**: Implement `Drop` trait for automatic resource cleanup

From Story 1-5 (PID File Management):
1. **Stale file handling**: Check and remove stale files before creating new
2. **Graceful cleanup**: Remove files on shutdown, implement Drop
3. **Race conditions**: Handle concurrent access appropriately

### Project Structure Notes

- This story creates `src/ipc/socket.rs` and `src/ipc/protocol.rs`
- IpcServer will be used by:
  - Story 1-7 (Unix Socket IPC Client) - client implementation
  - Story 1-8 (Daemon Start Command) - start IPC server
  - Story 1-10 (Daemon Status Command) - send STATUS via IPC
  - Story 3-7 (Pause Command) - send PAUSE via IPC
  - Story 3-8 (Resume Command) - send RESUME via IPC
  - Story 4-6 (Hot Reload) - send RELOAD via IPC

### Performance Considerations

- **NFR3: CLI response <500ms** - Unix socket IPC is fast, well under 500ms
- Connection timeout of 5s prevents hanging connections
- Non-blocking async design with tokio

### Security Considerations

- Socket permissions set to 600 (owner read/write only)
- No authentication needed - Unix socket inherently restricted to local user
- Socket in user-owned runtime directory

### References

- [Source: _bmad-output/planning-artifacts/architecture.md#API & Communication]
- [Source: _bmad-output/planning-artifacts/architecture.md#Platform-Specific Paths]
- [Source: _bmad-output/planning-artifacts/architecture.md#Project Structure & Boundaries]
- [Source: _bmad-output/planning-artifacts/epics.md#Story 1.6: Unix Socket IPC Server]

## Dev Agent Record

### Agent Model Used

openai/gpt-5.2-codex

### Implementation Plan

- Add IPC protocol types for commands and responses with text parsing/serialization.
- Implement Unix socket server bind/run/cleanup lifecycle with stale socket removal and timeouts.
- Add unit and integration tests for lifecycle, commands, concurrency, and timeouts.

### Debug Log References

- `cargo build`
- `cargo test`
- `cargo clippy`

### Completion Notes List

- Implemented IPC protocol and socket server with command dispatch and timeout handling.
- Added unit and integration tests covering socket lifecycle, commands, concurrency, and timeouts.
- Added shared env lock for tests and updated socket binding to create parent directories safely.
- Verified `cargo build`, `cargo test`, `cargo clippy`.

### File List

**Files to create:**
- `src/ipc/socket.rs`
- `src/ipc/protocol.rs`
- `src/test_utils.rs`
- `tests/ipc_test.rs`

**Files to modify:**
- `Cargo.toml` - Add tokio-util dependency
- `Cargo.lock`
- `src/ipc/mod.rs` - Export socket and protocol modules
- `src/ipc/socket.rs`
- `src/lib.rs`
- `src/config/paths.rs`
- `src/daemon/pid.rs`
- `_bmad-output/implementation-artifacts/1-6-unix-socket-ipc-server.md` - Update story status and tasks
- `_bmad-output/implementation-artifacts/sprint-status.yaml` - Update story status
- `logs/tasks/2026-02-05.jsonl`

## Change Log

- 2026-02-05: Story created and marked ready-for-dev
- 2026-02-05: Implemented Unix socket IPC server, protocol, and tests; verified build/test/clippy
- 2026-02-05: Code review completed - fixed 5 issues: removed dead code (IpcError::Protocol), replaced eprintln with tracing::warn, added documentation comment, added STATUS and unknown command unit tests. Marked done.
