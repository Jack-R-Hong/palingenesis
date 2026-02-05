# Story 3.8: Resume Command Implementation

Status: ready-for-dev

## Story

As a user,
I want to resume daemon monitoring after pausing,
So that auto-resume functionality is restored.

## Acceptance Criteria

**AC1: Resume Command Execution**
**Given** the daemon is in paused state
**When** I run `palingenesis resume`
**Then** the daemon transitions to monitoring state
**And** CLI displays "Monitoring resumed"

**AC2: Already Monitoring Handling**
**Given** the daemon is already monitoring
**When** I run `palingenesis resume`
**Then** CLI displays "Already monitoring"

**AC3: Restored Auto-Resume Behavior**
**Given** monitoring resumes
**When** a session stops (rate limit)
**Then** normal auto-resume behavior occurs

**AC4: Persistence**
**Given** monitoring is resumed
**When** the daemon restarts
**Then** it remains in monitoring state

**AC5: Audit Logging**
**Given** resume command is executed
**When** state changes
**Then** audit trail records the transition

**AC6: Immediate Resume Option**
**Given** the daemon is paused and a session is stopped
**When** I run `palingenesis resume --now`
**Then** it resumes monitoring AND triggers immediate resume action

## Tasks / Subtasks

- [ ] Add resume CLI command (AC: 1, 2, 6)
  - [ ] Add `resume` subcommand to CLI
  - [ ] Add `--now` flag for immediate action
  - [ ] Implement IPC message sending
  - [ ] Handle response display

- [ ] Implement RESUME IPC command (AC: 1, 3)
  - [ ] Add RESUME to IpcCommand enum
  - [ ] Add optional `immediate` field
  - [ ] Implement handler in IPC server
  - [ ] Send response to client

- [ ] Implement state transition (AC: 1, 4)
  - [ ] Implement Paused -> Monitoring transition
  - [ ] Persist monitoring state to disk
  - [ ] Emit state change event

- [ ] Handle already monitoring case (AC: 2)
  - [ ] Check current state in handler
  - [ ] Return specific response
  - [ ] Display appropriate message

- [ ] Implement immediate resume (AC: 6)
  - [ ] Check for pending stopped session
  - [ ] Trigger resume if session is stopped
  - [ ] Log immediate resume action

- [ ] Add audit logging (AC: 5)
  - [ ] Log state transition
  - [ ] Include immediate flag if used

- [ ] Add unit tests (AC: 1, 2, 3, 4, 5, 6)
  - [ ] Test resume command parsing
  - [ ] Test --now flag
  - [ ] Test IPC message handling
  - [ ] Test state transition
  - [ ] Test immediate resume trigger

- [ ] Add integration tests
  - [ ] Test full resume flow
  - [ ] Test resume -> session stop -> auto-resume
  - [ ] Test resume --now with stopped session

## Dev Notes

### Architecture Requirements

**From epics.md - Technical Notes:**

```
- Implements: FR19
- IPC command: RESUME
- State machine transition: Paused -> Monitoring
```

**State Machine:**

```
                    PAUSE
    ┌──────────┐  ───────►  ┌──────────┐
    │Monitoring│            │  Paused  │
    └──────────┘  ◄───────  └──────────┘
                   RESUME
```

**Implements:** FR19 (resume monitoring)

### Technical Implementation

**CLI Command:**

