use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use tokio::sync::mpsc;
use tokio::time::MissedTickBehavior;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, warn};

use crate::monitor::events::{MonitorEvent, MonitorEventReceiver, MonitorEventSender};

const DEFAULT_POLL_INTERVAL_MS: u64 = 1000;
const OPENCODE_PROCESS_NAME: &str = "opencode";
const EVENT_CHANNEL_CAPACITY: usize = 100;

/// Information about a tracked process.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessInfo {
    /// Process ID.
    pub pid: u32,
    /// Full command line.
    pub command_line: Vec<String>,
    /// Process start time (if available).
    pub start_time: Option<SystemTime>,
    /// Working directory (if available).
    pub working_dir: Option<PathBuf>,
}

/// Events emitted by the process monitor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProcessEvent {
    /// An opencode process started.
    ProcessStarted(ProcessInfo),
    /// An opencode process stopped.
    ProcessStopped {
        info: ProcessInfo,
        exit_code: Option<i32>,
    },
}

#[derive(Debug, thiserror::Error)]
pub enum ProcessError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Process enumeration failed: {0}")]
    EnumerationFailed(String),

    #[error("Permission denied reading process info")]
    PermissionDenied,
}

/// Access to process monitoring configuration from daemon state.
pub trait ProcessStateAccess: Send + Sync {
    fn process_poll_interval(&self) -> Duration;
}

pub trait ProcessEnumerator: Send + Sync {
    fn list_opencode_processes(&self) -> Result<Vec<ProcessInfo>, ProcessError>;
    fn try_get_exit_code(&self, _pid: u32) -> Option<i32> {
        None
    }
}

#[derive(Clone)]
pub struct ProcessMonitor {
    poll_interval: Duration,
    enumerator: Arc<dyn ProcessEnumerator>,
}

impl ProcessMonitor {
    /// Create a new ProcessMonitor with default poll interval.
    pub fn new() -> Self {
        Self {
            poll_interval: Duration::from_millis(DEFAULT_POLL_INTERVAL_MS),
            enumerator: Arc::new(DefaultProcessEnumerator),
        }
    }

    /// Create with custom poll interval.
    pub fn with_poll_interval(mut self, interval: Duration) -> Self {
        self.poll_interval = interval;
        self
    }

    /// Create with custom process enumerator (for testing).
    pub fn with_enumerator(mut self, enumerator: Arc<dyn ProcessEnumerator>) -> Self {
        self.enumerator = enumerator;
        self
    }

    /// Create from a state accessor that provides monitor configuration.
    pub fn from_state<S: ProcessStateAccess>(state: &S) -> Self {
        Self {
            poll_interval: state.process_poll_interval(),
            enumerator: Arc::new(DefaultProcessEnumerator),
        }
    }

    /// Run the process monitor, returning a receiver for monitor events.
    pub async fn run(self, cancel: CancellationToken) -> Result<MonitorEventReceiver, ProcessError> {
        let (tx, rx) = mpsc::channel(EVENT_CHANNEL_CAPACITY);
        let mut state = ProcessMonitorState::new(self.poll_interval, self.enumerator);

        tokio::spawn(async move {
            state.run_loop(tx, cancel).await;
        });

        Ok(rx)
    }
}

impl Default for ProcessMonitor {
    fn default() -> Self {
        Self::new()
    }
}

struct ProcessMonitorState {
    poll_interval: Duration,
    enumerator: Arc<dyn ProcessEnumerator>,
    tracked_processes: HashMap<u32, ProcessInfo>,
}

impl ProcessMonitorState {
    fn new(poll_interval: Duration, enumerator: Arc<dyn ProcessEnumerator>) -> Self {
        Self {
            poll_interval,
            enumerator,
            tracked_processes: HashMap::new(),
        }
    }

