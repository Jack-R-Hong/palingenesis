# Story 2.4: Stop Reason Classification - Rate Limit

Status: ready-for-dev

## Story

As a classifier,
I want to identify when a session stopped due to rate limiting,
So that the daemon knows to wait and resume the same session.

## Acceptance Criteria

**AC1: Rate Limit Error Detection**
**Given** a session file or log contains "rate_limit_error"
**When** the classifier analyzes the stop
**Then** it returns `StopReason::RateLimit`
**And** extracts `retry_after` duration if present

**AC2: HTTP 429 Detection**
**Given** HTTP 429 status code in session output
**When** the classifier analyzes the stop
**Then** it returns `StopReason::RateLimit`

**AC3: Retry-After Header Extraction**
**Given** a `Retry-After` header value is present
**When** the classifier extracts it
**Then** the duration is included in the classification result

**AC4: Default Wait Time**
**Given** no retry information is available
**When** the classifier returns RateLimit
**Then** it uses a default wait time from config

**AC5: Classification Priority**
**Given** multiple stop indicators are present
**When** the classifier analyzes the stop
**Then** rate limit takes precedence over other reasons
**And** the classification is deterministic

**AC6: Error Handling**
**Given** the classifier encounters a parsing error
**When** analyzing session data
**Then** it logs the error and returns `StopReason::Unknown`
**And** does not crash the daemon

## Tasks / Subtasks

- [ ] Create classifier module structure (AC: 1, 2, 5, 6)
  - [ ] Create `src/monitor/classifier.rs` with StopReasonClassifier
  - [ ] Define `StopReason` enum with all variants
  - [ ] Define `ClassificationResult` struct with metadata
  - [ ] Update `src/monitor/mod.rs` to export modules

- [ ] Define StopReason types (AC: 1, 2, 3, 4)
  - [ ] Define `StopReason` enum (RateLimit, ContextExhausted, UserExit, Completed, Unknown)
  - [ ] Define `RateLimitInfo` struct (retry_after, source)
  - [ ] Define `ClassifierError` enum with thiserror
  - [ ] Implement Display for user-friendly messages

- [ ] Implement rate limit detection patterns (AC: 1, 2)
  - [ ] Pattern: "rate_limit_error" in session/log content
  - [ ] Pattern: "429" HTTP status code
  - [ ] Pattern: "too many requests" error message
  - [ ] Pattern: "quota exceeded" variants
  - [ ] Pattern: Anthropic-specific rate limit indicators

- [ ] Implement Retry-After extraction (AC: 3, 4)
  - [ ] Parse `Retry-After` header value (seconds)
  - [ ] Parse `retry_after` JSON field
  - [ ] Parse "try again in X seconds" text patterns
  - [ ] Fall back to default wait time from config

- [ ] Implement StopReasonClassifier struct (AC: 1, 2, 5)
  - [ ] Accept session path and optional log content
  - [ ] Implement `classify()` method returning ClassificationResult
  - [ ] Implement pattern priority (rate limit > context > user exit)
  - [ ] Support multiple input sources (file, log, exit code)

- [ ] Implement classification logic (AC: 1, 2, 3, 5)
  - [ ] Read session file last N lines for error detection
  - [ ] Parse structured error messages (JSON/YAML)
  - [ ] Extract HTTP status codes from logs
  - [ ] Combine multiple signals for confidence scoring

- [ ] Implement error handling (AC: 6)
  - [ ] Handle file read errors gracefully
  - [ ] Handle malformed content gracefully
  - [ ] Return Unknown on classification failure
  - [ ] Log classification decisions for debugging

- [ ] Add configuration support (AC: 4)
  - [ ] Define `ClassifierConfig` with default_retry_wait
  - [ ] Support config override for wait times
  - [ ] Support custom patterns (future extensibility)

- [ ] Add unit tests (AC: 1, 2, 3, 4, 5, 6)
  - [ ] Test rate_limit_error detection
  - [ ] Test HTTP 429 detection
  - [ ] Test Retry-After extraction (seconds, text)
  - [ ] Test default wait time fallback
  - [ ] Test classification priority
  - [ ] Test error handling

