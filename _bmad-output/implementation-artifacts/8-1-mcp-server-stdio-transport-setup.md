# Story 8.1: MCP Server stdio Transport Setup

Status: done

## Story

As a daemon,
I want to support MCP stdio transport,
So that OpenCode can communicate with me via the MCP protocol.

## Acceptance Criteria

**AC1: MCP Serve Command**
**Given** the daemon is started with `palingenesis mcp serve`
**When** it initializes MCP mode
**Then** it reads JSON-RPC messages from stdin
**And** writes responses to stdout

**AC2: Valid JSON-RPC Request Handling**
**Given** the MCP server is running
**When** it receives a valid JSON-RPC request
**Then** it parses and processes the request
**And** returns a properly formatted JSON-RPC response

**AC3: Malformed JSON Handling**
**Given** the MCP server receives malformed JSON
**When** parsing fails
**Then** it returns a JSON-RPC error response with code -32700 (Parse error)
**And** continues running (doesn't crash)

**AC4: EOF/Graceful Shutdown**
**Given** the MCP server is running
**When** stdin is closed (EOF)
**Then** it shuts down gracefully

**AC5: Line-Delimited JSON Format**
**Given** the MCP transport uses stdio
**When** messages are exchanged
**Then** each message is a single line of JSON (line-delimited)
**And** newline characters separate messages

## Tasks / Subtasks

- [x] Add rmcp crate dependency (AC: 1)
  - [x] Add `rmcp = { version = "0.8", features = ["server", "transport-io"] }` to Cargo.toml
  - [x] Add `schemars = "0.8"` for JSON Schema generation
  - [x] Verify crate compiles with features

- [x] Create MCP module structure (AC: 1)
  - [x] Create `src/mcp/mod.rs` with module declarations
  - [x] Create `src/mcp/server.rs` for MCP server implementation
  - [x] Update `src/lib.rs` to export mcp module

- [x] Implement MCP CLI subcommand (AC: 1)
  - [x] Add `mcp` subcommand to `src/cli/app.rs`
  - [x] Add `serve` subcommand under `mcp`
  - [x] Create `src/cli/commands/mcp.rs` for command handler

- [x] Implement MCP server struct (AC: 1, 2, 5)
  - [x] Define `McpServer` struct with daemon state access
  - [x] Implement `ServerHandler` trait from rmcp
  - [x] Configure stdio transport using `rmcp::transport::io::stdio()`

- [x] Implement server lifecycle (AC: 1, 4)
  - [x] Implement `McpServer::run()` using rmcp's `.serve()` pattern
  - [x] Handle stdin EOF for graceful shutdown
  - [x] Integrate with daemon's CancellationToken

- [x] Implement error handling (AC: 3)
  - [x] Create `McpError` enum with thiserror
  - [x] Handle JSON parse errors with code -32700
  - [x] Ensure server continues after recoverable errors

- [x] Add unit tests (AC: 1, 2, 3, 4, 5)
  - [x] Test MCP server creation
  - [x] Test valid JSON-RPC request handling
  - [x] Test malformed JSON error response
  - [x] Test line-delimited message format

- [x] Add integration tests
  - [x] Test `palingenesis mcp serve` command starts server
  - [x] Test stdin/stdout communication
  - [x] Test graceful shutdown on EOF

## Dev Notes

### Architecture Requirements

**From architecture.md - MCP Server Module:**

> | FR Category | Module | Key Files |
> |-------------|--------|-----------|
> | MCP Server (FR41-FR44) | `src/mcp/` | `server.rs`, `tools.rs`, `handlers.rs` |

**From architecture.md - Project Structure:**

```
src/mcp/
    mod.rs                    # MCP module root
    server.rs                 # stdio transport, JSON-RPC 2.0
    tools.rs                  # MCP tool definitions
    handlers.rs               # Tool handlers -> DaemonState
```

**Implements:** FR41 (Daemon supports MCP stdio transport interface)

### Technical Implementation

**rmcp Crate Usage:**

The `rmcp` crate (v0.8.0) provides the official Rust SDK for the Model Context Protocol. Key features:
- `server` feature: Server-side MCP implementation
- `transport-io` feature: stdio transport support

**MCP Server Structure:**

```rust
// src/mcp/server.rs
use rmcp::{
    handler::server::tool::ToolRouter,
    model::*,
    service::ServiceExt,
    tool, tool_router,
    transport::io::stdio,
    ErrorData as McpError,
};
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

#[derive(Clone)]
pub struct McpServer {
    tool_router: ToolRouter<Self>,
    // daemon_state: Arc<RwLock<DaemonState>>, // Added in Story 8.3
}

#[tool_router]
impl McpServer {
    pub fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
        }
    }
    
    // Tools will be added in Story 8.3
}

impl McpServer {
    /// Run the MCP server using stdio transport.
    pub async fn run(self, cancel: CancellationToken) -> Result<(), McpServerError> {
        let (stdin, stdout) = stdio();
        
        // The serve pattern from rmcp handles the protocol
        let service = self.serve((stdin, stdout)).await?;
        
        // Wait for cancellation or EOF
        tokio::select! {
            _ = cancel.cancelled() => {
                tracing::info!("MCP server shutting down via cancellation");
            }
            result = service.waiting() => {
                if let Err(e) = result {
                    tracing::error!(error = %e, "MCP server error");
                }
            }
        }
        
        Ok(())
    }
}
```

**CLI Subcommand:**

```rust
// src/cli/app.rs
#[derive(Debug, Subcommand)]
pub enum Commands {
    // ... existing commands ...
    
    /// MCP server operations
    Mcp {
        #[command(subcommand)]
        command: McpCommands,
    },
}

#[derive(Debug, Subcommand)]
pub enum McpCommands {
    /// Start MCP server using stdio transport
    Serve,
}

// src/cli/commands/mcp.rs
pub async fn serve() -> Result<()> {
    let cancel = CancellationToken::new();
    let server = McpServer::new();
    
    // Handle signals for graceful shutdown
    let cancel_clone = cancel.clone();
    tokio::spawn(async move {
        tokio::signal::ctrl_c().await.ok();
        cancel_clone.cancel();
    });
    
    server.run(cancel).await?;
    Ok(())
}
```

**Error Types:**

```rust
// src/mcp/server.rs
#[derive(Debug, thiserror::Error)]
pub enum McpServerError {
    #[error("Transport error: {0}")]
    Transport(#[from] std::io::Error),
    
    #[error("Protocol error: {0}")]
    Protocol(String),
    
    #[error("Initialization failed: {0}")]
    InitializeFailed(String),
}
```

### Dependencies

**Add to Cargo.toml:**

```toml
# MCP Protocol
rmcp = { version = "0.8", features = ["server", "transport-io"] }
schemars = "0.8"
```

### JSON-RPC 2.0 Error Codes

| Code | Meaning | When Used |
|------|---------|-----------|
| -32700 | Parse error | Invalid JSON |
| -32600 | Invalid Request | Invalid JSON-RPC structure |
| -32601 | Method not found | Unknown method |
| -32602 | Invalid params | Invalid method parameters |
| -32603 | Internal error | Server error |

### MCP Protocol Flow

```
Client                          Server
   |                               |
   |---- initialize request ------>|
   |<--- initialize response ------|
   |---- initialized notification->|
   |                               |
   |---- tools/list request ------>|
   |<--- tools/list response ------|
   |                               |
   |---- tools/call request ------>|
   |<--- tools/call response ------|
```

### Testing Strategy

**Unit Tests:**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_mcp_server_creation() {
        let server = McpServer::new();
        // Server should be created without panic
    }
    
    #[tokio::test]
    async fn test_tool_router_initialization() {
        let server = McpServer::new();
        // Tool router should be initialized
    }
}
```

**Integration Tests:**

```rust
// tests/mcp_test.rs
use std::process::Stdio;
use tokio::process::Command;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

