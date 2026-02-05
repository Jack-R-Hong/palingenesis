# Story 2.5: Stop Reason Classification - Context Exhaustion

Status: ready-for-dev

## Story

As a classifier,
I want to identify when a session stopped due to context window exhaustion,
So that the daemon knows to start a new session.

## Acceptance Criteria

**AC1: Context Length Exceeded Detection**
**Given** a session file or log contains "context_length_exceeded"
**When** the classifier analyzes the stop
**Then** it returns `StopReason::ContextExhausted`

**AC2: Token Count Threshold Detection**
**Given** token count exceeds threshold (>80% of context window)
**When** the classifier analyzes the session
**Then** it considers this a context exhaustion risk

**AC3: Completion Status Check**
**Given** session frontmatter shows `stepsCompleted` at final step
**When** the classifier analyzes the stop
**Then** it returns `StopReason::Completed` (not context exhaustion)

**AC4: Context Truncation Detection**
**Given** logs indicate context was truncated or conversation reset
**When** the classifier analyzes the stop
**Then** it returns `StopReason::ContextExhausted`

**AC5: Differentiation from Rate Limit**
**Given** both context and rate limit indicators might be present
**When** the classifier analyzes the stop
**Then** rate limit takes priority (can retry same session)
**And** context exhaustion is secondary classification

**AC6: Error Handling**
**Given** the classifier encounters a parsing error
**When** analyzing session data
**Then** it logs the error and falls through to other patterns
**And** does not crash the daemon

## Tasks / Subtasks

- [ ] Extend classifier module (AC: 1, 2, 4, 5, 6)
  - [ ] Add context exhaustion patterns to `src/monitor/classifier.rs`
  - [ ] Define `ContextExhaustionInfo` struct with metadata
  - [ ] Implement detection methods in StopReasonClassifier
  - [ ] Update classification priority logic

- [ ] Define context exhaustion patterns (AC: 1, 4)
  - [ ] Pattern: "context_length_exceeded" error message
  - [ ] Pattern: "maximum context length" exceeded
  - [ ] Pattern: "token limit" exceeded
  - [ ] Pattern: "conversation too long"
  - [ ] Pattern: Claude/Anthropic-specific context errors

- [ ] Implement token count detection (AC: 2)
  - [ ] Parse token count from session/logs if available
  - [ ] Define threshold percentage (configurable, default 80%)
  - [ ] Track context window size per model
  - [ ] Calculate usage percentage

- [ ] Implement completion status integration (AC: 3)
  - [ ] Read session frontmatter using Story 2.2 parser
  - [ ] Check `stepsCompleted` array against total steps
  - [ ] Check `status` field for "complete"
  - [ ] Return `StopReason::Completed` when appropriate

- [ ] Implement classification priority (AC: 5)
  - [ ] Ensure rate limit check runs first
  - [ ] Context exhaustion as secondary check
  - [ ] Document priority order in code

- [ ] Implement error handling (AC: 6)
  - [ ] Handle missing frontmatter gracefully
  - [ ] Handle malformed content gracefully
  - [ ] Log errors with tracing
  - [ ] Continue to next pattern on failure

- [ ] Add configuration support (AC: 2)
  - [ ] Add `context_threshold_percent` to ClassifierConfig
  - [ ] Add `known_context_sizes` map (model -> tokens)
  - [ ] Support custom patterns (future extensibility)

- [ ] Add unit tests (AC: 1, 2, 3, 4, 5, 6)
  - [ ] Test context_length_exceeded detection
  - [ ] Test token threshold detection
  - [ ] Test completion status handling
  - [ ] Test priority over context when rate limited
  - [ ] Test error handling

- [ ] Add integration tests
  - [ ] Test with fixture files containing context errors
  - [ ] Test with completed session fixtures
  - [ ] Test classification pipeline end-to-end

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
    classifier.rs             # Stop reason classification (THIS STORY extends 2.4)
    error.rs                  # MonitorError type
```

**Implements:** FR4 (detect context window exhaustion)

### Technical Implementation

**Extend StopReason Types (from Story 2.4):**

```rust
// src/monitor/classifier.rs (extend existing)

/// Information about context exhaustion.
#[derive(Debug, Clone, PartialEq)]
pub struct ContextExhaustionInfo {
    /// Estimated token usage percentage (if available)
    pub usage_percent: Option<f32>,
    /// Model context window size (if known)
    pub context_size: Option<u32>,
    /// Raw error message if available
    pub message: Option<String>,
}