    async fn run_loop(&mut self, tx: MonitorEventSender, cancel: CancellationToken) {
        if let Err(err) = self.emit_existing_processes(&tx).await {
            warn!(error = %err, "Failed to enumerate existing processes");
        }

        let mut interval = tokio::time::interval(self.poll_interval);
        interval.set_missed_tick_behavior(MissedTickBehavior::Delay);

        loop {
            tokio::select! {
                biased;
                _ = cancel.cancelled() => {
                    info!("Process monitor shutting down");
                    break;
                }
                _ = interval.tick() => {
                    if let Err(err) = self.poll_once(&tx, &cancel).await {
                        warn!(error = %err, "Process polling error");
                    }
                }
            }
        }
    }

    async fn emit_existing_processes(&mut self, tx: &MonitorEventSender) -> Result<(), ProcessError> {
        let initial = self.enumerator.list_opencode_processes()?;
        for process in initial {
            self.tracked_processes.insert(process.pid, process.clone());
            info!(pid = process.pid, "Detected existing opencode process");
            let event = MonitorEvent::from(ProcessEvent::ProcessStarted(process));
            let _ = tx.send(event).await;
        }
        Ok(())
    }

    async fn poll_once(
        &mut self,
        tx: &MonitorEventSender,
        cancel: &CancellationToken,
    ) -> Result<(), ProcessError> {
        if cancel.is_cancelled() {
            return Ok(());
        }

        let current = self.enumerator.list_opencode_processes()?;
        let current_pids: HashSet<u32> = current.iter().map(|process| process.pid).collect();

        for process in &current {
            if !self.tracked_processes.contains_key(&process.pid) {
                info!(pid = process.pid, "New opencode process detected");
                self.tracked_processes
                    .insert(process.pid, process.clone());
                if cancel.is_cancelled() {
                    return Ok(());
                }
                let event = MonitorEvent::from(ProcessEvent::ProcessStarted(process.clone()));
                let _ = tx.send(event).await;
            }
        }

        let stopped: Vec<u32> = self
            .tracked_processes
            .keys()
            .filter(|pid| !current_pids.contains(pid))
            .copied()
            .collect();

        for pid in stopped {
            if let Some(info) = self.tracked_processes.remove(&pid) {
                let exit_code = self.enumerator.try_get_exit_code(pid);
                info!(pid = info.pid, "opencode process stopped");
                if cancel.is_cancelled() {
                    return Ok(());
                }
                let event = MonitorEvent::from(ProcessEvent::ProcessStopped { info, exit_code });
                let _ = tx.send(event).await;
            }
        }

        Ok(())
    }
}

struct DefaultProcessEnumerator;

impl ProcessEnumerator for DefaultProcessEnumerator {
    fn list_opencode_processes(&self) -> Result<Vec<ProcessInfo>, ProcessError> {
        enumerate_opencode_processes()
    }

    fn try_get_exit_code(&self, pid: u32) -> Option<i32> {
        try_get_exit_code(pid)
    }
}

#[cfg(target_os = "linux")]
fn enumerate_opencode_processes() -> Result<Vec<ProcessInfo>, ProcessError> {
    use std::fs;

    let mut processes = Vec::new();

    for entry in fs::read_dir("/proc")? {
        let entry = match entry {
            Ok(entry) => entry,
            Err(err) => {
                warn!(error = %err, "Failed to read /proc entry");
                continue;
            }
        };

        let path = entry.path();
        let pid = match path.file_name().and_then(|name| name.to_str()) {
            Some(name) => match name.parse::<u32>() {
                Ok(pid) => pid,
                Err(_) => continue,
            },
            None => continue,
        };

        let cmdline_path = path.join("cmdline");
        let cmdline_bytes = match fs::read(&cmdline_path) {
            Ok(bytes) => bytes,
            Err(err) => {
                if err.kind() != std::io::ErrorKind::PermissionDenied {
                    debug!(pid, error = %err, "Failed to read cmdline");
                }
                continue;
            }
        };

        let command_line = parse_cmdline(&cmdline_bytes);
        if !is_opencode_command(&command_line) {
            continue;
        }

        let working_dir = fs::read_link(path.join("cwd")).ok();

        processes.push(ProcessInfo {
            pid,
            command_line,
            start_time: None,
            working_dir,
        });
    }

    Ok(processes)
}

