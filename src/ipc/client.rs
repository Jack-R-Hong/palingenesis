use std::path::PathBuf;

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;
use tracing::debug;

use crate::config::Paths;
use crate::ipc::protocol::{DaemonStatus, IpcCommand, IpcResponse};

#[cfg(test)]
const CONNECTION_TIMEOUT_SECS: u64 = 1;

#[cfg(not(test))]
const CONNECTION_TIMEOUT_SECS: u64 = 5;

#[derive(Debug, thiserror::Error)]
pub enum IpcClientError {
    #[error("Daemon not running")]
    NotRunning,

    #[error("Daemon unresponsive")]
    Timeout,

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Protocol error: {0}")]
    Protocol(String),
}

pub struct IpcClient {
    path: PathBuf,
    reader: BufReader<tokio::net::unix::OwnedReadHalf>,
    writer: tokio::net::unix::OwnedWriteHalf,
}

impl IpcClient {
    /// Connect to the daemon's IPC socket.
    pub async fn connect() -> Result<Self, IpcClientError> {
        let path = Paths::runtime_dir().join("palingenesis.sock");
        Self::connect_with_path(path).await
    }

    async fn connect_with_path(path: PathBuf) -> Result<Self, IpcClientError> {
        if !path.exists() {
            return Err(IpcClientError::NotRunning);
        }

        debug!(path = %path.display(), "Connecting to IPC socket");

        let connect_result = tokio::time::timeout(
            std::time::Duration::from_secs(CONNECTION_TIMEOUT_SECS),
            UnixStream::connect(&path),
        )
        .await;

        let stream = match connect_result {
            Ok(Ok(stream)) => stream,
            Ok(Err(error)) => return Err(Self::map_connect_error(error)),
            Err(_) => return Err(IpcClientError::Timeout),
        };

        let (reader, writer) = stream.into_split();
        Ok(Self {
            path,
            reader: BufReader::new(reader),
            writer,
        })
    }

    /// Send a command to the daemon and read the response.
    pub async fn send_command(&mut self, cmd: IpcCommand) -> Result<IpcResponse, IpcClientError> {
        let command = Self::command_text(&cmd);
        debug!(
            path = %self.path.display(),
            command = %command.trim_end(),
            "Sending IPC command"
        );

        tokio::time::timeout(
            std::time::Duration::from_secs(CONNECTION_TIMEOUT_SECS),
            self.writer.write_all(command.as_bytes()),
        )
        .await
        .map_err(|_| IpcClientError::Timeout)??;

        self.writer.flush().await?;

        let mut response = String::new();
        let read_result = tokio::time::timeout(
            std::time::Duration::from_secs(CONNECTION_TIMEOUT_SECS),
            self.reader.read_line(&mut response),
        )
        .await;

        let bytes_read = match read_result {
            Ok(Ok(bytes_read)) => bytes_read,
            Ok(Err(error)) => return Err(IpcClientError::Io(error)),
            Err(_) => return Err(IpcClientError::Timeout),
        };

        if bytes_read == 0 {
            return Err(IpcClientError::Protocol("Empty response".to_string()));
        }

        Self::parse_response(&response)
    }

    /// Request daemon status.
    pub async fn status() -> Result<DaemonStatus, IpcClientError> {
        let mut client = Self::connect().await?;
        let response = client.send_command(IpcCommand::Status).await?;
        Self::expect_status(response)
    }

    /// Pause daemon monitoring.
    pub async fn pause() -> Result<(), IpcClientError> {
        let mut client = Self::connect().await?;
        let response = client.send_command(IpcCommand::Pause).await?;
        Self::expect_ok(response)
    }

    /// Resume daemon monitoring.
    pub async fn resume() -> Result<(), IpcClientError> {
        let mut client = Self::connect().await?;
        let response = client.send_command(IpcCommand::Resume).await?;
        Self::expect_ok(response)
    }

    /// Reload daemon configuration.
    pub async fn reload() -> Result<(), IpcClientError> {
        let mut client = Self::connect().await?;
        let response = client.send_command(IpcCommand::Reload).await?;
        Self::expect_ok(response)
    }

