#[cfg(unix)]
use tokio::signal::unix::{signal, SignalKind};
use tokio_util::sync::CancellationToken;
use tracing::{error, info};

pub async fn wait_for_shutdown_signal(cancel: CancellationToken) {
    #[cfg(unix)]
    {
        let mut sigterm = match signal(SignalKind::terminate()) {
            Ok(stream) => stream,
            Err(err) => {
                error!(error = %err, "Failed to register SIGTERM handler");
                cancel.cancel();
                return;
            }
        };
        let mut sigint = match signal(SignalKind::interrupt()) {
            Ok(stream) => stream,
            Err(err) => {
                error!(error = %err, "Failed to register SIGINT handler");
                cancel.cancel();
                return;
            }
        };
        let mut sighup = match signal(SignalKind::hangup()) {
            Ok(stream) => stream,
            Err(err) => {
                error!(error = %err, "Failed to register SIGHUP handler");
                cancel.cancel();
                return;
            }
        };

        tokio::select! {
            _ = sigterm.recv() => {
                info!("Received SIGTERM; initiating shutdown");
            }
            _ = sigint.recv() => {
                info!("Received SIGINT; initiating shutdown");
            }
            _ = sighup.recv() => {
                info!("Received SIGHUP; initiating shutdown");
            }
            _ = cancel.cancelled() => {
                info!("Shutdown already requested");
                return;
            }
        }

        cancel.cancel();
    }

    #[cfg(not(unix))]
    {
        let _ = cancel.cancelled().await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_wait_for_shutdown_signal_with_cancel() {
        let cancel = CancellationToken::new();
        let waiter = tokio::spawn(wait_for_shutdown_signal(cancel.clone()));
        cancel.cancel();
        let _ = waiter.await;
    }
}