#[cfg(not(target_os = "linux"))]
fn enumerate_opencode_processes() -> Result<Vec<ProcessInfo>, ProcessError> {
    Err(ProcessError::EnumerationFailed(
        "process enumeration not supported on this platform".to_string(),
    ))
}

#[cfg(target_os = "linux")]
fn try_get_exit_code(pid: u32) -> Option<i32> {
    use std::fs;

    let stat_path = Path::new("/proc").join(pid.to_string()).join("stat");
    let stat = fs::read_to_string(stat_path).ok()?;
    parse_exit_code_from_stat(&stat)
}

#[cfg(not(target_os = "linux"))]
fn try_get_exit_code(_pid: u32) -> Option<i32> {
    None
}

fn parse_cmdline(bytes: &[u8]) -> Vec<String> {
    let mut args = Vec::new();
    let mut start = 0;
    for (idx, byte) in bytes.iter().enumerate() {
        if *byte == 0 {
            if start < idx {
                let slice = &bytes[start..idx];
                let arg = String::from_utf8_lossy(slice).to_string();
                if !arg.is_empty() {
                    args.push(arg);
                }
            }
            start = idx + 1;
        }
    }

    if start < bytes.len() {
        let slice = &bytes[start..];
        let arg = String::from_utf8_lossy(slice).to_string();
        if !arg.is_empty() {
            args.push(arg);
        }
    }

    args
}

fn is_opencode_command(args: &[String]) -> bool {
    if args.is_empty() {
        return false;
    }

    let first = args.first().map(String::as_str).unwrap_or_default();
    if command_name_matches(first) {
        return true;
    }

    args.iter().any(|arg| command_name_matches(arg))
}

fn command_name_matches(value: &str) -> bool {
    let name = Path::new(value)
        .file_name()
        .and_then(|os| os.to_str())
        .unwrap_or(value);
    name.contains(OPENCODE_PROCESS_NAME)
}

