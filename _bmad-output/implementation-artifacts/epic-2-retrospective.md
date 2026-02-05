# Epic 2 Retrospective: Session Detection & Classification

**Date:** 2026-02-05  
**Epic:** 2 - Session Detection & Classification  
**Stories Completed:** 7/7 (2.1 - 2.7)  
**Status:** DONE

---

## Executive Summary

Epic 2 successfully delivered the core monitoring infrastructure for detecting AI assistant sessions and classifying stop reasons. The implementation established the `src/monitor/` module with 2,016 lines of well-tested Rust code implementing file watching, session parsing, process detection, and stop reason classification.

**Key Metrics:**
- Lines of Code: ~2,016 (8 source files in `src/monitor/`)
- Test Coverage: 7 unit tests passing (classifier and process modules)
- Crate Additions: notify-debouncer-full, regex (reused existing deps)
- Stories: 7 completed - watcher, parser, process detection, 4 classification types

---

## What Was Delivered

### Core Capabilities

1. **File System Watcher** (Story 2.1)
   - `SessionWatcher` with `notify-debouncer-full` crate (100ms debounce)
   - Watches `~/.opencode/` directory recursively
   - Waits for directory creation if not present
   - Retry logic (3 attempts) for watch setup failures
   - Events: `FileCreated`, `FileModified`, `FileDeleted`, `DirectoryCreated`

2. **Session File Parser** (Story 2.2)
   - YAML frontmatter extraction from markdown files
   - Efficient streaming read (stops after closing `---`)
   - `Session` and `SessionState` models with serde
   - `SessionParser` maintains state for change detection
   - Supports `stepsCompleted`, `lastStep`, `status` fields

3. **Process Detection** (Story 2.3)
   - `ProcessMonitor` polling `/proc` filesystem (Linux)
   - Detects `opencode` processes by command line matching
   - Tracks process start/stop events with exit codes
   - Working directory extraction via `/proc/{pid}/cwd`
   - Mockable via `ProcessEnumerator` trait

4. **Stop Reason Classification** (Stories 2.4-2.6)
   - **Rate Limit** (2.4): Detects 429, "rate limit", "too many requests", "throttle"
     - Extracts `Retry-After` from headers, JSON body, or text patterns
   - **Context Exhaustion** (2.5): Detects "context length exceeded", "token limit"
     - Extracts token usage (e.g., "used 150000 of 200000 tokens")
     - Model-aware context size inference (Claude, GPT-4)
   - **User Exit** (2.6): Detects Ctrl+C (130), SIGTERM (143), SIGHUP (129)
     - Pattern matching for "exit", "quit", "/bye" commands
   - **Completed**: Checks session frontmatter status

5. **Monitor Event Channel** (Story 2.7)
   - `Monitor` core orchestration with unified event loop
   - Merges file watcher and process detector streams
   - `MonitorEvent` enum: `SessionChanged`, `ProcessStarted`, `ProcessStopped`, `SessionStopped`
   - `try_send` with backpressure handling and dropped event tracking
   - Configurable via `MonitorConfig`

---

## What Went Well

### 1. Pattern Reuse from Epic 1

**CancellationToken Pattern**
Successfully applied the shutdown coordination pattern from Epic 1:
```rust
pub async fn run(
    &self,
    cancel: CancellationToken,
) -> Result<WatchEventReceiver, WatcherError> {
    tokio::spawn(async move {
        // Task respects cancellation
        tokio::select! {
            _ = cancel.cancelled() => break,
            // ...
        }
    });
}
```

**Trait-Based Abstraction**
Consistent with Epic 1, introduced traits for testability:
- `WatcherStateAccess` - watcher configuration
- `ProcessStateAccess` - process monitor configuration
- `ProcessEnumerator` - mockable process enumeration

### 2. Decoupled Architecture

The monitor module is fully decoupled from the daemon:
```
WatchEvent -> SessionParser -> MonitorEvent
ProcessEvent -----------------> MonitorEvent
```

Components communicate via typed channels (`mpsc::Sender`/`Receiver`), enabling:
- Independent testing of each subsystem
- Future flexibility (e.g., different event consumers)
- Clear data flow and ownership

### 3. Robust Classification Logic

The `StopReasonClassifier` handles edge cases well:
- Priority ordering: Rate Limit > Completed > Context > User Exit > Unknown
- Confidence scoring with evidence accumulation
- Extensible via `extra_rate_limit_patterns` and `extra_context_patterns`
- Safe defaults when parsing fails

### 4. Linux-Specific Optimization

Process detection uses `/proc` directly instead of external commands:
```rust
for entry in fs::read_dir("/proc")? {
    let cmdline = fs::read(&path.join("cmdline"))?;
    // Parse null-separated command line
}
```

Benefits:
- No subprocess spawning overhead
- Direct access to process metadata
- Graceful handling of permission errors

### 5. Test Infrastructure

Mock implementations enable thorough testing:
```rust
struct MockEnumerator {
    sequences: Mutex<VecDeque<Result<Vec<ProcessInfo>, ProcessError>>>,
    exit_codes: Mutex<HashMap<u32, i32>>,
}
```

Tests cover:
- Existing process detection on startup
- Individual process stop detection with exit codes
- Recovery from enumeration errors
- Clean shutdown after cancellation

---

