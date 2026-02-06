# Story 9.1: OpenCode Process Detection

Status: ready-for-dev

## Story

As a daemon,
I want to detect when OpenCode process starts, stops, or crashes,
So that I can respond appropriately and maintain continuous monitoring.

## Acceptance Criteria

**AC1: Detect OpenCode Process Start**
**Given** the daemon is monitoring
**When** OpenCode process starts (via `opencode serve`)
**Then** event `OpenCodeStarted` is logged with PID
**And** daemon begins tracking the process

**AC2: Detect Normal Exit**
**Given** OpenCode is running
**When** the process exits normally (exit code 0)
**Then** event `OpenCodeStopped` is logged
**And** reason is classified as `NormalExit`

**AC3: Detect Crash**
**Given** OpenCode is running
**When** the process crashes (non-zero exit code)
**Then** event `OpenCodeCrashed` is logged with exit code
**And** reason is classified as `Crash`

**AC4: Detect Kill Signal**
**Given** OpenCode is running
**When** the process is killed (SIGKILL/SIGTERM)
**Then** event `OpenCodeKilled` is logged with signal info
**And** reason is classified appropriately

**AC5: Detect Existing Process on Startup**
**Given** daemon starts
**When** OpenCode is already running
**Then** it detects the existing process
**And** begins monitoring it immediately

**AC6: Health Check Integration**
**Given** OpenCode server is running
**When** daemon checks health
**Then** it queries `http://localhost:{port}/global/health`
**And** considers process alive if health responds

## Tasks / Subtasks

- [ ] Create OpenCode process module structure (AC: 1, 2, 3, 4, 5)
  - [ ] Create `src/opencode/mod.rs` module root
  - [ ] Create `src/opencode/process.rs` for process monitoring
  - [ ] Define `OpenCodeProcess` struct with state tracking
  - [ ] Define `OpenCodeEvent` enum (Started, Stopped, Crashed, Killed)
  - [ ] Define `OpenCodeExitReason` enum (NormalExit, Crash, Signal)

- [ ] Implement process detection methods (AC: 1, 5)
  - [ ] Implement `find_opencode_process()` to scan process list
  - [ ] Check for process by name pattern: `opencode` with `serve` arg
  - [ ] Extract PID and command line arguments
  - [ ] Handle multiple OpenCode instances (warn, track first)
  - [ ] Use `sysinfo` crate for cross-platform process enumeration

- [ ] Implement process state machine (AC: 1, 2, 3, 4)
  - [ ] Define `ProcessState` enum (Unknown, Running, Stopped)
  - [ ] Track current PID when running
  - [ ] Track last known exit code/signal
  - [ ] Implement state transitions on events

- [ ] Implement exit classification (AC: 2, 3, 4)
  - [ ] Parse exit code from process termination
  - [ ] Classify exit code 0 as `NormalExit`
  - [ ] Classify non-zero exit as `Crash` with code
  - [ ] Detect signal termination (Unix: check if signaled)
  - [ ] Map common signals (SIGTERM=graceful, SIGKILL=forced)

- [ ] Implement health check detection (AC: 6)
  - [ ] Create `check_opencode_health()` async function
  - [ ] Send GET request to `/global/health` endpoint
  - [ ] Handle configurable port (default 4096)
  - [ ] Consider process alive if health returns 200
  - [ ] Timeout health check after 2 seconds

- [ ] Implement process monitoring loop (AC: 1, 2, 3, 4, 5)
  - [ ] Create `OpenCodeMonitor` struct with channel sender
  - [ ] Implement `start()` async method for monitoring loop
  - [ ] Poll process state at configurable interval (default 1s)
  - [ ] Emit events via channel on state changes
  - [ ] Handle graceful shutdown via CancellationToken

- [ ] Integrate with daemon core (AC: 1, 2, 3, 4, 5)
  - [ ] Add OpenCode monitor to daemon startup
  - [ ] Create channel for OpenCode events
  - [ ] Route events to appropriate handlers
  - [ ] Log events with structured fields (PID, exit_code, signal)

- [ ] Add configuration options (AC: 6)
  - [ ] Add `opencode.enabled` config option (default: false)
  - [ ] Add `opencode.health_port` config option (default: 4096)
  - [ ] Add `opencode.poll_interval_ms` config option (default: 1000)
  - [ ] Add `opencode.health_timeout_ms` config option (default: 2000)

- [ ] Add unit/integration tests (AC: 1, 2, 3, 4, 5, 6)
  - [ ] Test process detection finds running OpenCode
  - [ ] Test exit classification for exit code 0
  - [ ] Test exit classification for non-zero exit
  - [ ] Test signal detection (Unix only)
  - [ ] Test health check success/failure
  - [ ] Test monitoring loop emits correct events
  - [ ] Test daemon startup detects existing process

## Dev Notes

### Architecture Requirements

**From architecture.md - FR45 (OpenCode Management):**
> Daemon detects OpenCode process crash/exit