// Extend StopReason enum:
pub enum StopReason {
    RateLimit(RateLimitInfo),
    ContextExhausted(Option<ContextExhaustionInfo>),  // Updated
    UserExit,
    Completed,
    Unknown(String),
}
```

**Context Exhaustion Detection:**

```rust
// src/monitor/classifier.rs (extend existing)

impl StopReasonClassifier {
    // Add to existing patterns in new()
    fn build_context_patterns() -> Result<Vec<Regex>, ClassifierError> {
        Ok(vec![
            Regex::new(r"(?i)context.?length.?exceeded")?,
            Regex::new(r"(?i)maximum\s+context\s+length")?,
            Regex::new(r"(?i)token.?limit.?exceeded")?,
            Regex::new(r"(?i)conversation\s+too\s+long")?,
            Regex::new(r"(?i)context.?window.?(full|exceeded|limit)")?,
            Regex::new(r"(?i)max.?tokens.?reached")?,
            Regex::new(r"(?i)prompt\s+is\s+too\s+long")?,
        ])
    }
    
    /// Detect context exhaustion from content.
    fn detect_context_exhaustion(
        &self, 
        content: &str, 
        evidence: &mut Vec<String>
    ) -> Option<ContextExhaustionInfo> {
        // Check context exhaustion patterns
        for pattern in &self.context_patterns {
            if let Some(m) = pattern.find(content) {
                evidence.push(format!("Matched context pattern: {}", m.as_str()));
                
                // Try to extract token info
                let (usage_percent, context_size) = self.extract_token_info(content);
                
                return Some(ContextExhaustionInfo {
                    usage_percent,
                    context_size,
                    message: Some(m.as_str().to_string()),
                });
            }
        }
        
        // Check for high token usage
        if let Some((usage, size)) = self.check_token_threshold(content) {
            if usage > self.config.context_threshold_percent {
                evidence.push(format!(
                    "Token usage {}% exceeds threshold {}%",
                    usage * 100.0,
                    self.config.context_threshold_percent * 100.0
                ));
                return Some(ContextExhaustionInfo {
                    usage_percent: Some(usage),
                    context_size: Some(size),
                    message: None,
                });
            }
        }
        
        None
    }
    
    /// Check if session is completed (not context exhaustion).
    fn check_completed(
        &self,
        session_path: &Path,
        evidence: &mut Vec<String>
    ) -> Option<StopReason> {
        use crate::monitor::frontmatter::parse_session;
        
        match parse_session(session_path) {
            Ok(session) => {
                // Check explicit completion status
                if session.is_complete() {
                    evidence.push("Session status is 'complete'".to_string());
                    return Some(StopReason::Completed);
                }
                
                // Could also check if stepsCompleted matches total steps
                // if session has totalSteps field
                
                None
            }
            Err(e) => {
                debug!(error = %e, "Could not parse session for completion check");
                None
            }
        }
    }
    
    fn extract_token_info(&self, content: &str) -> (Option<f32>, Option<u32>) {
        // Pattern: "used 150000 of 200000 tokens"
        let usage_pattern = Regex::new(r"used\s+(\d+)\s+of\s+(\d+)\s+tokens").ok();
        if let Some(re) = usage_pattern {
            if let Some(caps) = re.captures(content) {
                let used: Option<u32> = caps.get(1).and_then(|m| m.as_str().parse().ok());
                let total: Option<u32> = caps.get(2).and_then(|m| m.as_str().parse().ok());
                
                if let (Some(used), Some(total)) = (used, total) {
                    return (Some(used as f32 / total as f32), Some(total));
                }
            }
        }
        
        (None, None)
    }
    
    fn check_token_threshold(&self, content: &str) -> Option<(f32, u32)> {
        // Look for token count indicators
        let count_pattern = Regex::new(r"(\d+)\s*tokens?\s*used").ok();
        if let Some(re) = count_pattern {
            if let Some(caps) = re.captures(content) {
                if let Some(count) = caps.get(1).and_then(|m| m.as_str().parse::<u32>().ok()) {
                    // Use default context size for estimation
                    let context_size = self.config.default_context_size;
                    return Some((count as f32 / context_size as f32, context_size));
                }
            }
        }
        
        None
    }
    
    /// Updated classify method with context exhaustion.
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
        
