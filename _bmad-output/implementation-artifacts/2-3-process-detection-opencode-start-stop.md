# Story 2.3: Process Detection (opencode Start/Stop)

Status: ready-for-dev

## Story

As a monitor,
I want to detect when opencode processes start and stop,
So that I can track active sessions.

## Acceptance Criteria

**AC1: Process Start Detection**
**Given** the daemon is monitoring
**When** an opencode process starts
**Then** a `ProcessStarted` event is emitted
**And** the event includes PID and command line

**AC2: Process Stop Detection**
**Given** an opencode process is running
**When** the process terminates
**Then** a `ProcessStopped` event is emitted
**And** the event includes exit code if available

**AC3: Multiple Process Handling**
**Given** multiple opencode processes are running
**When** any one terminates
**Then** only that specific process stop is detected
**And** other processes continue to be monitored

**AC4: Existing Process Detection**
**Given** the daemon starts
**When** opencode is already running
**Then** it detects the existing process
**And** begins monitoring it

**AC5: Graceful Shutdown Integration**
**Given** the daemon is shutting down
**When** CancellationToken is triggered
**Then** the process detector stops cleanly
**And** no events are emitted after cancellation

**AC6: Error Recovery**
**Given** a transient process enumeration error occurs
**When** the detector encounters it
**Then** it logs the error and continues monitoring
**And** does not crash the daemon

## Tasks / Subtasks

- [ ] Create process detection module (AC: 1, 2, 5, 6)
  - [ ] Create `src/monitor/process.rs` with ProcessDetector struct
  - [ ] Implement ProcessEvent enum (ProcessStarted, ProcessStopped)
  - [ ] Define ProcessInfo struct (pid, command_line, start_time)
  - [ ] Update `src/monitor/mod.rs` to export modules

- [ ] Implement process enumeration (AC: 1, 3, 4)
  - [ ] Linux: Use `/proc` filesystem scanning
  - [ ] macOS: Use `sysctl` or `libproc` bindings
  - [ ] Filter for opencode processes by command name
  - [ ] Extract PID and command line arguments

- [ ] Implement ProcessDetector struct (AC: 1, 2, 3)
  - [ ] Track known processes in HashMap<Pid, ProcessInfo>
  - [ ] Implement periodic polling (configurable interval, default 1s)
  - [ ] Detect new processes by comparing snapshots
  - [ ] Detect stopped processes by missing PIDs

- [ ] Implement start detection (AC: 1, 4)
  - [ ] Check for existing opencode processes on startup
  - [ ] Emit ProcessStarted for each existing process
  - [ ] Emit ProcessStarted when new process appears
  - [ ] Include full command line in event

- [ ] Implement stop detection (AC: 2, 3)
  - [ ] Detect when tracked PID disappears
  - [ ] Try to read exit code from `/proc/{pid}/stat` (Linux)
  - [ ] Handle case where exit code is unavailable
  - [ ] Remove process from tracking after stop event

- [ ] Implement graceful shutdown (AC: 5)
  - [ ] Accept `CancellationToken` in `ProcessDetector::run()`
  - [ ] Use `tokio::select!` for cancellation handling
  - [ ] Stop polling loop on cancellation
  - [ ] Drop tracking state cleanly

- [ ] Implement error handling (AC: 6)
  - [ ] Handle permission errors (process enumeration)
  - [ ] Handle race conditions (process exits during read)
  - [ ] Log errors without crashing
  - [ ] Continue polling after transient errors

- [ ] Integrate with monitor event channel (AC: 1, 2)
  - [ ] Follow `MonitorEvent` pattern from Story 2.7
  - [ ] Send events via `tokio::sync::mpsc` channel
  - [ ] Create ProcessStateAccess trait for daemon integration

- [ ] Add unit tests (AC: 1, 2, 3, 4, 5, 6)
  - [ ] Test process detection with mock process list
  - [ ] Test multiple process tracking
  - [ ] Test existing process detection on startup
  - [ ] Test graceful shutdown with CancellationToken
  - [ ] Test error recovery on transient failures

- [ ] Add integration tests
  - [ ] Test full detector lifecycle (start, detect, stop)
  - [ ] Test with real process spawn/kill (if feasible)
  - [ ] Test high-frequency start/stop scenarios

## Dev Notes

### Architecture Requirements

**From architecture.md - Project Structure:**

```
src/monitor/
    mod.rs                    # Monitor module root
    watcher.rs                # File system watcher (notify) - Story 2.1
    session.rs                # Session file parsing - Story 2.2
    frontmatter.rs            # YAML frontmatter extraction - Story 2.2
    process.rs                # Process detection (THIS STORY)
    classifier.rs             # Stop reason classification (Story 2.4-2.6)
    error.rs                  # MonitorError type
```

**From architecture.md - Internal Communication:**

> | From | To | Mechanism |
> |------|-----|-----------|
> | Monitor -> Daemon | `tokio::sync::mpsc` | `MonitorEvent` channel |

**Implements:** FR1 (detect opencode process start), FR2 (detect opencode process stop)