- [ ] Add integration tests
  - [ ] Test with fixture files containing rate limit errors
  - [ ] Test with real opencode log format samples
  - [ ] Test end-to-end classification pipeline

## Dev Notes

### Architecture Requirements

**From architecture.md - Project Structure:**

```
src/monitor/
    mod.rs                    # Monitor module root
    watcher.rs                # File system watcher - Story 2.1
    session.rs                # Session file parsing - Story 2.2
    frontmatter.rs            # YAML frontmatter extraction - Story 2.2
    process.rs                # Process detection - Story 2.3
    classifier.rs             # Stop reason classification (THIS STORY + 2.5, 2.6)
    error.rs                  # MonitorError type
```

**Implements:** FR3 (detect rate limit HTTP 429), FR11 (respect Retry-After headers)

### Technical Implementation

**StopReason Types:**

```rust
// src/monitor/classifier.rs
use std::time::Duration;

/// Reason why a session stopped.
#[derive(Debug, Clone, PartialEq)]
pub enum StopReason {
    /// Session hit rate limit (HTTP 429 or equivalent)
    RateLimit(RateLimitInfo),
    /// Session exhausted context window
    ContextExhausted,
    /// User explicitly exited (Ctrl+C, exit command)
    UserExit,
    /// Session completed successfully
    Completed,
    /// Unknown or unclassifiable reason
    Unknown(String),
}

/// Information about a rate limit stop.
#[derive(Debug, Clone, PartialEq)]
pub struct RateLimitInfo {
    /// Duration to wait before retry (from Retry-After or default)
    pub retry_after: Duration,
    /// Source of the retry_after value
    pub source: RetryAfterSource,
    /// Raw error message if available
    pub message: Option<String>,
}

/// Source of the retry_after duration.
#[derive(Debug, Clone, PartialEq)]
pub enum RetryAfterSource {
    /// From Retry-After HTTP header
    Header,
    /// From JSON/YAML error response
    ResponseBody,
    /// From text pattern extraction
    TextParsed,
    /// Default from configuration
    ConfigDefault,
}

/// Result of stop reason classification.
#[derive(Debug, Clone)]
pub struct ClassificationResult {
    /// The classified stop reason
    pub reason: StopReason,
    /// Confidence level (0.0 - 1.0)
    pub confidence: f32,
    /// Evidence used for classification
    pub evidence: Vec<String>,
}
```

**StopReasonClassifier Implementation:**

