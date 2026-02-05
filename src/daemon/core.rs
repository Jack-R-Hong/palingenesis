use std::sync::Arc;

use tokio::sync::mpsc;
use tokio::time;
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};

use crate::daemon::pid::{PidError, PidFile};
use crate::daemon::shutdown::{SHUTDOWN_TIMEOUT, ShutdownCoordinator, ShutdownResult};
use crate::daemon::signals::{listen_for_signals, DaemonSignal};
use crate::daemon::state::DaemonState;
use crate::http::HttpServer;
use crate::ipc::socket::{DaemonStateAccess, IpcError, IpcServer};

#[derive(Debug, thiserror::Error)]
pub enum DaemonError {
    #[error("PID error: {0}")]
    Pid(#[from] PidError),

    #[error("IPC error: {0}")]
    Ipc(#[from] IpcError),
}

pub struct Daemon {
    pid_file: PidFile,
    ipc_server: IpcServer,
    shutdown: ShutdownCoordinator,
    state: Arc<DaemonState>,
    http_handle: Option<tokio::task::JoinHandle<()>>,
}

impl Daemon {
    pub fn new() -> Self {
        Self {
            pid_file: PidFile::new(),
            ipc_server: IpcServer::new(),
            shutdown: ShutdownCoordinator::new(),
            state: Arc::new(DaemonState::new()),
            http_handle: None,
        }
    }

    pub async fn run(&mut self) -> Result<(), DaemonError> {
        info!("Starting daemon");
        self.pid_file.acquire()?;

        if let Err(err) = self.ipc_server.bind().await {
            if let Err(release_err) = self.pid_file.release() {
                error!(error = %release_err, "Failed to release PID file after IPC bind failure");
            }
            return Err(err.into());
        }

        let cancel = self.shutdown.cancel_token();

        let (signal_tx, mut signal_rx) = mpsc::channel(4);
        let signal_cancel = cancel.clone();
        self.shutdown
            .register_task(tokio::spawn(async move {
                listen_for_signals(signal_tx, signal_cancel).await;
            }));

        let signal_state = Arc::clone(&self.state);
        let signal_cancel = cancel.clone();
        self.shutdown.register_task(tokio::spawn(async move {
            while let Some(signal) = signal_rx.recv().await {
                match signal {
                    DaemonSignal::Shutdown => {
                        signal_cancel.cancel();
                        break;
                    }
                    DaemonSignal::Reload => {
                        if let Err(err) = signal_state.reload_config() {
                            error!(error = %err, "Failed to reload configuration");
                        }
                    }
                }
            }
        }));

        if self.state.auto_detect_active() {
            let detection_state = Arc::clone(&self.state);
            let detection_cancel = cancel.clone();
            self.shutdown.register_task(tokio::spawn(async move {
                run_auto_detection(detection_state, detection_cancel).await;
            }));
        }

        if let Some(config) = self.state.daemon_config() {
            match HttpServer::from_config(&config, cancel.clone()) {
                Ok(Some(server)) => {
                    let server_cancel = cancel.clone();
                    let handle = tokio::spawn(async move {
                        if let Err(err) = server.start().await {
                            error!(error = %err, "HTTP server stopped with error");
                            server_cancel.cancel();
                        }
                    });
                    self.http_handle = Some(handle);
                }
                Ok(None) => {}
                Err(err) => {
                    warn!(error = %err, "Failed to configure HTTP server");
                }
            }
        } else {
            warn!("Config lock poisoned; skipping HTTP server startup");
        }

        let server = std::mem::take(&mut self.ipc_server);
        let server_state = Arc::clone(&self.state);
        let server_cancel = cancel.clone();
        self.shutdown.register_task(tokio::spawn(async move {
            let error_cancel = server_cancel.clone();
            if let Err(err) = server.run(server_state, server_cancel).await {
                error!(error = %err, "IPC server stopped with error");
                error_cancel.cancel();
            }
        }));

        cancel.cancelled().await;
        info!("Shutdown requested");

        let shutdown = std::mem::take(&mut self.shutdown);
        match shutdown.shutdown().await {
            ShutdownResult::Graceful => info!("Shutdown completed"),
            ShutdownResult::TimedOut { hung_tasks } => {
                warn!(hung_tasks, "Shutdown timed out")
            }
        }

        if let Some(handle) = self.http_handle.take() {
            match time::timeout(SHUTDOWN_TIMEOUT, handle).await {
                Ok(Ok(())) => info!("HTTP server stopped"),
                Ok(Err(err)) => warn!(error = %err, "HTTP server task failed"),
                Err(_) => warn!("HTTP server shutdown timed out"),
            }
        }

        self.pid_file.release()?;
        Ok(())
    }
}

async fn run_auto_detection(state: Arc<DaemonState>, cancel: CancellationToken) {
    let mut interval = time::interval(state.auto_detect_interval());
    loop {
        tokio::select! {
            _ = cancel.cancelled() => {
                break;
            }
            _ = interval.tick() => {
                state.refresh_auto_detected_assistants();
            }
        }
    }
}

impl Default for Daemon {
    fn default() -> Self {
        Self::new()
    }
}