        // Priority 1: Check for rate limit (can retry same session)
        if let Some(info) = self.detect_rate_limit(&content, &mut evidence) {
            return ClassificationResult {
                reason: StopReason::RateLimit(info),
                confidence: 0.9,
                evidence,
            };
        }
        
        // Priority 2: Check for completion (workflow done)
        if let Some(reason) = self.check_completed(session_path, &mut evidence) {
            return ClassificationResult {
                reason,
                confidence: 0.95,
                evidence,
            };
        }
        
        // Priority 3: Check for context exhaustion (needs new session)
        if let Some(info) = self.detect_context_exhaustion(&content, &mut evidence) {
            return ClassificationResult {
                reason: StopReason::ContextExhausted(Some(info)),
                confidence: 0.85,
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
}
```

**Extended Configuration:**

```rust
// src/monitor/classifier.rs (extend ClassifierConfig)

#[derive(Debug, Clone)]
pub struct ClassifierConfig {
    /// Default wait time when no Retry-After is found
    pub default_retry_wait: Duration,
    /// Maximum lines to read from session file
    pub max_lines: usize,
    /// Context usage threshold for exhaustion detection (0.0-1.0)
    pub context_threshold_percent: f32,
    /// Default context window size for estimation (tokens)
    pub default_context_size: u32,
}

impl Default for ClassifierConfig {
    fn default() -> Self {
        Self {
            default_retry_wait: Duration::from_secs(30),
            max_lines: 100,
            context_threshold_percent: 0.80, // 80%
            default_context_size: 200_000,   // Claude 3.5 Sonnet
        }
    }
}
```

### Dependencies

Uses existing dependencies (no new dependencies):
- `regex` (added in Story 2.4) - pattern matching
- `tracing` (already in Cargo.toml) - structured logging
- `thiserror` (already in Cargo.toml) - error types

### Context Window Sizes

Known context window sizes for estimation:

| Model | Context Size |
|-------|--------------|
| Claude 3.5 Sonnet | 200,000 |
| Claude 3 Opus | 200,000 |
| GPT-4 Turbo | 128,000 |
| GPT-4 | 8,192 |

### Classification Priority (Updated)

1. **Rate Limit** - Retry same session (highest priority)
2. **Completed** - Workflow finished successfully
3. **Context Exhaustion** - Needs new session
4. **User Exit** - Respect user intent (Story 2.6)
5. **Unknown** - Default fallback

### Previous Story Learnings

From Story 2.2 (Session File Parser):
1. **parse_session()**: Reuse for checking completion status
2. **Session struct**: Check `is_complete()` and `steps_completed_count()`

From Story 2.4 (Rate Limit Classification):
1. **Pattern structure**: Extend existing classifier patterns
2. **Evidence collection**: Continue pattern for debugging
3. **Configuration**: Add new fields to ClassifierConfig

### Performance Considerations

- Reuse compiled regex patterns from Story 2.4
- Check rate limit first (most common) for fast path
- Lazy parse session only for completion check

### Testing Strategy

**Unit Tests:**
- Test context_length_exceeded detection
- Test token threshold calculation
- Test completion status detection
- Test priority (rate limit > context)

**Integration Tests:**
- Fixture files with context errors
- Completed session files
- Mixed indicator files

**Fixtures:**
- `tests/fixtures/context_exceeded.txt`
- `tests/fixtures/session_complete.md`
- `tests/fixtures/token_limit.txt`

### References

- [Source: _bmad-output/planning-artifacts/architecture.md#Data Architecture]
- [Source: _bmad-output/planning-artifacts/epics.md#Story 2.5: Stop Reason Classification - Context Exhaustion]
- [Source: _bmad-output/implementation-artifacts/2-2-session-file-parser-frontmatter-extraction.md]
- [Source: _bmad-output/implementation-artifacts/2-4-stop-reason-classification-rate-limit.md]

## File List

**Files to create:**
- `tests/fixtures/context_exceeded.txt`
- `tests/fixtures/session_complete.md`
- `tests/fixtures/token_limit.txt`

**Files to modify:**
- `src/monitor/classifier.rs` (extend with context exhaustion logic)
- `tests/classifier_test.rs` (add context exhaustion tests)
- `_bmad-output/implementation-artifacts/sprint-status.yaml`

## Change Log

- 2026-02-05: Story created and marked ready-for-dev
