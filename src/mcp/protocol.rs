use std::fmt;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

const JSONRPC_VERSION: &str = "2.0";
const MCP_PROTOCOL_VERSION: &str = "2024-11-05";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCode {
    ParseError = -32700,
    InvalidRequest = -32600,
    MethodNotFound = -32601,
    InvalidParams = -32602,
    InternalError = -32603,
}

impl ErrorCode {
    pub fn message(self) -> &'static str {
        match self {
            Self::ParseError => "Parse error",
            Self::InvalidRequest => "Invalid Request",
            Self::MethodNotFound => "Method not found",
            Self::InvalidParams => "Invalid params",
            Self::InternalError => "Internal error",
        }
    }
}

impl fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.message())
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

impl JsonRpcError {
    pub fn new(code: ErrorCode, data: Option<Value>) -> Self {
        Self {
            code: code as i32,
            message: code.message().to_string(),
            data,
        }
    }

    pub fn parse_error() -> Self {
        Self::new(ErrorCode::ParseError, None)
    }

    pub fn invalid_request() -> Self {
        Self::new(ErrorCode::InvalidRequest, None)
    }

    pub fn method_not_found() -> Self {
        Self::new(ErrorCode::MethodNotFound, None)
    }

    pub fn invalid_params() -> Self {
        Self::new(ErrorCode::InvalidParams, None)
    }

    pub fn internal_error() -> Self {
        Self::new(ErrorCode::InternalError, None)
    }
}

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
    pub fn is_notification(&self) -> bool {
        self.id.is_none()
    }
}

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
            jsonrpc: JSONRPC_VERSION.to_string(),
            result: Some(result),
            error: None,
            id,
        }
    }

    pub fn error(id: Value, error: JsonRpcError) -> Self {
        Self {
            jsonrpc: JSONRPC_VERSION.to_string(),
            result: None,
            error: Some(error),
            id,
        }
    }
}

pub trait JsonRpcHandler {
    fn handle(&self, method: &str, params: Option<Value>) -> Result<Value, JsonRpcError>;
}

pub fn default_initialize_response() -> Value {
    json!({
        "protocolVersion": MCP_PROTOCOL_VERSION,
        "serverInfo": {
            "name": "palingenesis",
            "version": env!("CARGO_PKG_VERSION"),
        },
        "capabilities": {
            "tools": {}
        }
    })
}

pub fn process_input<H: JsonRpcHandler>(handler: &H, input: &str) -> Option<String> {
    let value: Value = match serde_json::from_str(input) {
        Ok(value) => value,
        Err(_) => {
            return serialize_response(JsonRpcResponse::error(
                Value::Null,
                JsonRpcError::parse_error(),
            ));
        }
    };

    match value {
        Value::Array(items) => process_batch(handler, items),
        other => process_single(handler, other),
    }
}

fn process_batch<H: JsonRpcHandler>(handler: &H, items: Vec<Value>) -> Option<String> {
    if items.is_empty() {
        return serialize_response(JsonRpcResponse::error(
            Value::Null,
            JsonRpcError::invalid_request(),
        ));
    }

    let responses: Vec<JsonRpcResponse> = items
        .into_iter()
        .filter_map(|item| process_value(handler, item))
        .collect();

    if responses.is_empty() {
        None
    } else {
        serde_json::to_string(&responses).ok().or_else(|| {
            serialize_response(JsonRpcResponse::error(
                Value::Null,
                JsonRpcError::internal_error(),
            ))
        })
    }
}

fn process_single<H: JsonRpcHandler>(handler: &H, value: Value) -> Option<String> {
    match process_value(handler, value) {
        Some(response) => serialize_response(response),
        None => None,
    }
}

fn process_value<H: JsonRpcHandler>(handler: &H, value: Value) -> Option<JsonRpcResponse> {
    let request = match parse_request(value) {
        Ok(request) => request,
        Err(error) => return Some(JsonRpcResponse::error(Value::Null, error)),
    };

    if request.is_notification() {
        let _ = handler.handle(&request.method, request.params);
        return None;
    }

    let id = request.id.unwrap_or(Value::Null);
    match handler.handle(&request.method, request.params) {
        Ok(result) => Some(JsonRpcResponse::success(id, result)),
        Err(error) => Some(JsonRpcResponse::error(id, error)),
    }
}

fn parse_request(value: Value) -> Result<JsonRpcRequest, JsonRpcError> {
    let request: JsonRpcRequest =
        serde_json::from_value(value).map_err(|_| JsonRpcError::invalid_request())?;
    if request.jsonrpc != JSONRPC_VERSION || request.method.trim().is_empty() {
        return Err(JsonRpcError::invalid_request());
    }
    Ok(request)
}

