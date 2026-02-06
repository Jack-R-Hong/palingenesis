# Story 8.2: JSON-RPC 2.0 Protocol Implementation

Status: done

## Story

As an MCP server,
I want full JSON-RPC 2.0 compliance,
So that any MCP client can communicate reliably.

## Acceptance Criteria

**AC1: Request-Response with ID**
**Given** a JSON-RPC request with `method` and `id`
**When** processed successfully
**Then** response includes matching `id` and `result`

**AC2: Method Not Found Error**
**Given** a JSON-RPC request with invalid method
**When** processed
**Then** response includes error with code -32601 (Method not found)

**AC3: Notification Handling**
**Given** a JSON-RPC notification (no `id`)
**When** processed
**Then** no response is sent

**AC4: Batch Request Support**
**Given** a batch request (array of requests)
**When** processed
**Then** batch response is returned with results in same order

**AC5: JSON-RPC Version Header**
**Given** JSON-RPC 2.0 spec
**When** any request is processed
**Then** response always includes `"jsonrpc": "2.0"`

## Tasks / Subtasks

- [x] Create protocol types module (AC: 1, 5)
  - [x] Define `JsonRpcRequest` struct with jsonrpc, method, params, id
  - [x] Define `JsonRpcResponse` struct with jsonrpc, result/error, id
  - [x] Define `JsonRpcError` struct with code, message, data
  - [x] Implement serde Serialize/Deserialize for all types

- [x] Implement JSON-RPC error codes (AC: 2)
  - [x] Define `ErrorCode` enum with standard codes (-32700, -32600, -32601, -32602, -32603)
  - [x] Add Display impl for user-friendly messages
  - [x] Create helper constructors: `parse_error()`, `method_not_found()`, etc.

- [x] Implement request processing (AC: 1, 3)
  - [x] Create `process_request()` that routes to method handlers
  - [x] Return `Some(response)` for requests with `id`
  - [x] Return `None` for notifications (no `id`)

- [x] Implement batch request support (AC: 4)
  - [x] Detect batch by checking if input is JSON array
  - [x] Process each request in order
  - [x] Collect responses (excluding notifications)
  - [x] Return batch response as JSON array

- [x] Integrate with MCP server (AC: 1, 2, 3, 4, 5)
  - [x] Update `McpServer` to use protocol types
  - [x] Route MCP methods through protocol layer
  - [x] Ensure all responses include `"jsonrpc": "2.0"`

- [x] Add unit tests (AC: 1, 2, 3, 4, 5)
  - [x] Test valid request returns result with matching id
  - [x] Test invalid method returns -32601 error
  - [x] Test notification (no id) returns no response
  - [x] Test batch request returns batch response
  - [x] Test all responses include jsonrpc: "2.0"

- [x] Add integration tests
  - [x] Test full request/response cycle via MCP server
  - [x] Test batch processing end-to-end

## Dev Notes

### Architecture Requirements

**From architecture.md - MCP Server Module:**

> | FR Category | Module | Key Files |
> |-------------|--------|-----------|
> | MCP Server (FR41-FR44) | `src/mcp/` | `server.rs`, `tools.rs`, `handlers.rs` |

**Implements:** FR42 (MCP interface uses JSON-RPC 2.0 protocol)

### Technical Implementation

**JSON-RPC 2.0 Protocol Types:**

The rmcp crate (v0.8.0) already provides JSON-RPC 2.0 implementation internally. This story focuses on ensuring compliance and adding any custom handling needed.

```rust
// src/mcp/protocol.rs
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// JSON-RPC 2.0 Error Codes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCode {
    /// Invalid JSON was received
    ParseError = -32700,
    /// The JSON sent is not a valid Request object
    InvalidRequest = -32600,
    /// The method does not exist / is not available
    MethodNotFound = -32601,
    /// Invalid method parameter(s)
    InvalidParams = -32602,
    /// Internal JSON-RPC error
    InternalError = -32603,
}

impl ErrorCode {
    pub fn message(&self) -> &'static str {
        match self {
            Self::ParseError => "Parse error",
            Self::InvalidRequest => "Invalid Request",
            Self::MethodNotFound => "Method not found",
            Self::InvalidParams => "Invalid params",
            Self::InternalError => "Internal error",
        }
    }
}

/// JSON-RPC 2.0 Request
#[derive(Debug, Clone, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub method: String,
    #[serde(default)]
    pub params: Option<Value>,
    #[serde(default)]
    pub id: Option<Value>,
}

impl JsonRpcRequest {
    /// Returns true if this is a notification (no id field)
    pub fn is_notification(&self) -> bool {
        self.id.is_none()
    }
}

/// JSON-RPC 2.0 Response
#[derive(Debug, Clone, Serialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
    pub id: Value,
}

impl JsonRpcResponse {
    pub fn success(id: Value, result: Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            result: Some(result),
            error: None,
            id,
        }
    }
    
    pub fn error(id: Value, code: ErrorCode, data: Option<Value>) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            result: None,
            error: Some(JsonRpcError {
                code: code as i32,
                message: code.message().to_string(),
                data,
            }),
            id,
        }
    }
}

/// JSON-RPC 2.0 Error Object
#[derive(Debug, Clone, Serialize)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}
```

**Batch Processing:**

