# Story 2.7: Monitor Event Channel

Status: ready-for-dev

## Story

As a monitor,
I want to emit events via a channel to the daemon core,
So that monitoring is decoupled from resume logic.

## Acceptance Criteria

**AC1: Session Changed Event Emission**
**Given** the monitor detects a session file change
**When** it processes the change
**Then** it sends a `MonitorEvent::SessionChanged` to the channel

**AC2: Session Stopped Event Emission**
**Given** the monitor classifies a stop reason
**When** classification completes
**Then** it sends a `MonitorEvent::SessionStopped { reason, session }` to the channel

**AC3: Event Routing to Daemon**
**Given** the daemon core receives a `SessionStopped` event
**When** it processes the event
**Then** it routes to the appropriate resume strategy

**AC4: Error Resilience**
**Given** the monitor encounters an error
**When** the error is transient
**Then** it logs the error and continues monitoring
**And** does not crash the daemon

**AC5: Channel Backpressure Handling**
**Given** the event channel is full
**When** monitor tries to send an event
**Then** it handles backpressure gracefully
**And** logs a warning if events are dropped

**AC6: Graceful Shutdown**
**Given** the daemon is shutting down
**When** CancellationToken is triggered
**Then** the monitor stops sending events
**And** channel is closed cleanly

## Tasks / Subtasks

- [ ] Define MonitorEvent enum (AC: 1, 2, 3)
  - [ ] Define `MonitorEvent` enum with all variants
  - [ ] Define `SessionStopped` payload with reason and session
  - [ ] Define `SessionChanged` payload with session data
  - [ ] Define `MonitorError` event for transient errors
  - [ ] Update `src/monitor/events.rs` with comprehensive types

- [ ] Create Monitor orchestrator (AC: 1, 2, 3, 4)
  - [ ] Create `src/monitor/mod.rs` with Monitor struct
  - [ ] Combine watcher, parser, process detector, classifier
  - [ ] Implement event aggregation from sub-components
  - [ ] Implement unified event emission to daemon

- [ ] Implement event channel setup (AC: 1, 2, 5)
  - [ ] Use `tokio::sync::mpsc` for event channel
  - [ ] Configure channel capacity (default 100)
  - [ ] Return receiver to daemon for consumption
  - [ ] Handle sender in monitor components

- [ ] Implement watcher integration (AC: 1, 4)
  - [ ] Receive WatchEvent from SessionWatcher
  - [ ] Parse session file on FileModified events
  - [ ] Emit SessionChanged with parsed data
  - [ ] Handle parse errors gracefully

- [ ] Implement process detection integration (AC: 2, 4)
  - [ ] Receive ProcessEvent from ProcessDetector
  - [ ] On ProcessStopped, run classifier
  - [ ] Emit SessionStopped with classification result
  - [ ] Associate process with session file

- [ ] Implement classification integration (AC: 2, 3)
  - [ ] Use StopReasonClassifier from Stories 2.4-2.6
  - [ ] Include session context in classification
  - [ ] Package result into SessionStopped event
  - [ ] Include exit code from process detector

- [ ] Implement error handling (AC: 4)
  - [ ] Catch and log transient errors
  - [ ] Emit MonitorEvent::Error for significant issues
  - [ ] Continue monitoring after recoverable errors
  - [ ] Track error counts for health reporting

- [ ] Implement backpressure handling (AC: 5)
  - [ ] Use try_send() with timeout
  - [ ] Log warning when channel is full
  - [ ] Implement dropping strategy (oldest first)
  - [ ] Track dropped events for metrics

- [ ] Implement graceful shutdown (AC: 6)
  - [ ] Accept CancellationToken in Monitor::run()
  - [ ] Propagate cancellation to sub-components
  - [ ] Drain pending events on shutdown
  - [ ] Close channel sender cleanly

