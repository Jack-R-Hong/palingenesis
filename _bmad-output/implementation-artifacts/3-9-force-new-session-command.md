# Story 3.9: Force New Session Command

Status: ready-for-dev

## Story

As a user,
I want to force a new session manually,
So that I can recover from stuck states or start fresh.

## Acceptance Criteria

**AC1: New Session Command Execution**
**Given** a session is active or stopped
**When** I run `palingenesis new-session`
**Then** the daemon backs up the current session
**And** starts a new session from Next-step.md
**And** CLI displays "New session started"

**AC2: No Session Handling**
**Given** no session exists
**When** I run `palingenesis new-session`
**Then** CLI displays "No active session to replace"
**And** exits with code 1

**AC3: Backup Failure Handling**
**Given** backup fails during new-session
**When** the error occurs
**Then** it warns but proceeds anyway

**AC4: Custom Prompt Option**
**Given** user wants custom starting point
**When** I run `palingenesis new-session --prompt "Start from step 5"`
**Then** new session uses the custom prompt instead of Next-step.md

**AC5: Skip Backup Option**
**Given** user doesn't want backup
**When** I run `palingenesis new-session --no-backup`
**Then** no backup is created before new session

**AC6: Audit Logging**
**Given** new-session command is executed
**When** new session is created
**Then** audit trail records the forced transition

## Tasks / Subtasks

- [ ] Add new-session CLI command (AC: 1, 2, 4, 5)
  - [ ] Add `new-session` subcommand to CLI
  - [ ] Add `--prompt` option for custom prompt
  - [ ] Add `--no-backup` flag
  - [ ] Implement IPC message sending
  - [ ] Handle response display

- [ ] Implement NEW-SESSION IPC command (AC: 1, 3)
  - [ ] Add NEW_SESSION to IpcCommand enum
  - [ ] Include prompt and skip_backup fields
  - [ ] Implement handler in IPC server
  - [ ] Return session path on success

- [ ] Implement force new session logic (AC: 1, 3)
  - [ ] Call session backup (unless skipped)
  - [ ] Handle backup failure gracefully
  - [ ] Read Next-step.md or use custom prompt
  - [ ] Create new session
  - [ ] Update daemon state

- [ ] Handle no session case (AC: 2)
  - [ ] Check for current session
  - [ ] Return specific error response
  - [ ] CLI exits with code 1

- [ ] Add audit logging (AC: 6)
  - [ ] Log forced session creation
  - [ ] Include backup status
  - [ ] Include prompt source

- [ ] Add unit tests (AC: 1, 2, 3, 4, 5, 6)
  - [ ] Test command parsing with options
  - [ ] Test IPC message handling
  - [ ] Test backup + new session flow
  - [ ] Test no session error
  - [ ] Test custom prompt
  - [ ] Test skip backup

- [ ] Add integration tests
  - [ ] Test full new-session flow
  - [ ] Test with backup failure simulation
  - [ ] Test with custom prompt

## Dev Notes

### Architecture Requirements

**From epics.md - Technical Notes:**

```
- Implements: FR20
- IPC command: NEW-SESSION
- Forces the new-session strategy regardless of stop reason
```

**Command Flow:**

```
User runs: palingenesis new-session
         │
         ▼
┌─────────────────────────────┐
│ 1. Check session exists     │
│ 2. Backup session (optional)│
│ 3. Read Next-step.md        │
│ 4. Create new session       │
│ 5. Log to audit trail       │
└─────────────────────────────┘
```

**Implements:** FR20 (force new session)

### Technical Implementation

**CLI Command:**

```rust
// src/cli/mod.rs
#[derive(Parser)]
pub enum Commands {
    // ... existing commands ...
    
    /// Force creation of a new session (backup + fresh start)
    #[command(name = "new-session")]
    NewSession {
        /// Custom prompt instead of Next-step.md
        #[arg(long)]
        prompt: Option<String>,
        
        /// Skip session backup
        #[arg(long)]
        no_backup: bool,
    },
}

// src/cli/new_session.rs
use crate::ipc::{IpcClient, IpcCommand, IpcResponse};

pub async fn execute_new_session(
    prompt: Option<String>,
    no_backup: bool,
) -> Result<(), CliError> {
    let client = IpcClient::connect().await?;
    
    let response = client.send(IpcCommand::NewSession {
        custom_prompt: prompt,
        skip_backup: no_backup,
    }).await?;
    
    match response {
        IpcResponse::SessionCreated { path, message } => {
            println!("{}", message);
            println!("New session: {}", path.display());
            Ok(())
        }
        IpcResponse::Error(msg) => {
            eprintln!("Error: {}", msg);
            std::process::exit(1);
        }
        _ => {
            eprintln!("Unexpected response");
            std::process::exit(1);
        }
    }
}
```

**IPC Command:**

