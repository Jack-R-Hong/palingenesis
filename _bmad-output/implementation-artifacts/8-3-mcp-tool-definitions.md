# Story 8.3: MCP Tool Definitions

Status: ready-for-dev

## Story

As an MCP server,
I want to expose palingenesis control functions as MCP tools,
So that AI agents can discover and invoke them.

## Acceptance Criteria

**AC1: Tools List Response**
**Given** MCP client sends `tools/list` request
**When** server processes it
**Then** response includes tool definitions for: `status`, `pause`, `resume`, `new_session`, `logs`

**AC2: Tool Schema Definition**
**Given** each tool definition
**When** listed
**Then** it includes: name, description, inputSchema (JSON Schema)

**AC3: Status Tool**
**Given** MCP client invokes `tools/call` with tool `status`
**When** executed
**Then** response includes current daemon status as JSON

**AC4: Pause Tool**
**Given** MCP client invokes `tools/call` with tool `pause`
**When** daemon is monitoring
**Then** daemon pauses and response confirms success

**AC5: Resume Tool**
**Given** MCP client invokes `tools/call` with tool `resume`
**When** daemon is paused
**Then** daemon resumes and response confirms success

**AC6: New Session Tool**
**Given** MCP client invokes `tools/call` with tool `new_session`
**When** a session exists
**Then** new session is started and response includes session info

**AC7: Logs Tool**
**Given** MCP client invokes `tools/call` with tool `logs` and `lines: 10`
**When** executed
**Then** response includes last 10 log lines

## Tasks / Subtasks

- [ ] Create tool definitions module (AC: 1, 2)
  - [ ] Create `src/mcp/tools.rs` for tool definitions
  - [ ] Define `PalingenesisTool` enum with all tools
  - [ ] Implement JSON Schema for each tool's input

- [ ] Implement status tool (AC: 3)
  - [ ] Define `StatusInput` (empty schema, no params)
  - [ ] Create `#[tool_handler]` for status
  - [ ] Return daemon state, uptime, session info, stats

- [ ] Implement pause tool (AC: 4)
  - [ ] Define `PauseInput` (empty schema)
  - [ ] Create `#[tool_handler]` for pause
  - [ ] Call daemon's pause functionality
  - [ ] Return success/error response

- [ ] Implement resume tool (AC: 5)
  - [ ] Define `ResumeInput` (empty schema)
  - [ ] Create `#[tool_handler]` for resume
  - [ ] Call daemon's resume functionality
  - [ ] Return success/error response

- [ ] Implement new_session tool (AC: 6)
  - [ ] Define `NewSessionInput` with optional prompt field
  - [ ] Create `#[tool_handler]` for new_session
  - [ ] Trigger new session creation
  - [ ] Return session info in response

- [ ] Implement logs tool (AC: 7)
  - [ ] Define `LogsInput` with lines (default: 20), follow (bool)
  - [ ] Create `#[tool_handler]` for logs
  - [ ] Read log file and return lines
  - [ ] Support line count parameter

- [ ] Create handlers module (AC: 3, 4, 5, 6, 7)
  - [ ] Create `src/mcp/handlers.rs` for tool implementations
  - [ ] Inject `DaemonState` reference
  - [ ] Route tool calls to existing daemon logic

- [ ] Integrate with McpServer (AC: 1)
  - [ ] Add `#[tool_router]` macro to McpServer
  - [ ] Wire up all tool handlers
  - [ ] Ensure tools/list returns all defined tools

- [ ] Add unit tests (AC: 1, 2, 3, 4, 5, 6, 7)
  - [ ] Test tools/list returns all 5 tools
  - [ ] Test each tool has valid JSON Schema
  - [ ] Test status tool returns expected fields
  - [ ] Test pause/resume state transitions
  - [ ] Test logs tool respects line count

- [ ] Add integration tests
  - [ ] Test full tool call via MCP server
  - [ ] Test tool error handling

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

**Implements:** FR43 (Daemon exposes control functions as MCP tools)

### Technical Implementation

**rmcp Tool Macros:**

The `rmcp` crate (v0.8.0) provides `#[tool_router]` and `#[tool_handler]` macros for defining MCP tools.

```rust
// src/mcp/tools.rs
use rmcp::{tool, tool_router, tool_handler};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Input for status tool (no parameters)
#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct StatusInput {}

/// Input for pause tool (no parameters)
#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct PauseInput {}

/// Input for resume tool (no parameters)
#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct ResumeInput {}

/// Input for new_session tool
#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct NewSessionInput {
    /// Optional prompt to start the new session with
    #[serde(default)]
    pub prompt: Option<String>,
}

/// Input for logs tool
#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct LogsInput {
    /// Number of log lines to return (default: 20)
    #[serde(default = "default_lines")]
    pub lines: u32,
}

fn default_lines() -> u32 {
    20
}
```

**Tool Handler Implementation:**