#[cfg(target_os = "linux")]
fn parse_exit_code_from_stat(stat: &str) -> Option<i32> {
    let close_paren = stat.rfind(')')?;
    let rest = stat.get(close_paren + 1..)?.trim();
    let fields: Vec<&str> = rest.split_whitespace().collect();
    if fields.len() <= 49 {
        return None;
    }

    fields[49].parse::<i32>().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::collections::VecDeque;
    use std::sync::{Arc, Mutex};

    use tokio::time::{Duration, timeout};

    #[derive(Default)]
    struct MockEnumerator {
        sequences: Mutex<VecDeque<Result<Vec<ProcessInfo>, ProcessError>>>,
        exit_codes: Mutex<HashMap<u32, i32>>, 
    }

    impl MockEnumerator {
        fn with_sequences(sequences: Vec<Result<Vec<ProcessInfo>, ProcessError>>) -> Self {
            Self {
                sequences: Mutex::new(sequences.into()),
                exit_codes: Mutex::new(HashMap::new()),
            }
        }

        fn with_exit_code(self, pid: u32, code: i32) -> Self {
            self.exit_codes
                .lock()
                .expect("lock exit codes")
                .insert(pid, code);
            self
        }
    }

    impl ProcessEnumerator for MockEnumerator {
        fn list_opencode_processes(&self) -> Result<Vec<ProcessInfo>, ProcessError> {
            let mut sequences = self.sequences.lock().expect("lock sequences");
            sequences.pop_front().unwrap_or_else(|| Ok(Vec::new()))
        }

        fn try_get_exit_code(&self, pid: u32) -> Option<i32> {
            self.exit_codes.lock().expect("lock exit codes").get(&pid).copied()
        }
    }

    fn process(pid: u32, command: &str) -> ProcessInfo {
        ProcessInfo {
            pid,
            command_line: vec![command.to_string()],
            start_time: None,
            working_dir: None,
        }
    }

    #[tokio::test]
    async fn detects_existing_processes_on_startup() {
        let enumerator = Arc::new(MockEnumerator::with_sequences(vec![Ok(vec![
            process(100, "opencode"),
            process(200, "opencode"),
        ])]));
        let monitor = ProcessMonitor::new()
            .with_poll_interval(Duration::from_millis(5))
            .with_enumerator(enumerator);
        let cancel = CancellationToken::new();

        let mut rx = monitor.run(cancel.clone()).await.expect("run monitor");

        let event = timeout(Duration::from_millis(50), rx.recv())
            .await
            .expect("event")
            .expect("event value");
        assert!(matches!(event, MonitorEvent::ProcessStarted { .. }));

        let event = timeout(Duration::from_millis(50), rx.recv())
            .await
            .expect("event")
            .expect("event value");
        assert!(matches!(event, MonitorEvent::ProcessStarted { .. }));

        cancel.cancel();
    }

    #[tokio::test]
    async fn detects_stopped_processes_individually() {
        let enumerator = Arc::new(
            MockEnumerator::with_sequences(vec![
                Ok(vec![process(1, "opencode"), process(2, "opencode")]),
                Ok(vec![process(1, "opencode")]),
            ])
            .with_exit_code(2, 0),
        );
        let monitor = ProcessMonitor::new()
            .with_poll_interval(Duration::from_millis(5))
            .with_enumerator(enumerator);
        let cancel = CancellationToken::new();

        let mut rx = monitor.run(cancel.clone()).await.expect("run monitor");

        let _ = timeout(Duration::from_millis(50), rx.recv()).await.expect("start event");
        let _ = timeout(Duration::from_millis(50), rx.recv()).await.expect("start event");

        let event = timeout(Duration::from_millis(100), rx.recv())
            .await
            .expect("stop event")
            .expect("event value");

        assert!(matches!(
            event,
            MonitorEvent::ProcessStopped {
                exit_code: Some(0),
                ..
            }
        ));

        cancel.cancel();
    }

    #[tokio::test]
    async fn continues_after_enumeration_error() {
        let enumerator = Arc::new(MockEnumerator::with_sequences(vec![
            Err(ProcessError::EnumerationFailed("boom".to_string())),
            Ok(vec![process(3, "opencode")]),
        ]));

        let monitor = ProcessMonitor::new()
            .with_poll_interval(Duration::from_millis(5))
            .with_enumerator(enumerator);
        let cancel = CancellationToken::new();

        let mut rx = monitor.run(cancel.clone()).await.expect("run monitor");

        let event = timeout(Duration::from_millis(100), rx.recv())
            .await
            .expect("event")
            .expect("event value");
        assert!(matches!(event, MonitorEvent::ProcessStarted { .. }));

        cancel.cancel();
    }

    #[tokio::test]
    async fn stops_emitting_after_cancellation() {
        let enumerator = Arc::new(MockEnumerator::with_sequences(vec![
            Ok(vec![process(10, "opencode")]),
            Ok(vec![process(10, "opencode")]),
        ]));
        let monitor = ProcessMonitor::new()
            .with_poll_interval(Duration::from_millis(5))
            .with_enumerator(enumerator);
        let cancel = CancellationToken::new();

        let mut rx = monitor.run(cancel.clone()).await.expect("run monitor");
        let _ = timeout(Duration::from_millis(50), rx.recv()).await.expect("start event");

        cancel.cancel();

        let mut closed = false;
        let start = std::time::Instant::now();
        while start.elapsed() < Duration::from_millis(100) {
            match timeout(Duration::from_millis(20), rx.recv()).await {
                Ok(Some(_)) => continue,
                Ok(None) => {
                    closed = true;
                    break;
                }
                Err(_) => continue,
            }
        }

        assert!(closed);
    }
}
