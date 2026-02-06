use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use serde::Deserialize;
use tokio::sync::mpsc;
use tokio::time::MissedTickBehavior;
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};

use crate::config::schema::OpenCodeConfig;
use crate::monitor::process::{
    DefaultProcessEnumerator, ProcessEnumerator, ProcessError, ProcessInfo,
};

const EVENT_CHANNEL_CAPACITY: usize = 32;
const OPENCODE_PROCESS_NAME: &str = "opencode";
const OPENCODE_SERVE_ARG: &str = "serve";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenCodeProcess {
    pub pid: u32,
    pub command_line: Vec<String>,
    pub start_time: Option<SystemTime>,
    pub working_dir: Option<PathBuf>,
}

impl From<ProcessInfo> for OpenCodeProcess {
    fn from(process: ProcessInfo) -> Self {
        Self {
            pid: process.pid,
            command_line: process.command_line,
            start_time: process.start_time,
            working_dir: process.working_dir,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OpenCodeExitReason {
    NormalExit,
    Signal { signal: i32 },
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OpenCodeEvent {
    OpenCodeStarted(OpenCodeProcess),
    OpenCodeStopped {
        process: OpenCodeProcess,
        reason: OpenCodeExitReason,
    },
    OpenCodeCrashed {
        process: OpenCodeProcess,
        exit_code: i32,
    },
}

pub type OpenCodeProcessSender = mpsc::Sender<OpenCodeEvent>;
pub type OpenCodeProcessReceiver = mpsc::Receiver<OpenCodeEvent>;

#[derive(Clone)]
pub struct OpenCodeMonitor {
    poll_interval: Duration,
    health_port: u16,
    health_timeout: Duration,
    enumerator: Arc<dyn ProcessEnumerator>,
}

impl OpenCodeMonitor {
    pub fn new(config: &OpenCodeConfig) -> Self {
        Self {
            poll_interval: Duration::from_millis(config.poll_interval_ms),
            health_port: config.health_port,
            health_timeout: Duration::from_millis(config.health_timeout_ms),
            enumerator: Arc::new(DefaultProcessEnumerator),
        }
    }

    pub fn with_enumerator(mut self, enumerator: Arc<dyn ProcessEnumerator>) -> Self {
        self.enumerator = enumerator;
        self
    }

    pub async fn run(
        self,
        cancel: CancellationToken,
    ) -> Result<OpenCodeProcessReceiver, ProcessError> {
        let (tx, rx) = mpsc::channel(EVENT_CHANNEL_CAPACITY);
        let mut state = OpenCodeMonitorState::new(
            self.poll_interval,
            self.health_port,
            self.health_timeout,
            self.enumerator,
        );

        tokio::spawn(async move {
            state.run_loop(tx, cancel).await;
        });

        Ok(rx)
    }
}

struct OpenCodeMonitorState {
    poll_interval: Duration,
    health_port: u16,
    health_timeout: Duration,
    enumerator: Arc<dyn ProcessEnumerator>,
    tracked_process: Option<ProcessInfo>,
}

impl OpenCodeMonitorState {
    fn new(
        poll_interval: Duration,
        health_port: u16,
        health_timeout: Duration,
        enumerator: Arc<dyn ProcessEnumerator>,
    ) -> Self {
        Self {
            poll_interval,
            health_port,
            health_timeout,
            enumerator,
            tracked_process: None,
        }
    }

    async fn run_loop(&mut self, tx: OpenCodeProcessSender, cancel: CancellationToken) {
        if let Err(err) = self.emit_existing_process(&tx).await {
            warn!(error = %err, "Failed to enumerate existing OpenCode processes");
        }

        let mut interval = tokio::time::interval(self.poll_interval);
        interval.set_missed_tick_behavior(MissedTickBehavior::Delay);

        loop {
            tokio::select! {
                biased;
                _ = cancel.cancelled() => {
                    info!("OpenCode monitor shutting down");
                    break;
                }
                _ = interval.tick() => {
                    if let Err(err) = self.poll_once(&tx, &cancel).await {
                        warn!(error = %err, "OpenCode polling error");
                    }
                }
            }
        }
    }

    async fn emit_existing_process(
        &mut self,
        tx: &OpenCodeProcessSender,
    ) -> Result<(), ProcessError> {
        if let Some(process) = self.find_opencode_process()? {
            self.tracked_process = Some(process.clone());
            info!(pid = process.pid, "Detected existing OpenCode process");
            let _ = tx
                .send(OpenCodeEvent::OpenCodeStarted(process.into()))
                .await;
        }
        Ok(())
    }

    async fn poll_once(
        &mut self,
        tx: &OpenCodeProcessSender,
        cancel: &CancellationToken,
    ) -> Result<(), ProcessError> {
        if cancel.is_cancelled() {
            return Ok(());
        }

        let current = self.find_opencode_process()?;

        match (self.tracked_process.as_ref(), current.as_ref()) {
            (Option::None, Some(process)) => {
                let process = process.clone();
                self.tracked_process = Some(process.clone());
                info!(pid = process.pid, "OpenCode process started");
                if cancel.is_cancelled() {
                    return Ok(());
                }
                let _ = tx
                    .send(OpenCodeEvent::OpenCodeStarted(process.into()))
                    .await;
            }
            (Some(previous), Option::None) => {
                let previous = previous.clone();
                self.tracked_process = None;
                self.emit_exit_event(tx, previous, cancel).await;
            }
            (Some(previous), Some(process)) if previous.pid != process.pid => {
                let previous = previous.clone();
                self.tracked_process = None;
                self.emit_exit_event(tx, previous, cancel).await;

                let process = process.clone();
                self.tracked_process = Some(process.clone());
                info!(pid = process.pid, "OpenCode process started");
                if cancel.is_cancelled() {
                    return Ok(());
                }
                let _ = tx
                    .send(OpenCodeEvent::OpenCodeStarted(process.into()))
                    .await;
            }
            (Some(process), Some(_)) => {
                if !check_health(self.health_port, self.health_timeout).await {
                    warn!(pid = process.pid, "OpenCode health check failed");
                }
            }
            (Option::None, Option::None) => {}
        }

        Ok(())
    }

    async fn emit_exit_event(
        &self,
        tx: &OpenCodeProcessSender,
        process: ProcessInfo,
        cancel: &CancellationToken,
    ) {
        let exit_code = self.enumerator.try_get_exit_code(process.pid);
        let event = match exit_code {
            Some(0) => OpenCodeEvent::OpenCodeStopped {
                process: process.into(),
                reason: OpenCodeExitReason::NormalExit,
            },
            Some(code) => OpenCodeEvent::OpenCodeCrashed {
                process: process.into(),
                exit_code: code,
            },
            Option::None => OpenCodeEvent::OpenCodeStopped {
                process: process.into(),
                reason: OpenCodeExitReason::Unknown,
            },
        };

        if cancel.is_cancelled() {
            return;
        }

        let _ = tx.send(event).await;
    }

    fn find_opencode_process(&self) -> Result<Option<ProcessInfo>, ProcessError> {
        let processes = self.enumerator.list_opencode_processes()?;
        let mut matches: Vec<ProcessInfo> = processes
            .into_iter()
            .filter(|process| is_opencode_serve_command(&process.command_line))
            .collect();

        if matches.len() > 1 {
            warn!(
                count = matches.len(),
                "Multiple OpenCode serve processes detected; tracking the first"
            );
        }

        matches.sort_by_key(|process| process.pid);
        Ok(matches.into_iter().next())
    }
}

#[derive(Debug, Deserialize)]
struct HealthResponse {
    healthy: bool,
}

async fn check_health(health_port: u16, health_timeout: Duration) -> bool {
    let url = format!("http://localhost:{}/global/health", health_port);
    let client = match reqwest::Client::builder().timeout(health_timeout).build() {
        Ok(client) => client,
        Err(_) => return false,
    };

    let response = match client.get(url).send().await {
        Ok(response) => response,
        Err(_) => return false,
    };

    if !response.status().is_success() {
        return false;
    }

    match response.json::<HealthResponse>().await {
        Ok(payload) => payload.healthy,
        Err(_) => false,
    }
}

fn is_opencode_serve_command(command_line: &[String]) -> bool {
    if command_line.is_empty() {
        return false;
    }

    let has_serve = command_line
        .iter()
        .any(|arg| arg.eq_ignore_ascii_case(OPENCODE_SERVE_ARG));
    if !has_serve {
        return false;
    }

    command_line.iter().any(|arg| command_name_matches(arg))
}

fn command_name_matches(value: &str) -> bool {
    let name = Path::new(value)
        .file_name()
        .and_then(|os| os.to_str())
        .unwrap_or(value);
    name.contains(OPENCODE_PROCESS_NAME)
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::collections::VecDeque;
    use std::sync::Mutex;

    use tokio::time::timeout;

    #[derive(Default)]
    struct MockEnumerator {
        sequences: Mutex<VecDeque<Result<Vec<ProcessInfo>, ProcessError>>>,
        exit_codes: Mutex<std::collections::HashMap<u32, i32>>,
    }

    impl MockEnumerator {
        fn with_sequences(sequences: Vec<Result<Vec<ProcessInfo>, ProcessError>>) -> Self {
            Self {
                sequences: Mutex::new(sequences.into()),
                exit_codes: Mutex::new(std::collections::HashMap::new()),
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
            self.exit_codes
                .lock()
                .expect("lock exit codes")
                .get(&pid)
                .copied()
        }
    }

    fn opencode_process(pid: u32) -> ProcessInfo {
        ProcessInfo {
            pid,
            command_line: vec!["opencode".to_string(), "serve".to_string()],
            start_time: None,
            working_dir: None,
        }
    }

    fn config_with_poll(poll_ms: u64) -> OpenCodeConfig {
        OpenCodeConfig {
            enabled: true,
            health_port: 4096,
            poll_interval_ms: poll_ms,
            health_timeout_ms: 2000,
        }
    }

    #[tokio::test]
    async fn detects_existing_process_on_startup() {
        let enumerator = Arc::new(MockEnumerator::with_sequences(vec![Ok(vec![
            opencode_process(42),
        ])]));
        let monitor = OpenCodeMonitor::new(&config_with_poll(5)).with_enumerator(enumerator);
        let cancel = CancellationToken::new();

        let mut rx = monitor.run(cancel.clone()).await.expect("run monitor");

        let event = timeout(Duration::from_millis(50), rx.recv())
            .await
            .expect("event")
            .expect("event value");
        assert!(matches!(event, OpenCodeEvent::OpenCodeStarted(_)));

        cancel.cancel();
    }

    #[tokio::test]
    async fn emits_stopped_on_normal_exit() {
        let enumerator = Arc::new(
            MockEnumerator::with_sequences(vec![Ok(vec![opencode_process(7)]), Ok(vec![])])
                .with_exit_code(7, 0),
        );
        let monitor = OpenCodeMonitor::new(&config_with_poll(5)).with_enumerator(enumerator);
        let cancel = CancellationToken::new();

        let mut rx = monitor.run(cancel.clone()).await.expect("run monitor");

        let _ = timeout(Duration::from_millis(50), rx.recv())
            .await
            .expect("start event");

        let event = timeout(Duration::from_millis(100), rx.recv())
            .await
            .expect("stop event")
            .expect("event value");

        assert!(matches!(
            event,
            OpenCodeEvent::OpenCodeStopped {
                reason: OpenCodeExitReason::NormalExit,
                ..
            }
        ));

        cancel.cancel();
    }

    #[tokio::test]
    async fn emits_crashed_on_nonzero_exit() {
        let enumerator = Arc::new(
            MockEnumerator::with_sequences(vec![Ok(vec![opencode_process(9)]), Ok(vec![])])
                .with_exit_code(9, 2),
        );
        let monitor = OpenCodeMonitor::new(&config_with_poll(5)).with_enumerator(enumerator);
        let cancel = CancellationToken::new();

        let mut rx = monitor.run(cancel.clone()).await.expect("run monitor");

        let _ = timeout(Duration::from_millis(50), rx.recv())
            .await
            .expect("start event");

        let event = timeout(Duration::from_millis(100), rx.recv())
            .await
            .expect("crash event")
            .expect("event value");

        assert!(matches!(
            event,
            OpenCodeEvent::OpenCodeCrashed { exit_code: 2, .. }
        ));

        cancel.cancel();
    }

    #[tokio::test]
    async fn health_check_returns_true_on_healthy_response() {
        use axum::{Json, Router, routing::get};
        use std::future::IntoFuture;
        use tokio::net::TcpListener;

        #[derive(Deserialize, serde::Serialize)]
        struct Response {
            healthy: bool,
            version: String,
        }

        async fn handler() -> Json<Response> {
            Json(Response {
                healthy: true,
                version: "dev".to_string(),
            })
        }

        let app = Router::new().route("/global/health", get(handler));
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let port = listener.local_addr().expect("addr").port();
        let server = axum::serve(listener, app).into_future();
        let handle = tokio::spawn(async move {
            let _ = server.await;
        });

        let healthy = check_health(port, Duration::from_millis(200)).await;

        handle.abort();
        assert!(healthy);
    }
}