```rust
// src/ipc/commands.rs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum IpcCommand {
    // ... existing commands ...
    
    /// Force new session creation
    NewSession {
        /// Custom prompt (None = use Next-step.md)
        custom_prompt: Option<String>,
        /// Skip backup
        skip_backup: bool,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum IpcResponse {
    // ... existing variants ...
    
    /// New session created successfully
    SessionCreated {
        path: PathBuf,
        message: String,
    },
}

// src/ipc/server.rs
impl IpcServer {
    async fn handle_new_session(
        &self,
        custom_prompt: Option<String>,
        skip_backup: bool,
    ) -> IpcResponse {
        let state = self.state.read().await;
        
        // Check if session exists
        let session = match &state.current_session {
            Some(s) => s.clone(),
            None => {
                return IpcResponse::Error("No active session to replace".to_string());
            }
        };
        drop(state);
        
        // Backup if not skipped
        let backup_result = if !skip_backup {
            match self.backup_session(&session.path).await {
                Ok(backup_path) => {
                    info!(backup = %backup_path.display(), "Session backed up");
                    Some(backup_path)
                }
                Err(e) => {
                    warn!(error = %e, "Backup failed, proceeding anyway");
                    None
                }
            }
        } else {
            info!("Backup skipped by user request");
            None
        };
        
        // Determine prompt
        let prompt = match custom_prompt {
            Some(p) => p,
            None => self.read_next_step_prompt(&session.path).await
                .unwrap_or_else(|| "Continue workflow".to_string()),
        };
        
        // Create new session
        match self.create_new_session(&prompt, &session.path).await {
            Ok(new_path) => {
                // Log to audit
                if let Some(audit) = &self.audit_logger {
                    let mut entry = AuditEntry::new(
                        AuditEventType::SessionCreated,
                        "Forced new session by user"
                    )
                    .with_session(new_path.clone())
                    .with_outcome(AuditOutcome::Success);
                    
                    if let Some(backup) = &backup_result {
                        entry = entry.with_metadata("backup_path", backup.display().to_string());
                    }
                    entry = entry.with_metadata("prompted", custom_prompt.is_some());
                    
                    audit.log(&entry);
                }
                
                // Update state
                let mut state = self.state.write().await;
                state.current_session = Some(Session::new(new_path.clone()));
                state.persist().await.ok();
                
                IpcResponse::SessionCreated {
                    path: new_path,
                    message: "New session started".to_string(),
                }
            }
            Err(e) => {
                error!(error = %e, "Failed to create new session");
                IpcResponse::Error(format!("Failed to create session: {}", e))
            }
        }
    }
    
    async fn backup_session(&self, path: &Path) -> Result<PathBuf, BackupError> {
        let backup = SessionBackup::new(10);
        backup.backup(path).await
    }
    
    async fn read_next_step_prompt(&self, session_path: &Path) -> Option<String> {
        let dir = session_path.parent()?;
        let next_step_path = dir.join("Next-step.md");
        
        tokio::fs::read_to_string(&next_step_path).await.ok()
    }
    
    async fn create_new_session(
        &self,
        prompt: &str,
        old_session_path: &Path,
    ) -> Result<PathBuf, ResumeError> {
        let dir = old_session_path.parent()
            .ok_or_else(|| ResumeError::SessionNotFound {
                path: old_session_path.to_path_buf(),
            })?;
        
        // Execute opencode to create new session
        let output = tokio::process::Command::new("opencode")
            .arg("new")
            .arg("--prompt")
            .arg(prompt)
            .arg("--workdir")
            .arg(dir)
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
        
        // Parse new session path from output
        let stdout = String::from_utf8_lossy(&output.stdout);
        let session_path = stdout.lines()
            .find(|line| line.contains("session:"))
            .and_then(|line| line.split("session:").nth(1))
            .map(|s| PathBuf::from(s.trim()))
            .unwrap_or_else(|| dir.join("session.md"));
        
        Ok(session_path)
    }
}
```

### Dependencies

Uses existing dependencies:
- `clap` (already in Cargo.toml) - CLI framework
- `tokio` (already in Cargo.toml) - async runtime
- `tracing` (already in Cargo.toml) - logging

Integrates with:
- Story 3.4 (SessionBackup)
- Story 3.3 (NewSessionStrategy patterns)
- Story 3.6 (AuditLogger)

### Usage Examples

```bash
# Basic new session (backup + Next-step.md)
$ palingenesis new-session
New session started
New session: /home/user/.opencode/sessions/session-123.md

# Custom prompt
$ palingenesis new-session --prompt "Start implementing the authentication module"
New session started
New session: /home/user/.opencode/sessions/session-124.md

# Skip backup
$ palingenesis new-session --no-backup
New session started
New session: /home/user/.opencode/sessions/session-125.md

# No session error
$ palingenesis new-session
Error: No active session to replace
```

### Testing Strategy

**Unit Tests:**
- Test CLI argument parsing
- Test IPC command serialization
- Test no session error handling
- Test backup integration
- Test custom prompt handling

**Integration Tests:**
- Test full new-session flow
- Test with backup failure (mock)
- Test prompt from Next-step.md
- Test custom prompt override

### References

- [Source: _bmad-output/planning-artifacts/epics.md#Story 3.9: Force New Session Command]
- [Source: _bmad-output/implementation-artifacts/3-3-new-session-resume-strategy.md]
- [Source: _bmad-output/implementation-artifacts/3-4-session-backup-before-new-session.md]
- [Source: _bmad-output/implementation-artifacts/3-6-audit-trail-logging.md]

## File List

**Files to create:**
- `src/cli/new_session.rs`
- `tests/new_session_command_test.rs`

**Files to modify:**
- `src/cli/mod.rs` (add new-session command)
- `src/ipc/commands.rs` (add NEW_SESSION command, SessionCreated response)
- `src/ipc/server.rs` (add handler)
- `_bmad-output/implementation-artifacts/sprint-status.yaml`

## Change Log

- 2026-02-05: Story created and marked ready-for-dev
