use crate::ipc::client::{IpcClient, IpcClientError};

pub async fn handle_pause() -> anyhow::Result<()> {
    match IpcClient::pause().await {
        Ok(()) => {
            println!("Monitoring paused");
            Ok(())
        }
        Err(IpcClientError::NotRunning) => {
            eprintln!("Daemon not running");
            std::process::exit(1);
        }
        Err(IpcClientError::Timeout) => {
            eprintln!("Daemon unresponsive");
            std::process::exit(1);
        }
        Err(IpcClientError::Protocol(message)) => {
            if message.eq_ignore_ascii_case("Daemon already paused") {
                println!("Already paused");
                Ok(())
            } else {
                Err(IpcClientError::Protocol(message).into())
            }
        }
        Err(err) => Err(err.into()),
    }
}

pub async fn handle_resume() -> anyhow::Result<()> {
    println!("resume not implemented (Story 3.8)");
    Ok(())
}

pub async fn handle_new_session() -> anyhow::Result<()> {
    println!("new-session not implemented (Story 3.9)");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    use tempfile::tempdir;
    use tokio_util::sync::CancellationToken;

    use crate::ipc::protocol::DaemonStatus;
    use crate::ipc::socket::{DaemonStateAccess, IpcServer};
    use crate::test_utils::ENV_LOCK;

    #[derive(Default)]
    struct MockState {
        paused: AtomicBool,
    }

    impl MockState {
        fn is_paused(&self) -> bool {
            self.paused.load(Ordering::SeqCst)
        }
    }

    impl DaemonStateAccess for MockState {
        fn get_status(&self) -> DaemonStatus {
            DaemonStatus {
                state: if self.paused.load(Ordering::SeqCst) {
                    "paused".to_string()
                } else {
                    "monitoring".to_string()
                },
                uptime_secs: 1,
                current_session: Some("/tmp/session.md".to_string()),
                saves_count: 1,
                total_resumes: 1,
            }
        }

        fn pause(&self) -> Result<(), String> {
            if self.paused.swap(true, Ordering::SeqCst) {
                return Err("Daemon already paused".to_string());
            }
            Ok(())
        }

        fn resume(&self) -> Result<(), String> {
            let was_paused = self.paused.swap(false, Ordering::SeqCst);
            if !was_paused {
                return Err("Daemon is not paused".to_string());
            }
            Ok(())
        }

        fn new_session(&self) -> Result<(), String> {
            Ok(())
        }

        fn reload_config(&self) -> Result<(), String> {
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

    async fn start_server(state: Arc<MockState>) -> CancellationToken {
        let mut server = IpcServer::new();
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
    async fn test_handle_pause() {
        let _lock = ENV_LOCK.lock().unwrap();
        let temp = tempdir().unwrap();
        set_env_var("PALINGENESIS_RUNTIME", temp.path());

        let state = Arc::new(MockState::default());
        let cancel = start_server(Arc::clone(&state)).await;

        handle_pause().await.unwrap();
        assert!(state.is_paused());

        cancel.cancel();
        remove_env_var("PALINGENESIS_RUNTIME");
    }
}