```rust
// src/mcp/server.rs
use crate::daemon::state::DaemonState;
use crate::mcp::tools::*;
use rmcp::{
    handler::server::tool::ToolRouter,
    model::*,
    tool, tool_router,
    ErrorData as McpError,
};
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Clone)]
pub struct McpServer {
    tool_router: ToolRouter<Self>,
    daemon_state: Arc<RwLock<DaemonState>>,
}

#[tool_router]
impl McpServer {
    pub fn new(daemon_state: Arc<RwLock<DaemonState>>) -> Self {
        Self {
            tool_router: Self::tool_router(),
            daemon_state,
        }
    }
    
    /// Get current daemon status
    #[tool(description = "Get the current status of the palingenesis daemon")]
    async fn status(&self, _input: StatusInput) -> Result<StatusOutput, McpError> {
        let state = self.daemon_state.read().await;
        Ok(StatusOutput {
            running: true,
            state: state.monitoring_state.to_string(),
            uptime_seconds: state.uptime().as_secs(),
            current_session: state.current_session.clone(),
            stats: state.stats.clone(),
        })
    }
    
    /// Pause daemon monitoring
    #[tool(description = "Pause the daemon's monitoring functionality")]
    async fn pause(&self, _input: PauseInput) -> Result<PauseOutput, McpError> {
        let mut state = self.daemon_state.write().await;
        state.pause();
        Ok(PauseOutput {
            success: true,
            message: "Monitoring paused".to_string(),
        })
    }
    
    /// Resume daemon monitoring
    #[tool(description = "Resume the daemon's monitoring functionality")]
    async fn resume(&self, _input: ResumeInput) -> Result<ResumeOutput, McpError> {
        let mut state = self.daemon_state.write().await;
        state.resume();
        Ok(ResumeOutput {
            success: true,
            message: "Monitoring resumed".to_string(),
        })
    }
    
    /// Start a new session
    #[tool(description = "Start a new OpenCode session, optionally with a prompt")]
    async fn new_session(&self, input: NewSessionInput) -> Result<NewSessionOutput, McpError> {
        // Trigger new session creation
        // This would interact with the resume system
        Ok(NewSessionOutput {
            success: true,
            session_id: None, // Will be populated by actual implementation
            message: "New session started".to_string(),
        })
    }
    
    /// Get daemon logs
    #[tool(description = "Retrieve recent daemon log lines")]
    async fn logs(&self, input: LogsInput) -> Result<LogsOutput, McpError> {
        // Read log file
        let lines = read_log_lines(input.lines).await?;
        Ok(LogsOutput {
            lines,
            total_available: 0, // Populate from actual log
        })
    }
}
```

**Tool Output Types:**

```rust
// src/mcp/tools.rs (continued)

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct StatusOutput {
    pub running: bool,
    pub state: String,
    pub uptime_seconds: u64,
    pub current_session: Option<String>,
    pub stats: DaemonStats,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct PauseOutput {
    pub success: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct ResumeOutput {
    pub success: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct NewSessionOutput {
    pub success: bool,
    pub session_id: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct LogsOutput {
    pub lines: Vec<String>,
    pub total_available: u64,
}
```

### MCP Tool Schema Format

Each tool must provide a JSON Schema for its input:

```json
{
  "name": "status",
  "description": "Get the current status of the palingenesis daemon",
  "inputSchema": {
    "type": "object",
    "properties": {},
    "required": []
  }
}
```

```json
{
  "name": "logs",
  "description": "Retrieve recent daemon log lines",
  "inputSchema": {
    "type": "object",
    "properties": {
      "lines": {
        "type": "integer",
        "description": "Number of log lines to return",
        "default": 20
      }
    },
    "required": []
  }
}
```

### Dependencies

- Story 8.1: MCP server with stdio transport
- Story 8.2: JSON-RPC 2.0 protocol compliance
- Existing daemon state and control logic

**Files to create:**
- `src/mcp/tools.rs` - Tool input/output types
- `src/mcp/handlers.rs` - Tool implementation logic

**Files to modify:**
- `src/mcp/mod.rs` - Export new modules
- `src/mcp/server.rs` - Add tool handlers and DaemonState

### Testing Strategy

**Unit Tests:**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_status_tool_returns_state() {
        let state = Arc::new(RwLock::new(DaemonState::default()));
        let server = McpServer::new(state);
        
        let result = server.status(StatusInput {}).await;
        assert!(result.is_ok());
        assert!(result.unwrap().running);
    }
    
    #[tokio::test]
    async fn test_pause_tool_changes_state() {
        let state = Arc::new(RwLock::new(DaemonState::default()));
        let server = McpServer::new(state.clone());
        
        let result = server.pause(PauseInput {}).await;
        assert!(result.is_ok());
        
        let state = state.read().await;
        assert!(state.is_paused());
    }
    
    #[tokio::test]
    async fn test_logs_tool_respects_line_count() {
        let state = Arc::new(RwLock::new(DaemonState::default()));
        let server = McpServer::new(state);
        
        let result = server.logs(LogsInput { lines: 5 }).await;
        assert!(result.is_ok());
        assert!(result.unwrap().lines.len() <= 5);
    }
}
```

**Integration Tests:**

```rust
// tests/mcp_tools.rs
#[tokio::test]
async fn test_tools_list_returns_all_tools() {
    // Start MCP server, send tools/list request
    // Verify response contains all 5 tools
}

#[tokio::test]
async fn test_tool_call_status() {
    // Start MCP server, send tools/call with status
    // Verify response contains status fields
}
```

### Previous Story Learnings

From Story 8.1:
1. McpServer uses rmcp's `.serve()` pattern
2. CancellationToken for shutdown coordination
3. stdio transport handles message framing

### References

- [Source: _bmad-output/planning-artifacts/architecture.md#MCP Server Module]
- [Source: _bmad-output/planning-artifacts/epics.md#Story 8.3]
- [rmcp crate tools: https://docs.rs/rmcp/latest/rmcp/macro.tool.html]
- [MCP tools specification: https://modelcontextprotocol.io/docs/concepts/tools]

## Change Log

- 2026-02-06: Story created and marked ready-for-dev
