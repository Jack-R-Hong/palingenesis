use std::time::Duration;

use tokio_util::sync::CancellationToken;
use tracing::{info, warn};

pub const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(10);

pub struct ShutdownCoordinator {
    cancel: CancellationToken,
    tasks: Vec<tokio::task::JoinHandle<()>>,
}

impl ShutdownCoordinator {
    pub fn new() -> Self {
        Self {
            cancel: CancellationToken::new(),
            tasks: Vec::new(),
        }
    }

    pub fn cancel_token(&self) -> CancellationToken {
        self.cancel.clone()
    }

    pub fn register_task(&mut self, handle: tokio::task::JoinHandle<()>) {
        self.tasks.push(handle);
    }

    pub async fn shutdown(self) -> ShutdownResult {
        let task_count = self.tasks.len();
        info!(tasks = task_count, "Shutdown initiated; notifying tasks");
        self.cancel.cancel();

        let mut handles = self.tasks;
        let wait_result = tokio::time::timeout(SHUTDOWN_TIMEOUT, async {
            for handle in handles.iter_mut() {
                let _ = handle.await;
            }
        })
        .await;

        match wait_result {
            Ok(()) => {
                info!("All tasks stopped gracefully");
                ShutdownResult::Graceful
            }
            Err(_) => {
                let hung_tasks = handles
                    .iter()
                    .filter(|handle| !handle.is_finished())
                    .count();
                warn!(hung_tasks, "Shutdown timed out; aborting remaining tasks");
                for handle in handles {
                    if !handle.is_finished() {
                        handle.abort();
                    }
                }
                ShutdownResult::TimedOut { hung_tasks }
            }
        }
    }
}

impl Default for ShutdownCoordinator {
    fn default() -> Self {
        Self::new()
    }
}

pub enum ShutdownResult {
    Graceful,
    TimedOut { hung_tasks: usize },
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[tokio::test]
    async fn test_shutdown_graceful_completes_work() {
        let mut coordinator = ShutdownCoordinator::new();
        let cancel = coordinator.cancel_token();
        let progress = Arc::new(AtomicUsize::new(0));
        let task_progress = Arc::clone(&progress);

        coordinator.register_task(tokio::spawn(async move {
            loop {
                task_progress.fetch_add(1, Ordering::SeqCst);
                tokio::time::sleep(Duration::from_millis(5)).await;
                if cancel.is_cancelled() {
                    break;
                }
            }
        }));

        let result = coordinator.shutdown().await;
        assert!(matches!(result, ShutdownResult::Graceful));
        assert!(progress.load(Ordering::SeqCst) > 0);
    }

    #[tokio::test(start_paused = true)]
    async fn test_shutdown_timeout_aborts_tasks() {
        let mut coordinator = ShutdownCoordinator::new();

        coordinator.register_task(tokio::spawn(async move {
            tokio::time::sleep(Duration::from_secs(3600)).await;
        }));

        let shutdown_task = tokio::spawn(async move { coordinator.shutdown().await });
        tokio::time::advance(SHUTDOWN_TIMEOUT + Duration::from_secs(1)).await;

        let result = shutdown_task.await.unwrap();
        assert!(matches!(result, ShutdownResult::TimedOut { hung_tasks: 1 }));
    }
}
