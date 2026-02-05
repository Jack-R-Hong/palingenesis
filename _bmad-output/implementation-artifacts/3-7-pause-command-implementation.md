# Story 3.7: Pause Command Implementation

Status: ready-for-dev

## Story

As a user,
I want to pause daemon monitoring,
So that I can work without auto-resume interference.

## Acceptance Criteria

**AC1: Pause Command Execution**
**Given** the daemon is in monitoring state
**When** I run `palingenesis pause`
**Then** the daemon transitions to paused state
**And** CLI displays "Monitoring paused"

**AC2: Paused Behavior**
**Given** the daemon is paused
**When** a session stops
**Then** the daemon does NOT auto-resume
**And** logs "Session stopped but monitoring is paused"

**AC3: Already Paused Handling**
**Given** the daemon is already paused
**When** I run `palingenesis pause`
**Then** CLI displays "Already paused"

**AC4: Status Reflection**
**Given** the daemon is paused
**When** I check status
**Then** status shows "State: paused"

**AC5: Persistence Across Restart**
**Given** the daemon is paused
**When** the daemon restarts
**Then** it remains in paused state

**AC6: Audit Logging**
**Given** pause command is executed
**When** state changes
**Then** audit trail records the transition

## Tasks / Subtasks

- [ ] Add pause CLI command (AC: 1, 3)
  - [ ] Add `pause` subcommand to CLI
  - [ ] Implement IPC message sending
  - [ ] Handle response display
  - [ ] Handle error cases

- [ ] Implement PAUSE IPC command (AC: 1, 2)
  - [ ] Add PAUSE to IpcCommand enum
  - [ ] Implement handler in IPC server
  - [ ] Send response to client
  - [ ] Return appropriate error if daemon not running

- [ ] Implement state transition (AC: 1, 5)
  - [ ] Add `Paused` variant to DaemonState enum
  - [ ] Implement Monitoring -> Paused transition
  - [ ] Persist paused state to disk
  - [ ] Load paused state on startup

- [ ] Modify resume logic for paused state (AC: 2)
  - [ ] Check daemon state before auto-resume
  - [ ] Skip resume when paused
  - [ ] Log skip reason
  - [ ] Emit monitor event for visibility

- [ ] Handle already paused case (AC: 3)
  - [ ] Check current state in handler
  - [ ] Return specific response for already paused
  - [ ] Display appropriate message in CLI

- [ ] Update status command (AC: 4)
  - [ ] Include state in status output
  - [ ] Format "State: paused" or "State: monitoring"
  - [ ] Include in JSON output

- [ ] Add audit logging (AC: 6)
  - [ ] Log state transition in audit trail
  - [ ] Include timestamp and reason

- [ ] Add unit tests (AC: 1, 2, 3, 4, 5, 6)
  - [ ] Test pause command parsing
  - [ ] Test IPC message handling
  - [ ] Test state transition
  - [ ] Test persistence
  - [ ] Test resume skip when paused
  - [ ] Test status reflection

- [ ] Add integration tests
  - [ ] Test full pause flow
  - [ ] Test pause -> session stop -> no resume
  - [ ] Test restart with paused state

## Dev Notes

### Architecture Requirements

**From epics.md - Technical Notes:**

```
- Implements: FR18
- IPC command: PAUSE
- State machine transition: Monitoring -> Paused
```

**State Machine:**

```
                    PAUSE
    ┌──────────┐  ───────►  ┌──────────┐
    │Monitoring│            │  Paused  │
    └──────────┘  ◄───────  └──────────┘
                   RESUME
```

**Implements:** FR18 (pause monitoring)

### Technical Implementation

**CLI Command:**

