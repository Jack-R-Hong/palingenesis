# Story 2.1: File System Watcher Setup

Status: ready-for-dev

## Story

As a daemon,
I want to watch the opencode session directory for changes,
So that I can detect session file modifications in real-time.

## Acceptance Criteria

**AC1: File Watcher Initialization**
**Given** the daemon starts monitoring
**When** it initializes the file watcher
**Then** it watches `~/.opencode/` directory recursively
**And** the watcher uses inotify on Linux, FSEvents on macOS

**AC2: Missing Session Directory Handling**
**Given** the session directory doesn't exist
**When** the daemon starts
**Then** it logs a warning and waits for the directory to appear
**And** starts watching once the directory is created

**AC3: File Change Event Emission**
**Given** a file is modified in the session directory
**When** the watcher detects the change
**Then** it emits a `FileChanged` event with the file path
**And** event is sent via channel to the monitor

**AC4: Event Debouncing**
**Given** high-frequency file changes occur
**When** the watcher receives events
**Then** it debounces events (100ms window)
**And** processes only the latest state

**AC5: Graceful Shutdown Integration**
**Given** the daemon is shutting down
**When** CancellationToken is triggered
**Then** the file watcher stops cleanly
**And** no events are emitted after cancellation

**AC6: Error Recovery**
**Given** a transient file system error occurs
**When** the watcher encounters it
**Then** it logs the error and continues watching
**And** does not crash the daemon

## Tasks / Subtasks

- [ ] Create monitor module structure (AC: 1, 5, 6)
  - [ ] Create `src/monitor/watcher.rs` with SessionWatcher struct
  - [ ] Create `src/monitor/events.rs` with WatchEvent types
  - [ ] Update `src/monitor/mod.rs` to export modules

- [ ] Define watcher event types (AC: 3)
  - [ ] Define `WatchEvent` enum (FileCreated, FileModified, FileDeleted)
  - [ ] Define `WatchError` enum with thiserror
  - [ ] Implement event channel sender type alias

- [ ] Implement SessionWatcher struct (AC: 1, 2)
  - [ ] Define `WatcherError` enum (Io, NotifyError, DirectoryNotFound, AlreadyRunning)
  - [ ] Implement `SessionWatcher::new()` with configurable session path
  - [ ] Implement default path resolution (`~/.opencode/`)
  - [ ] Implement `SessionWatcher::with_path()` for custom paths (testing)

- [ ] Implement directory existence handling (AC: 2)
  - [ ] Check if session directory exists on startup
  - [ ] Log warning if directory doesn't exist
  - [ ] Implement directory creation watcher (watch parent directory)
  - [ ] Start recursive watch once directory appears

- [ ] Implement file watcher core (AC: 1, 3)
  - [ ] Use `notify::RecommendedWatcher` for cross-platform support
  - [ ] Configure recursive watching mode
  - [ ] Map notify events to WatchEvent types
  - [ ] Send events via `tokio::sync::mpsc` channel

- [ ] Implement event debouncing (AC: 4)
  - [ ] Create debounce buffer with 100ms window
  - [ ] Use `tokio::time::interval` for debounce timing
  - [ ] Coalesce multiple events for same file path
  - [ ] Emit only latest state after debounce window

- [ ] Implement graceful shutdown (AC: 5)
  - [ ] Accept `CancellationToken` in `SessionWatcher::run()`
  - [ ] Use `tokio::select!` for cancellation handling
  - [ ] Drop notify watcher cleanly on cancellation
  - [ ] Flush pending debounced events on shutdown

- [ ] Implement error handling (AC: 6)
  - [ ] Handle transient I/O errors gracefully
  - [ ] Log errors without crashing
  - [ ] Implement retry logic for recoverable errors
  - [ ] Report fatal errors via channel

- [ ] Integrate with daemon state access pattern (AC: 1, 3)
  - [ ] Follow `DaemonStateAccess` trait pattern from Story 1.6
  - [ ] Create `WatcherStateAccess` trait for watcher configuration
  - [ ] Return event receiver channel for monitor integration

- [ ] Add unit tests (AC: 1, 2, 3, 4, 5, 6)
  - [ ] Test watcher creation with valid directory
  - [ ] Test watcher behavior with missing directory
  - [ ] Test file change event emission
  - [ ] Test event debouncing (multiple rapid changes)
  - [ ] Test graceful shutdown with CancellationToken
  - [ ] Test error recovery on transient failures

- [ ] Add integration tests
  - [ ] Test full watcher lifecycle (start, watch, stop)
  - [ ] Test recursive directory watching
  - [ ] Test watching for new directory creation
  - [ ] Test high-frequency file change scenarios

## Dev Notes

### Architecture Requirements

**From architecture.md - Project Structure:**

