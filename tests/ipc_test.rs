use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use tempfile::{TempDir, tempdir};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;
use tokio_util::sync::CancellationToken;

use palingenesis::ipc::protocol::DaemonStatus;
use palingenesis::ipc::socket::{DaemonStateAccess, IpcServer};

#[derive(Default)]
struct MockState {
    paused: AtomicBool,
}

impl DaemonStateAccess for MockState {
    fn get_status(&self) -> DaemonStatus {
        DaemonStatus {
            state: if self.paused.load(Ordering::SeqCst) {
                "paused".to_string()
            } else {
                "monitoring".to_string()
            },
            uptime_secs: 120,
            current_session: None,
            saves_count: 0,
            total_resumes: 0,
            time_saved_seconds: 0.0,
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

    fn new_session(&self) -> Result<(), String> {
        Ok(())
    }

    fn reload_config(&self) -> Result<(), String> {
        Ok(())
    }
}

async fn start_server() -> (
    Arc<IpcServer>,
    CancellationToken,
    Arc<MockState>,
    TempDir,
    tokio::task::JoinHandle<()>,
) {
    let temp = tempdir().unwrap();
    let sock_path = temp.path().join("ipc.sock");

    let mut server = IpcServer::with_path(sock_path);
    server.bind().await.unwrap();

    let server = Arc::new(server);
    let cancel = CancellationToken::new();
    let state = Arc::new(MockState::default());
    let server_state = Arc::clone(&state);
    let server_cancel = cancel.clone();
    let server_ref = Arc::clone(&server);

    let handle = tokio::spawn(async move {
        let _ = server_ref.run(server_state, server_cancel).await;
    });

    (server, cancel, state, temp, handle)
}

#[tokio::test]
async fn test_ipc_status_command() {
    let (server, cancel, _state, _temp, handle) = start_server().await;
    let stream = UnixStream::connect(server.path()).await.unwrap();
    let (reader, mut writer) = stream.into_split();
    let mut reader = BufReader::new(reader);

    writer.write_all(b"STATUS\n").await.unwrap();
    writer.flush().await.unwrap();

    let mut response = String::new();
    reader.read_line(&mut response).await.unwrap();

    let status: DaemonStatus = serde_json::from_str(response.trim_end()).unwrap();
    assert_eq!(status.state, "monitoring");

    cancel.cancel();
    handle.await.unwrap();
    server.cleanup().unwrap();
}

#[tokio::test]
async fn test_ipc_concurrent_connections() {
    let (server, cancel, _state, _temp, handle) = start_server().await;
    let mut tasks = Vec::new();

    for _ in 0..5 {
        let path = server.path().to_path_buf();
        tasks.push(tokio::spawn(async move {
            let stream = UnixStream::connect(path).await.unwrap();
            let (reader, mut writer) = stream.into_split();
            let mut reader = BufReader::new(reader);

            writer.write_all(b"STATUS\n").await.unwrap();
            writer.flush().await.unwrap();

            let mut response = String::new();
            reader.read_line(&mut response).await.unwrap();
            let status: DaemonStatus = serde_json::from_str(response.trim_end()).unwrap();
            status.state
        }));
    }

    for task in tasks {
        let state = task.await.unwrap();
        assert_eq!(state, "monitoring");
    }

    cancel.cancel();
    handle.await.unwrap();
    server.cleanup().unwrap();
}

#[tokio::test]
async fn test_ipc_graceful_shutdown() {
    let (server, cancel, _state, _temp, handle) = start_server().await;

    cancel.cancel();
    handle.await.unwrap();
    server.cleanup().unwrap();
    assert!(!server.path().exists());
}
