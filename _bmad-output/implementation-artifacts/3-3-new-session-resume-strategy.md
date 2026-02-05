# Story 3.3: New-Session Resume Strategy

Status: done

## Story

As a daemon,
I want to start a new session from Next-step.md after context exhaustion,
So that I can continue the workflow with fresh context.

## Acceptance Criteria

**AC1: Next-step.md Reading**
**Given** a context exhaustion stop
**When** the new-session strategy executes
**Then** it reads the `Next-step.md` file to determine continuation point

**AC2: Session Start from Next-step**
**Given** the new session starts
**When** it begins execution
**Then** it starts from the correct step (from Next-step.md or stepsCompleted)
**And** logs "Starting new session from step N"

**AC3: Fallback to stepsCompleted**
**Given** no Next-step.md exists
**When** the strategy looks for continuation point
**Then** it uses stepsCompleted from frontmatter to determine next step
**And** creates appropriate prompt to continue

**AC4: Stats and Audit Update**
**Given** new session creation succeeds
**When** the session starts
**Then** stats.total_resumes is incremented
**And** audit trail records the transition

**AC5: Session Backup Integration**
**Given** a context exhaustion triggers new session
**When** before new session creation
**Then** it calls session backup (Story 3.4)
**And** proceeds even if backup fails (with warning)

**AC6: Prompt Generation**
**Given** continuation point is determined
**When** new session is created
**Then** appropriate prompt is generated for opencode
**And** includes context from previous session

## Tasks / Subtasks

- [x] Create NewSessionStrategy struct (AC: 1, 2, 3)
  - [x] Create `src/resume/new_session.rs`
  - [x] Implement ResumeStrategy trait
  - [x] Add configuration for prompt templates
  - [x] Add configuration for Next-step.md path

- [x] Implement Next-step.md reading (AC: 1)
  - [x] Define Next-step.md location relative to session
  - [x] Parse Next-step.md content
  - [x] Extract step number and description
  - [x] Handle missing or malformed file

- [x] Implement stepsCompleted fallback (AC: 3)
  - [x] Read session frontmatter
  - [x] Get stepsCompleted array
  - [x] Calculate next step (max(stepsCompleted) + 1)
  - [x] Generate continuation prompt

- [x] Implement prompt generation (AC: 6)
  - [x] Create prompt template system
  - [x] Include step number
  - [x] Include context summary
  - [x] Include any special instructions

- [x] Implement session creation (AC: 2, 4)
  - [x] Research opencode new session API
  - [x] Execute new session command
  - [x] Wait for session to start
  - [x] Verify session is running

- [x] Integrate with session backup (AC: 5)
  - [x] Call SessionBackup from Story 3.4
  - [x] Handle backup failure gracefully
  - [x] Log backup status
  - [x] Proceed with new session regardless

- [x] Implement stats update (AC: 4)
  - [x] Increment stats.total_resumes
  - [x] Record session transition in audit
  - [x] Persist state changes

- [x] Add unit tests (AC: 1, 2, 3, 4, 5, 6)
  - [x] Test Next-step.md parsing
  - [x] Test stepsCompleted fallback
  - [x] Test prompt generation
  - [x] Test session creation
  - [x] Test backup integration
  - [x] Test error handling

- [x] Add integration tests
  - [x] Test full new-session flow
  - [x] Test with various Next-step.md formats
  - [x] Test backup + new session sequence

## Dev Notes

### Architecture Requirements

**From architecture.md - Resume Flow:**

```
Context Exhaustion Detected
       │
       ▼
┌─────────────────┐
│ NewSession      │
│ Strategy        │
├─────────────────┤
│ 1. Backup old   │
│    session      │
│ 2. Read Next-   │
│    step.md      │
│ 3. Create new   │
│    session      │
│ 4. Update audit │
└─────────────────┘
```

**Implements:** FR9 (new session from Next-step.md)

### Technical Implementation

**NewSessionStrategy:**

