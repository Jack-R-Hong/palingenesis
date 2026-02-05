# Story 3.2: Same-Session Resume Strategy

Status: ready-for-dev

## Story

As a daemon,
I want to resume the same session after rate limit clears,
So that I preserve context and continue where I left off.

## Acceptance Criteria

**AC1: Retry-After Wait Implementation**
**Given** a rate limit stop with Retry-After: 60s
**When** the same-session strategy executes
**Then** it waits for 60 seconds
**And** then triggers opencode to continue

**AC2: Exponential Backoff Fallback**
**Given** no Retry-After header
**When** the same-session strategy executes
**Then** it uses exponential backoff starting at configured base (default 30s)

**AC3: Resume Trigger**
**Given** the wait period completes
**When** resume is triggered
**Then** it sends the appropriate signal/command to opencode
**And** logs "Resuming session after rate limit"

**AC4: Stats Update**
**Given** resume succeeds
**When** the session continues
**Then** stats.total_resumes is incremented
**And** state is persisted

**AC5: Cancellation Support**
**Given** the daemon is shutting down
**When** waiting for rate limit
**Then** the wait is cancelled gracefully
**And** no resume is attempted

**AC6: Multiple Retry Handling**
**Given** resume fails after first attempt
**When** retry is configured
**Then** backoff increases exponentially
**And** max_retries is respected

## Tasks / Subtasks

- [ ] Create SameSessionStrategy struct (AC: 1, 2, 3)
  - [ ] Create `src/resume/same_session.rs`
  - [ ] Implement ResumeStrategy trait
  - [ ] Add configuration for default wait time
  - [ ] Add configuration for max retries

- [ ] Implement wait logic (AC: 1, 2, 5)
  - [ ] Use Retry-After from context if available
  - [ ] Fall back to exponential backoff (Story 3.5)
  - [ ] Use tokio::time::sleep for waiting
  - [ ] Support cancellation via CancellationToken

- [ ] Implement opencode resume trigger (AC: 3)
  - [ ] Research opencode session continuation mechanism
  - [ ] Implement signal-based continuation if supported
  - [ ] Implement command-based continuation as fallback
  - [ ] Log resume attempt with session details

- [ ] Implement state update (AC: 4)
  - [ ] Increment stats.total_resumes on success
  - [ ] Record resume timestamp
  - [ ] Persist state to disk
  - [ ] Update current_session state

- [ ] Implement retry logic (AC: 6)
  - [ ] Track attempt number in context
  - [ ] Check against max_retries configuration
  - [ ] Return Delayed outcome for retries
  - [ ] Return Failure when max exceeded

- [ ] Add integration with backoff (AC: 2, 6)
  - [ ] Use Backoff struct from Story 3.5
  - [ ] Pass attempt number for backoff calculation
  - [ ] Respect jitter configuration

- [ ] Add unit tests (AC: 1, 2, 3, 4, 5, 6)
  - [ ] Test wait with Retry-After
  - [ ] Test wait with exponential backoff
  - [ ] Test cancellation during wait
  - [ ] Test stats update on success
  - [ ] Test retry limit handling
  - [ ] Test resume trigger mechanism

- [ ] Add integration tests
  - [ ] Test full same-session resume flow
  - [ ] Test with mock opencode process
  - [ ] Test multiple retry scenarios

## Dev Notes

### Architecture Requirements

**From architecture.md - Resume Flow:**

```
Rate Limit Detected
       │
       ▼
┌─────────────────┐
│ SameSession     │
│ Strategy        │
├─────────────────┤
│ 1. Wait for     │
│    Retry-After  │
│ 2. Resume same  │
│    session      │
│ 3. Update stats │
└─────────────────┘
```

**Implements:** FR8 (auto-resume after rate limit)

### Technical Implementation

**SameSessionStrategy:**