#[tokio::test]
async fn test_mcp_serve_starts() {
    let mut child = Command::new("cargo")
        .args(["run", "--", "mcp", "serve"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("Failed to start MCP server");
    
    let stdin = child.stdin.take().unwrap();
    let stdout = child.stdout.take().unwrap();
    
    // Send initialize request, verify response
    // ...
    
    // Close stdin to trigger shutdown
    drop(stdin);
    
    let status = child.wait().await.unwrap();
    assert!(status.success());
}
```

### Previous Story Learnings

From Story 1-6 (Unix Socket IPC Server):
1. **Error handling**: Use `thiserror` for domain errors
2. **Graceful shutdown**: Use `CancellationToken` pattern
3. **Module structure**: Export through `mod.rs`
4. **Testing**: Use mock state for unit tests

From Story 6-1 (HTTP API Server Setup):
1. **Service patterns**: Similar service lifecycle management
2. **CLI integration**: Subcommand structure pattern

### OpenCode MCP Configuration

When complete, users will configure palingenesis as an MCP server in OpenCode:

```json
{
  "mcpServers": {
    "palingenesis": {
      "type": "local",
      "command": "palingenesis",
      "args": ["mcp", "serve"]
    }
  }
}
```

### Project Structure Notes

This story creates:
- `src/mcp/mod.rs` - Module root
- `src/mcp/server.rs` - MCP server with stdio transport
- `src/cli/commands/mcp.rs` - CLI handler

Future stories will add:
- Story 8.2: `src/mcp/protocol.rs` - JSON-RPC 2.0 details
- Story 8.3: `src/mcp/tools.rs` and `src/mcp/handlers.rs` - Tool definitions

### Performance Considerations

- **NFR**: MCP should not add significant overhead when not in use
- Stdio transport is blocking on the main thread; consider spawning
- Line-delimited JSON is efficient for streaming

### Security Considerations

- MCP server runs only when explicitly invoked via `palingenesis mcp serve`
- stdio transport is inherently local (no network exposure)
- No authentication needed for local MCP (OpenCode handles security)

### References

- [Source: _bmad-output/planning-artifacts/architecture.md#Project Structure & Boundaries]
- [Source: _bmad-output/planning-artifacts/architecture.md#Integration Points]
- [Source: _bmad-output/planning-artifacts/epics.md#Story 8.1: MCP Server stdio Transport Setup]
- [rmcp crate documentation: https://docs.rs/rmcp/latest/rmcp/]

## Dev Agent Record

### Agent Model Used

openai/gpt-5.2-codex

### Implementation Plan

- Add MCP dependencies, module structure, and CLI entrypoint for stdio serve.
- Implement McpServer with stdio transport, parse error handling, and cancellation-aware lifecycle.
- Add unit and integration tests for initialization, parse errors, and EOF shutdown.

### Debug Log References

- `cargo fmt`
- `cargo clippy`
- `cargo test`

### Completion Notes List

- Added MCP stdio server with custom line-delimited transport and JSON parse error responses.
- Wired `palingenesis mcp serve` CLI command to daemon shutdown coordination and cancellation.
- Added unit and integration coverage for initialize, parse error recovery, and EOF shutdown.

### File List

**Files to create:**
- `src/mcp/mod.rs`
- `src/mcp/server.rs`
- `src/cli/commands/mcp.rs`
- `tests/mcp_stdio.rs`

**Files to modify:**
- `Cargo.toml`
- `Cargo.lock`
- `src/lib.rs`
- `src/cli/app.rs`
- `src/cli/mod.rs`
- `src/cli/commands/mod.rs`
- `src/main.rs`
- `src/daemon/core.rs`
- `_bmad-output/implementation-artifacts/8-1-mcp-server-stdio-transport-setup.md`
- `_bmad-output/implementation-artifacts/sprint-status.yaml`
- `logs/tasks/2026-02-06.jsonl`

## Change Log

- 2026-02-06: Story created and marked ready-for-dev
- 2026-02-06: Implemented MCP stdio server, CLI integration, and tests; ran fmt/clippy/test