## What Could Be Improved

### 1. macOS Support

Process detection is Linux-only (`#[cfg(target_os = "linux")]`):
```rust
#[cfg(not(target_os = "linux"))]
fn enumerate_opencode_processes() -> Result<Vec<ProcessInfo>, ProcessError> {
    Err(ProcessError::EnumerationFailed("not supported on this platform"))
}
```

**Recommendation for Epic 3:**
- Add macOS support via `sysctl` or `ps` command
- Consider `sysinfo` crate for cross-platform process enumeration

### 2. Session File Location Discovery

Currently hardcoded to `~/.opencode/`:
```rust
const DEFAULT_SESSION_DIR: &str = ".opencode";
```

**Recommendation:**
- Auto-detect session directory from opencode config
- Support multiple watched directories

### 3. Classifier Integration Tests

Unit tests exist, but integration tests with real session files are limited.

**Recommendation:**
- Add fixture files with representative frontmatter
- Test classifier against real opencode output samples

### 4. Event Channel Backpressure

`try_send` drops events on full channel:
```rust
Err(mpsc::error::TrySendError::Full(event)) => {
    self.dropped_events += 1;
    warn!("dropping event");
}
```

**Recommendation for Epic 3:**
- Consider bounded buffer with oldest-drop policy
- Expose `dropped_events` metric via status endpoint

---

## Technical Debt Identified

### Low Priority (Track for Later)

1. **macOS process enumeration** - Stubbed but not implemented
2. **Integration with daemon state** - Monitor runs but doesn't update daemon state yet
3. **Health check interval** - Defined but not used in event loop
4. **File watcher RAII** - `RunningGuard` works but debouncer cleanup could be more explicit

### No Immediate Action Required

- These items don't block Epic 3 (Automatic Session Resumption)
- Process detection works on Linux, primary development platform

---

## Patterns Established for Future Epics

### 1. Event Sourcing via mpsc Channels

```rust
pub type MonitorEventSender = mpsc::Sender<MonitorEvent>;
pub type MonitorEventReceiver = mpsc::Receiver<MonitorEvent>;
```

All async components should emit events through typed channels.

### 2. Configurable Subsystems

```rust
#[derive(Debug, Clone)]
pub struct MonitorConfig {
    pub session_dir: PathBuf,
    pub channel_capacity: usize,
    pub classifier_config: ClassifierConfig,
    pub enable_process_detection: bool,
}
```

Group configuration into per-subsystem structs with sensible defaults.

### 3. Classification with Confidence

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct ClassificationResult {
    pub reason: StopReason,
    pub confidence: f32,
    pub evidence: Vec<String>,
}
```

When making decisions from heuristics, track confidence and evidence for debugging.

### 4. Retry Logic Pattern

```rust
async fn watch_with_retry<W: Watcher>(
    watcher: &mut W,
    path: &Path,
    mode: RecursiveMode,
) -> Result<(), WatcherError> {
    for attempt in 0..=WATCH_RETRY_ATTEMPTS {
        match watcher.watch(path, mode) {
            Ok(()) => return Ok(()),
            Err(err) => tokio::time::sleep(RETRY_DELAY).await,
        }
    }
    Err(last_error)
}
```

Wrap fallible operations in retry loops with exponential backoff consideration.

---

## Impact on Epic 3

### Ready for Use

- `MonitorEvent::SessionStopped` with classified `StopReason`
- `should_auto_resume()` method on `StopReason`
- `RateLimitInfo.retry_after` duration for wait timing
- `ContextExhaustionInfo` for new-session-required detection

### New Work Required

Epic 3 (Automatic Session Resumption) will need:
- `src/resume/strategy.rs` - Resume strategy trait and implementations
- `src/resume/same_session.rs` - Continue existing session
- `src/resume/new_session.rs` - Start new session from Next-step.md
- `src/resume/backoff.rs` - Exponential backoff with jitter
- Integration: Subscribe to `MonitorEventReceiver`, act on `SessionStopped`

### Architecture Notes

The resume system should:
1. Subscribe to `MonitorEventReceiver` in daemon core
2. On `SessionStopped { reason, .. }`:
   - Check `reason.should_auto_resume()`
   - Select strategy based on reason type
   - Execute resume with appropriate waiting

---

## Lessons Learned

1. **notify-debouncer-full > raw notify** - Debouncing is essential; file systems emit many events per write
2. **Trait bounds enable mocking** - `ProcessEnumerator` made process tests deterministic
3. **Exit codes are goldmine** - SIGINT (130) vs SIGTERM (143) tells user intent
4. **Confidence > binary** - Classification isn't black/white; track certainty
5. **Event channels decouple** - Monitor doesn't need to know about resume; just emits events

---

## Conclusion

Epic 2 delivered a robust session detection and classification system. The `src/monitor/` module provides:
- Real-time file watching with debouncing
- Session state parsing from YAML frontmatter
- Process lifecycle detection on Linux
- Intelligent stop reason classification with confidence scoring
- Unified event stream for downstream consumers

The architecture cleanly separates concerns and follows patterns established in Epic 1. The system is ready for Epic 3 to implement automatic session resumption.

**Epic 2 Status: COMPLETE**

**Recommended Next Action:** Begin Epic 3, Story 3.1 (Resume Strategy Trait)
