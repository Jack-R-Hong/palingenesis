use std::path::PathBuf;

use tokio::sync::mpsc;

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
