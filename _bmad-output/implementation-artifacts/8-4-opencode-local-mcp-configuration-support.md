# Story 8.4: OpenCode Local MCP Configuration Support

Status: ready-for-dev

## Story

As a user,
I want to configure palingenesis as a local MCP server in OpenCode,
So that OpenCode can control the daemon automatically.

## Acceptance Criteria

**AC1: OpenCode MCP Config Format**
**Given** OpenCode's MCP config format
**When** user adds palingenesis
**Then** config looks like:
```json
{
  "mcpServers": {
    "palingenesis": {
      "type": "local",
      "command": "palingenesis",
      "args": ["mcp", "serve"],
      "enabled": true
    }
  }
}
```

**AC2: MCP Server Startup via OpenCode**
**Given** the MCP server starts via OpenCode
**When** initialization completes
**Then** it sends proper MCP initialization response

**AC3: Initialize Request Handling**
**Given** OpenCode sends `initialize` request
**When** server responds
**Then** response includes server info and capabilities

**AC4: Initialized Notification Handling**
**Given** OpenCode sends `initialized` notification
**When** server receives it
**Then** server is ready to accept tool calls

**AC5: Quick Setup Documentation**
**Given** config documentation
**When** user reads it
**Then** they can set up palingenesis MCP in <2 minutes

## Tasks / Subtasks

- [ ] Implement MCP initialize handler (AC: 2, 3)
  - [ ] Handle `initialize` request with client info
  - [ ] Return server name, version, capabilities
  - [ ] Include tools capability in response

- [ ] Implement initialized notification handler (AC: 4)
  - [ ] Handle `initialized` notification
  - [ ] Mark server as ready for tool calls
  - [ ] Log successful initialization

- [ ] Define server capabilities (AC: 3)
  - [ ] Declare tools capability
  - [ ] Optionally declare prompts/resources if needed
  - [ ] Set protocol version to latest MCP spec

- [ ] Add config generation command (AC: 1)
  - [ ] Add `palingenesis mcp config` subcommand
  - [ ] Output JSON config snippet for OpenCode
  - [ ] Include all required fields

- [ ] Update documentation (AC: 5)
  - [ ] Add MCP setup section to README
  - [ ] Include OpenCode config example
  - [ ] Document available tools
  - [ ] Add troubleshooting tips

- [ ] Add integration tests (AC: 1, 2, 3, 4)
  - [ ] Test initialize handshake
  - [ ] Test initialized notification
  - [ ] Test tool availability after init

## Dev Notes

### Architecture Requirements

**From architecture.md - MCP Server Module:**

> | FR Category | Module | Key Files |
> |-------------|--------|-----------|
> | MCP Server (FR41-FR44) | `src/mcp/` | `server.rs`, `tools.rs`, `handlers.rs` |

**Implements:** FR44 (Supports OpenCode `type: "local"` MCP configuration format)

### Technical Implementation

**OpenCode MCP Configuration:**

OpenCode uses a JSON configuration file to define MCP servers. The `type: "local"` configuration spawns the server as a subprocess with stdio communication.

```json
// ~/.config/opencode/mcp.json or similar
{
  "mcpServers": {
    "palingenesis": {
      "type": "local",
      "command": "palingenesis",
      "args": ["mcp", "serve"],
      "enabled": true
    }
  }
}
```

**MCP Initialization Handshake:**

```
Client (OpenCode)                 Server (palingenesis)
    |                                    |
    |---- initialize request ----------->|
    |     { protocolVersion, clientInfo }|
    |                                    |
    |<--- initialize response -----------|
    |     { protocolVersion, serverInfo, |
    |       capabilities }               |
    |                                    |
    |---- initialized notification ----->|
    |                                    |
    |---- (ready for tool calls) ------->|
```

**Initialize Response:**

```rust
// The rmcp crate handles initialization automatically when using ServerHandler trait
// Our McpServer already implements this via #[tool_router]

// The response includes:
// - protocolVersion: "2024-11-05" (or latest)
// - serverInfo: { name: "palingenesis", version: "0.1.0" }
// - capabilities: { tools: {} }
```

**Server Info Implementation:**

```rust
// src/mcp/server.rs
use rmcp::model::{ServerInfo, ServerCapabilities};

impl McpServer {
    fn server_info() -> ServerInfo {
        ServerInfo {
            name: "palingenesis".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }
    
    fn capabilities() -> ServerCapabilities {
        ServerCapabilities {
            tools: Some(ToolsCapability::default()),
            prompts: None,
            resources: None,
            logging: None,
        }
    }
}
```

**Config Generation Command:**

```rust
// src/cli/commands/mcp.rs

/// Generate OpenCode MCP configuration
pub fn config() -> Result<()> {
    let config = serde_json::json!({
        "mcpServers": {
            "palingenesis": {
                "type": "local",
                "command": "palingenesis",
                "args": ["mcp", "serve"],
                "enabled": true
            }
        }
    });
    
    println!("{}", serde_json::to_string_pretty(&config)?);
    
    println!("\n# Add this to your OpenCode MCP configuration file");
    println!("# Typically: ~/.config/opencode/mcp.json");
    
    Ok(())
}
```

