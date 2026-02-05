# Story 3.5: Exponential Backoff Implementation

Status: ready-for-dev

## Story

As a daemon,
I want exponential backoff for retry attempts,
So that I don't overwhelm services and respect rate limits.

## Acceptance Criteria

**AC1: Initial Retry Delay**
**Given** an initial retry
**When** backoff calculates delay
**Then** delay = base_delay (default 30s)

**AC2: Second Retry Delay**
**Given** a second retry
**When** backoff calculates delay
**Then** delay = base_delay * 2 = 60s

**AC3: Subsequent Retry Delays**
**Given** subsequent retries
**When** backoff calculates delay
**Then** delay = min(base_delay * 2^(attempt-1), max_delay)
**And** max_delay is configurable (default 5 minutes)

**AC4: Jitter Implementation**
**Given** jitter is enabled (default)
**When** backoff calculates delay
**Then** delay is randomized +/- 10%

**AC5: Max Retries Enforcement**
**Given** max_retries is reached
**When** another retry would be attempted
**Then** it gives up and logs error
**And** sends notification if configured

**AC6: Configuration Options**
**Given** backoff is created
**When** configuration is provided
**Then** base_delay, max_delay, max_retries, jitter are configurable

## Tasks / Subtasks

- [ ] Create Backoff struct (AC: 1, 2, 3, 6)
  - [ ] Create `src/resume/backoff.rs`
  - [ ] Define BackoffConfig with all parameters
  - [ ] Implement delay calculation formula
  - [ ] Add builder pattern for configuration

- [ ] Implement delay calculation (AC: 1, 2, 3)
  - [ ] Implement base delay for attempt 1
  - [ ] Implement exponential growth (2^n)
  - [ ] Implement max_delay capping
  - [ ] Handle attempt number edge cases

- [ ] Implement jitter (AC: 4)
  - [ ] Add random jitter +/- 10%
  - [ ] Make jitter configurable (enable/disable, percentage)
  - [ ] Use thread-safe random number generation
  - [ ] Ensure reproducible tests with seed

- [ ] Implement retry tracking (AC: 5)
  - [ ] Track current attempt number
  - [ ] Check against max_retries
  - [ ] Return error when exceeded
  - [ ] Provide reset functionality

- [ ] Add configuration validation (AC: 6)
  - [ ] Validate base_delay > 0
  - [ ] Validate max_delay >= base_delay
  - [ ] Validate max_retries > 0
  - [ ] Validate jitter_percent in valid range

- [ ] Add unit tests (AC: 1, 2, 3, 4, 5, 6)
  - [ ] Test initial delay calculation
  - [ ] Test exponential growth
  - [ ] Test max_delay capping
  - [ ] Test jitter range
  - [ ] Test max_retries enforcement
  - [ ] Test configuration validation

- [ ] Add integration tests
  - [ ] Test with SameSessionStrategy
  - [ ] Test retry sequence
  - [ ] Test reset functionality

## Dev Notes

### Architecture Requirements

**From epics.md - Technical Notes:**

```
- Implements: FR12
- Create `src/resume/backoff.rs`
- Configurable: base_delay, max_delay, max_retries, jitter
```

**Backoff Formula:**

```
delay(n) = min(base_delay * 2^(n-1), max_delay)

With jitter:
delay(n) = delay(n) * (1 + random(-0.1, 0.1))
```

**Example sequence (base=30s, max=300s):**
| Attempt | Base Delay | With Max Cap |
|---------|------------|--------------|
| 1 | 30s | 30s |
| 2 | 60s | 60s |
| 3 | 120s | 120s |
| 4 | 240s | 240s |
| 5 | 480s | 300s (capped) |

**Implements:** FR12 (exponential backoff)

### Technical Implementation

**Backoff:**