- [ ] Add unit tests (AC: 1, 2, 3, 4, 5, 6)
  - [ ] Test event emission from each source
  - [ ] Test event routing
  - [ ] Test error handling
  - [ ] Test backpressure behavior
  - [ ] Test graceful shutdown

- [ ] Add integration tests
  - [ ] Test full monitor pipeline
  - [ ] Test watcher -> parser -> event flow
  - [ ] Test process -> classifier -> event flow
  - [ ] Test concurrent event handling

## Dev Notes

### Architecture Requirements

**From architecture.md - Project Structure:**

```
src/monitor/
    mod.rs                    # Monitor module root (THIS STORY - orchestration)
    watcher.rs                # File system watcher - Story 2.1
    session.rs                # Session file parsing - Story 2.2
    frontmatter.rs            # YAML frontmatter extraction - Story 2.2
    process.rs                # Process detection - Story 2.3
    classifier.rs             # Stop reason classification - Stories 2.4-2.6
    events.rs                 # MonitorEvent types (THIS STORY)
    error.rs                  # MonitorError type
```

**From architecture.md - Internal Communication:**

> | From | To | Mechanism |
> |------|-----|-----------|
> | Monitor -> Daemon | `tokio::sync::mpsc` | `MonitorEvent` channel |

**From architecture.md - Async Patterns:**

> **Channel Selection:**
> | Use Case | Channel Type |
> |----------|--------------|
> | One producer, one consumer | `tokio::sync::mpsc` |
> | Broadcast (shutdown signal) | `tokio::sync::broadcast` |

**Implements:** Decoupled monitor architecture, supports all monitoring FRs

### Technical Implementation

**MonitorEvent Types:**

```rust
// src/monitor/events.rs (comprehensive definition)
use std::path::PathBuf;

use crate::monitor::session::Session;
use crate::monitor::classifier::{StopReason, ClassificationResult};
use crate::monitor::process::ProcessInfo;

/// Events emitted by the monitor to the daemon core.
#[derive(Debug, Clone)]
pub enum MonitorEvent {
    /// Session file was created
    SessionCreated {
        path: PathBuf,
        session: Option<Session>,
    },
    
    /// Session file was modified
    SessionChanged {
        path: PathBuf,
        session: Session,
        previous: Option<Session>,
    },
    
    /// Session file was deleted
    SessionDeleted {
        path: PathBuf,
    },
    
    /// opencode process started
    ProcessStarted {
        info: ProcessInfo,
    },
    
    /// opencode process stopped with classified reason
    SessionStopped {
        /// The session that stopped
        session: Option<Session>,
        /// Classified stop reason
        reason: StopReason,
        /// Full classification result with evidence
        classification: ClassificationResult,
        /// Process info if available
        process_info: Option<ProcessInfo>,
    },
    
    /// Monitor encountered a transient error
    Error {
        source: String,
        message: String,
        recoverable: bool,
    },
    
    /// Monitor health status (periodic)
    HealthCheck {
        watcher_running: bool,
        process_detector_running: bool,
        events_processed: u64,
        errors_count: u64,
    },
}

impl MonitorEvent {
    /// Check if this event requires immediate daemon action.
    pub fn is_actionable(&self) -> bool {
        matches!(
            self,
            MonitorEvent::SessionStopped { .. } |
            MonitorEvent::Error { recoverable: false, .. }
        )
    }
    
    /// Get the session path if applicable.
    pub fn session_path(&self) -> Option<&PathBuf> {
        match self {
            MonitorEvent::SessionCreated { path, .. } => Some(path),
            MonitorEvent::SessionChanged { path, .. } => Some(path),
            MonitorEvent::SessionDeleted { path } => Some(path),
            MonitorEvent::SessionStopped { session, .. } => session.as_ref().map(|s| &s.path),
            _ => None,
        }
    }
}
```

**Monitor Orchestrator:**

