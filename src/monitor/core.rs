use std::fs;
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, warn};

use crate::monitor::classifier::{ClassifierConfig, ClassifierError, StopReasonClassifier};
use crate::monitor::events::{
    MonitorEvent, MonitorEventReceiver, MonitorEventSender, WatchEvent, WatchEventReceiver,
};
use crate::monitor::frontmatter::SessionParser;
use crate::monitor::process::{ProcessError, ProcessEvent, ProcessEventReceiver, ProcessMonitor};
use crate::monitor::session::Session;
use crate::monitor::watcher::{SessionWatcher, WatcherError};
use crate::telemetry::Metrics;

const DEFAULT_CHANNEL_CAPACITY: usize = 100;
const DEFAULT_HEALTH_CHECK_INTERVAL_SECS: u64 = 60;

#[derive(Debug, Clone)]
pub struct MonitorConfig {
    pub session_dir: PathBuf,
    pub channel_capacity: usize,
    pub classifier_config: ClassifierConfig,
    pub enable_process_detection: bool,
    pub health_check_interval: Duration,
}

impl Default for MonitorConfig {
    fn default() -> Self {
        let session_dir = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".opencode");
        Self {
            session_dir,
            channel_capacity: DEFAULT_CHANNEL_CAPACITY,
            classifier_config: ClassifierConfig::default(),
            enable_process_detection: true,
            health_check_interval: Duration::from_secs(DEFAULT_HEALTH_CHECK_INTERVAL_SECS),
        }
    }
}

pub struct Monitor {
    config: MonitorConfig,
    classifier: StopReasonClassifier,
    parser: SessionParser,
    current_session: Option<Session>,
    errors_count: u64,
    dropped_events: u64,
}

impl Monitor {
    pub fn new() -> Result<Self, MonitorError> {
        Self::with_config(MonitorConfig::default())
    }

    pub fn with_config(config: MonitorConfig) -> Result<Self, MonitorError> {
        let classifier = StopReasonClassifier::with_config(config.classifier_config.clone())?;
        Ok(Self {
            config,
            classifier,
            parser: SessionParser::new(),
            current_session: None,
            errors_count: 0,
            dropped_events: 0,
        })
    }

    pub async fn run(
        self,
        cancel: CancellationToken,
    ) -> Result<MonitorEventReceiver, MonitorError> {
        let watcher = SessionWatcher::with_path(self.config.session_dir.clone());
        let watcher_rx = watcher.run(cancel.clone()).await?;

        let process_rx = if self.config.enable_process_detection {
            let detector = ProcessMonitor::new();
            Some(detector.run(cancel.clone()).await?)
        } else {
            None
        };

        Ok(self
            .run_with_receivers(cancel, watcher_rx, process_rx)
            .await)
    }

    pub async fn run_with_receivers(
        mut self,
        cancel: CancellationToken,
        watcher_rx: WatchEventReceiver,
        process_rx: Option<ProcessEventReceiver>,
    ) -> MonitorEventReceiver {
        let (tx, rx) = mpsc::channel(self.config.channel_capacity);

        tokio::spawn(async move {
            self.event_loop(tx, watcher_rx, process_rx, cancel).await;
        });

        rx
    }

    async fn event_loop(
        &mut self,
        tx: MonitorEventSender,
        mut watcher_rx: WatchEventReceiver,
        mut process_rx: Option<ProcessEventReceiver>,
        cancel: CancellationToken,
    ) {
        loop {
            tokio::select! {
                _ = cancel.cancelled() => {
                    info!("Monitor shutting down");
                    break;
                }
                event = watcher_rx.recv() => {
                    match event {
                        Some(event) => self.handle_watch_event(event, &tx).await,
                        None => {
                            debug!("Watcher channel closed");
                            break;
                        }
                    }
                }
                event = async {
                    match &mut process_rx {
                        Some(rx) => rx.recv().await,
                        None => std::future::pending().await,
                    }
                } => {
                    match event {
                        Some(event) => self.handle_process_event(event, &tx).await,
                        None => {
                            debug!("Process channel closed");
                            process_rx = None;
                        }
                    }
                }
            }
        }
    }

    async fn handle_watch_event(&mut self, event: WatchEvent, tx: &MonitorEventSender) {
        if let WatchEvent::FileDeleted(path) = &event {
            if self
                .current_session
                .as_ref()
                .is_some_and(|session| session.path == *path)
            {
                self.current_session = None;
            }
        }

        if let Some(event) = self.parser.handle_event(event) {
            if let MonitorEvent::SessionChanged { session, .. } = &event {
                self.current_session = Some(session.clone());
            }

            if let MonitorEvent::Error {
                source,
                message,
                recoverable: true,
            } = &event
            {
                self.errors_count += 1;
                warn!(source, message, "Recoverable monitor error");
            }

            let _ = self.try_send(tx, event).await;
        }
    }

    async fn handle_process_event(&mut self, event: ProcessEvent, tx: &MonitorEventSender) {
        match event {
            ProcessEvent::ProcessStarted(info) => {
                let _ = self
                    .try_send(tx, MonitorEvent::ProcessStarted { info })
                    .await;
            }
            ProcessEvent::ProcessStopped { info, exit_code } => {
                let _ = self
                    .try_send(
                        tx,
                        MonitorEvent::ProcessStopped {
                            info: info.clone(),
                            exit_code,
                        },
                    )
                    .await;

                let classification = if let Some(session) = &self.current_session {
                    self.classifier.classify(&session.path, exit_code)
                } else {
                    self.classifier.classify_content("", exit_code)
                };

                if let Some(metrics) = Metrics::global() {
                    let reason = classification
                        .reason
                        .metrics_reason_label()
                        .unwrap_or("unknown");
                    if let Some(latency) = estimate_detection_latency(self.current_session.as_ref()) {
                        metrics.record_detection(latency, reason);
                    }
                }

                let _ = self
                    .try_send(
                        tx,
                        MonitorEvent::SessionStopped {
                            session: self.current_session.clone(),
                            reason: classification.reason.clone(),
                            classification,
                            process_info: Some(info),
                        },
                    )
                    .await;
            }
        }
    }

    async fn try_send(&mut self, tx: &MonitorEventSender, event: MonitorEvent) -> bool {
        match tx.try_send(event) {
            Ok(_) => true,
            Err(mpsc::error::TrySendError::Full(event)) => {
                self.dropped_events += 1;
                warn!(dropped_events = self.dropped_events, event = ?event, "Monitor event channel full, dropping event");
                false
            }
            Err(mpsc::error::TrySendError::Closed(_)) => {
                debug!("Monitor event channel closed");
                false
            }
        }
    }
}

fn estimate_detection_latency(session: Option<&Session>) -> Option<Duration> {
    let session = session?;
    let metadata = fs::metadata(&session.path).ok()?;
    let modified = metadata.modified().ok()?;
    SystemTime::now().duration_since(modified).ok()
}

impl Default for Monitor {
    fn default() -> Self {
        Self::new().expect("Failed to create default monitor")
    }
}

#[derive(Debug, thiserror::Error)]
pub enum MonitorError {
    #[error("Watcher error: {0}")]
    Watcher(#[from] WatcherError),
    #[error("Process monitor error: {0}")]
    Process(#[from] ProcessError),
    #[error("Classifier error: {0}")]
    Classifier(#[from] ClassifierError),
}