```rust
// src/monitor/classifier.rs
use std::fs;
use std::path::Path;
use std::time::Duration;

use regex::Regex;
use tracing::{debug, warn};

const DEFAULT_RETRY_WAIT_SECS: u64 = 30;
const MAX_LINES_TO_READ: usize = 100;

#[derive(Debug, thiserror::Error)]
pub enum ClassifierError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("Pattern compilation error: {0}")]
    Pattern(#[from] regex::Error),
}

/// Configuration for the classifier.
#[derive(Debug, Clone)]
pub struct ClassifierConfig {
    /// Default wait time when no Retry-After is found
    pub default_retry_wait: Duration,
    /// Maximum lines to read from session file
    pub max_lines: usize,
}

impl Default for ClassifierConfig {
    fn default() -> Self {
        Self {
            default_retry_wait: Duration::from_secs(DEFAULT_RETRY_WAIT_SECS),
            max_lines: MAX_LINES_TO_READ,
        }
    }
}

pub struct StopReasonClassifier {
    config: ClassifierConfig,
    rate_limit_patterns: Vec<Regex>,
}

impl StopReasonClassifier {
    /// Create a new classifier with default configuration.
    pub fn new() -> Result<Self, ClassifierError> {
        Self::with_config(ClassifierConfig::default())
    }
    
    /// Create with custom configuration.
    pub fn with_config(config: ClassifierConfig) -> Result<Self, ClassifierError> {
        let rate_limit_patterns = vec![
            Regex::new(r"(?i)rate.?limit")?,
            Regex::new(r"(?i)429")?,
            Regex::new(r"(?i)too\s+many\s+requests")?,
            Regex::new(r"(?i)quota\s+exceeded")?,
            Regex::new(r"(?i)rate_limit_error")?,
            Regex::new(r"(?i)overloaded")?,
            Regex::new(r"(?i)throttl")?,
        ];
        
        Ok(Self {
            config,
            rate_limit_patterns,
        })
    }
    
    /// Classify the stop reason from session file content.
    pub fn classify(&self, session_path: &Path, exit_code: Option<i32>) -> ClassificationResult {
        let mut evidence = Vec::new();
        
        // Read session file tail
        let content = match self.read_file_tail(session_path, self.config.max_lines) {
            Ok(c) => c,
            Err(e) => {
                warn!(error = %e, "Failed to read session file");
                return ClassificationResult {
                    reason: StopReason::Unknown(format!("Read error: {}", e)),
                    confidence: 0.0,
                    evidence: vec![format!("Error: {}", e)],
                };
            }
        };
        
        // Check for rate limit indicators
        if let Some(info) = self.detect_rate_limit(&content, &mut evidence) {
            return ClassificationResult {
                reason: StopReason::RateLimit(info),
                confidence: 0.9,
                evidence,
            };
        }
        
        // Fallback to unknown
        ClassificationResult {
            reason: StopReason::Unknown("No matching patterns".to_string()),
            confidence: 0.5,
            evidence,
        }
    }
    
    /// Classify from raw content (for log analysis).
    pub fn classify_content(&self, content: &str, exit_code: Option<i32>) -> ClassificationResult {
        let mut evidence = Vec::new();
        
        // Check for rate limit indicators
        if let Some(info) = self.detect_rate_limit(content, &mut evidence) {
            return ClassificationResult {
                reason: StopReason::RateLimit(info),
                confidence: 0.9,
                evidence,
            };
        }
        
        // Fallback to unknown
        ClassificationResult {
            reason: StopReason::Unknown("No matching patterns".to_string()),
            confidence: 0.5,
            evidence,
        }
    }
    
    fn detect_rate_limit(&self, content: &str, evidence: &mut Vec<String>) -> Option<RateLimitInfo> {
        // Check rate limit patterns
        for pattern in &self.rate_limit_patterns {
            if let Some(m) = pattern.find(content) {
                evidence.push(format!("Matched pattern: {}", m.as_str()));
                
                // Try to extract Retry-After
                let (retry_after, source) = self.extract_retry_after(content);
                
                return Some(RateLimitInfo {
                    retry_after,
                    source,
                    message: Some(m.as_str().to_string()),
                });
            }
        }
        
        None
    }
    
    fn extract_retry_after(&self, content: &str) -> (Duration, RetryAfterSource) {
        // Pattern 1: Retry-After: 60
        let header_pattern = Regex::new(r"(?i)retry.?after[:\s]+(\d+)").ok();
        if let Some(re) = header_pattern {
            if let Some(caps) = re.captures(content) {
                if let Some(secs) = caps.get(1).and_then(|m| m.as_str().parse::<u64>().ok()) {
                    return (Duration::from_secs(secs), RetryAfterSource::Header);
                }
            }
        }
        
        // Pattern 2: "retry_after": 60 (JSON)
        let json_pattern = Regex::new(r#""retry_after"\s*:\s*(\d+)"#).ok();
        if let Some(re) = json_pattern {
            if let Some(caps) = re.captures(content) {
                if let Some(secs) = caps.get(1).and_then(|m| m.as_str().parse::<u64>().ok()) {
                    return (Duration::from_secs(secs), RetryAfterSource::ResponseBody);
                }
            }
        }
        
        // Pattern 3: "try again in 60 seconds"
        let text_pattern = Regex::new(r"(?i)try\s+again\s+in\s+(\d+)\s*(?:second|sec|s)").ok();
        if let Some(re) = text_pattern {
            if let Some(caps) = re.captures(content) {
                if let Some(secs) = caps.get(1).and_then(|m| m.as_str().parse::<u64>().ok()) {
                    return (Duration::from_secs(secs), RetryAfterSource::TextParsed);
                }
            }
        }
        
        // Default fallback
        (self.config.default_retry_wait, RetryAfterSource::ConfigDefault)
    }
    
    fn read_file_tail(&self, path: &Path, max_lines: usize) -> Result<String, std::io::Error> {
        let content = fs::read_to_string(path)?;
        let lines: Vec<&str> = content.lines().collect();
        let start = lines.len().saturating_sub(max_lines);
        Ok(lines[start..].join("\n"))
    }
}

impl Default for StopReasonClassifier {
    fn default() -> Self {
        Self::new().expect("Failed to create default classifier")
    }
}
```

