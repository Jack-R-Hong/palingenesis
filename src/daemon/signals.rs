#[cfg(unix)]
use tokio::signal::unix::{signal, SignalKind};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::{error, info};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DaemonSignal {
    Shutdown,
    Reload,
}

pub async fn listen_for_signals(tx: mpsc::Sender<DaemonSignal>, cancel: CancellationToken) {
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

        loop {
            tokio::select! {
                _ = sigterm.recv() => {
                    info!("Received SIGTERM; initiating shutdown");
                    let _ = tx.send(DaemonSignal::Shutdown).await;
                    cancel.cancel();
                    break;
                }
                _ = sigint.recv() => {
                    info!("Received SIGINT; initiating shutdown");
                    let _ = tx.send(DaemonSignal::Shutdown).await;
                    cancel.cancel();
                    break;
                }
                _ = sighup.recv() => {
                    info!("Received SIGHUP; reloading configuration");
                    let _ = tx.send(DaemonSignal::Reload).await;
                }
                _ = cancel.cancelled() => {
                    info!("Shutdown already requested");
                    break;
                }
            }
        }
    }

    #[cfg(not(unix))]
    {
        let _ = tx;
        let _ = cancel.cancelled().await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(unix)]
    use nix::sys::signal::{kill, Signal};
    #[cfg(unix)]
    use nix::unistd::Pid;
    use tokio::sync::mpsc;
    #[cfg(unix)]
    use tokio::time::{sleep, timeout, Duration};

    #[tokio::test]
    async fn test_listen_for_signals_with_cancel() {
        let cancel = CancellationToken::new();
        let (tx, mut rx) = mpsc::channel(1);
        let waiter = tokio::spawn(listen_for_signals(tx, cancel.clone()));
        cancel.cancel();
        let _ = waiter.await;
        assert!(rx.try_recv().is_err());
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn test_listen_for_signals_receives_sighup() {
        let cancel = CancellationToken::new();
        let (tx, mut rx) = mpsc::channel(1);
        let waiter = tokio::spawn(listen_for_signals(tx, cancel.clone()));

        sleep(Duration::from_millis(50)).await;
        let pid = Pid::from_raw(std::process::id() as i32);
        kill(pid, Signal::SIGHUP).unwrap();

        let signal = timeout(Duration::from_secs(1), rx.recv()).await.unwrap();
        assert_eq!(signal, Some(DaemonSignal::Reload));

        cancel.cancel();
        let _ = waiter.await;
    }
}