```rust
// src/resume/new_session.rs
use std::path::{Path, PathBuf};

use async_trait::async_trait;
use tokio::fs;
use tracing::{info, warn, debug, error};

use crate::resume::backup::SessionBackup;
use crate::resume::context::ResumeContext;
use crate::resume::error::ResumeError;
use crate::resume::outcome::ResumeOutcome;
use crate::resume::strategy::ResumeStrategy;
use crate::monitor::session::Session;

/// Configuration for new-session resume.
#[derive(Debug, Clone)]
pub struct NewSessionConfig {
    /// Name of Next-step file
    pub next_step_filename: String,
    /// Prompt template for continuation
    pub prompt_template: String,
    /// Enable session backup before new session
    pub enable_backup: bool,
    /// Maximum backups to keep
    pub max_backups: usize,
}

impl Default for NewSessionConfig {
    fn default() -> Self {
        Self {
            next_step_filename: "Next-step.md".to_string(),
            prompt_template: "Continue from step {step}: {description}".to_string(),
            enable_backup: true,
            max_backups: 10,
        }
    }
}

/// Information extracted from Next-step.md.
#[derive(Debug, Clone)]
pub struct NextStepInfo {
    /// Step number to continue from
    pub step_number: u32,
    /// Description of the step
    pub description: String,
    /// Full content of Next-step.md
    pub raw_content: String,
}

/// Strategy for creating new session after context exhaustion.
pub struct NewSessionStrategy {
    config: NewSessionConfig,
    backup: SessionBackup,
}

impl NewSessionStrategy {
    pub fn new() -> Self {
        let config = NewSessionConfig::default();
        Self {
            backup: SessionBackup::new(config.max_backups),
            config,
        }
    }
    
    pub fn with_config(config: NewSessionConfig) -> Self {
        Self {
            backup: SessionBackup::new(config.max_backups),
            config,
        }
    }
    
    /// Find and read Next-step.md file.
    async fn read_next_step(&self, session_dir: &Path) -> Option<NextStepInfo> {
        let next_step_path = session_dir.join(&self.config.next_step_filename);
        
        match fs::read_to_string(&next_step_path).await {
            Ok(content) => {
                debug!(path = %next_step_path.display(), "Found Next-step.md");
                Some(self.parse_next_step(&content))
            }
            Err(e) => {
                debug!(
                    path = %next_step_path.display(),
                    error = %e,
                    "Next-step.md not found, will use stepsCompleted"
                );
                None
            }
        }
    }
    
    /// Parse Next-step.md content to extract step info.
    fn parse_next_step(&self, content: &str) -> NextStepInfo {
        // Try to extract step number from content
        // Format expected: "## Step N: Description" or "# N. Description"
        let step_number = self.extract_step_number(content).unwrap_or(1);
        
        // Use first line as description, or whole content if short
        let description = content.lines()
            .find(|line| !line.is_empty() && !line.starts_with('#'))
            .unwrap_or("Continue workflow")
            .to_string();
        
        NextStepInfo {
            step_number,
            description,
            raw_content: content.to_string(),
        }
    }
    
    /// Extract step number from content using regex patterns.
    fn extract_step_number(&self, content: &str) -> Option<u32> {
        // Pattern: "Step N" or "step N" or "#N" or "N."
        for line in content.lines() {
            let line = line.trim().to_lowercase();
            if let Some(num_str) = line
                .strip_prefix("step ")
                .or_else(|| line.strip_prefix("# step "))
                .or_else(|| line.strip_prefix("## step "))
            {
                if let Some(num) = num_str.split(|c: char| !c.is_ascii_digit()).next() {
                    if let Ok(n) = num.parse() {
                        return Some(n);
                    }
                }
            }
        }
        None
    }
    
    /// Calculate next step from stepsCompleted in session metadata.
    fn calculate_from_steps_completed(&self, session: &Session) -> u32 {
        if let Some(steps) = &session.steps_completed {
            steps.iter().max().copied().unwrap_or(0) + 1
        } else {
            1
        }
    }
    
    /// Generate prompt for new session.
    fn generate_prompt(&self, info: &NextStepInfo) -> String {
        self.config.prompt_template
            .replace("{step}", &info.step_number.to_string())
            .replace("{description}", &info.description)
    }
    
    /// Create new opencode session with the generated prompt.
    async fn create_session(&self, prompt: &str, session_dir: &Path) -> Result<PathBuf, ResumeError> {
        info!(prompt = %prompt, "Creating new session");
        
        // Execute opencode to create new session
        let output = tokio::process::Command::new("opencode")
            .arg("new")
            .arg("--prompt")
            .arg(prompt)
            .arg("--workdir")
            .arg(session_dir)
            .output()
            .await
            .map_err(|e| ResumeError::Io(e))?;
        
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(ResumeError::CommandFailed {
                command: "opencode new".to_string(),
                stderr: stderr.to_string(),
            });
        }
        
        // Parse session path from output
        let stdout = String::from_utf8_lossy(&output.stdout);
        let session_path = stdout.lines()
            .find(|line| line.contains("session:"))
            .and_then(|line| line.split("session:").nth(1))
            .map(|s| PathBuf::from(s.trim()))
            .unwrap_or_else(|| session_dir.join("session.md"));
        
        Ok(session_path)
    }
}

#[async_trait]
impl ResumeStrategy for NewSessionStrategy {
    async fn execute(&self, ctx: &ResumeContext) -> Result<ResumeOutcome, ResumeError> {
        let session_dir = ctx.session_path.parent()
            .ok_or_else(|| ResumeError::SessionNotFound {
                path: ctx.session_path.clone(),
            })?;
        
        // Step 1: Backup existing session (if enabled)
        if self.config.enable_backup {
            match self.backup.backup(&ctx.session_path).await {
                Ok(backup_path) => {
                    info!(backup = %backup_path.display(), "Session backed up");
                }
                Err(e) => {
                    warn!(error = %e, "Session backup failed, proceeding anyway");
                }
            }
        }
        
        // Step 2: Determine continuation point
        let next_step = if let Some(info) = self.read_next_step(session_dir).await {
            info
        } else if let Some(session) = &ctx.session_metadata {
            let step = self.calculate_from_steps_completed(session);
            NextStepInfo {
                step_number: step,
                description: format!("Continue from step {}", step),
                raw_content: String::new(),
            }
        } else {
            NextStepInfo {
                step_number: 1,
                description: "Continue workflow".to_string(),
                raw_content: String::new(),
            }
        };
        
        info!(
            step = next_step.step_number,
            description = %next_step.description,
            "Starting new session from step"
        );
        
        // Step 3: Generate prompt and create session
        let prompt = self.generate_prompt(&next_step);
        let new_session_path = self.create_session(&prompt, session_dir).await?;
        
        Ok(ResumeOutcome::success(
            new_session_path,
            format!("Started new session from step {}", next_step.step_number),
        ))
    }
    
    fn name(&self) -> &'static str {
        "NewSessionStrategy"
    }
}

impl Default for NewSessionStrategy {
    fn default() -> Self {
        Self::new()
    }
}
```