### MCP Protocol Version

The MCP protocol is versioned. Current version: `2024-11-05`

The server should:
1. Accept client's requested protocol version
2. Respond with the version it will use
3. Fall back gracefully if versions differ

### Capabilities Declaration

```json
{
  "capabilities": {
    "tools": {
      "listChanged": false
    }
  }
}
```

- `tools`: We expose tools (status, pause, resume, etc.)
- `listChanged`: false (we don't dynamically change tools)
- No prompts or resources for initial implementation

### CLI Subcommand Structure

```
palingenesis mcp
├── serve     # Start MCP server (existing from 8.1)
└── config    # Output OpenCode config snippet (new)
```

### Documentation Template

**README.md section:**

```markdown
## OpenCode Integration

palingenesis can be controlled via OpenCode's MCP interface.

### Setup

1. Generate the configuration:
   ```bash
   palingenesis mcp config
   ```

2. Add the output to your OpenCode MCP config file:
   - Linux: `~/.config/opencode/mcp.json`
   - macOS: `~/Library/Application Support/opencode/mcp.json`

3. Restart OpenCode to load the new MCP server.

### Available Tools

| Tool | Description |
|------|-------------|
| `status` | Get daemon status, uptime, and stats |
| `pause` | Pause monitoring |
| `resume` | Resume monitoring |
| `new_session` | Start a new session |
| `logs` | View recent log lines |

### Troubleshooting

**MCP server not connecting:**
- Ensure `palingenesis` is in your PATH
- Check OpenCode logs for MCP errors
- Verify the config JSON is valid
```

### Dependencies

- Story 8.1: MCP server with stdio transport
- Story 8.2: JSON-RPC 2.0 protocol
- Story 8.3: Tool definitions

**Files to modify:**
- `src/mcp/server.rs` - Server info and capabilities
- `src/cli/commands/mcp.rs` - Add config subcommand
- `src/cli/app.rs` - Add config to McpCommands enum
- `README.md` - Add OpenCode integration section

### Testing Strategy

**Unit Tests:**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_server_info() {
        let info = McpServer::server_info();
        assert_eq!(info.name, "palingenesis");
        assert!(!info.version.is_empty());
    }
    
    #[test]
    fn test_capabilities_includes_tools() {
        let caps = McpServer::capabilities();
        assert!(caps.tools.is_some());
    }
}
```

**Integration Tests:**

```rust
// tests/mcp_init.rs
use std::process::Stdio;
use tokio::process::Command;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

#[tokio::test]
async fn test_mcp_initialize_handshake() {
    let mut child = Command::new("cargo")
        .args(["run", "--", "mcp", "serve"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("Failed to start MCP server");
    
    let mut stdin = child.stdin.take().unwrap();
    let stdout = child.stdout.take().unwrap();
    let mut reader = BufReader::new(stdout).lines();
    
    // Send initialize request
    let init_request = r#"{"jsonrpc":"2.0","method":"initialize","params":{"protocolVersion":"2024-11-05","clientInfo":{"name":"test","version":"1.0"}},"id":1}"#;
    stdin.write_all(init_request.as_bytes()).await.unwrap();
    stdin.write_all(b"\n").await.unwrap();
    stdin.flush().await.unwrap();
    
    // Read response
    let response = reader.next_line().await.unwrap().unwrap();
    let json: serde_json::Value = serde_json::from_str(&response).unwrap();
    
    assert_eq!(json["jsonrpc"], "2.0");
    assert_eq!(json["id"], 1);
    assert!(json["result"]["serverInfo"]["name"].as_str().unwrap().contains("palingenesis"));
    
    // Send initialized notification
    let init_notif = r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#;
    stdin.write_all(init_notif.as_bytes()).await.unwrap();
    stdin.write_all(b"\n").await.unwrap();
    
    // Close stdin to shutdown
    drop(stdin);
    
    let status = child.wait().await.unwrap();
    assert!(status.success());
}

#[tokio::test]
async fn test_mcp_config_command() {
    let output = Command::new("cargo")
        .args(["run", "--", "mcp", "config"])
        .output()
        .await
        .expect("Failed to run config command");
    
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("mcpServers"));
    assert!(stdout.contains("palingenesis"));
    assert!(stdout.contains("mcp"));
    assert!(stdout.contains("serve"));
}
```

### Previous Story Learnings

From Story 8.1:
1. stdio transport with line-delimited JSON
2. CancellationToken for graceful shutdown
3. rmcp handles protocol details

From Story 8.3:
1. Tool definitions with JSON Schema
2. Handler integration with DaemonState

### References

- [Source: _bmad-output/planning-artifacts/architecture.md#Integration Points]
- [Source: _bmad-output/planning-artifacts/epics.md#Story 8.4]
- [MCP Specification: https://modelcontextprotocol.io/docs/spec]
- [OpenCode MCP Configuration: https://opencode.io/docs/mcp]

## Change Log

- 2026-02-06: Story created and marked ready-for-dev