**From architecture.md - Module Location:**
```
src/opencode/
    mod.rs                    # OpenCode integration root
    process.rs                # Process monitoring & restart (THIS STORY)
    client.rs                 # HTTP client for OpenCode API (Story 9.3)
    session.rs                # Session management via API (Story 9.3)
```

**From architecture.md - Integration Points:**
> OpenCode Server API | `opencode/client.rs` | HTTP (REST API on port 4096)
> OpenCode process | `opencode/process.rs` | Process spawn (`opencode serve`)

### Technical Implementation

**Process Detection Strategy:**

```rust
// src/opencode/process.rs
use sysinfo::{ProcessExt, System, SystemExt};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

/// Events emitted by OpenCode process monitor
#[derive(Debug, Clone)]
pub enum OpenCodeEvent {
    Started { pid: u32 },
    Stopped { reason: ExitReason },
    HealthCheckFailed { error: String },
}

/// Why OpenCode process exited
#[derive(Debug, Clone)]
pub enum ExitReason {
    NormalExit,
    Crash { exit_code: i32 },
    Signal { signal: i32 },
    Unknown,
}

/// Current state of OpenCode process
#[derive(Debug, Clone, Default)]
pub struct ProcessState {
    pub running: bool,
    pub pid: Option<u32>,
    pub last_exit: Option<ExitReason>,
}

/// Monitors OpenCode process lifecycle
pub struct OpenCodeMonitor {
    state: ProcessState,
    health_port: u16,
    poll_interval: Duration,
    event_tx: mpsc::Sender<OpenCodeEvent>,
    cancel_token: CancellationToken,
}

impl OpenCodeMonitor {
    pub fn new(
        config: &OpenCodeConfig,
        event_tx: mpsc::Sender<OpenCodeEvent>,
        cancel_token: CancellationToken,
    ) -> Self {
        Self {
            state: ProcessState::default(),
            health_port: config.health_port,
            poll_interval: Duration::from_millis(config.poll_interval_ms),
            event_tx,
            cancel_token,
        }
    }

    /// Find OpenCode process by scanning process list
    pub fn find_opencode_process(&self) -> Option<u32> {
        let mut sys = System::new();
        sys.refresh_processes();
        
        for (pid, process) in sys.processes() {
            let name = process.name();
            let cmd = process.cmd();
            
            // Match "opencode" binary with "serve" argument
            if name.contains("opencode") && cmd.iter().any(|arg| arg == "serve") {
                return Some(pid.as_u32());
            }
        }
        None
    }

    /// Check if OpenCode is healthy via HTTP
    pub async fn check_health(&self) -> bool {
        let url = format!("http://localhost:{}/global/health", self.health_port);
        let client = reqwest::Client::builder()
            .timeout(Duration::from_millis(2000))
            .build()
            .ok()?;
        
        match client.get(&url).send().await {
            Ok(resp) => resp.status().is_success(),
            Err(_) => false,
        }
    }

    /// Main monitoring loop
    pub async fn start(&mut self) -> anyhow::Result<()> {
        // Initial detection
        if let Some(pid) = self.find_opencode_process() {
            self.state.running = true;
            self.state.pid = Some(pid);
            tracing::info!(pid = pid, "Detected existing OpenCode process");
            self.event_tx.send(OpenCodeEvent::Started { pid }).await?;
        }

        loop {
            tokio::select! {
                _ = self.cancel_token.cancelled() => {
                    tracing::info!("OpenCode monitor shutting down");
                    break;
                }
                _ = tokio::time::sleep(self.poll_interval) => {
                    self.check_process_state().await?;
                }
            }
        }
        Ok(())
    }

    async fn check_process_state(&mut self) -> anyhow::Result<()> {
        let current_pid = self.find_opencode_process();
        
        match (self.state.running, current_pid) {
            // Was not running, now running
            (false, Some(pid)) => {
                self.state.running = true;
                self.state.pid = Some(pid);
                tracing::info!(pid = pid, "OpenCode process started");
                self.event_tx.send(OpenCodeEvent::Started { pid }).await?;
            }
            // Was running, now stopped
            (true, None) => {
                let reason = self.classify_exit();
                self.state.running = false;
                self.state.last_exit = Some(reason.clone());
                tracing::warn!(?reason, "OpenCode process stopped");
                self.event_tx.send(OpenCodeEvent::Stopped { reason }).await?;
            }
            // Still running, check health
            (true, Some(_)) => {
                if !self.check_health().await {
                    tracing::warn!("OpenCode health check failed");
                    self.event_tx.send(OpenCodeEvent::HealthCheckFailed {
                        error: "Health endpoint not responding".into()
                    }).await?;
                }
            }
            // Still stopped
            (false, None) => {}
        }
        Ok(())
    }

    fn classify_exit(&self) -> ExitReason {
        // In real implementation, we'd capture exit code/signal
        // from process termination. For now, return Unknown.
        // See platform-specific implementation notes below.
        ExitReason::Unknown
    }
}
```

**Platform-Specific Exit Detection (Unix):**