```rust
// src/resume/same_session.rs
use std::time::Duration;

use async_trait::async_trait;
use tokio_util::sync::CancellationToken;
use tracing::{info, warn, debug};

use crate::resume::backoff::Backoff;
use crate::resume::context::ResumeContext;
use crate::resume::error::ResumeError;
use crate::resume::outcome::ResumeOutcome;
use crate::resume::strategy::ResumeStrategy;

/// Configuration for same-session resume.
#[derive(Debug, Clone)]
pub struct SameSessionConfig {
    /// Default wait time if no Retry-After (seconds)
    pub default_wait_secs: u64,
    /// Maximum retries before giving up
    pub max_retries: u32,
    /// Base delay for exponential backoff (seconds)
    pub backoff_base_secs: u64,
    /// Maximum backoff delay (seconds)
    pub backoff_max_secs: u64,
    /// Enable jitter in backoff
    pub backoff_jitter: bool,
}

impl Default for SameSessionConfig {
    fn default() -> Self {
        Self {
            default_wait_secs: 30,
            max_retries: 5,
            backoff_base_secs: 30,
            backoff_max_secs: 300,
            backoff_jitter: true,
        }
    }
}

/// Strategy for resuming the same session after rate limit.
pub struct SameSessionStrategy {
    config: SameSessionConfig,
    cancel: Option<CancellationToken>,
    backoff: Backoff,
}

impl SameSessionStrategy {
    pub fn new() -> Self {
        let config = SameSessionConfig::default();
        Self {
            backoff: Backoff::new(
                Duration::from_secs(config.backoff_base_secs),
                Duration::from_secs(config.backoff_max_secs),
            ).with_jitter(config.backoff_jitter),
            config,
            cancel: None,
        }
    }
    
    pub fn with_config(config: SameSessionConfig) -> Self {
        Self {
            backoff: Backoff::new(
                Duration::from_secs(config.backoff_base_secs),
                Duration::from_secs(config.backoff_max_secs),
            ).with_jitter(config.backoff_jitter),
            config,
            cancel: None,
        }
    }
    
    pub fn with_cancellation(mut self, cancel: CancellationToken) -> Self {
        self.cancel = Some(cancel);
        self
    }
    
    /// Calculate wait duration based on context.
    fn calculate_wait(&self, ctx: &ResumeContext) -> Duration {
        // Prefer Retry-After if available
        if let Some(retry_after) = ctx.retry_after {
            return retry_after;
        }
        
        // Fall back to exponential backoff
        self.backoff.delay_for_attempt(ctx.attempt_number)
    }
    
    /// Wait for the calculated duration, respecting cancellation.
    async fn wait(&self, duration: Duration) -> Result<(), ResumeError> {
        debug!(duration_secs = duration.as_secs(), "Waiting before resume");
        
        if let Some(cancel) = &self.cancel {
            tokio::select! {
                _ = cancel.cancelled() => {
                    info!("Wait cancelled by shutdown");
                    return Err(ResumeError::Cancelled);
                }
                _ = tokio::time::sleep(duration) => {
                    // Wait completed normally
                }
            }
        } else {
            tokio::time::sleep(duration).await;
        }
        
        Ok(())
    }
    
    /// Trigger opencode to continue the session.
    async fn trigger_resume(&self, ctx: &ResumeContext) -> Result<(), ResumeError> {
        info!(
            session = %ctx.session_path.display(),
            attempt = ctx.attempt_number,
            "Resuming session after rate limit"
        );
        
        // Option 1: Send continue signal to opencode process
        // Option 2: Execute opencode continue command
        // Option 3: Write to session file to trigger resume
        
        // Implementation depends on opencode's continuation mechanism
        // For now, use command-based approach
        let output = tokio::process::Command::new("opencode")
            .arg("continue")
            .arg("--session")
            .arg(&ctx.session_path)
            .output()
            .await
            .map_err(|e| ResumeError::Io(e))?;
        
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(ResumeError::CommandFailed {
                command: "opencode continue".to_string(),
                stderr: stderr.to_string(),
            });
        }
        
        Ok(())
    }
}

#[async_trait]
impl ResumeStrategy for SameSessionStrategy {
    async fn execute(&self, ctx: &ResumeContext) -> Result<ResumeOutcome, ResumeError> {
        // Check retry limit
        if ctx.attempt_number > self.config.max_retries {
            return Err(ResumeError::RetryExceeded {
                attempts: ctx.attempt_number,
            });
        }
        
        // Calculate and perform wait
        let wait_duration = self.calculate_wait(ctx);
        self.wait(wait_duration).await?;
        
        // Trigger resume
        match self.trigger_resume(ctx).await {
            Ok(_) => {
                Ok(ResumeOutcome::success(
                    ctx.session_path.clone(),
                    "Resumed same session after rate limit",
                ))
            }
            Err(e) => {
                warn!(error = %e, "Resume trigger failed");
                
                // Check if retryable
                let retryable = ctx.attempt_number < self.config.max_retries;
                if retryable {
                    let next_delay = self.backoff.delay_for_attempt(ctx.attempt_number + 1);
                    Ok(ResumeOutcome::delayed(
                        next_delay,
                        format!("Resume failed, will retry: {}", e),
                    ))
                } else {
                    Ok(ResumeOutcome::failure(e.to_string(), false))
                }
            }
        }
    }
    
    fn name(&self) -> &'static str {
        "SameSessionStrategy"
    }
}

impl Default for SameSessionStrategy {
    fn default() -> Self {
        Self::new()
    }
}
```

### Dependencies

Uses existing dependencies:
- `tokio` (already in Cargo.toml) - async runtime, sleep, process
- `tokio-util` (already in Cargo.toml) - CancellationToken
- `async-trait` (from Story 3.1) - async trait methods
- `tracing` (already in Cargo.toml) - structured logging

### Integration with Backoff (Story 3.5)

The SameSessionStrategy uses the Backoff struct from Story 3.5 for exponential backoff calculation when no Retry-After header is available.

### opencode Integration Points

**Research needed:**
1. How does opencode handle session continuation?
2. Is there a CLI command like `opencode continue`?
3. Can we send signals to the opencode process?
4. Does opencode watch for file changes to trigger resume?

**Possible approaches:**
1. **Command-based**: `opencode continue --session <path>`
2. **Signal-based**: Send SIGUSR1 to opencode process
3. **File-based**: Write marker to session file

### Testing Strategy

**Unit Tests:**
- Mock opencode command for resume trigger
- Test wait calculation with/without Retry-After
- Test cancellation during wait
- Test retry logic

**Integration Tests:**
- Test with actual file system
- Test with mock process detection
- Test full resume cycle

### References

- [Source: _bmad-output/planning-artifacts/epics.md#Story 3.2: Same-Session Resume Strategy]
- [Source: _bmad-output/implementation-artifacts/3-1-resume-strategy-trait.md]
- [Source: _bmad-output/implementation-artifacts/2-4-stop-reason-classification-rate-limit.md]

## File List

**Files to create:**
- `src/resume/same_session.rs`
- `tests/same_session_strategy_test.rs`

**Files to modify:**
- `src/resume/mod.rs` (add same_session module)
- `_bmad-output/implementation-artifacts/sprint-status.yaml`

## Change Log

- 2026-02-05: Story created and marked ready-for-dev
