use std::path::{Path, PathBuf};
use std::sync::Arc;

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, warn};

use crate::config::Paths;
use crate::ipc::protocol::{DaemonStatus, IpcCommand, IpcResponse};

#[cfg(test)]
const CONNECTION_TIMEOUT_SECS: u64 = 1;

#[cfg(not(test))]
const CONNECTION_TIMEOUT_SECS: u64 = 5;

#[derive(Debug, thiserror::Error)]
pub enum IpcError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

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

        if self.path.exists() {
            warn!(path = %self.path.display(), "Removing stale socket file");
            std::fs::remove_file(&self.path)?;
        }

        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o700))?;
            }
        } else {
            return Err(IpcError::Io(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Socket path has no parent directory",
            )));
        }

        let listener = UnixListener::bind(&self.path)?;
        info!(path = %self.path.display(), "IPC socket bound");

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
            warn!(error = %e, "Failed to clean up IPC socket");
        }
    }
}

impl Default for IpcServer {
    fn default() -> Self {
        Self::new()
    }
}

async fn handle_connection<S: DaemonStateAccess>(
    stream: UnixStream,
    state: Arc<S>,
) -> Result<(), IpcError> {
    let (reader, mut writer) = stream.into_split();
    let mut reader = BufReader::new(reader);
    let mut line = String::new();

    let read_result = tokio::time::timeout(
        std::time::Duration::from_secs(CONNECTION_TIMEOUT_SECS),
        reader.read_line(&mut line),
    )
    .await;

    let response = match read_result {
        Ok(Ok(0)) => {
            return Ok(());
        }
        Ok(Ok(_)) => match IpcCommand::parse(&line) {
            Some(cmd) => handle_command(cmd, &*state),
            None => IpcResponse::Error {
                message: format!("Unknown command: {}", line.trim()),
            },
        },
        Ok(Err(e)) => return Err(IpcError::Io(e)),
        Err(_) => IpcResponse::Error {
            message: "Connection timeout".to_string(),
        },
    };

    writer.write_all(response.to_text().as_bytes()).await?;
    writer.flush().await?;

    Ok(())
}

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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use tempfile::tempdir;
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

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

        std::fs::write(&sock_path, "stale").unwrap();

        let mut server = IpcServer::with_path(sock_path.clone());
        server.bind().await.unwrap();

        assert!(sock_path.exists());
    }

    #[tokio::test]
    async fn test_pause_resume_and_reload_commands() {
        let temp = tempdir().unwrap();
        let sock_path = temp.path().join("test.sock");
        let mut server = IpcServer::with_path(sock_path.clone());
        server.bind().await.unwrap();

        let server = Arc::new(server);
        let cancel = CancellationToken::new();
        let state = Arc::new(MockState::default());
        let server_ref = Arc::clone(&server);
        let server_state = Arc::clone(&state);
        let server_cancel = cancel.clone();
        let server_task =
            tokio::spawn(async move { server_ref.run(server_state, server_cancel).await });

        let stream = UnixStream::connect(&sock_path).await.unwrap();
        let (reader, mut writer) = stream.into_split();
        let mut reader = BufReader::new(reader);

        writer.write_all(b"PAUSE\n").await.unwrap();
        writer.flush().await.unwrap();

        let mut response = String::new();
        reader.read_line(&mut response).await.unwrap();
        assert_eq!(response, "OK\n");
        assert!(state.is_paused());

        let stream = UnixStream::connect(&sock_path).await.unwrap();
        let (reader, mut writer) = stream.into_split();
        let mut reader = BufReader::new(reader);

        writer.write_all(b"RESUME\n").await.unwrap();
        writer.flush().await.unwrap();

        response.clear();
        reader.read_line(&mut response).await.unwrap();
        assert_eq!(response, "OK\n");
        assert!(!state.is_paused());

        let stream = UnixStream::connect(&sock_path).await.unwrap();
        let (reader, mut writer) = stream.into_split();
        let mut reader = BufReader::new(reader);

        writer.write_all(b"RELOAD\n").await.unwrap();
        writer.flush().await.unwrap();

        response.clear();
        reader.read_line(&mut response).await.unwrap();
        assert_eq!(response, "OK\n");
        assert_eq!(state.reload_count(), 1);

        cancel.cancel();
        server_task.await.unwrap().unwrap();
        server.cleanup().unwrap();
    }

    #[tokio::test]
    async fn test_connection_timeout() {
        let temp = tempdir().unwrap();
        let sock_path = temp.path().join("test.sock");
        let mut server = IpcServer::with_path(sock_path.clone());
        server.bind().await.unwrap();

        let server = Arc::new(server);
        let cancel = CancellationToken::new();
        let state = Arc::new(MockState::default());
        let server_ref = Arc::clone(&server);
        let server_state = Arc::clone(&state);
        let server_cancel = cancel.clone();
        let server_task =
            tokio::spawn(async move { server_ref.run(server_state, server_cancel).await });

        let stream = UnixStream::connect(&sock_path).await.unwrap();
        let mut reader = BufReader::new(stream);

        tokio::time::sleep(std::time::Duration::from_secs(CONNECTION_TIMEOUT_SECS + 1)).await;

        let mut response = String::new();
        reader.read_line(&mut response).await.unwrap();
        assert_eq!(response, "ERR: Connection timeout\n");

        cancel.cancel();
        server_task.await.unwrap().unwrap();
        server.cleanup().unwrap();
    }

    #[tokio::test]
    async fn test_cleanup_on_drop() {
        let temp = tempdir().unwrap();
        let sock_path = temp.path().join("test.sock");

        {
            let mut server = IpcServer::with_path(sock_path.clone());
            server.bind().await.unwrap();
            assert!(sock_path.exists());
        }

        assert!(!sock_path.exists());
    }

    #[tokio::test]
    async fn test_status_command() {
        let temp = tempdir().unwrap();
        let sock_path = temp.path().join("test.sock");
        let mut server = IpcServer::with_path(sock_path.clone());
        server.bind().await.unwrap();

        let server = Arc::new(server);
        let cancel = CancellationToken::new();
        let state = Arc::new(MockState::default());
        let server_ref = Arc::clone(&server);
        let server_state = Arc::clone(&state);
        let server_cancel = cancel.clone();
        let server_task =
            tokio::spawn(async move { server_ref.run(server_state, server_cancel).await });

        let stream = tokio::net::UnixStream::connect(&sock_path).await.unwrap();
        let (reader, mut writer) = stream.into_split();
        let mut reader = BufReader::new(reader);

        writer.write_all(b"STATUS\n").await.unwrap();
        writer.flush().await.unwrap();

        let mut response = String::new();
        reader.read_line(&mut response).await.unwrap();
        let status: crate::ipc::protocol::DaemonStatus =
            serde_json::from_str(response.trim_end()).unwrap();
        assert_eq!(status.state, "monitoring");
        assert_eq!(status.uptime_secs, 3600);

        cancel.cancel();
        server_task.await.unwrap().unwrap();
        server.cleanup().unwrap();
    }

    #[tokio::test]
    async fn test_unknown_command_returns_error() {
        let temp = tempdir().unwrap();
        let sock_path = temp.path().join("test.sock");
        let mut server = IpcServer::with_path(sock_path.clone());
        server.bind().await.unwrap();

        let server = Arc::new(server);
        let cancel = CancellationToken::new();
        let state = Arc::new(MockState::default());
        let server_ref = Arc::clone(&server);
        let server_state = Arc::clone(&state);
        let server_cancel = cancel.clone();
        let server_task =
            tokio::spawn(async move { server_ref.run(server_state, server_cancel).await });

        let stream = tokio::net::UnixStream::connect(&sock_path).await.unwrap();
        let (reader, mut writer) = stream.into_split();
        let mut reader = BufReader::new(reader);

        writer.write_all(b"INVALID_COMMAND\n").await.unwrap();
        writer.flush().await.unwrap();

        let mut response = String::new();
        reader.read_line(&mut response).await.unwrap();
        assert!(response.starts_with("ERR: Unknown command:"));

        cancel.cancel();
        server_task.await.unwrap().unwrap();
        server.cleanup().unwrap();
    }
}