### Technical Implementation

**ProcessEvent Types:**

```rust
// src/monitor/process.rs
use std::path::PathBuf;

/// Information about a tracked process.
#[derive(Debug, Clone)]
pub struct ProcessInfo {
    /// Process ID
    pub pid: u32,
    /// Full command line
    pub command_line: Vec<String>,
    /// Process start time (if available)
    pub start_time: Option<std::time::SystemTime>,
    /// Working directory (if available)
    pub working_dir: Option<PathBuf>,
}

/// Events emitted by the process detector.
#[derive(Debug, Clone)]
pub enum ProcessEvent {
    /// An opencode process started
    ProcessStarted(ProcessInfo),
    /// An opencode process stopped
    ProcessStopped {
        info: ProcessInfo,
        exit_code: Option<i32>,
    },
}
```

**ProcessDetector Implementation:**

```rust
// src/monitor/process.rs
use std::collections::HashMap;
use std::time::Duration;

use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, warn};

const DEFAULT_POLL_INTERVAL_MS: u64 = 1000;
const OPENCODE_PROCESS_NAME: &str = "opencode";

#[derive(Debug, thiserror::Error)]
pub enum ProcessError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("Process enumeration failed: {0}")]
    EnumerationFailed(String),
    
    #[error("Permission denied reading process info")]
    PermissionDenied,
}

pub struct ProcessDetector {
    poll_interval: Duration,
    tracked_processes: HashMap<u32, ProcessInfo>,
}

impl ProcessDetector {
    /// Create a new ProcessDetector with default poll interval.
    pub fn new() -> Self {
        Self {
            poll_interval: Duration::from_millis(DEFAULT_POLL_INTERVAL_MS),
            tracked_processes: HashMap::new(),
        }
    }
    
    /// Set custom poll interval.
    pub fn with_poll_interval(mut self, interval: Duration) -> Self {
        self.poll_interval = interval;
        self
    }
    
    /// Run the process detector, returning a receiver for process events.
    pub async fn run(
        mut self,
        cancel: CancellationToken,
    ) -> Result<mpsc::Receiver<ProcessEvent>, ProcessError> {
        let (tx, rx) = mpsc::channel(100);
        
        // Detect existing processes on startup
        let initial_processes = self.enumerate_opencode_processes()?;
        for process in initial_processes {
            info!(pid = process.pid, "Detected existing opencode process");
            self.tracked_processes.insert(process.pid, process.clone());
            let _ = tx.send(ProcessEvent::ProcessStarted(process)).await;
        }
        
        // Spawn polling task
        let poll_interval = self.poll_interval;
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(poll_interval);
            
            loop {
                tokio::select! {
                    _ = cancel.cancelled() => {
                        info!("Process detector shutting down");
                        break;
                    }
                    _ = interval.tick() => {
                        if let Err(e) = self.poll_processes(&tx).await {
                            error!(error = %e, "Process polling error");
                        }
                    }
                }
            }
        });
        
        Ok(rx)
    }
    
    async fn poll_processes(
        &mut self,
        tx: &mpsc::Sender<ProcessEvent>,
    ) -> Result<(), ProcessError> {
        let current_processes = self.enumerate_opencode_processes()?;
        let current_pids: std::collections::HashSet<_> = 
            current_processes.iter().map(|p| p.pid).collect();
        
        // Detect new processes
        for process in &current_processes {
            if !self.tracked_processes.contains_key(&process.pid) {
                info!(pid = process.pid, "New opencode process detected");
                self.tracked_processes.insert(process.pid, process.clone());
                let _ = tx.send(ProcessEvent::ProcessStarted(process.clone())).await;
            }
        }
        
        // Detect stopped processes
        let stopped_pids: Vec<_> = self.tracked_processes
            .keys()
            .filter(|pid| !current_pids.contains(pid))
            .copied()
            .collect();
        
        for pid in stopped_pids {
            if let Some(info) = self.tracked_processes.remove(&pid) {
                info!(pid = info.pid, "opencode process stopped");
                let exit_code = self.try_get_exit_code(pid);
                let _ = tx.send(ProcessEvent::ProcessStopped { info, exit_code }).await;
            }
        }
        
        Ok(())
    }
    
    #[cfg(target_os = "linux")]
    fn enumerate_opencode_processes(&self) -> Result<Vec<ProcessInfo>, ProcessError> {
        use std::fs;
        
        let mut processes = Vec::new();
        
        for entry in fs::read_dir("/proc")? {
            let entry = entry?;
            let path = entry.path();
            
            // Only process numeric directories (PIDs)
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if let Ok(pid) = name.parse::<u32>() {
                    if let Ok(cmdline) = fs::read_to_string(path.join("cmdline")) {
                        let args: Vec<String> = cmdline
                            .split('\0')
                            .filter(|s| !s.is_empty())
                            .map(String::from)
                            .collect();
                        
                        if args.first().map(|s| s.contains(OPENCODE_PROCESS_NAME)).unwrap_or(false) {
                            let cwd = fs::read_link(path.join("cwd")).ok();
                            processes.push(ProcessInfo {
                                pid,
                                command_line: args,
                                start_time: None,
                                working_dir: cwd,
                            });
                        }
                    }
                }
            }
        }
        
        Ok(processes)
    }
    
    #[cfg(target_os = "macos")]
    fn enumerate_opencode_processes(&self) -> Result<Vec<ProcessInfo>, ProcessError> {
        // macOS: Use sysctl or ps command
        use std::process::Command;
        
        let output = Command::new("pgrep")
            .args(["-l", OPENCODE_PROCESS_NAME])
            .output()
            .map_err(|e| ProcessError::EnumerationFailed(e.to_string()))?;
        
        let mut processes = Vec::new();
        
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines() {
                let parts: Vec<&str> = line.splitn(2, ' ').collect();
                if let Some(pid_str) = parts.first() {
                    if let Ok(pid) = pid_str.parse::<u32>() {
                        // Get full command line with ps
                        if let Ok(ps_output) = Command::new("ps")
                            .args(["-p", &pid.to_string(), "-o", "args="])
                            .output()
                        {
                            let cmdline = String::from_utf8_lossy(&ps_output.stdout);
                            let args: Vec<String> = cmdline
                                .trim()
                                .split_whitespace()
                                .map(String::from)
                                .collect();
                            
                            processes.push(ProcessInfo {
                                pid,
                                command_line: args,
                                start_time: None,
                                working_dir: None,
                            });
                        }
                    }
                }
            }
        }
        
        Ok(processes)
    }
    
    fn try_get_exit_code(&self, _pid: u32) -> Option<i32> {
        // Exit code is typically not available after process exits
        // Would need to use waitpid() if we're the parent, or
        // read from /proc on Linux before the zombie is reaped
        None
    }
}

impl Default for ProcessDetector {
    fn default() -> Self {
        Self::new()
    }
}
```

