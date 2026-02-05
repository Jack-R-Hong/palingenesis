# Story 3.1: Resume Strategy Trait

Status: ready-for-dev

## Story

As a developer,
I want a common trait for resume strategies,
So that different resume approaches are interchangeable.

## Acceptance Criteria

**AC1: ResumeStrategy Trait Definition**
**Given** the ResumeStrategy trait definition
**When** implemented
**Then** it has an async `execute` method
**And** the method takes session context and returns Result

**AC2: Rate Limit Strategy Selection**
**Given** a rate limit stop
**When** the strategy selector chooses
**Then** it selects `SameSessionStrategy`

**AC3: Context Exhaustion Strategy Selection**
**Given** a context exhaustion stop
**When** the strategy selector chooses
**Then** it selects `NewSessionStrategy`

**AC4: ResumeContext Structure**
**Given** a resume operation is triggered
**When** ResumeContext is constructed
**Then** it contains session path, stop reason, retry info, and session metadata

**AC5: ResumeOutcome Structure**
**Given** a resume strategy executes
**When** it completes
**Then** ResumeOutcome indicates success/failure, action taken, and next steps

**AC6: Strategy Factory Pattern**
**Given** a StopReason from the classifier
**When** the factory creates a strategy
**Then** it returns the appropriate strategy implementation

## Tasks / Subtasks

- [ ] Create resume module structure (AC: 1, 4, 5)
  - [ ] Create `src/resume/mod.rs` with module exports
  - [ ] Create `src/resume/strategy.rs` with ResumeStrategy trait
  - [ ] Create `src/resume/context.rs` with ResumeContext struct
  - [ ] Create `src/resume/outcome.rs` with ResumeOutcome struct

- [ ] Define ResumeContext struct (AC: 4)
  - [ ] Add session_path: PathBuf field
  - [ ] Add stop_reason: StopReason field (from classifier)
  - [ ] Add retry_after: Option<Duration> field
  - [ ] Add session_metadata: Option<Session> field
  - [ ] Add attempt_number: u32 field
  - [ ] Add timestamp: DateTime<Utc> field

- [ ] Define ResumeOutcome enum (AC: 5)
  - [ ] Define `Success` variant with session continuation info
  - [ ] Define `Failure` variant with error details
  - [ ] Define `Skipped` variant with reason (e.g., user exit)
  - [ ] Define `Delayed` variant with next attempt time

- [ ] Define ResumeStrategy trait (AC: 1)
  - [ ] Define `async fn execute(&self, ctx: &ResumeContext) -> Result<ResumeOutcome, ResumeError>`
  - [ ] Define `fn name(&self) -> &'static str` for logging
  - [ ] Define `fn should_retry(&self, outcome: &ResumeOutcome) -> bool`
  - [ ] Use async_trait for async trait methods

- [ ] Implement StrategySelector (AC: 2, 3, 6)
  - [ ] Create `src/resume/selector.rs`
  - [ ] Implement `select(&self, reason: &StopReason) -> Box<dyn ResumeStrategy>`
  - [ ] Map RateLimit -> SameSessionStrategy
  - [ ] Map ContextExhausted -> NewSessionStrategy
  - [ ] Map UserExit -> no strategy (skip resume)
  - [ ] Map Completed -> no strategy (skip resume)
  - [ ] Map Unknown -> configurable default

- [ ] Define ResumeError type (AC: 1, 5)
  - [ ] Create `src/resume/error.rs` with thiserror
  - [ ] Define error variants: Io, SessionNotFound, CommandFailed, Timeout
  - [ ] Include context information in errors

- [ ] Add unit tests (AC: 1, 2, 3, 4, 5, 6)
  - [ ] Test ResumeContext construction
  - [ ] Test ResumeOutcome variants
  - [ ] Test strategy selection for RateLimit
  - [ ] Test strategy selection for ContextExhausted
  - [ ] Test strategy selection for UserExit
  - [ ] Test mock strategy implementation

## Dev Notes

### Architecture Requirements

**From architecture.md - Project Structure:**

```
src/resume/
    mod.rs                    # Resume module root (THIS STORY)
    strategy.rs               # ResumeStrategy trait (THIS STORY)
    context.rs                # ResumeContext struct (THIS STORY)
    outcome.rs                # ResumeOutcome struct (THIS STORY)
    selector.rs               # Strategy selector (THIS STORY)
    same_session.rs           # SameSessionStrategy (Story 3.2)
    new_session.rs            # NewSessionStrategy (Story 3.3)
    backoff.rs                # Exponential backoff (Story 3.5)
    error.rs                  # ResumeError type (THIS STORY)
```

