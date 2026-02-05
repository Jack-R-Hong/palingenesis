use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
use std::time::Duration;

use notify::{Config as NotifyConfig, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use notify_debouncer_full::{
    new_debouncer, DebounceEventResult, DebouncedEvent, Debouncer, FileIdCache,
};
use tokio::sync::mpsc;
use tokio::time::MissedTickBehavior;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, warn};

use crate::monitor::events::{WatchEvent, WatchEventReceiver, WatchEventSender};

const DEFAULT_SESSION_DIR: &str = ".opencode";
const DEFAULT_DEBOUNCE_MS: u64 = 100;
const WATCH_RETRY_ATTEMPTS: usize = 3;
const WATCH_RETRY_DELAY_MS: u64 = 200;

#[derive(Debug, thiserror::Error)]
pub enum WatcherError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Notify error: {0}")]
    Notify(#[from] notify::Error),

    #[error("Session directory not found: {path}")]
    DirectoryNotFound { path: PathBuf },

    #[error("Watcher already running")]
    AlreadyRunning,
}

/// Access to watcher configuration from daemon state.
pub trait WatcherStateAccess: Send + Sync {
    fn session_dir(&self) -> PathBuf;
    fn debounce_duration(&self) -> Duration;
}

pub struct SessionWatcher {
    session_dir: PathBuf,
    debounce: Duration,
    running: Arc<AtomicBool>,
}