```rust
// src/cli/mod.rs
#[derive(Parser)]
pub enum Commands {
    // ... existing commands ...
    
    /// Resume daemon monitoring
    Resume {
        /// Also trigger immediate resume if session is stopped
        #[arg(long)]
        now: bool,
    },
}

// src/cli/resume.rs
use crate::ipc::{IpcClient, IpcCommand, IpcResponse};

pub async fn execute_resume(now: bool) -> Result<(), CliError> {
    let client = IpcClient::connect().await?;
    
    let response = client.send(IpcCommand::Resume { immediate: now }).await?;
    
    match response {
        IpcResponse::Ok(msg) => {
            println!("{}", msg);
            Ok(())
        }
        IpcResponse::Error(msg) => {
            eprintln!("Error: {}", msg);
            Err(CliError::CommandFailed(msg))
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
    
    /// Resume monitoring
    Resume {
        /// Trigger immediate resume of stopped session
        immediate: bool,
    },
}

// src/ipc/server.rs
impl IpcServer {
    async fn handle_resume(&self, immediate: bool) -> IpcResponse {
        let mut state = self.state.write().await;
        
        match state.daemon_state {
            DaemonState::Monitoring => {
                IpcResponse::Ok("Already monitoring".to_string())
            }
            DaemonState::Paused => {
                state.daemon_state = DaemonState::Monitoring;
                state.persist().await.ok();
                
                // Log to audit trail
                if let Some(audit) = &self.audit_logger {
                    let mut entry = AuditEntry::new(
                        AuditEventType::ConfigChanged,
                        "Monitoring resumed by user"
                    ).with_outcome(AuditOutcome::Success);
                    
                    if immediate {
                        entry = entry.with_metadata("immediate", true);
                    }
                    audit.log(&entry);
                }
                
                info!("Monitoring resumed");
                
                // Handle immediate resume
                if immediate {
                    if let Some(session) = &state.current_session {
                        if self.is_session_stopped(session).await {
                            self.trigger_immediate_resume(session).await;
                            return IpcResponse::Ok(
                                "Monitoring resumed and immediate resume triggered".to_string()
                            );
                        }
                    }
                }
                
                IpcResponse::Ok("Monitoring resumed".to_string())
            }
            _ => {
                IpcResponse::Error("Cannot resume: daemon not in paused state".to_string())
            }
        }
    }
    
    async fn is_session_stopped(&self, session: &Session) -> bool {
        // Check if session is in stopped state
        // Could check process, file state, etc.
        !session.is_active
    }
    
    async fn trigger_immediate_resume(&self, session: &Session) {
        info!(session = %session.path.display(), "Triggering immediate resume");
        
        // Send event to daemon to trigger resume
        if let Some(tx) = &self.resume_trigger {
            let _ = tx.send(ResumeRequest {
                session_path: session.path.clone(),
                reason: "User requested immediate resume".to_string(),
            }).await;
        }
    }
}
```

**Resume Trigger Channel:**

```rust
// For --now functionality, daemon needs to accept external resume triggers
pub struct ResumeRequest {
    pub session_path: PathBuf,
    pub reason: String,
}

// In daemon core
impl Daemon {
    async fn run(&mut self) {
        loop {
            tokio::select! {
                // Normal monitor events
                Some(event) = self.monitor_rx.recv() => {
                    self.handle_monitor_event(event).await;
                }
                
                // External resume triggers (from --now)
                Some(request) = self.resume_trigger_rx.recv() => {
                    self.handle_manual_resume(request).await;
                }
                
                // Shutdown
                _ = self.cancel.cancelled() => break,
            }
        }
    }
    
    async fn handle_manual_resume(&mut self, request: ResumeRequest) {
        info!(
            session = %request.session_path.display(),
            reason = %request.reason,
            "Processing manual resume request"
        );
        
        // Construct context and execute appropriate strategy
        // ...
    }
}
```

### Dependencies

Uses existing dependencies (no new dependencies needed):
- `clap` (already in Cargo.toml) - CLI framework
- `serde` (already in Cargo.toml) - state serialization
- `tokio` (already in Cargo.toml) - async runtime
- `tracing` (already in Cargo.toml) - logging

### Immediate Resume Flow

```
User runs: palingenesis resume --now
         │
         ▼
┌─────────────────────────────────┐
│ 1. Transition Paused->Monitoring │
│ 2. Check if session is stopped   │
│ 3. If stopped, trigger resume    │
└─────────────────────────────────┘
```

### Usage Examples

```bash
# Basic resume (just re-enable monitoring)
$ palingenesis resume
Monitoring resumed

# Resume and trigger immediate action
$ palingenesis resume --now
Monitoring resumed and immediate resume triggered

# Already monitoring
$ palingenesis resume
Already monitoring
```

### Testing Strategy

**Unit Tests:**
- Test CLI argument parsing with --now
- Test IPC command serialization
- Test state transition
- Test immediate flag handling

**Integration Tests:**
- Test full resume flow
- Test resume --now with stopped session
- Test resume --now without stopped session

### References

- [Source: _bmad-output/planning-artifacts/epics.md#Story 3.8: Resume Command Implementation]
- [Source: _bmad-output/implementation-artifacts/3-7-pause-command-implementation.md]
- [Source: _bmad-output/implementation-artifacts/1-6-unix-socket-ipc-server.md]

## File List

**Files to create:**
- `src/cli/resume_cmd.rs` (named to avoid conflict with resume module)
- `tests/resume_command_test.rs`

**Files to modify:**
- `src/cli/mod.rs` (add resume command)
- `src/ipc/commands.rs` (add RESUME command)
- `src/ipc/server.rs` (add handler)
- `src/daemon/core.rs` (add resume trigger channel)
- `_bmad-output/implementation-artifacts/sprint-status.yaml`

## Change Log

- 2026-02-05: Story created and marked ready-for-dev