    fn command_text(cmd: &IpcCommand) -> &'static str {
        match cmd {
            IpcCommand::Status => "STATUS\n",
            IpcCommand::Pause => "PAUSE\n",
            IpcCommand::Resume => "RESUME\n",
            IpcCommand::Reload => "RELOAD\n",
        }
    }

    fn parse_response(response: &str) -> Result<IpcResponse, IpcClientError> {
        let trimmed = response.trim_end();
        if trimmed.is_empty() {
            return Err(IpcClientError::Protocol("Empty response".to_string()));
        }

        if trimmed == "OK" {
            return Ok(IpcResponse::Ok);
        }

        if let Some(message) = trimmed.strip_prefix("ERR:") {
            return Ok(IpcResponse::Error {
                message: message.trim().to_string(),
            });
        }

        let status: DaemonStatus = serde_json::from_str(trimmed)
            .map_err(|error| IpcClientError::Protocol(format!("Invalid response: {error}")))?;
        Ok(IpcResponse::Status(status))
    }

    fn expect_ok(response: IpcResponse) -> Result<(), IpcClientError> {
        match response {
            IpcResponse::Ok => Ok(()),
            IpcResponse::Error { message } => Err(IpcClientError::Protocol(message)),
            IpcResponse::Status(_) => Err(IpcClientError::Protocol(
                "Unexpected status response".to_string(),
            )),
        }
    }

    fn expect_status(response: IpcResponse) -> Result<DaemonStatus, IpcClientError> {
        match response {
            IpcResponse::Status(status) => Ok(status),
            IpcResponse::Error { message } => Err(IpcClientError::Protocol(message)),
            IpcResponse::Ok => Err(IpcClientError::Protocol(
                "Unexpected OK response".to_string(),
            )),
        }
    }

    fn map_connect_error(error: std::io::Error) -> IpcClientError {
        match error.kind() {
            std::io::ErrorKind::NotFound | std::io::ErrorKind::ConnectionRefused => {
                IpcClientError::NotRunning
            }
            _ => IpcClientError::Io(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

    use tempfile::tempdir;
    use tokio::net::UnixListener;
    use tokio::sync::oneshot;
    use tokio_util::sync::CancellationToken;

    use crate::ipc::socket::{DaemonStateAccess, IpcServer};
    use crate::test_utils::ENV_LOCK;

    #[derive(Default)]
    struct MockState {
        paused: AtomicBool,
        reloads: AtomicUsize,
    }

    impl MockState {
        fn is_paused(&self) -> bool {
            self.paused.load(Ordering::SeqCst)
        }

        fn reload_count(&self) -> usize {
            self.reloads.load(Ordering::SeqCst)
        }
    }

    impl DaemonStateAccess for MockState {
        fn get_status(&self) -> DaemonStatus {
            DaemonStatus {
                state: if self.is_paused() {
                    "paused".to_string()
                } else {
                    "monitoring".to_string()
                },
                uptime_secs: 3600,
                current_session: Some("/tmp/session.md".to_string()),
                saves_count: 42,
                total_resumes: 10,
            }
        }

        fn pause(&self) -> Result<(), String> {
            self.paused.store(true, Ordering::SeqCst);
            Ok(())
        }

        fn resume(&self) -> Result<(), String> {
            self.paused.store(false, Ordering::SeqCst);
            Ok(())
        }

        fn reload_config(&self) -> Result<(), String> {
            self.reloads.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }

    fn set_env_var(key: &str, value: impl AsRef<std::ffi::OsStr>) {
        unsafe {
            std::env::set_var(key, value);
        }
    }

    fn remove_env_var(key: &str) {
        unsafe {
            std::env::remove_var(key);
        }
    }

    async fn start_server(sock_path: PathBuf, state: Arc<MockState>) -> CancellationToken {
        let mut server = IpcServer::with_path(sock_path);
        server.bind().await.unwrap();

        let server = Arc::new(server);
        let cancel = CancellationToken::new();
        let server_ref = Arc::clone(&server);
        let server_state = Arc::clone(&state);
        let server_cancel = cancel.clone();
        tokio::spawn(async move { server_ref.run(server_state, server_cancel).await });
        cancel
    }

    #[tokio::test]
    async fn test_status_command_success() {
        let _lock = ENV_LOCK.lock().unwrap();
        let temp = tempdir().unwrap();
        set_env_var("PALINGENESIS_RUNTIME", temp.path());

        let sock_path = temp.path().join("palingenesis.sock");
        let state = Arc::new(MockState::default());
        let cancel = start_server(sock_path, Arc::clone(&state)).await;

        let status = IpcClient::status().await.unwrap();
        assert_eq!(status.state, "monitoring");
        assert_eq!(status.uptime_secs, 3600);

        cancel.cancel();
        remove_env_var("PALINGENESIS_RUNTIME");
    }

    #[tokio::test]
    async fn test_daemon_not_running() {
        let _lock = ENV_LOCK.lock().unwrap();
        let temp = tempdir().unwrap();
        set_env_var("PALINGENESIS_RUNTIME", temp.path());

        let error = IpcClient::status().await.err().unwrap();
        assert!(matches!(error, IpcClientError::NotRunning));

        remove_env_var("PALINGENESIS_RUNTIME");
    }

    #[tokio::test]
    async fn test_timeout_handling() {
        let _lock = ENV_LOCK.lock().unwrap();
        let temp = tempdir().unwrap();
        set_env_var("PALINGENESIS_RUNTIME", temp.path());
        let sock_path = temp.path().join("palingenesis.sock");

        let listener = UnixListener::bind(&sock_path).unwrap();
        let (ready_tx, ready_rx) = oneshot::channel();
        let server_task = tokio::spawn(async move {
            let _ = ready_tx.send(());
            if let Ok((stream, _)) = listener.accept().await {
                let mut buffer = [0u8; 128];
                let _ = stream.readable().await;
                let _ = stream.try_read(&mut buffer);
                tokio::time::sleep(std::time::Duration::from_secs(CONNECTION_TIMEOUT_SECS + 1))
                    .await;
            }
        });

        let _ = ready_rx.await;
        let error = IpcClient::status().await.err().unwrap();
        assert!(matches!(error, IpcClientError::Timeout));

        server_task.abort();
        let _ = server_task.await;
        remove_env_var("PALINGENESIS_RUNTIME");
    }

    #[tokio::test]
    async fn test_pause_resume_reload_commands() {
        let _lock = ENV_LOCK.lock().unwrap();
        let temp = tempdir().unwrap();
        set_env_var("PALINGENESIS_RUNTIME", temp.path());

        let sock_path = temp.path().join("palingenesis.sock");
        let state = Arc::new(MockState::default());
        let cancel = start_server(sock_path, Arc::clone(&state)).await;

        IpcClient::pause().await.unwrap();
        assert!(state.is_paused());

        IpcClient::resume().await.unwrap();
        assert!(!state.is_paused());

        IpcClient::reload().await.unwrap();
        assert_eq!(state.reload_count(), 1);

        cancel.cancel();
        remove_env_var("PALINGENESIS_RUNTIME");
    }
}