```rust
// src/monitor/mod.rs
pub mod classifier;
pub mod events;
pub mod frontmatter;
pub mod process;
pub mod session;
pub mod watcher;

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, warn};

use crate::monitor::classifier::{StopReasonClassifier, ClassifierConfig};
use crate::monitor::events::MonitorEvent;
use crate::monitor::frontmatter::parse_session;
use crate::monitor::process::{ProcessDetector, ProcessEvent};
use crate::monitor::session::Session;
use crate::monitor::watcher::{SessionWatcher, WatchEvent};

const DEFAULT_CHANNEL_CAPACITY: usize = 100;
const HEALTH_CHECK_INTERVAL_SECS: u64 = 60;

/// Configuration for the Monitor.
#[derive(Debug, Clone)]
pub struct MonitorConfig {
    /// Path to session directory
    pub session_dir: PathBuf,
    /// Event channel capacity
    pub channel_capacity: usize,
    /// Classifier configuration
    pub classifier_config: ClassifierConfig,
    /// Enable process detection
    pub enable_process_detection: bool,
    /// Health check interval
    pub health_check_interval: Duration,
}

impl Default for MonitorConfig {
    fn default() -> Self {
        let home = dirs::home_dir().expect("Could not determine home directory");
        Self {
            session_dir: home.join(".opencode"),
            channel_capacity: DEFAULT_CHANNEL_CAPACITY,
            classifier_config: ClassifierConfig::default(),
            enable_process_detection: true,
            health_check_interval: Duration::from_secs(HEALTH_CHECK_INTERVAL_SECS),
        }
    }
}

/// Monitor orchestrates file watching, process detection, and classification.
pub struct Monitor {
    config: MonitorConfig,
    classifier: StopReasonClassifier,
    current_session: Option<Session>,
    events_processed: u64,
    errors_count: u64,
}

impl Monitor {
    /// Create a new Monitor with default configuration.
    pub fn new() -> Result<Self, MonitorError> {
        Self::with_config(MonitorConfig::default())
    }
    
    /// Create with custom configuration.
    pub fn with_config(config: MonitorConfig) -> Result<Self, MonitorError> {
        let classifier = StopReasonClassifier::with_config(config.classifier_config.clone())?;
        
        Ok(Self {
            config,
            classifier,
            current_session: None,
            events_processed: 0,
            errors_count: 0,
        })
    }
    
    /// Run the monitor, returning a receiver for monitor events.
    pub async fn run(
        mut self,
        cancel: CancellationToken,
    ) -> Result<mpsc::Receiver<MonitorEvent>, MonitorError> {
        let (tx, rx) = mpsc::channel(self.config.channel_capacity);
        
        // Start file watcher
        let watcher = SessionWatcher::with_path(self.config.session_dir.clone());
        let watcher_rx = watcher.run(cancel.clone()).await?;
        
        // Start process detector (if enabled)
        let process_rx = if self.config.enable_process_detection {
            let detector = ProcessDetector::new();
            Some(detector.run(cancel.clone()).await?)
        } else {
            None
        };
        
        // Spawn event processing task
        let config = self.config.clone();
        tokio::spawn(async move {
            self.event_loop(tx, watcher_rx, process_rx, cancel).await;
        });
        
        Ok(rx)
    }
    
    async fn event_loop(
        &mut self,
        tx: mpsc::Sender<MonitorEvent>,
        mut watcher_rx: mpsc::Receiver<WatchEvent>,
        mut process_rx: Option<mpsc::Receiver<ProcessEvent>>,
        cancel: CancellationToken,
    ) {
        let mut health_interval = tokio::time::interval(self.config.health_check_interval);
        
        loop {
            tokio::select! {
                _ = cancel.cancelled() => {
                    info!("Monitor shutting down");
                    break;
                }
                
                Some(event) = watcher_rx.recv() => {
                    self.handle_watch_event(event, &tx).await;
                }
                
                Some(event) = async {
                    match &mut process_rx {
                        Some(rx) => rx.recv().await,
                        None => std::future::pending().await,
                    }
                } => {
                    self.handle_process_event(event, &tx).await;
                }
                
                _ = health_interval.tick() => {
                    self.emit_health_check(&tx).await;
                }
            }
        }
    }
    
    async fn handle_watch_event(&mut self, event: WatchEvent, tx: &mpsc::Sender<MonitorEvent>) {
        self.events_processed += 1;
        
        match event {
            WatchEvent::FileCreated(path) => {
                debug!(path = %path.display(), "Session file created");
                let session = parse_session(&path).ok();
                self.current_session = session.clone();
                
                let _ = self.try_send(tx, MonitorEvent::SessionCreated { path, session }).await;
            }
            
            WatchEvent::FileModified(path) => {
                debug!(path = %path.display(), "Session file modified");
                match parse_session(&path) {
                    Ok(session) => {
                        let previous = self.current_session.replace(session.clone());
                        let _ = self.try_send(tx, MonitorEvent::SessionChanged {
                            path,
                            session,
                            previous,
                        }).await;
                    }
                    Err(e) => {
                        warn!(error = %e, path = %path.display(), "Failed to parse session");
                        self.errors_count += 1;
                    }
                }
            }
            
            WatchEvent::FileDeleted(path) => {
                debug!(path = %path.display(), "Session file deleted");
                self.current_session = None;
                let _ = self.try_send(tx, MonitorEvent::SessionDeleted { path }).await;
            }
            
            WatchEvent::Error(msg) => {
                warn!(error = %msg, "Watcher error");
                self.errors_count += 1;
                let _ = self.try_send(tx, MonitorEvent::Error {
                    source: "watcher".to_string(),
                    message: msg,
                    recoverable: true,
                }).await;
            }
            
            _ => {}
        }
    }
    
    async fn handle_process_event(&mut self, event: ProcessEvent, tx: &mpsc::Sender<MonitorEvent>) {
        self.events_processed += 1;
        
        match event {
            ProcessEvent::ProcessStarted(info) => {
                info!(pid = info.pid, "opencode process started");
                let _ = self.try_send(tx, MonitorEvent::ProcessStarted { info }).await;
            }
            
            ProcessEvent::ProcessStopped { info, exit_code } => {
                info!(pid = info.pid, exit_code = ?exit_code, "opencode process stopped");
                
                // Classify stop reason
                let session_path = self.current_session.as_ref().map(|s| s.path.clone());
                let classification = if let Some(path) = &session_path {
                    self.classifier.classify(path, exit_code)
                } else {
                    // No session file, classify from exit code only
                    self.classifier.classify_from_exit_code(exit_code)
                };
                
                let _ = self.try_send(tx, MonitorEvent::SessionStopped {
                    session: self.current_session.clone(),
                    reason: classification.reason.clone(),
                    classification,
                    process_info: Some(info),
                }).await;
            }
        }
    }
    
    async fn emit_health_check(&self, tx: &mpsc::Sender<MonitorEvent>) {
        let _ = self.try_send(tx, MonitorEvent::HealthCheck {
            watcher_running: true, // Would check actual state
            process_detector_running: true,
            events_processed: self.events_processed,
            errors_count: self.errors_count,
        }).await;
    }
    
    async fn try_send(&self, tx: &mpsc::Sender<MonitorEvent>, event: MonitorEvent) -> bool {
        match tx.try_send(event) {
            Ok(_) => true,
            Err(mpsc::error::TrySendError::Full(event)) => {
                warn!(event = ?event.session_path(), "Event channel full, dropping event");
                false
            }
            Err(mpsc::error::TrySendError::Closed(_)) => {
                debug!("Event channel closed");
                false
            }
        }
    }
}

impl Default for Monitor {
    fn default() -> Self {
        Self::new().expect("Failed to create default monitor")
    }
}

#[derive(Debug, thiserror::Error)]
pub enum MonitorError {
    #[error("Watcher error: {0}")]
    Watcher(#[from] watcher::WatcherError),
    
    #[error("Process detector error: {0}")]
    ProcessDetector(#[from] process::ProcessError),
    
    #[error("Classifier error: {0}")]
    Classifier(#[from] classifier::ClassifierError),
    
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}
```