```rust
// src/resume/backoff.rs
use std::time::Duration;

use rand::Rng;
use tracing::debug;

/// Configuration for exponential backoff.
#[derive(Debug, Clone)]
pub struct BackoffConfig {
    /// Base delay for first retry
    pub base_delay: Duration,
    /// Maximum delay (cap)
    pub max_delay: Duration,
    /// Maximum number of retries
    pub max_retries: u32,
    /// Enable jitter
    pub jitter_enabled: bool,
    /// Jitter percentage (0.0 to 1.0)
    pub jitter_percent: f64,
}

impl Default for BackoffConfig {
    fn default() -> Self {
        Self {
            base_delay: Duration::from_secs(30),
            max_delay: Duration::from_secs(300), // 5 minutes
            max_retries: 5,
            jitter_enabled: true,
            jitter_percent: 0.1, // +/- 10%
        }
    }
}

impl BackoffConfig {
    /// Validate configuration values.
    pub fn validate(&self) -> Result<(), BackoffError> {
        if self.base_delay.is_zero() {
            return Err(BackoffError::InvalidConfig("base_delay must be > 0".into()));
        }
        if self.max_delay < self.base_delay {
            return Err(BackoffError::InvalidConfig(
                "max_delay must be >= base_delay".into()
            ));
        }
        if self.max_retries == 0 {
            return Err(BackoffError::InvalidConfig("max_retries must be > 0".into()));
        }
        if self.jitter_percent < 0.0 || self.jitter_percent > 1.0 {
            return Err(BackoffError::InvalidConfig(
                "jitter_percent must be between 0.0 and 1.0".into()
            ));
        }
        Ok(())
    }
}

/// Errors for backoff operations.
#[derive(Debug, thiserror::Error)]
pub enum BackoffError {
    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),
    
    #[error("Maximum retries ({max}) exceeded")]
    MaxRetriesExceeded { max: u32 },
}

/// Exponential backoff calculator.
#[derive(Debug, Clone)]
pub struct Backoff {
    config: BackoffConfig,
}

impl Backoff {
    /// Create new backoff with default configuration.
    pub fn new(base_delay: Duration, max_delay: Duration) -> Self {
        Self {
            config: BackoffConfig {
                base_delay,
                max_delay,
                ..Default::default()
            },
        }
    }
    
    /// Create with full configuration.
    pub fn with_config(config: BackoffConfig) -> Result<Self, BackoffError> {
        config.validate()?;
        Ok(Self { config })
    }
    
    /// Builder: set jitter enabled.
    pub fn with_jitter(mut self, enabled: bool) -> Self {
        self.config.jitter_enabled = enabled;
        self
    }
    
    /// Builder: set max retries.
    pub fn with_max_retries(mut self, max: u32) -> Self {
        self.config.max_retries = max;
        self
    }
    
    /// Builder: set jitter percentage.
    pub fn with_jitter_percent(mut self, percent: f64) -> Self {
        self.config.jitter_percent = percent.clamp(0.0, 1.0);
        self
    }
    
    /// Calculate delay for given attempt number (1-indexed).
    pub fn delay_for_attempt(&self, attempt: u32) -> Duration {
        if attempt == 0 {
            return self.config.base_delay;
        }
        
        // Calculate exponential delay: base * 2^(attempt-1)
        let exponent = (attempt - 1).min(31); // Prevent overflow
        let multiplier = 2u64.saturating_pow(exponent);
        let base_millis = self.config.base_delay.as_millis() as u64;
        let delay_millis = base_millis.saturating_mul(multiplier);
        
        // Cap at max_delay
        let max_millis = self.config.max_delay.as_millis() as u64;
        let capped_millis = delay_millis.min(max_millis);
        
        let mut delay = Duration::from_millis(capped_millis);
        
        // Apply jitter if enabled
        if self.config.jitter_enabled {
            delay = self.apply_jitter(delay);
        }
        
        debug!(
            attempt = attempt,
            delay_secs = delay.as_secs_f64(),
            "Calculated backoff delay"
        );
        
        delay
    }
    
    /// Apply random jitter to delay.
    fn apply_jitter(&self, delay: Duration) -> Duration {
        let mut rng = rand::thread_rng();
        let jitter_range = self.config.jitter_percent;
        
        // Generate random factor between (1 - jitter) and (1 + jitter)
        let factor = 1.0 + rng.gen_range(-jitter_range..jitter_range);
        
        let millis = delay.as_millis() as f64 * factor;
        Duration::from_millis(millis as u64)
    }
    
    /// Check if attempt exceeds max retries.
    pub fn check_retry_limit(&self, attempt: u32) -> Result<(), BackoffError> {
        if attempt > self.config.max_retries {
            return Err(BackoffError::MaxRetriesExceeded {
                max: self.config.max_retries,
            });
        }
        Ok(())
    }
    
    /// Get the maximum number of retries.
    pub fn max_retries(&self) -> u32 {
        self.config.max_retries
    }
    
    /// Create iterator over all retry delays.
    pub fn iter(&self) -> BackoffIterator {
        BackoffIterator {
            backoff: self.clone(),
            attempt: 1,
        }
    }
}

impl Default for Backoff {
    fn default() -> Self {
        Self::new(
            Duration::from_secs(30),
            Duration::from_secs(300),
        )
    }
}

/// Iterator over backoff delays.
pub struct BackoffIterator {
    backoff: Backoff,
    attempt: u32,
}

impl Iterator for BackoffIterator {
    type Item = Duration;
    
    fn next(&mut self) -> Option<Self::Item> {
        if self.attempt > self.backoff.config.max_retries {
            return None;
        }
        
        let delay = self.backoff.delay_for_attempt(self.attempt);
        self.attempt += 1;
        Some(delay)
    }
}

/// Helper to run async operation with backoff retries.
pub struct BackoffRetry<F, T, E>
where
    F: Fn() -> futures::future::BoxFuture<'static, Result<T, E>>,
{
    operation: F,
    backoff: Backoff,
}

impl<F, T, E> BackoffRetry<F, T, E>
where
    F: Fn() -> futures::future::BoxFuture<'static, Result<T, E>>,
    E: std::fmt::Display,
{
    pub fn new(operation: F, backoff: Backoff) -> Self {
        Self { operation, backoff }
    }
    
    /// Execute operation with retries.
    pub async fn execute(&self) -> Result<T, BackoffError>
    where
        T: 'static,
        E: 'static,
    {
        let mut attempt = 1;
        
        loop {
            self.backoff.check_retry_limit(attempt)?;
            
            match (self.operation)().await {
                Ok(result) => return Ok(result),
                Err(e) => {
                    tracing::warn!(
                        attempt = attempt,
                        error = %e,
                        "Operation failed, will retry"
                    );
                    
                    let delay = self.backoff.delay_for_attempt(attempt);
                    tokio::time::sleep(delay).await;
                    attempt += 1;
                }
            }
        }
    }
}
```