**From architecture.md - Integration Points:**

> | From | To | Mechanism |
> |------|-----|-----------|
> | Monitor -> Daemon | `MonitorEvent` channel | SessionStopped triggers resume |
> | Daemon -> Resume | Strategy selection | Based on StopReason |

**Implements:** FR8 (auto-resume rate limit), FR9 (new session), foundation for Epic 3

### Technical Implementation

**ResumeContext:**

```rust
// src/resume/context.rs
use std::path::PathBuf;
use std::time::Duration;
use chrono::{DateTime, Utc};

use crate::monitor::classifier::StopReason;
use crate::monitor::session::Session;

/// Context provided to resume strategies.
#[derive(Debug, Clone)]
pub struct ResumeContext {
    /// Path to the session file
    pub session_path: PathBuf,
    /// Classified stop reason
    pub stop_reason: StopReason,
    /// Retry-After duration from rate limit response
    pub retry_after: Option<Duration>,
    /// Parsed session metadata
    pub session_metadata: Option<Session>,
    /// Current attempt number (1-indexed)
    pub attempt_number: u32,
    /// When the stop was detected
    pub timestamp: DateTime<Utc>,
}

impl ResumeContext {
    pub fn new(session_path: PathBuf, stop_reason: StopReason) -> Self {
        Self {
            session_path,
            stop_reason,
            retry_after: None,
            session_metadata: None,
            attempt_number: 1,
            timestamp: Utc::now(),
        }
    }
    
    pub fn with_retry_after(mut self, duration: Duration) -> Self {
        self.retry_after = Some(duration);
        self
    }
    
    pub fn with_session(mut self, session: Session) -> Self {
        self.session_metadata = Some(session);
        self
    }
    
    pub fn increment_attempt(&mut self) {
        self.attempt_number += 1;
    }
}
```

**ResumeOutcome:**

```rust
// src/resume/outcome.rs
use std::time::Duration;
use std::path::PathBuf;

/// Outcome of a resume strategy execution.
#[derive(Debug, Clone)]
pub enum ResumeOutcome {
    /// Resume succeeded
    Success {
        /// Session that was resumed/created
        session_path: PathBuf,
        /// Description of action taken
        action: String,
    },
    /// Resume failed
    Failure {
        /// Error message
        message: String,
        /// Whether retry is possible
        retryable: bool,
    },
    /// Resume skipped intentionally
    Skipped {
        /// Reason for skipping
        reason: String,
    },
    /// Resume delayed for later
    Delayed {
        /// When to retry
        next_attempt: Duration,
        /// Reason for delay
        reason: String,
    },
}

impl ResumeOutcome {
    pub fn success(session_path: PathBuf, action: impl Into<String>) -> Self {
        Self::Success {
            session_path,
            action: action.into(),
        }
    }
    
    pub fn failure(message: impl Into<String>, retryable: bool) -> Self {
        Self::Failure {
            message: message.into(),
            retryable,
        }
    }
    
    pub fn skipped(reason: impl Into<String>) -> Self {
        Self::Skipped {
            reason: reason.into(),
        }
    }
    
    pub fn delayed(next_attempt: Duration, reason: impl Into<String>) -> Self {
        Self::Delayed {
            next_attempt,
            reason: reason.into(),
        }
    }
    
    pub fn is_success(&self) -> bool {
        matches!(self, Self::Success { .. })
    }
    
    pub fn should_retry(&self) -> bool {
        matches!(self, Self::Failure { retryable: true, .. } | Self::Delayed { .. })
    }
}
```

**ResumeStrategy Trait:**

```rust
// src/resume/strategy.rs
use async_trait::async_trait;

use crate::resume::context::ResumeContext;
use crate::resume::outcome::ResumeOutcome;
use crate::resume::error::ResumeError;

/// Trait for resume strategy implementations.
#[async_trait]
pub trait ResumeStrategy: Send + Sync {
    /// Execute the resume strategy.
    async fn execute(&self, ctx: &ResumeContext) -> Result<ResumeOutcome, ResumeError>;
    
    /// Name of the strategy for logging.
    fn name(&self) -> &'static str;
    
    /// Check if retry should be attempted after this outcome.
    fn should_retry(&self, outcome: &ResumeOutcome) -> bool {
        outcome.should_retry()
    }
}
```