fn serialize_response(response: JsonRpcResponse) -> Option<String> {
    serde_json::to_string(&response).ok().or_else(|| {
        serde_json::to_string(&JsonRpcResponse::error(
            Value::Null,
            JsonRpcError::internal_error(),
        ))
        .ok()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestHandler;

    impl JsonRpcHandler for TestHandler {
        fn handle(&self, method: &str, _params: Option<Value>) -> Result<Value, JsonRpcError> {
            match method {
                "initialize" => Ok(default_initialize_response()),
                "tools/list" => Ok(json!({"tools": []})),
                "tools/call" => Ok(json!({"content": []})),
                _ => Err(JsonRpcError::method_not_found()),
            }
        }
    }

    #[test]
    fn test_request_with_id_returns_response() {
        let response = process_input(
            &TestHandler,
            r#"{"jsonrpc":"2.0","method":"initialize","id":1}"#,
        )
        .expect("response");
        let value: Value = serde_json::from_str(&response).expect("json");
        assert_eq!(value["id"], 1);
        assert_eq!(value["jsonrpc"], "2.0");
    }

    #[test]
    fn test_invalid_method_returns_method_not_found() {
        let response = process_input(
            &TestHandler,
            r#"{"jsonrpc":"2.0","method":"unknown","id":"abc"}"#,
        )
        .expect("response");
        let value: Value = serde_json::from_str(&response).expect("json");
        assert_eq!(value["error"]["code"], -32601);
        assert_eq!(value["id"], "abc");
    }

    #[test]
    fn test_notification_returns_no_response() {
        let response = process_input(&TestHandler, r#"{"jsonrpc":"2.0","method":"initialize"}"#);
        assert!(response.is_none());
    }

    #[test]
    fn test_batch_request_returns_batch_response() {
        let response = process_input(
            &TestHandler,
            r#"[{"jsonrpc":"2.0","method":"initialize","id":1},{"jsonrpc":"2.0","method":"tools/list","id":2}]"#,
        )
        .expect("response");
        let value: Value = serde_json::from_str(&response).expect("json");
        assert_eq!(value.as_array().unwrap().len(), 2);
        assert_eq!(value[0]["id"], 1);
        assert_eq!(value[1]["id"], 2);
    }

    #[test]
    fn test_responses_include_jsonrpc_version() {
        let response = process_input(
            &TestHandler,
            r#"{"jsonrpc":"2.0","method":"tools/list","id":99}"#,
        )
        .expect("response");
        let value: Value = serde_json::from_str(&response).expect("json");
        assert_eq!(value["jsonrpc"], "2.0");
    }

    #[test]
    fn test_parse_error_on_invalid_json() {
        let response = process_input(&TestHandler, r#"{not valid json}"#).expect("response");
        let value: Value = serde_json::from_str(&response).expect("json");
        assert_eq!(value["error"]["code"], -32700);
        assert_eq!(value["id"], Value::Null);
    }

    #[test]
    fn test_invalid_jsonrpc_version_rejected() {
        let response = process_input(
            &TestHandler,
            r#"{"jsonrpc":"1.0","method":"initialize","id":1}"#,
        )
        .expect("response");
        let value: Value = serde_json::from_str(&response).expect("json");
        assert_eq!(value["error"]["code"], -32600);
    }

    #[test]
    fn test_empty_batch_returns_invalid_request() {
        let response = process_input(&TestHandler, r#"[]"#).expect("response");
        let value: Value = serde_json::from_str(&response).expect("json");
        assert_eq!(value["error"]["code"], -32600);
    }

    #[test]
    fn test_batch_with_only_notifications_returns_none() {
        let response = process_input(
            &TestHandler,
            r#"[{"jsonrpc":"2.0","method":"initialize"},{"jsonrpc":"2.0","method":"tools/list"}]"#,
        );
        assert!(response.is_none());
    }

    #[test]
    fn test_empty_method_rejected() {
        let response = process_input(&TestHandler, r#"{"jsonrpc":"2.0","method":"","id":1}"#)
            .expect("response");
        let value: Value = serde_json::from_str(&response).expect("json");
        assert_eq!(value["error"]["code"], -32600);
    }

    #[test]
    fn test_whitespace_only_method_rejected() {
        let response = process_input(&TestHandler, r#"{"jsonrpc":"2.0","method":"   ","id":1}"#)
            .expect("response");
        let value: Value = serde_json::from_str(&response).expect("json");
        assert_eq!(value["error"]["code"], -32600);
    }
}