```
src/monitor/
    mod.rs                    # Monitor module root
    watcher.rs                # File system watcher (notify)
    session.rs                # Session file parsing (Story 2.2)
    frontmatter.rs            # YAML frontmatter extraction (Story 2.2)
    classifier.rs             # Stop reason classification (Story 2.4-2.6)
    error.rs                  # MonitorError type
```

**From architecture.md - Verified Dependencies:**

```
| notify | 8.2.0 | File system watching |
```

**From architecture.md - Async Patterns:**

> **Channel Selection:**
> | Use Case | Channel Type |
> |----------|--------------|
> | One producer, one consumer | `tokio::sync::mpsc` |
> | Broadcast (shutdown signal) | `tokio::sync::broadcast` |
>
> **Graceful Shutdown:** Use `CancellationToken` for coordinated shutdown.

**From architecture.md - Internal Communication:**

> | From | To | Mechanism |
> |------|-----|-----------|
> | Monitor -> Daemon | `tokio::sync::mpsc` | `MonitorEvent` channel |

**Implements:** NFR1 (<5s detection latency), NFR5 (<1% CPU idle)

### Technical Implementation

**WatchEvent Types:**

```rust
// src/monitor/events.rs
use std::path::PathBuf;

/// Events emitted by the file system watcher.
#[derive(Debug, Clone, PartialEq)]
pub enum WatchEvent {
    /// File was created in the session directory
    FileCreated(PathBuf),
    /// File was modified in the session directory
    FileModified(PathBuf),
    /// File was deleted from the session directory
    FileDeleted(PathBuf),
    /// Directory was created (for session directory appearance)
    DirectoryCreated(PathBuf),
    /// Watcher encountered an error
    Error(String),
}
```

**SessionWatcher Implementation:**

```rust
// src/monitor/watcher.rs
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use notify::{RecommendedWatcher, RecursiveMode, Watcher, Config as NotifyConfig};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, warn};

use crate::monitor::events::WatchEvent;

const DEBOUNCE_DURATION_MS: u64 = 100;
const DEFAULT_SESSION_PATH: &str = ".opencode";

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

pub struct SessionWatcher {
    session_dir: PathBuf,
    debounce_ms: u64,
}

impl SessionWatcher {
    /// Create a new SessionWatcher with default session directory (~/.opencode/).
    pub fn new() -> Self {
        let home = dirs::home_dir().expect("Could not determine home directory");
        Self {
            session_dir: home.join(DEFAULT_SESSION_PATH),
            debounce_ms: DEBOUNCE_DURATION_MS,
        }
    }
    
    /// Create with custom session directory (for testing).
    pub fn with_path(path: PathBuf) -> Self {
        Self {
            session_dir: path,
            debounce_ms: DEBOUNCE_DURATION_MS,
        }
    }
    
    /// Set custom debounce duration.
    pub fn with_debounce(mut self, duration_ms: u64) -> Self {
        self.debounce_ms = duration_ms;
        self
    }
    
    /// Returns the session directory path being watched.
    pub fn session_dir(&self) -> &Path {
        &self.session_dir
    }
    
    /// Run the file watcher, returning a receiver for watch events.
    /// 
    /// This method handles:
    /// - Waiting for session directory to appear if missing
    /// - Starting recursive file watching
    /// - Debouncing rapid file changes
    /// - Graceful shutdown on cancellation
    pub async fn run(
        &self,
        cancel: CancellationToken,
    ) -> Result<mpsc::Receiver<WatchEvent>, WatcherError> {
        let (tx, rx) = mpsc::channel(100);
        
        // Check if session directory exists
        if !self.session_dir.exists() {
            warn!(
                path = %self.session_dir.display(),
                "Session directory does not exist, waiting for creation"
            );
            // Watch parent directory for session dir creation
            self.watch_for_directory_creation(tx.clone(), cancel.clone()).await?;
        }
        
        // Start watching the session directory
        self.start_watching(tx, cancel).await?;
        
        Ok(rx)
    }
    
    async fn watch_for_directory_creation(
        &self,
        tx: mpsc::Sender<WatchEvent>,
        cancel: CancellationToken,
    ) -> Result<(), WatcherError> {
        // Implementation: Watch parent dir until session_dir appears
        // ...
        Ok(())
    }
    
    async fn start_watching(
        &self,
        tx: mpsc::Sender<WatchEvent>,
        cancel: CancellationToken,
    ) -> Result<(), WatcherError> {
        let session_dir = self.session_dir.clone();
        let debounce_ms = self.debounce_ms;
        
        // Create notify watcher with async event handler
        let (notify_tx, mut notify_rx) = mpsc::channel(100);
        
        let mut watcher = RecommendedWatcher::new(
            move |res: Result<notify::Event, notify::Error>| {
                let _ = notify_tx.blocking_send(res);
            },
            NotifyConfig::default(),
        )?;
        
        watcher.watch(&session_dir, RecursiveMode::Recursive)?;
        info!(path = %session_dir.display(), "Started watching session directory");
        
        // Debouncing and event processing loop
        tokio::spawn(async move {
            let mut debounce_buffer = std::collections::HashMap::new();
            let mut debounce_timer = tokio::time::interval(Duration::from_millis(debounce_ms));
            
            loop {
                tokio::select! {
                    _ = cancel.cancelled() => {
                        info!("File watcher shutting down");
                        break;
                    }
                    Some(result) = notify_rx.recv() => {
                        match result {
                            Ok(event) => {
                                // Add to debounce buffer
                                for path in event.paths {
                                    debounce_buffer.insert(path.clone(), event.kind);
                                }
                            }
                            Err(e) => {
                                error!(error = %e, "File watcher error");
                                let _ = tx.send(WatchEvent::Error(e.to_string())).await;
                            }
                        }
                    }
                    _ = debounce_timer.tick() => {
                        // Emit debounced events
                        for (path, kind) in debounce_buffer.drain() {
                            let event = match kind {
                                notify::EventKind::Create(_) => WatchEvent::FileCreated(path),
                                notify::EventKind::Modify(_) => WatchEvent::FileModified(path),
                                notify::EventKind::Remove(_) => WatchEvent::FileDeleted(path),
                                _ => continue,
                            };
                            if tx.send(event).await.is_err() {
                                break;
                            }
                        }
                    }
                }
            }
        });
        
        Ok(())
    }
}

impl Default for SessionWatcher {
    fn default() -> Self {
        Self::new()
    }
}
```