### Dependencies

Uses existing dependencies:
- `tokio` (already in Cargo.toml) - async runtime, channels
- `tokio-util` (already in Cargo.toml) - CancellationToken
- `tracing` (already in Cargo.toml) - structured logging
- `thiserror` (already in Cargo.toml) - error types

### Error Handling Pattern

Uses `thiserror` following project conventions from architecture.md:
- `ProcessError::Io` - File system operations failed
- `ProcessError::EnumerationFailed` - Process list retrieval failed
- `ProcessError::PermissionDenied` - Insufficient permissions

### Previous Story Learnings

From Story 2.1 (File System Watcher Setup):
1. **CancellationToken**: Use for graceful shutdown coordination
2. **tokio::select!**: Use for concurrent operation handling
3. **Error resilience**: Continue monitoring on transient failures

From Story 2.2 (Session File Parser):
1. **MonitorEvent integration**: Extend event types for new detection
2. **Channel integration**: Events flow through mpsc channel

### Platform Considerations

**Linux:**
- Use `/proc` filesystem for process enumeration
- Read `/proc/{pid}/cmdline` for command line
- Read `/proc/{pid}/cwd` for working directory
- Efficient kernel-provided interface

**macOS:**
- Use `pgrep` and `ps` commands (simpler than `sysctl`)
- Consider `libproc` bindings for better performance later
- `kqueue` with `EVFILT_PROC` for event-based detection (future optimization)

### Alternative Approaches

**File-based detection (simpler):**
Instead of process detection, could watch for specific files opencode creates:
- `~/.opencode/lock` file creation/deletion
- Session file modifications
- This may be more reliable than process scanning

**Decision:** Start with process detection for explicit semantics, but consider file-based approach if process detection proves unreliable.

### Performance Considerations

- **NFR1: <5s detection latency** - 1s polling interval ensures detection within 1s
- **NFR5: <1% CPU idle** - Polling every 1s is minimal overhead
- Could optimize with `kqueue`/`inotify` on process events later

### Testing Strategy

**Unit Tests:**
- Mock process enumeration for controlled testing
- Test new process detection
- Test stopped process detection
- Test multiple process handling

**Integration Tests:**
- Spawn actual process and verify detection
- Kill process and verify stop event
- Test with CancellationToken

### References

- [Source: _bmad-output/planning-artifacts/architecture.md#Project Structure & Boundaries]
- [Source: _bmad-output/planning-artifacts/architecture.md#Internal Communication]
- [Source: _bmad-output/planning-artifacts/epics.md#Story 2.3: Process Detection]
- [Source: _bmad-output/implementation-artifacts/2-1-file-system-watcher-setup.md]
- [Source: _bmad-output/implementation-artifacts/2-2-session-file-parser-frontmatter-extraction.md]

## File List

**Files to create:**
- `src/monitor/process.rs`
- `tests/process_detector_test.rs`

**Files to modify:**
- `src/monitor/mod.rs` (export new modules)
- `src/monitor/events.rs` (add ProcessStarted/ProcessStopped variants)
- `_bmad-output/implementation-artifacts/sprint-status.yaml`

## Change Log

- 2026-02-05: Story created and marked ready-for-dev
