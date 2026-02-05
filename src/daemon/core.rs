use std::sync::Arc;

use chrono::Utc;
use tokio::sync::mpsc;
use tokio::time;
use tokio_util::sync::CancellationToken;
use tracing::{error, info, info_span, warn, Instrument};

use crate::daemon::pid::{PidError, PidFile};
use crate::daemon::shutdown::{SHUTDOWN_TIMEOUT, ShutdownCoordinator, ShutdownResult};
use crate::daemon::signals::{DaemonSignal, listen_for_signals};
use crate::daemon::state::DaemonState;
use crate::http::{EventBroadcaster, HttpServer};
use crate::ipc::socket::{DaemonStateAccess, IpcError, IpcServer};
use crate::notify::events::NotificationEvent;

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
    event_broadcaster: EventBroadcaster,
}

impl Daemon {
    pub fn new() -> Self {
        Self {
            pid_file: PidFile::new(),
            ipc_server: IpcServer::new(),
            shutdown: ShutdownCoordinator::new(),
            state: Arc::new(DaemonState::new()),
            http_handle: None,
            event_broadcaster: EventBroadcaster::default(),
        }
    }

    pub async fn run(&mut self) -> Result<(), DaemonError> {
        let root_span = info_span!("daemon.run");
        let _enter = root_span.enter();
        info!("Starting daemon");
        self.pid_file.acquire()?;

        if let Err(err) = self.ipc_server.bind().await {
            if let Err(release_err) = self.pid_file.release() {
                error!(error = %release_err, "Failed to release PID file after IPC bind failure");
            }
            return Err(err.into());
        }

        if let Err(err) = self
            .event_broadcaster
            .send(NotificationEvent::DaemonStarted {
                timestamp: Utc::now(),
                version: env!("CARGO_PKG_VERSION").to_string(),
            })
        {
            tracing::debug!(error = %err, "No SSE subscribers for daemon_started event (expected at startup)");
        }

        let cancel = self.shutdown.cancel_token();

        let (signal_tx, mut signal_rx) = mpsc::channel(4);
        let signal_cancel = cancel.clone();
        let signal_span = info_span!("daemon.signals");
        self.shutdown.register_task(tokio::spawn(
            async move {
                listen_for_signals(signal_tx, signal_cancel).await;
            }
            .instrument(signal_span),
        ));

        let signal_state = Arc::clone(&self.state);
        let signal_cancel = cancel.clone();
        let handler_span = info_span!("daemon.signal_handler");
        self.shutdown.register_task(tokio::spawn(
            async move {
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
            }
            .instrument(handler_span),
        ));

        if self.state.auto_detect_active() {
            let detection_state = Arc::clone(&self.state);
            let detection_cancel = cancel.clone();
            let monitor_span = info_span!("daemon.monitor");
            self.shutdown.register_task(tokio::spawn(
                async move {
                    run_auto_detection(detection_state, detection_cancel).await;
                }
                .instrument(monitor_span),
            ));
        }

        if let Some(config) = self.state.daemon_config() {
            match HttpServer::from_config(
                &config,
                cancel.clone(),
                Arc::clone(&self.state),
                self.event_broadcaster.clone(),
            ) {
                Ok(Some(server)) => {
                    let server_cancel = cancel.clone();
                    let http_span = info_span!("daemon.http");
                    let handle = tokio::spawn(
                        async move {
                            if let Err(err) = server.start().await {
                                error!(error = %err, "HTTP server stopped with error");
                                server_cancel.cancel();
                            }
                        }
                        .instrument(http_span),
                    );
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
        let ipc_span = info_span!("daemon.ipc");
        self.shutdown.register_task(tokio::spawn(
            async move {
                let error_cancel = server_cancel.clone();
                if let Err(err) = server.run(server_state, server_cancel).await {
                    error!(error = %err, "IPC server stopped with error");
                    error_cancel.cancel();
                }
            }
            .instrument(ipc_span),
        ));

        cancel.cancelled().await;
        info!("Shutdown requested");

        // Send DaemonStopped event BEFORE shutting down HTTP server
        // so SSE clients can receive it
        if let Err(err) = self
            .event_broadcaster
            .send(NotificationEvent::DaemonStopped {
                timestamp: Utc::now(),
                reason: "shutdown".to_string(),
            })
        {
            tracing::debug!(error = %err, "No SSE subscribers to receive daemon_stopped event");
        }

        // Give SSE clients a brief moment to receive the event
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

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
