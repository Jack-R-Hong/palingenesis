# Story 2.6: Stop Reason Classification - User Exit

Status: ready-for-dev

## Story

As a classifier,
I want to identify when a user explicitly exited a session,
So that the daemon respects their intent and doesn't auto-resume.

## Acceptance Criteria

**AC1: Ctrl+C Exit Detection**
**Given** the user pressed Ctrl+C in the opencode session
**When** the classifier analyzes the stop
**Then** it returns `StopReason::UserExit`

**AC2: Clean Exit Detection**
**Given** the session exited with a clean exit code (0) without errors
**When** the classifier analyzes the stop
**Then** it returns `StopReason::UserExit` or `StopReason::Completed`

**AC3: No Auto-Resume Behavior**
**Given** a user exit is detected
**When** the daemon considers resumption
**Then** it does NOT auto-resume (respects user intent)
**And** logs "Session ended by user, not auto-resuming"

**AC4: Exit Command Detection**
**Given** user typed an exit command (exit, quit, /bye)
**When** the classifier analyzes the stop
**Then** it returns `StopReason::UserExit`

**AC5: Differentiation from Errors**
**Given** an error caused the exit (non-zero exit code with error)
**When** the classifier analyzes the stop
**Then** it does NOT classify as UserExit
**And** continues to other classification patterns

**AC6: Classification Priority**
**Given** user exit indicators are present along with errors
**When** the classifier analyzes the stop
**Then** error patterns (rate limit, context) take priority
**And** user exit is lower priority

## Tasks / Subtasks

- [ ] Extend classifier module (AC: 1, 2, 4, 5, 6)
  - [ ] Add user exit patterns to `src/monitor/classifier.rs`
  - [ ] Define `UserExitInfo` struct with metadata
  - [ ] Implement detection methods in StopReasonClassifier
  - [ ] Update classification priority logic

- [ ] Define user exit patterns (AC: 1, 4)
  - [ ] Pattern: SIGINT/SIGTERM signal detection
  - [ ] Pattern: "exit" command in session
  - [ ] Pattern: "quit" command in session
  - [ ] Pattern: "/bye" command (chat-style exit)
  - [ ] Pattern: "goodbye" or "done" user messages
  - [ ] Pattern: Clean exit without error output

- [ ] Implement exit code analysis (AC: 2, 5)
  - [ ] Exit code 0: Likely clean exit (user or completed)
  - [ ] Exit code 130: SIGINT (Ctrl+C)
  - [ ] Exit code 143: SIGTERM
  - [ ] Non-zero with error: Not user exit

- [ ] Implement signal detection (AC: 1)
  - [ ] Detect SIGINT (2) - Ctrl+C
  - [ ] Detect SIGTERM (15) - Normal termination
  - [ ] Detect SIGHUP (1) - Terminal closed
  - [ ] Map exit codes to signal numbers (128 + signal)

- [ ] Implement classification priority (AC: 6)
  - [ ] Rate limit > Completed > Context > User Exit > Unknown
  - [ ] User exit only when no error patterns match
  - [ ] Document priority in code comments

- [ ] Implement no-resume behavior (AC: 3)
  - [ ] Add `should_auto_resume()` method to StopReason
  - [ ] UserExit returns false for auto-resume
  - [ ] Log decision with appropriate message

- [ ] Add configuration support
  - [ ] Option to override auto-resume for user exit
  - [ ] Custom exit patterns (future extensibility)
  - [ ] Grace period before re-enabling auto-resume

- [ ] Add unit tests (AC: 1, 2, 4, 5, 6)
  - [ ] Test Ctrl+C (exit code 130) detection
  - [ ] Test clean exit (exit code 0) detection
  - [ ] Test exit command detection
  - [ ] Test error vs user exit differentiation
  - [ ] Test classification priority

- [ ] Add integration tests
  - [ ] Test with fixture files containing user exits
  - [ ] Test full classification pipeline
  - [ ] Test should_auto_resume() behavior

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
    classifier.rs             # Stop reason classification (THIS STORY extends 2.4, 2.5)
    error.rs                  # MonitorError type
```

**Implements:** FR5 (detect user explicit exit)

### Technical Implementation

**Extend StopReason Types:**

```rust
// src/monitor/classifier.rs (extend existing)

/// Information about user-initiated exit.
#[derive(Debug, Clone, PartialEq)]
pub struct UserExitInfo {
    /// How the exit was detected
    pub exit_type: UserExitType,
    /// Exit code if available
    pub exit_code: Option<i32>,
    /// Additional context
    pub message: Option<String>,
}