```rust
// src/mcp/protocol.rs (continued)

/// Process a potentially batch request
pub fn process_batch(input: &str) -> Result<Option<String>, JsonRpcError> {
    let value: Value = serde_json::from_str(input)
        .map_err(|_| JsonRpcError {
            code: ErrorCode::ParseError as i32,
            message: "Parse error".to_string(),
            data: None,
        })?;
    
    match value {
        Value::Array(requests) => {
            // Batch request
            let responses: Vec<JsonRpcResponse> = requests
                .into_iter()
                .filter_map(|req| process_single_value(req))
                .collect();
            
            if responses.is_empty() {
                Ok(None) // All were notifications
            } else {
                Ok(Some(serde_json::to_string(&responses).unwrap()))
            }
        }
        _ => {
            // Single request
            process_single_value(value)
                .map(|r| Some(serde_json::to_string(&r).unwrap()))
        }
    }
}

fn process_single_value(value: Value) -> Option<JsonRpcResponse> {
    let request: JsonRpcRequest = match serde_json::from_value(value) {
        Ok(r) => r,
        Err(_) => return Some(JsonRpcResponse::error(
            Value::Null,
            ErrorCode::InvalidRequest,
            None,
        )),
    };
    
    if request.is_notification() {
        // Notifications don't get responses
        return None;
    }
    
    // Process request and return response
    // ... method routing logic
    Some(JsonRpcResponse::success(
        request.id.unwrap(),
        serde_json::json!({}),
    ))
}
```

### rmcp Crate Integration

The `rmcp` crate handles most JSON-RPC 2.0 details internally. Key points:
- Request/Response serialization is automatic
- Error codes are mapped to MCP-specific errors
- Batch requests may not be fully supported by MCP spec

**MCP-Specific Methods:**
- `initialize` / `initialized`
- `tools/list`
- `tools/call`

### JSON-RPC 2.0 Error Codes Reference

| Code | Meaning | When Used |
|------|---------|-----------|
| -32700 | Parse error | Invalid JSON |
| -32600 | Invalid Request | Invalid JSON-RPC structure |
| -32601 | Method not found | Unknown method |
| -32602 | Invalid params | Invalid method parameters |
| -32603 | Internal error | Server error |
| -32000 to -32099 | Server error | Implementation-defined |

### Testing Strategy

**Unit Tests:**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_request_with_id_returns_response() {
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: "test".to_string(),
            params: None,
            id: Some(serde_json::json!(1)),
        };
        assert!(!request.is_notification());
    }
    
    #[test]
    fn test_notification_has_no_id() {
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: "test".to_string(),
            params: None,
            id: None,
        };
        assert!(request.is_notification());
    }
    
    #[test]
    fn test_response_includes_jsonrpc_version() {
        let response = JsonRpcResponse::success(
            serde_json::json!(1),
            serde_json::json!({"status": "ok"}),
        );
        assert_eq!(response.jsonrpc, "2.0");
    }
    
    #[test]
    fn test_method_not_found_error() {
        let response = JsonRpcResponse::error(
            serde_json::json!(1),
            ErrorCode::MethodNotFound,
            None,
        );
        assert_eq!(response.error.unwrap().code, -32601);
    }
}
```

### Dependencies

Story 8.1 provides the MCP server with stdio transport. This story adds protocol-level compliance.

**Files to modify:**
- `src/mcp/mod.rs` - Add protocol module
- `src/mcp/server.rs` - Integrate protocol types

**Files to create:**
- `src/mcp/protocol.rs` - JSON-RPC 2.0 types and processing

### Previous Story Learnings

From Story 8.1:
1. rmcp crate handles most protocol details
2. Custom line-delimited transport already parses JSON
3. Error handling uses McpError enum

### References

- [Source: _bmad-output/planning-artifacts/architecture.md#MCP Server Module]
- [Source: _bmad-output/planning-artifacts/epics.md#Story 8.2]
- [JSON-RPC 2.0 Specification: https://www.jsonrpc.org/specification]
- [rmcp crate documentation: https://docs.rs/rmcp/latest/rmcp/]

## Dev Agent Record

### Agent Model Used

openai/gpt-5.2-codex

### Implementation Plan

- Add JSON-RPC 2.0 protocol module with request/response/error types and batch processing helpers.
- Route MCP initialize/tools methods through the protocol layer and expose process entrypoint.
- Add unit and integration tests for request/notification/batch handling and jsonrpc header.

### Debug Log References

- `cargo fmt`
- `cargo clippy`
- `cargo test`

### Completion Notes List

- Added JSON-RPC 2.0 protocol types, error codes, and batch processing with notification handling.
- Routed MCP initialize/tools methods through the protocol handler with line-delimited stdio processing.
- Added unit and integration tests covering request/response, method not found, notifications, and batches.

### File List

**Files to create:**
- `src/mcp/protocol.rs`
- `tests/mcp_jsonrpc.rs`

**Files to modify:**
- `src/mcp/mod.rs`
- `src/mcp/server.rs`
- `_bmad-output/implementation-artifacts/8-2-json-rpc-2-0-protocol-implementation.md`
- `_bmad-output/implementation-artifacts/sprint-status.yaml`
- `logs/tasks/2026-02-06.jsonl`

## Change Log

- 2026-02-06: Story created and marked ready-for-dev
- 2026-02-06: Implemented JSON-RPC protocol handling, routing, and tests; ran fmt/clippy/test
- 2026-02-06: Code review completed - added 6 edge case tests, removed params echo; marked done