**StrategySelector:**

```rust
// src/resume/selector.rs
use std::sync::Arc;

use crate::monitor::classifier::StopReason;
use crate::resume::strategy::ResumeStrategy;
use crate::resume::same_session::SameSessionStrategy;
use crate::resume::new_session::NewSessionStrategy;

/// Selects the appropriate resume strategy based on stop reason.
pub struct StrategySelector {
    same_session: Arc<SameSessionStrategy>,
    new_session: Arc<NewSessionStrategy>,
}

impl StrategySelector {
    pub fn new() -> Self {
        Self {
            same_session: Arc::new(SameSessionStrategy::new()),
            new_session: Arc::new(NewSessionStrategy::new()),
        }
    }
    
    /// Select strategy based on stop reason.
    /// Returns None if no resume should occur (user exit, completed).
    pub fn select(&self, reason: &StopReason) -> Option<Arc<dyn ResumeStrategy>> {
        match reason {
            StopReason::RateLimit { .. } => Some(self.same_session.clone()),
            StopReason::ContextExhausted => Some(self.new_session.clone()),
            StopReason::UserExit => None, // Respect user intent
            StopReason::Completed => None, // Task done
            StopReason::Unknown => {
                // Configurable: could default to same_session
                tracing::warn!("Unknown stop reason, skipping resume");
                None
            }
        }
    }
}

impl Default for StrategySelector {
    fn default() -> Self {
        Self::new()
    }
}
```

### Dependencies

Uses existing dependencies:
- `async-trait = "0.1"` (add to Cargo.toml) - async trait methods
- `chrono = "0.4"` (already in Cargo.toml) - timestamps
- `thiserror` (already in Cargo.toml) - error types
- `tracing` (already in Cargo.toml) - structured logging

### Integration with Epic 2

**StopReason from classifier (Story 2.4-2.6):**

```rust
// From src/monitor/classifier.rs
pub enum StopReason {
    RateLimit { retry_after: Option<Duration> },
    ContextExhausted,
    UserExit,
    Completed,
    Unknown,
}
```

**MonitorEvent from Story 2.7:**

```rust
// Daemon receives SessionStopped event and routes to resume
MonitorEvent::SessionStopped { reason, session, .. } => {
    if let Some(strategy) = selector.select(&reason) {
        let ctx = ResumeContext::new(session.path.clone(), reason);
        let outcome = strategy.execute(&ctx).await?;
        // Handle outcome
    }
}
```

### Error Handling

```rust
// src/resume/error.rs
use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ResumeError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("Session not found: {path}")]
    SessionNotFound { path: PathBuf },
    
    #[error("Command execution failed: {command}")]
    CommandFailed { command: String, stderr: String },
    
    #[error("Operation timed out after {duration:?}")]
    Timeout { duration: std::time::Duration },
    
    #[error("Configuration error: {0}")]
    Config(String),
    
    #[error("Retry limit exceeded after {attempts} attempts")]
    RetryExceeded { attempts: u32 },
}
```

### Testing Strategy

**Unit Tests:**
- Test ResumeContext construction and builders
- Test ResumeOutcome variants and predicates
- Test StrategySelector routing for each StopReason
- Mock strategy for trait testing

**Integration Tests:**
- Test strategy selection with real classifier output
- Test context flow through strategy execution

### References

- [Source: _bmad-output/planning-artifacts/architecture.md#Project Structure]
- [Source: _bmad-output/planning-artifacts/architecture.md#Integration Points]
- [Source: _bmad-output/planning-artifacts/epics.md#Story 3.1: Resume Strategy Trait]
- [Source: _bmad-output/implementation-artifacts/2-4-stop-reason-classification-rate-limit.md]
- [Source: _bmad-output/implementation-artifacts/2-7-monitor-event-channel.md]

## File List

**Files to create:**
- `src/resume/mod.rs`
- `src/resume/strategy.rs`
- `src/resume/context.rs`
- `src/resume/outcome.rs`
- `src/resume/selector.rs`
- `src/resume/error.rs`
- `tests/resume_strategy_test.rs`

**Files to modify:**
- `Cargo.toml` (add async-trait if not present)
- `src/lib.rs` (add resume module)
- `_bmad-output/implementation-artifacts/sprint-status.yaml`

## Change Log

- 2026-02-05: Story created and marked ready-for-dev