/// Type of user exit.
#[derive(Debug, Clone, PartialEq)]
pub enum UserExitType {
    /// User pressed Ctrl+C (SIGINT)
    CtrlC,
    /// User typed exit/quit command
    ExitCommand,
    /// Clean exit with code 0
    CleanExit,
    /// Terminal closed (SIGHUP)
    TerminalClosed,
    /// Generic user termination
    UserTerminated,
}

// Extend StopReason enum:
pub enum StopReason {
    RateLimit(RateLimitInfo),
    ContextExhausted(Option<ContextExhaustionInfo>),
    UserExit(UserExitInfo),  // Updated with info
    Completed,
    Unknown(String),
}

impl StopReason {
    /// Check if this stop reason should trigger auto-resume.
    pub fn should_auto_resume(&self) -> bool {
        match self {
            StopReason::RateLimit(_) => true,      // Wait and retry
            StopReason::ContextExhausted(_) => true, // Start new session
            StopReason::UserExit(_) => false,      // Respect user intent
            StopReason::Completed => false,        // Nothing to resume
            StopReason::Unknown(_) => false,       // Don't assume
        }
    }
    
    /// Get human-readable description for logging.
    pub fn description(&self) -> &'static str {
        match self {
            StopReason::RateLimit(_) => "rate limited",
            StopReason::ContextExhausted(_) => "context window exhausted",
            StopReason::UserExit(_) => "user exit",
            StopReason::Completed => "completed",
            StopReason::Unknown(_) => "unknown",
        }
    }
}
```

**User Exit Detection:**

```rust
// src/monitor/classifier.rs (extend existing)

// Exit codes for signals (128 + signal number)
const EXIT_SIGINT: i32 = 130;   // 128 + 2 (SIGINT/Ctrl+C)
const EXIT_SIGTERM: i32 = 143;  // 128 + 15 (SIGTERM)
const EXIT_SIGHUP: i32 = 129;   // 128 + 1 (SIGHUP)

impl StopReasonClassifier {
    // Add to existing patterns in new()
    fn build_user_exit_patterns() -> Result<Vec<Regex>, ClassifierError> {
        Ok(vec![
            Regex::new(r"(?i)^exit\s*$")?,
            Regex::new(r"(?i)^quit\s*$")?,
            Regex::new(r"(?i)^/bye\s*$")?,
            Regex::new(r"(?i)^goodbye\s*$")?,
            Regex::new(r"(?i)user\s+cancelled")?,
            Regex::new(r"(?i)keyboard\s+interrupt")?,
            Regex::new(r"(?i)interrupted\s+by\s+user")?,
            Regex::new(r"(?i)sigint\s+received")?,
        ])
    }
    
    /// Detect user exit from content and exit code.
    fn detect_user_exit(
        &self,
        content: &str,
        exit_code: Option<i32>,
        evidence: &mut Vec<String>,
    ) -> Option<UserExitInfo> {
        // Check exit code for signals
        if let Some(code) = exit_code {
            match code {
                EXIT_SIGINT => {
                    evidence.push("Exit code 130 (SIGINT/Ctrl+C)".to_string());
                    return Some(UserExitInfo {
                        exit_type: UserExitType::CtrlC,
                        exit_code: Some(code),
                        message: Some("User pressed Ctrl+C".to_string()),
                    });
                }
                EXIT_SIGTERM => {
                    evidence.push("Exit code 143 (SIGTERM)".to_string());
                    return Some(UserExitInfo {
                        exit_type: UserExitType::UserTerminated,
                        exit_code: Some(code),
                        message: Some("Process terminated".to_string()),
                    });
                }
                EXIT_SIGHUP => {
                    evidence.push("Exit code 129 (SIGHUP)".to_string());
                    return Some(UserExitInfo {
                        exit_type: UserExitType::TerminalClosed,
                        exit_code: Some(code),
                        message: Some("Terminal closed".to_string()),
                    });
                }
                0 => {
                    // Clean exit - might be user or completed
                    // Only classify as user exit if no error patterns
                    evidence.push("Clean exit code 0".to_string());
                }
                _ => {
                    // Non-zero exit with potential error - not user exit
                    return None;
                }
            }
        }
        
        // Check for exit commands in content
        for pattern in &self.user_exit_patterns {
            if let Some(m) = pattern.find(content) {
                evidence.push(format!("Matched user exit pattern: {}", m.as_str()));
                return Some(UserExitInfo {
                    exit_type: UserExitType::ExitCommand,
                    exit_code,
                    message: Some(m.as_str().to_string()),
                });
            }
        }
        
        // Clean exit without errors and no other patterns = likely user exit
        if exit_code == Some(0) && !self.has_error_indicators(content) {
            evidence.push("Clean exit without errors".to_string());
            return Some(UserExitInfo {
                exit_type: UserExitType::CleanExit,
                exit_code: Some(0),
                message: None,
            });
        }
        
        None
    }
    