### Dependencies

Uses existing dependencies (no new dependencies):
- `tokio` (already in Cargo.toml) - async runtime, channels
- `tokio-util` (already in Cargo.toml) - CancellationToken
- `tracing` (already in Cargo.toml) - structured logging
- `thiserror` (already in Cargo.toml) - error types
- `dirs` (already in Cargo.toml) - home directory

### Error Handling Strategy

| Error Type | Behavior |
|------------|----------|
| Watcher error | Log, emit Error event, continue |
| Parse error | Log, skip session update, continue |
| Process detection error | Log, emit Error event, continue |
| Classification error | Log, return Unknown reason |
| Channel full | Log warning, drop event |
| Channel closed | Stop sending, prepare for shutdown |

### Backpressure Handling

```rust
// Strategy: try_send with warning on full channel
match tx.try_send(event) {
    Ok(_) => { /* Success */ }
    Err(TrySendError::Full(_)) => {
        warn!("Event channel full, dropping event");
        // Could implement oldest-first dropping with bounded queue
    }
    Err(TrySendError::Closed(_)) => {
        // Channel closed, likely shutdown
    }
}
```

### Previous Story Learnings

From Story 2.1 (File System Watcher):
1. **WatchEvent types**: Integrate watcher events
2. **CancellationToken**: Propagate shutdown