```rust
// src/cli/mod.rs
#[derive(Parser)]
pub enum Commands {
    // ... existing commands ...
    
    /// Pause daemon monitoring (disable auto-resume)
    Pause,
    
    /// Resume daemon monitoring
    Resume,
}

// src/cli/pause.rs
use crate::ipc::{IpcClient, IpcCommand, IpcResponse};

pub async fn execute_pause() -> Result<(), CliError> {
    let client = IpcClient::connect().await?;
    
    let response = client.send(IpcCommand::Pause).await?;
    
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
    
    /// Pause monitoring
    Pause,
    
    /// Resume monitoring
    Resume,
}

// src/ipc/server.rs
impl IpcServer {
    async fn handle_command(&self, cmd: IpcCommand) -> IpcResponse {
        match cmd {
            IpcCommand::Pause => self.handle_pause().await,
            // ...
        }
    }
    
    async fn handle_pause(&self) -> IpcResponse {
        let mut state = self.state.write().await;
        
        match state.daemon_state {
            DaemonState::Paused => {
                IpcResponse::Ok("Already paused".to_string())
            }
            DaemonState::Monitoring => {
                state.daemon_state = DaemonState::Paused;
                state.persist().await.ok();
                
                // Log to audit trail
                if let Some(audit) = &self.audit_logger {
                    audit.log(&AuditEntry::new(
                        AuditEventType::ConfigChanged,
                        "Monitoring paused by user"
                    ).with_outcome(AuditOutcome::Success));
                }
                
                info!("Monitoring paused");
                IpcResponse::Ok("Monitoring paused".to_string())
            }
            _ => {
                IpcResponse::Error("Cannot pause: daemon not in monitoring state".to_string())
            }
        }
    }
}
```

**DaemonState Update:**

```rust
// src/daemon/state.rs
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DaemonState {
    Starting,
    Monitoring,
    Paused,      // NEW: Added for pause functionality
    Resuming,
    Stopping,
}

impl DaemonState {
    /// Check if auto-resume should occur.
    pub fn should_auto_resume(&self) -> bool {
        matches!(self, DaemonState::Monitoring)
    }
}
```

**Resume Logic Check:**

```rust
// src/daemon/core.rs
impl Daemon {
    async fn handle_session_stopped(&mut self, event: MonitorEvent) {
        // Check if paused before attempting resume
        if !self.state.daemon_state.should_auto_resume() {
            info!(
                state = ?self.state.daemon_state,
                "Session stopped but monitoring is paused"
            );
            return;
        }
        
        // Normal resume logic...
    }
}
```

### Dependencies

Uses existing dependencies (no new dependencies needed):
- `clap` (already in Cargo.toml) - CLI framework
- `serde` (already in Cargo.toml) - state serialization
- `tokio` (already in Cargo.toml) - async runtime
- `tracing` (already in Cargo.toml) - logging

### Integration with Story 2.7 (Monitor Event Channel)

When paused, the daemon still receives monitor events but doesn't act on them:

```rust
MonitorEvent::SessionStopped { reason, session, .. } => {
    if self.state.daemon_state == DaemonState::Paused {
        debug!(reason = ?reason, "Ignoring stop event while paused");
        return;
    }
    // Normal handling...
}
```

### Status Output Format

```
$ palingenesis status
palingenesis daemon: running (PID: 12345)
State: paused                          # <-- Shows paused state
Uptime: 2h 30m
Current session: /path/to/session.md
```

### Testing Strategy

**Unit Tests:**
- Test CLI argument parsing
- Test IPC command serialization
- Test state transition logic
- Test persistence of paused state

**Integration Tests:**
- Test full pause flow end-to-end
- Test that session stops don't trigger resume when paused
- Test restart preserves paused state

### References

- [Source: _bmad-output/planning-artifacts/epics.md#Story 3.7: Pause Command Implementation]
- [Source: _bmad-output/implementation-artifacts/1-6-unix-socket-ipc-server.md]
- [Source: _bmad-output/implementation-artifacts/1-4-state-persistence-layer.md]

## File List

**Files to create:**
- `src/cli/pause.rs`
- `tests/pause_command_test.rs`

**Files to modify:**
- `src/cli/mod.rs` (add pause command)
- `src/ipc/commands.rs` (add PAUSE command)
- `src/ipc/server.rs` (add handler)
- `src/daemon/state.rs` (add Paused state)
- `src/daemon/core.rs` (check paused before resume)
- `_bmad-output/implementation-artifacts/sprint-status.yaml`

## Change Log

- 2026-02-05: Story created and marked ready-for-dev
