use std::path::PathBuf;

use tokio::sync::mpsc;

use crate::monitor::process::{ProcessEvent, ProcessInfo};
use crate::monitor::session::Session;

/// Events emitted by the file system watcher.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WatchEvent {
    /// File was created in the session directory.
    FileCreated(PathBuf),
    /// File was modified in the session directory.
    FileModified(PathBuf),
    /// File was deleted from the session directory.
    FileDeleted(PathBuf),
    /// Session directory was created.
    DirectoryCreated(PathBuf),
    /// Watcher encountered an error.
    Error(String),
}

/// Events emitted by the monitor after parsing session state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MonitorEvent {
    /// File was created in the session directory.
    FileCreated(PathBuf),
    /// File was modified in the session directory.
    FileModified(PathBuf),
    /// File was deleted from the session directory.
    FileDeleted(PathBuf),
    /// Session directory was created.
    DirectoryCreated(PathBuf),
    /// Session state changed (parsed from frontmatter).
    SessionChanged {
        session: Session,
        previous: Option<Session>,
    },
    /// An opencode process started.
    ProcessStarted { info: ProcessInfo },
    /// An opencode process stopped.
    ProcessStopped {
        info: ProcessInfo,
        exit_code: Option<i32>,
    },
    /// Watcher or parser encountered an error.
    Error(String),
}

#[derive(Debug, thiserror::Error)]
pub enum WatchError {
    #[error("Watcher channel closed")]
    ChannelClosed,

    #[error("Notify error: {0}")]
    Notify(#[from] notify::Error),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

pub type WatchEventSender = mpsc::Sender<WatchEvent>;
pub type WatchEventReceiver = mpsc::Receiver<WatchEvent>;
pub type MonitorEventSender = mpsc::Sender<MonitorEvent>;
pub type MonitorEventReceiver = mpsc::Receiver<MonitorEvent>;

impl From<WatchEvent> for MonitorEvent {
    fn from(event: WatchEvent) -> Self {
        match event {
            WatchEvent::FileCreated(path) => MonitorEvent::FileCreated(path),
            WatchEvent::FileModified(path) => MonitorEvent::FileModified(path),
            WatchEvent::FileDeleted(path) => MonitorEvent::FileDeleted(path),
            WatchEvent::DirectoryCreated(path) => MonitorEvent::DirectoryCreated(path),
            WatchEvent::Error(message) => MonitorEvent::Error(message),
        }
    }
}

impl From<ProcessEvent> for MonitorEvent {
    fn from(event: ProcessEvent) -> Self {
        match event {
            ProcessEvent::ProcessStarted(info) => MonitorEvent::ProcessStarted { info },
            ProcessEvent::ProcessStopped { info, exit_code } => {
                MonitorEvent::ProcessStopped { info, exit_code }
            }
        }
    }
}