```rust
#[cfg(unix)]
fn get_exit_info(status: std::process::ExitStatus) -> ExitReason {
    use std::os::unix::process::ExitStatusExt;
    
    if let Some(code) = status.code() {
        if code == 0 {
            ExitReason::NormalExit
        } else {
            ExitReason::Crash { exit_code: code }
        }
    } else if let Some(signal) = status.signal() {
        ExitReason::Signal { signal }
    } else {
        ExitReason::Unknown
    }
}
```

**Configuration Schema Addition:**

```rust
// src/config/schema.rs
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OpenCodeConfig {
    /// Enable OpenCode process management
    #[serde(default)]
    pub enabled: bool,
    
    /// Port for OpenCode health checks
    #[serde(default = "default_health_port")]
    pub health_port: u16,
    
    /// Process poll interval in milliseconds
    #[serde(default = "default_poll_interval")]
    pub poll_interval_ms: u64,
    
    /// Health check timeout in milliseconds
    #[serde(default = "default_health_timeout")]
    pub health_timeout_ms: u64,
}

fn default_health_port() -> u16 { 4096 }
fn default_poll_interval() -> u64 { 1000 }
fn default_health_timeout() -> u64 { 2000 }

impl Default for OpenCodeConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            health_port: default_health_port(),
            poll_interval_ms: default_poll_interval(),
            health_timeout_ms: default_health_timeout(),
        }
    }
}
```

**Daemon Integration:**

```rust
// src/daemon/core.rs (modifications)
use crate::opencode::{OpenCodeMonitor, OpenCodeEvent};

impl Daemon {
    pub async fn start(&mut self) -> anyhow::Result<()> {
        // ... existing startup code ...
        
        // Start OpenCode monitor if enabled
        let config = self.config.read().await;
        if config.opencode.enabled {
            let (event_tx, event_rx) = mpsc::channel(32);
            
            let mut monitor = OpenCodeMonitor::new(
                &config.opencode,
                event_tx,
                self.cancel_token.clone(),
            );
            
            // Spawn monitor task
            tokio::spawn(async move {
                if let Err(e) = monitor.start().await {
                    tracing::error!("OpenCode monitor error: {}", e);
                }
            });
            
            // Spawn event handler task
            self.spawn_opencode_event_handler(event_rx);
        }
        
        Ok(())
    }
    
    fn spawn_opencode_event_handler(&self, mut rx: mpsc::Receiver<OpenCodeEvent>) {
        let cancel = self.cancel_token.clone();
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = cancel.cancelled() => break,
                    Some(event) = rx.recv() => {
                        match event {
                            OpenCodeEvent::Started { pid } => {
                                tracing::info!(pid, "OpenCode started");
                            }
                            OpenCodeEvent::Stopped { reason } => {
                                tracing::warn!(?reason, "OpenCode stopped");
                                // Story 9.2 will handle restart logic here
                            }
                            OpenCodeEvent::HealthCheckFailed { error } => {
                                tracing::warn!(error, "OpenCode health check failed");
                            }
                        }
                    }
                }
            }
        });
    }
}
```

### OpenCode Context

**OpenCode Server Details:**
- Default port: 4096
- Health endpoint: `/global/health`
- Started via: `opencode serve`
- Config location: `~/.config/opencode/opencode.json`

**Detection Methods (Priority Order):**
1. **Health Check**: Most reliable - HTTP GET to health endpoint
2. **Process Scan**: Backup - scan process list for `opencode serve`
3. **PID File**: If OpenCode creates one (check OpenCode docs)

### Dependencies

Add to Cargo.toml:
```toml
[dependencies]
sysinfo = "0.32"  # Cross-platform process information
```

Already available:
- `reqwest` for health checks
- `tokio` for async runtime
- `tracing` for logging

### Testing Strategy

**Unit Tests:**
- Test `ExitReason` classification logic
- Test `ProcessState` transitions
- Test configuration defaults

**Integration Tests:**
- Mock process list for detection tests
- Test health check with mock HTTP server
- Test event emission on state changes

**Manual Testing:**
1. Start daemon with `opencode.enabled = true`
2. Start `opencode serve` manually
3. Verify "OpenCode started" log appears
4. Kill OpenCode process
5. Verify "OpenCode stopped" log with reason

### References

- [Source: _bmad-output/planning-artifacts/architecture.md - Module: src/opencode/]
- [Source: _bmad-output/planning-artifacts/epics.md#Story 9.1: OpenCode Process Detection]
- [Source: _bmad-output/planning-artifacts/prd.md - FR45]
- [Existing: src/monitor/classifier.rs - Stop reason classification pattern]
- [Existing: src/daemon/core.rs - Daemon startup pattern]

## File List

**Files to create:**
- `src/opencode/mod.rs`
- `src/opencode/process.rs`

**Files to modify:**
- `src/config/schema.rs` (add OpenCodeConfig)
- `src/daemon/core.rs` (integrate OpenCode monitor)
- `Cargo.toml` (add sysinfo dependency)
- `_bmad-output/implementation-artifacts/sprint-status.yaml`

## Change Log

- 2026-02-06: Story created and marked ready-for-dev
