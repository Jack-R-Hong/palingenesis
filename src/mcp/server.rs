use std::sync::Arc;

use rmcp::handler::server::tool::ToolRouter;
use rmcp::model::{ServerCapabilities, ServerInfo};
use rmcp::service::{ServerInitializeError, ServiceError};
use rmcp::{ServerHandler, tool_handler, tool_router};
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;
use tracing::info;

use crate::ipc::socket::DaemonStateAccess;
use crate::mcp::protocol::{self, JsonRpcError, JsonRpcHandler};

#[derive(Clone)]
pub struct McpServer {
    tool_router: ToolRouter<Self>,
    state: Arc<dyn DaemonStateAccess>,
}

#[derive(Debug, thiserror::Error)]
pub enum McpServerError {
    #[error("MCP initialization error: {0}")]
    Initialize(#[from] ServerInitializeError),

    #[error("MCP service error: {0}")]
    Service(#[from] ServiceError),

    #[error("MCP IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("MCP task error: {0}")]
    Task(#[from] tokio::task::JoinError),
}

#[tool_router]
impl McpServer {
    pub fn new(state: Arc<dyn DaemonStateAccess>) -> Self {
        Self {
            tool_router: Self::tool_router(),
            state,
        }
    }
}

#[tool_handler]
impl ServerHandler for McpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            instructions: Some("palingenesis MCP server".to_string()),
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            ..Default::default()
        }
    }
}

impl McpServer {
    pub fn state(&self) -> &Arc<dyn DaemonStateAccess> {
        &self.state
    }

    pub async fn run(self, cancel: CancellationToken) -> Result<(), McpServerError> {
        let mut transport = StdioTransport::new();
        self.run_json_rpc(&mut transport, cancel).await
    }

    pub fn process_json_rpc(&self, input: &str) -> Option<String> {
        protocol::process_input(self, input)
    }

    async fn run_json_rpc(
        &self,
        transport: &mut StdioTransport,
        cancel: CancellationToken,
    ) -> Result<(), McpServerError> {
        loop {
            let line = tokio::select! {
                _ = cancel.cancelled() => {
                    info!("MCP server shutting down via cancellation");
                    return Ok(());
                }
                line = transport.read_next_line() => line?,
            };

            let Some(line) = line else {
                return Ok(());
            };

            if let Some(response) = self.process_json_rpc(&line) {
                transport.write_raw(&response).await?;
            }
        }
    }
}

struct StdioTransport {
    read: BufReader<tokio::io::Stdin>,
    write: Arc<Mutex<Option<tokio::io::Stdout>>>,
}

impl StdioTransport {
    fn new() -> Self {
        let (stdin, stdout) = rmcp::transport::io::stdio();
        let read = BufReader::new(stdin);
        let write = Arc::new(Mutex::new(Some(stdout)));
        Self { read, write }
    }

    async fn write_raw(&self, payload: &str) -> Result<(), std::io::Error> {
        let payload = payload.as_bytes();
        let mut write = self.write.lock().await;
        if let Some(ref mut write) = *write {
            write.write_all(payload).await?;
            write.write_all(b"\n").await?;
            write.flush().await?;
            Ok(())
        } else {
            Err(std::io::Error::new(
                std::io::ErrorKind::NotConnected,
                "Transport is closed",
            ))
        }
    }

    async fn read_next_line(&mut self) -> Result<Option<String>, std::io::Error> {
        loop {
            let mut line = String::new();
            let bytes = self.read.read_line(&mut line).await?;
            if bytes == 0 {
                return Ok(None);
            }

            let line = line.trim_end_matches(['\n', '\r']);
            if line.is_empty() {
                continue;
            }

            return Ok(Some(line.to_string()));
        }
    }
}

impl JsonRpcHandler for McpServer {
    fn handle(&self, method: &str, params: Option<Value>) -> Result<Value, JsonRpcError> {
        match method {
            "initialize" => Ok(protocol::default_initialize_response()),
            "tools/list" => Ok(json!({"tools": []})),
            "tools/call" => Ok(json!({"content": [], "params": params})),
            _ => Err(JsonRpcError::method_not_found()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ipc::protocol::DaemonStatus;

    struct MockState;

    impl DaemonStateAccess for MockState {
        fn get_status(&self) -> DaemonStatus {
            DaemonStatus {
                state: "monitoring".to_string(),
                uptime_secs: 0,
                current_session: None,
                saves_count: 0,
                total_resumes: 0,
                time_saved_seconds: 0.0,
                time_saved_human: None,
            }
        }

        fn pause(&self) -> Result<(), String> {
            Ok(())
        }

        fn resume(&self) -> Result<(), String> {
            Ok(())
        }

        fn new_session(&self) -> Result<(), String> {
            Ok(())
        }

        fn reload_config(&self) -> Result<(), String> {
            Ok(())
        }
    }

    #[test]
    fn test_mcp_server_creation() {
        let server = McpServer::new(Arc::new(MockState));
        let _ = server.state();
        let info = server.get_info();
        assert!(info.capabilities.tools.is_some());
    }
}