### Dependencies

Uses existing dependencies:
- `notify = "8.2.0"` (already in Cargo.toml) - cross-platform file watching
- `tokio` (already in Cargo.toml) - async runtime, channels
- `tokio-util` (already in Cargo.toml) - CancellationToken
- `tracing` (already in Cargo.toml) - structured logging
- `thiserror` (already in Cargo.toml) - error types
- `dirs` (already in Cargo.toml) - home directory resolution

### Error Handling Pattern

Uses `thiserror` following project conventions from architecture.md:
- `WatcherError::Io` - File system operations failed
- `WatcherError::Notify` - notify crate errors
- `WatcherError::DirectoryNotFound` - Session directory doesn't exist
- `WatcherError::AlreadyRunning` - Watcher already started

### Previous Story Learnings

From Story 1.6 (Unix Socket IPC Server):
1. **Trait pattern**: Use `DaemonStateAccess` pattern for shared state access
2. **CancellationToken**: Use for graceful shutdown coordination
3. **tokio::select!**: Use for concurrent operation handling
4. **Error types**: Use thiserror for domain errors
5. **Structured logging**: Use tracing macros with fields

From Story 1.3 (Platform-Specific Path Resolution):
1. **Home directory**: Use `dirs::home_dir()` for platform-agnostic home
2. **Path resolution**: Build paths relative to home directory

From Story 1.4 (State Persistence Layer):
1. **Error recovery**: Handle transient errors gracefully
2. **Graceful degradation**: Continue operation on non-fatal errors

### Platform Considerations

**Linux:**
- Uses `inotify` via notify crate
- Efficient kernel-level notification
- Very low CPU overhead

**macOS:**
- Uses `FSEvents` via notify crate
- Coalesced events may require additional debouncing
- Similar low overhead

### Performance Considerations

- **NFR1: <5s detection latency** - File system events are near-instant; debounce adds 100ms max
- **NFR5: <1% CPU idle** - Event-driven (inotify/FSEvents), not polling
- Debounce buffer prevents event storm during rapid file changes
- mpsc channel with capacity 100 prevents backpressure issues

### Testing Strategy

**Unit Tests:**
- Mock notify events for controlled testing
- Test debounce logic with synthetic rapid events
- Test error handling paths

**Integration Tests:**
- Create temp directory with actual file operations
- Verify events are received for file create/modify/delete
- Test cancellation behavior

### References

- [Source: _bmad-output/planning-artifacts/architecture.md#Project Structure & Boundaries]
- [Source: _bmad-output/planning-artifacts/architecture.md#Async Patterns]
- [Source: _bmad-output/planning-artifacts/architecture.md#Requirements to Module Mapping]
- [Source: _bmad-output/planning-artifacts/epics.md#Story 2.1: File System Watcher Setup]

## File List

**Files to create:**
- `src/monitor/watcher.rs`
- `src/monitor/events.rs`
- `tests/watcher_test.rs`

**Files to modify:**
- `src/monitor/mod.rs` - Export watcher and events modules
- `src/lib.rs` - Ensure monitor module is public if needed
- `_bmad-output/implementation-artifacts/sprint-status.yaml` - Update story status

## Change Log

- 2026-02-05: Story created and marked ready-for-dev