### Dependencies

Uses existing dependencies:
- `tokio` (already in Cargo.toml) - async runtime, fs, process
- `async-trait` (from Story 3.1) - async trait methods
- `tracing` (already in Cargo.toml) - structured logging

### Integration with Session Backup (Story 3.4)

```rust
// Before creating new session
if self.config.enable_backup {
    match self.backup.backup(&ctx.session_path).await {
        Ok(backup_path) => info!(backup = %backup_path.display(), "Backed up"),
        Err(e) => warn!(error = %e, "Backup failed, proceeding"),
    }
}
```

### Next-step.md Format

Expected formats that the parser should handle:

```markdown
# Step 5: Implement user authentication
Continue with the OAuth2 integration...

---

## Step 5
Implement user authentication

---

5. Implement authentication
```

### opencode Integration

**Research needed:**
1. How to create a new opencode session programmatically
2. How to pass initial prompt to new session
3. How to specify working directory
4. How to get new session file path

### Testing Strategy

**Unit Tests:**
- Test Next-step.md parsing with various formats
- Test stepsCompleted calculation
- Test prompt generation
- Mock opencode for session creation

**Integration Tests:**
- Test with actual file system
- Test backup + new session sequence
- Test error handling scenarios

### References

- [Source: _bmad-output/planning-artifacts/epics.md#Story 3.3: New-Session Resume Strategy]
- [Source: _bmad-output/implementation-artifacts/3-1-resume-strategy-trait.md]
- [Source: _bmad-output/implementation-artifacts/2-5-stop-reason-classification-context-exhaustion.md]

## File List

**Files to create:**
- `src/resume/new_session.rs`
- `tests/new_session_strategy_test.rs`

**Files to modify:**
- `src/resume/mod.rs` (add new_session module)
- `src/resume/selector.rs` (wire new-session strategy)
- `_bmad-output/implementation-artifacts/3-3-new-session-resume-strategy.md`
- `_bmad-output/implementation-artifacts/sprint-status.yaml`

## Change Log

- 2026-02-05: Story created and marked ready-for-dev
- 2026-02-05: Implemented NewSessionStrategy, added tests, and updated resume wiring