### Dependencies

Uses existing and new dependencies:
- `rand = "0.8"` (add to Cargo.toml) - random jitter
- `tracing` (already in Cargo.toml) - structured logging
- `thiserror` (already in Cargo.toml) - error types
- `futures` (already in Cargo.toml) - async utilities

### Integration with SameSessionStrategy (Story 3.2)

```rust
// In SameSessionStrategy
let backoff = Backoff::new(
    Duration::from_secs(self.config.backoff_base_secs),
    Duration::from_secs(self.config.backoff_max_secs),
).with_jitter(self.config.backoff_jitter);

// Calculate delay
let delay = if let Some(retry_after) = ctx.retry_after {
    retry_after // Use server-provided delay
} else {
    backoff.delay_for_attempt(ctx.attempt_number)
};
```

### Jitter Rationale

Jitter prevents "thundering herd" when multiple clients retry simultaneously:

```
Without jitter:
Client A: 30s, 60s, 120s...
Client B: 30s, 60s, 120s...
-> All clients hit server at same time

With 10% jitter:
Client A: 27s, 63s, 108s...
Client B: 33s, 57s, 132s...
-> Requests spread out
```

### Testing Strategy

**Unit Tests (deterministic):**
- Disable jitter for predictable tests
- Test delay calculation formula
- Test max_delay capping
- Test max_retries enforcement
- Test configuration validation

**Unit Tests (with jitter):**
- Test jitter range bounds
- Test jitter distribution (statistical)
- Use seeded RNG for reproducibility

**Integration Tests:**
- Test with SameSessionStrategy
- Test full retry sequence
- Test cancellation during backoff

### References

- [Source: _bmad-output/planning-artifacts/epics.md#Story 3.5: Exponential Backoff Implementation]
- [Source: _bmad-output/implementation-artifacts/3-2-same-session-resume-strategy.md]

## File List

**Files to create:**
- `src/resume/backoff.rs`
- `tests/backoff_test.rs`

**Files to modify:**
- `Cargo.toml` (add rand dependency if not present)
- `src/resume/mod.rs` (add backoff module)
- `_bmad-output/implementation-artifacts/sprint-status.yaml`

## Change Log

- 2026-02-05: Story created and marked ready-for-dev