impl SessionWatcher {
    /// Create a new SessionWatcher with the default session directory (~/.opencode/).
    pub fn new() -> Self {
        let session_dir = default_session_dir();
        Self {
            session_dir,
            debounce: Duration::from_millis(DEFAULT_DEBOUNCE_MS),
            running: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Create with custom session directory (for testing).
    pub fn with_path(path: PathBuf) -> Self {
        Self {
            session_dir: path,
            debounce: Duration::from_millis(DEFAULT_DEBOUNCE_MS),
            running: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Create from a state accessor that provides watcher configuration.
    pub fn from_state<S: WatcherStateAccess>(state: &S) -> Self {
        Self {
            session_dir: state.session_dir(),
            debounce: state.debounce_duration(),
            running: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Set custom debounce duration.
    pub fn with_debounce(mut self, duration: Duration) -> Self {
        self.debounce = duration;
        self
    }

    /// Returns the session directory path being watched.
    pub fn session_dir(&self) -> &Path {
        &self.session_dir
    }

    /// Run the file watcher, returning a receiver for watch events.
    pub async fn run(
        &self,
        cancel: CancellationToken,
    ) -> Result<WatchEventReceiver, WatcherError> {
        if self.running.swap(true, Ordering::SeqCst) {
            return Err(WatcherError::AlreadyRunning);
        }

        let (tx, rx) = mpsc::channel(100);
        let session_dir = self.session_dir.clone();
        let debounce = self.debounce;
        let running = Arc::clone(&self.running);

        tokio::spawn(async move {
            let _guard = RunningGuard::new(running);
            if let Err(err) = run_watcher_task(session_dir, debounce, tx, cancel).await {
                error!(error = %err, "Watcher task failed");
            }
        });

        Ok(rx)
    }
}

impl Default for SessionWatcher {
    fn default() -> Self {
        Self::new()
    }
}

struct RunningGuard {
    running: Arc<AtomicBool>,
}

impl RunningGuard {
    fn new(running: Arc<AtomicBool>) -> Self {
        Self { running }
    }
}

impl Drop for RunningGuard {
    fn drop(&mut self) {
        self.running.store(false, Ordering::SeqCst);
    }
}

fn default_session_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(DEFAULT_SESSION_DIR)
}

async fn run_watcher_task(
    session_dir: PathBuf,
    debounce: Duration,
    tx: WatchEventSender,
    cancel: CancellationToken,
) -> Result<(), WatcherError> {
    if !session_dir.exists() {
        warn!(path = %session_dir.display(), "Session directory does not exist, waiting for creation");
        wait_for_directory_creation(&session_dir, &tx, cancel.clone()).await?;
    }

    start_watching(session_dir, debounce, tx, cancel).await
}

async fn wait_for_directory_creation(
    session_dir: &Path,
    tx: &WatchEventSender,
    cancel: CancellationToken,
) -> Result<(), WatcherError> {
    let parent = session_dir
        .parent()
        .ok_or_else(|| WatcherError::DirectoryNotFound {
            path: session_dir.to_path_buf(),
        })?;

    if session_dir.exists() {
        let _ = tx
            .send(WatchEvent::DirectoryCreated(session_dir.to_path_buf()))
            .await;
        return Ok(());
    }

    let (notify_tx, mut notify_rx) = mpsc::channel(32);
    let mut watcher = RecommendedWatcher::new(
        move |res: Result<Event, notify::Error>| {
            let _ = notify_tx.blocking_send(res);
        },
        NotifyConfig::default(),
    )?;

    watch_with_retry(&mut watcher, parent, RecursiveMode::NonRecursive).await?;
    info!(path = %parent.display(), "Watching for session directory creation");

    loop {
        if session_dir.exists() {
            let _ = tx
                .send(WatchEvent::DirectoryCreated(session_dir.to_path_buf()))
                .await;
            break;
        }

        tokio::select! {
            _ = cancel.cancelled() => {
                info!("Session directory creation watcher cancelled");
                break;
            }
            Some(result) = notify_rx.recv() => {
                match result {
                    Ok(event) => {
                        if is_core_event(&event.kind)
                            && event.paths.iter().any(|path| path == session_dir)
                            && event.kind.is_create()
                        {
                            let _ = tx
                                .send(WatchEvent::DirectoryCreated(session_dir.to_path_buf()))
                                .await;
                            break;
                        }
                    }
                    Err(err) => {
                        warn!(error = %err, "Directory watcher error");
                    }
                }
            }
        }
    }

    Ok(())
}

async fn start_watching(
    session_dir: PathBuf,
    debounce: Duration,
    tx: WatchEventSender,
    cancel: CancellationToken,
) -> Result<(), WatcherError> {
    let (debounce_tx, mut debounce_rx) = mpsc::channel(128);
    let mut debouncer = new_debouncer(debounce, None, move |result: DebounceEventResult| {
        let _ = debounce_tx.blocking_send(result);
    })?;

    watch_debouncer_with_retry(&mut debouncer, &session_dir, RecursiveMode::Recursive).await?;
    info!(path = %session_dir.display(), "Started watching session directory");

    let mut debounce_buffer: HashMap<PathBuf, EventKind> = HashMap::new();
    let mut interval = tokio::time::interval(debounce);
    interval.set_missed_tick_behavior(MissedTickBehavior::Delay);

    loop {
        tokio::select! {
            _ = cancel.cancelled() => {
                info!("File watcher shutting down");
                flush_buffer(&mut debounce_buffer, &tx).await;
                break;
            }
            Some(result) = debounce_rx.recv() => {
                handle_debounce_result(result, &mut debounce_buffer, &tx).await;
            }
            _ = interval.tick() => {
                flush_buffer(&mut debounce_buffer, &tx).await;
            }
        }
    }

    Ok(())
}

async fn handle_debounce_result(
    result: DebounceEventResult,
    buffer: &mut HashMap<PathBuf, EventKind>,
    tx: &WatchEventSender,
) {
    match result {
        Ok(events) => {
            for event in events {
                buffer_event(buffer, &event);
            }
        }
        Err(errors) => {
            for err in errors {
                warn!(error = %err, "File watcher error");
                let _ = tx.send(WatchEvent::Error(err.to_string())).await;
            }
        }
    }
}

fn buffer_event(buffer: &mut HashMap<PathBuf, EventKind>, event: &DebouncedEvent) {
    if !is_core_event(&event.kind) {
        return;
    }

    for path in &event.paths {
        buffer.insert(path.clone(), event.kind);
    }
}

async fn flush_buffer(buffer: &mut HashMap<PathBuf, EventKind>, tx: &WatchEventSender) {
    if buffer.is_empty() {
        return;
    }

    for (path, kind) in buffer.drain() {
        if let Some(event) = map_event(kind, path) {
            if tx.send(event).await.is_err() {
                debug!("Watcher event receiver dropped");
                break;
            }
        }
    }
}

fn map_event(kind: EventKind, path: PathBuf) -> Option<WatchEvent> {
    if !is_core_event(&kind) {
        return None;
    }

    match kind {
        EventKind::Create(create_kind) => match create_kind {
            notify::event::CreateKind::Folder => Some(WatchEvent::DirectoryCreated(path)),
            _ => Some(WatchEvent::FileCreated(path)),
        },
        EventKind::Modify(_) => Some(WatchEvent::FileModified(path)),
        EventKind::Remove(_) => Some(WatchEvent::FileDeleted(path)),
        _ => None,
    }
}

fn is_core_event(kind: &EventKind) -> bool {
    // Equivalent to filtering with EventKindMask::CORE (exclude access/open/close noise).
    matches!(kind, EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_))
}

async fn watch_with_retry<W: Watcher>(
    watcher: &mut W,
    path: &Path,
    mode: RecursiveMode,
) -> Result<(), WatcherError> {
    let mut last_error: Option<notify::Error> = None;
    for attempt in 0..=WATCH_RETRY_ATTEMPTS {
        match watcher.watch(path, mode) {
            Ok(()) => return Ok(()),
            Err(err) => {
                last_error = Some(err);
                if attempt < WATCH_RETRY_ATTEMPTS {
                    warn!(
                        attempt = attempt + 1,
                        error = %last_error.as_ref().expect("error set"),
                        "Watch setup failed; retrying"
                    );
                    tokio::time::sleep(Duration::from_millis(WATCH_RETRY_DELAY_MS)).await;
                }
            }
        }
    }

    Err(last_error.expect("watch setup failed").into())
}

async fn watch_debouncer_with_retry<T: Watcher, C: FileIdCache>(
    debouncer: &mut Debouncer<T, C>,
    path: &Path,
    mode: RecursiveMode,
) -> Result<(), WatcherError> {
    let mut last_error: Option<notify::Error> = None;
    for attempt in 0..=WATCH_RETRY_ATTEMPTS {
        match debouncer.watch(path, mode) {
            Ok(()) => return Ok(()),
            Err(err) => {
                last_error = Some(err);
                if attempt < WATCH_RETRY_ATTEMPTS {
                    warn!(
                        attempt = attempt + 1,
                        error = %last_error.as_ref().expect("error set"),
                        "Debouncer watch setup failed; retrying"
                    );
                    tokio::time::sleep(Duration::from_millis(WATCH_RETRY_DELAY_MS)).await;
                }
            }
        }
    }

    Err(last_error.expect("watch setup failed").into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_map_event_skips_access_events() {
        let path = PathBuf::from("/tmp/file.txt");
        assert!(map_event(EventKind::Access(notify::event::AccessKind::Any), path).is_none());
    }

    #[test]
    fn test_map_event_creates_expected_variants() {
        let created = map_event(
            EventKind::Create(notify::event::CreateKind::File),
            PathBuf::from("/tmp/file.txt"),
        );
        assert!(matches!(created, Some(WatchEvent::FileCreated(_))));

        let modified = map_event(
            EventKind::Modify(notify::event::ModifyKind::Any),
            PathBuf::from("/tmp/file.txt"),
        );
        assert!(matches!(modified, Some(WatchEvent::FileModified(_))));

        let removed = map_event(
            EventKind::Remove(notify::event::RemoveKind::Any),
            PathBuf::from("/tmp/file.txt"),
        );
        assert!(matches!(removed, Some(WatchEvent::FileDeleted(_))));
    }

    #[test]
    fn test_buffer_event_tracks_latest_kind() {
        let mut buffer = HashMap::new();
        let event = DebouncedEvent::new(
            Event::new(EventKind::Modify(notify::event::ModifyKind::Any))
                .add_path(PathBuf::from("/tmp/file.txt")),
            std::time::Instant::now(),
        );

        buffer_event(&mut buffer, &event);
        assert_eq!(buffer.len(), 1);
        assert!(matches!(
            buffer.values().next(),
            Some(EventKind::Modify(_))
        ));
    }
}