### Dependencies

Uses existing dependencies:
- `regex` (need to add to Cargo.toml) - pattern matching
- `tracing` (already in Cargo.toml) - structured logging
- `thiserror` (already in Cargo.toml) - error types

**New dependency to add:**

```toml
# Cargo.toml
[dependencies]
regex = "1.11"
```

### Error Handling Pattern

Uses `thiserror` following project conventions from architecture.md:
- `ClassifierError::Io` - File read operations failed
- `ClassifierError::Pattern` - Regex compilation failed

### Rate Limit Patterns

Common patterns from AI API providers:

| Provider | Error Pattern | Retry-After |
|----------|---------------|-------------|
| Anthropic | "rate_limit_error", "overloaded_error" | JSON field |
| OpenAI | "Rate limit reached", HTTP 429 | Header |
| Generic | "too many requests", "quota exceeded" | Varies |

### Previous Story Learnings

From Story 2.2 (Session File Parser):
1. **Efficient reading**: Read only necessary content (tail of file)
2. **Error resilience**: Continue on parse failures

From Story 2.3 (Process Detection):
1. **MonitorEvent integration**: Classification results feed into events
2. **Configuration pattern**: Use config struct for tunables

### Classification Priority

When multiple indicators are present:
1. **Rate Limit** (highest priority) - always resume same session
2. **Context Exhaustion** - requires new session
3. **User Exit** - respect user intent
4. **Completed** - workflow finished successfully
5. **Unknown** (lowest) - default fallback

### Performance Considerations

- Read only last N lines of file (configurable, default 100)
- Compile regex patterns once, reuse
- Fail fast on first match for efficiency

### Testing Strategy

**Unit Tests:**
- Test each rate limit pattern individually
- Test Retry-After extraction variants
- Test priority handling
- Test error cases

**Integration Tests:**
- Fixture files with real error messages
- End-to-end classification

**Fixtures:**
- `tests/fixtures/rate_limit_429.txt`
- `tests/fixtures/rate_limit_anthropic.json`
- `tests/fixtures/rate_limit_retry_after.txt`

### References

- [Source: _bmad-output/planning-artifacts/architecture.md#Data Architecture]
- [Source: _bmad-output/planning-artifacts/epics.md#Story 2.4: Stop Reason Classification - Rate Limit]
- [Source: _bmad-output/implementation-artifacts/2-2-session-file-parser-frontmatter-extraction.md]
- [Source: _bmad-output/implementation-artifacts/2-3-process-detection-opencode-start-stop.md]

## File List

**Files to create:**
- `src/monitor/classifier.rs`
- `tests/classifier_test.rs`
- `tests/fixtures/rate_limit_429.txt`
- `tests/fixtures/rate_limit_anthropic.json`
- `tests/fixtures/rate_limit_retry_after.txt`

**Files to modify:**
- `Cargo.toml` (add regex)
- `src/monitor/mod.rs` (export classifier module)
- `_bmad-output/implementation-artifacts/sprint-status.yaml`

## Change Log

- 2026-02-05: Story created and marked ready-for-dev
