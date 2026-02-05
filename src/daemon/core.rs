use std::sync::Arc;

use tracing::{error, info, warn};

use crate::daemon::pid::{PidError, PidFile};
use crate::daemon::shutdown::{ShutdownCoordinator, ShutdownResult};
use crate::daemon::signals::wait_for_shutdown_signal;
use crate::daemon::state::DaemonState;
use crate::ipc::socket::{IpcError, IpcServer};

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
}

impl Daemon {
    pub fn new() -> Self {
        Self {
            pid_file: PidFile::new(),
            ipc_server: IpcServer::new(),
            shutdown: ShutdownCoordinator::new(),
            state: Arc::new(DaemonState::new()),
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

        let signal_cancel = cancel.clone();
        self.shutdown
            .register_task(tokio::spawn(async move {
                wait_for_shutdown_signal(signal_cancel).await;
            }));

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

        self.pid_file.release()?;
        Ok(())
    }
}

impl Default for Daemon {
    fn default() -> Self {
        Self::new()
    }
}