From Story 2.2 (Session File Parser):
1. **parse_session()**: Use for SessionChanged events
2. **Error handling**: Continue on parse failures

From Story 2.3 (Process Detection):
1. **ProcessEvent types**: Integrate process events
2. **Exit code**: Pass to classifier

From Stories 2.4-2.6 (Classification):
1. **StopReasonClassifier**: Use for SessionStopped
2. **ClassificationResult**: Include full evidence

### Integration with Daemon

The daemon consumes events and routes to appropriate handlers:

```rust
// In daemon core (not this story, but shows integration)
async fn handle_monitor_event(&mut self, event: MonitorEvent) {
    match event {
        MonitorEvent::SessionStopped { reason, session, .. } => {
            if reason.should_auto_resume() {
                self.resume_handler.handle(reason, session).await;
            } else {
                info!(reason = reason.description(), "Not auto-resuming");
            }
        }
        MonitorEvent::Error { message, recoverable, .. } => {
            if !recoverable {
                error!(message, "Non-recoverable monitor error");
            }
        }
        _ => {}
    }
}
```

### Testing Strategy

**Unit Tests:**
- Test MonitorEvent construction
- Test event routing logic
- Test backpressure handling
- Test health check emission

**Integration Tests:**
- Test full monitor pipeline
- Test watcher integration
- Test process detector integration
- Test graceful shutdown

### References

- [Source: _bmad-output/planning-artifacts/architecture.md#Internal Communication]
- [Source: _bmad-output/planning-artifacts/architecture.md#Async Patterns]
- [Source: _bmad-output/planning-artifacts/epics.md#Story 2.7: Monitor Event Channel]
- [Source: _bmad-output/implementation-artifacts/2-1-file-system-watcher-setup.md]
- [Source: _bmad-output/implementation-artifacts/2-3-process-detection-opencode-start-stop.md]
- [Source: _bmad-output/implementation-artifacts/2-4-stop-reason-classification-rate-limit.md]

## File List

**Files to create:**
- `tests/monitor_integration_test.rs`

**Files to modify:**
- `src/monitor/mod.rs` (add Monitor orchestrator)
- `src/monitor/events.rs` (comprehensive MonitorEvent enum)
- `_bmad-output/implementation-artifacts/sprint-status.yaml`

## Change Log

- 2026-02-05: Story created and marked ready-for-dev
