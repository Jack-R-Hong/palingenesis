use std::borrow::Cow;
use std::sync::Arc;

use rmcp::handler::server::tool::ToolRouter;
use rmcp::model::{
    ErrorData, NumberOrString, ServerCapabilities, ServerInfo, ServerJsonRpcMessage,
};
use rmcp::service::{
    QuitReason, RoleServer, RxJsonRpcMessage, ServerInitializeError, ServiceError, ServiceExt,
    TxJsonRpcMessage,
};
use rmcp::transport::Transport;
use rmcp::{ServerHandler, tool_handler, tool_router};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;
use tracing::{error, info};

use crate::ipc::socket::DaemonStateAccess;

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
        let transport = StdioTransport::new();
        let service = self.serve_with_ct(transport, cancel.clone()).await?;
        let service_cancel = service.cancellation_token();
        let mut waiting = Box::pin(service.waiting());

        let quit_reason = tokio::select! {
            _ = cancel.cancelled() => {
                info!("MCP server shutting down via cancellation");
                service_cancel.cancel();
                waiting.await?
            }
            result = &mut waiting => result?,
        };

        if let QuitReason::JoinError(err) = quit_reason {
            error!(error = %err, "MCP server task failed");
            return Err(err.into());
        }

        Ok(())
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

    async fn send_parse_error(&self, error: impl Into<Cow<'static, str>>) {
        let error = ErrorData::parse_error(error, None);
        let message = ServerJsonRpcMessage::error(error, NumberOrString::Number(0));
        let _ = self.write_message(message).await;
    }

    async fn write_message(
        &self,
        message: TxJsonRpcMessage<RoleServer>,
    ) -> Result<(), std::io::Error> {
        let payload = serde_json::to_vec(&message)
            .map_err(|error| std::io::Error::new(std::io::ErrorKind::Other, error))?;
        let mut write = self.write.lock().await;
        if let Some(ref mut write) = *write {
            write.write_all(&payload).await?;
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
}

impl Transport<RoleServer> for StdioTransport {
    type Error = std::io::Error;

    fn send(
        &mut self,
        item: TxJsonRpcMessage<RoleServer>,
    ) -> impl std::future::Future<Output = Result<(), Self::Error>> + Send + 'static {
        let write = Arc::clone(&self.write);
        async move {
            let payload = serde_json::to_vec(&item)
                .map_err(|error| std::io::Error::new(std::io::ErrorKind::Other, error))?;
            let mut write = write.lock().await;
            if let Some(ref mut write) = *write {
                write.write_all(&payload).await?;
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
    }

    #[allow(clippy::manual_async_fn)]
    fn receive(
        &mut self,
    ) -> impl std::future::Future<Output = Option<RxJsonRpcMessage<RoleServer>>> + Send {
        async move {
            loop {
                let mut line = String::new();
                let bytes = match self.read.read_line(&mut line).await {
                    Ok(bytes) => bytes,
                    Err(err) => {
                        self.send_parse_error(err.to_string()).await;
                        continue;
                    }
                };

                if bytes == 0 {
                    return None;
                }

                let line = line.trim_end_matches(['\n', '\r']);
                if line.is_empty() {
                    continue;
                }

                match serde_json::from_str::<RxJsonRpcMessage<RoleServer>>(line) {
                    Ok(message) => return Some(message),
                    Err(err) => {
                        self.send_parse_error(err.to_string()).await;
                        continue;
                    }
                }
            }
        }
    }

    fn close(&mut self) -> impl std::future::Future<Output = Result<(), Self::Error>> + Send {
        let write = Arc::clone(&self.write);
        async move {
            let mut write = write.lock().await;
            drop(write.take());
            Ok(())
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
