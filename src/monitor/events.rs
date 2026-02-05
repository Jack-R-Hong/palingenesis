use std::path::PathBuf;

use tokio::sync::mpsc;

use crate::monitor::classifier::{ClassificationResult, StopReason};
use crate::monitor::process::ProcessInfo;
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
#[derive(Debug, Clone, PartialEq)]
pub enum MonitorEvent {
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
    /// Session stopped with a classified reason.
    SessionStopped {
        session: Option<Session>,
        reason: StopReason,
        classification: ClassificationResult,
        process_info: Option<ProcessInfo>,
    },
    /// Monitor encountered an error.
    Error {
        source: String,
        message: String,
        recoverable: bool,
    },
}

pub type WatchEventSender = mpsc::Sender<WatchEvent>;
pub type WatchEventReceiver = mpsc::Receiver<WatchEvent>;
pub type MonitorEventSender = mpsc::Sender<MonitorEvent>;
pub type MonitorEventReceiver = mpsc::Receiver<MonitorEvent>;