    /// Check if content has error indicators (to exclude from user exit).
    fn has_error_indicators(&self, content: &str) -> bool {
        let error_patterns = [
            r"(?i)error",
            r"(?i)exception",
            r"(?i)failed",
            r"(?i)panic",
            r"(?i)crash",
        ];
        
        for pattern in &error_patterns {
            if Regex::new(pattern).map(|re| re.is_match(content)).unwrap_or(false) {
                return true;
            }
        }
        
        false
    }
    
    /// Updated classify method with user exit.
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
        
        // Priority 1: Check for rate limit (highest priority - can retry same session)
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
        
        // Priority 4: Check for user exit (respect intent)
        if let Some(info) = self.detect_user_exit(&content, exit_code, &mut evidence) {
            info!(
                exit_type = ?info.exit_type,
                "Session ended by user, not auto-resuming"
            );
            return ClassificationResult {
                reason: StopReason::UserExit(info),
                confidence: 0.8,
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

### Dependencies

Uses existing dependencies (no new dependencies):
- `regex` (added in Story 2.4) - pattern matching
- `tracing` (already in Cargo.toml) - structured logging
- `thiserror` (already in Cargo.toml) - error types

### Exit Code Reference

Unix signal to exit code mapping:

| Signal | Number | Exit Code | Meaning |
|--------|--------|-----------|---------|
| SIGHUP | 1 | 129 | Terminal closed |
| SIGINT | 2 | 130 | Ctrl+C |
| SIGQUIT | 3 | 131 | Ctrl+\ |
| SIGTERM | 15 | 143 | Terminate |
| SIGKILL | 9 | 137 | Force kill |

### Classification Priority (Final)

1. **Rate Limit** - Retry same session (highest priority)
2. **Completed** - Workflow finished successfully
3. **Context Exhaustion** - Needs new session
4. **User Exit** - Respect user intent, no auto-resume
5. **Unknown** - Default fallback, no auto-resume

### Previous Story Learnings

From Story 2.4 (Rate Limit Classification):
1. **Pattern structure**: Follow established classifier patterns
2. **Evidence collection**: Continue for debugging

From Story 2.5 (Context Exhaustion Classification):
1. **Priority handling**: User exit is lower than errors
2. **Integration**: Works with existing classify() flow

From Story 2.3 (Process Detection):
1. **Exit code**: Receive exit code from ProcessStopped event
2. **Signal detection**: Map exit codes to signals

### Behavioral Considerations

**Auto-Resume Decision:**
- Rate limit: Yes, wait and retry same session
- Context exhaustion: Yes, start new session
- Completed: No, workflow is done
- User exit: No, respect user intent
- Unknown: No, don't assume (safe default)

**Logging for User Exit:**
```rust
info!(
    reason = "user_exit",
    exit_type = ?info.exit_type,
    "Session ended by user, not auto-resuming"
);
```

### Testing Strategy

**Unit Tests:**
- Test Ctrl+C detection (exit code 130)
- Test SIGTERM detection (exit code 143)
- Test exit command detection
- Test clean exit vs error differentiation
- Test should_auto_resume() behavior

**Integration Tests:**
- Test with real signal simulations
- Test full classification pipeline
- Test logging output

**Fixtures:**
- `tests/fixtures/user_exit_ctrl_c.txt`
- `tests/fixtures/user_exit_command.txt`
- `tests/fixtures/clean_exit.txt`

### References

- [Source: _bmad-output/planning-artifacts/architecture.md#Data Architecture]
- [Source: _bmad-output/planning-artifacts/epics.md#Story 2.6: Stop Reason Classification - User Exit]
- [Source: _bmad-output/implementation-artifacts/2-4-stop-reason-classification-rate-limit.md]
- [Source: _bmad-output/implementation-artifacts/2-5-stop-reason-classification-context-exhaustion.md]

## File List

**Files to create:**
- `tests/fixtures/user_exit_ctrl_c.txt`
- `tests/fixtures/user_exit_command.txt`
- `tests/fixtures/clean_exit.txt`

**Files to modify:**
- `src/monitor/classifier.rs` (extend with user exit logic)
- `tests/classifier_test.rs` (add user exit tests)
- `_bmad-output/implementation-artifacts/sprint-status.yaml`

## Change Log

- 2026-02-05: Story created and marked ready-for-dev
