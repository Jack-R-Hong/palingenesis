# Story 4.6: Hot Reload via SIGHUP

Status: done

## Story

As a user,
I want the daemon to reload config without restarting,
So that I can update settings without interrupting monitoring.

## Acceptance Criteria

**AC1: CLI Reload Command**
**Given** the daemon is running
**When** I run `palingenesis daemon reload`
**Then** SIGHUP is sent to the daemon process

**AC2: SIGHUP Handling**
**Given** the daemon receives SIGHUP
**When** it handles the signal
**Then** it re-reads the config file
**And** applies new settings
**And** logs "Configuration reloaded"

**AC3: Invalid Config Protection**
**Given** the new config is invalid
**When** reload is attempted
**Then** the daemon logs error "Invalid config, keeping current"
**And** continues with the old config

**AC4: Immediate Effect**
**Given** certain settings change (e.g., check_interval)
**When** config is reloaded
**Then** the new value takes effect immediately

**AC5: Non-Reloadable Settings**
**Given** certain settings cannot be changed at runtime (e.g., socket path)
**When** they are changed in config
**Then** a warning is logged "Setting X requires restart to take effect"

**AC6: IPC Reload Command**
**Given** the daemon is running
**When** `RELOAD` is sent via IPC socket
**Then** config is reloaded (alternative to SIGHUP)

## Tasks / Subtasks

- [x] Add daemon reload subcommand to CLI (AC: 1)
  - [x] Add `reload` subcommand to `DaemonCmd` enum
  - [x] Implement sending SIGHUP to daemon process
  - [x] Use PID from PID file

- [x] Implement SIGHUP signal handler (AC: 2)
  - [x] Register SIGHUP handler in daemon startup
  - [x] Use tokio signal handling
  - [x] Trigger config reload on signal

- [x] Implement config reload logic (AC: 2, 4)
  - [x] Create `reload_config()` method on daemon
  - [x] Re-read config file from disk
  - [x] Parse and validate new config
  - [x] Apply new settings to running daemon

- [x] Implement validation during reload (AC: 3)
  - [x] Validate new config before applying
  - [x] If invalid, log error and keep current
  - [x] Return error status for CLI feedback

- [x] Track non-reloadable settings (AC: 5)
  - [x] Define list of settings that require restart
  - [x] Compare old vs new config
  - [x] Log warnings for changed non-reloadable settings

- [x] Apply reloadable settings (AC: 4)
  - [x] Update log level dynamically
  - [x] Update resume timing settings
  - [x] Update notification settings
  - [x] Update monitoring debounce

- [x] Implement IPC RELOAD command (AC: 6)
  - [x] Add `RELOAD` to IPC protocol
  - [x] Handle in IPC server
  - [x] Return success/error response

- [x] Add logging (AC: 2, 3, 5)
  - [x] Log "Configuration reloaded" on success
  - [x] Log "Invalid config, keeping current" on failure
  - [x] Log warnings for non-reloadable settings

- [x] Add unit/integration tests (AC: 1, 2, 3, 4, 5, 6)
  - [x] Test SIGHUP handler triggers reload
  - [x] Test reload with valid config
  - [x] Test reload with invalid config
  - [x] Test non-reloadable setting warnings
  - [x] Test IPC RELOAD command

## Dev Notes

### Architecture Requirements

**From architecture.md - Daemon Module:**

```
src/daemon/
    signals.rs                # SIGTERM, SIGHUP, SIGINT
```

**From architecture.md - IPC Protocol:**

```
RELOAD  -> OK / ERR (config reload)
```

**Implements:** FR24 (Daemon can reload config without restart)

### Technical Implementation

**CLI Reload Command:**

```rust
// src/cli/commands/daemon.rs
use std::fs;

use nix::sys::signal::{kill, Signal};
use nix::unistd::Pid;

use crate::daemon::pid::read_pid_file;

pub fn handle_reload() -> anyhow::Result<()> {
    let pid = read_pid_file()?;
    
    // Send SIGHUP
    kill(Pid::from_raw(pid), Signal::SIGHUP)?;
    
    println!("Sent reload signal to daemon (PID: {})", pid);
    Ok(())
}
```

**SIGHUP Handler in Daemon:**

```rust
// src/daemon/signals.rs
use tokio::signal::unix::{signal, SignalKind};
use tokio::sync::mpsc;

pub enum DaemonSignal {
    Shutdown,
    Reload,
}

pub async fn signal_handler(tx: mpsc::Sender<DaemonSignal>) -> anyhow::Result<()> {
    let mut sigterm = signal(SignalKind::terminate())?;
    let mut sigint = signal(SignalKind::interrupt())?;
    let mut sighup = signal(SignalKind::hangup())?;
    
    loop {
        tokio::select! {
            _ = sigterm.recv() => {
                tracing::info!("Received SIGTERM, initiating shutdown");
                tx.send(DaemonSignal::Shutdown).await?;
                break;
            }
            _ = sigint.recv() => {
                tracing::info!("Received SIGINT, initiating shutdown");
                tx.send(DaemonSignal::Shutdown).await?;
                break;
            }
            _ = sighup.recv() => {
                tracing::info!("Received SIGHUP, reloading configuration");
                tx.send(DaemonSignal::Reload).await?;
            }
        }
    }
    
    Ok(())
}
```

**Config Reload Logic:**

```rust
// src/daemon/core.rs
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::config::loader::load_config;
use crate::config::schema::Config;
use crate::config::validation::validate_config;

pub struct Daemon {
    config: Arc<RwLock<Config>>,
    // ... other fields
}

impl Daemon {
    pub async fn reload_config(&self) -> anyhow::Result<()> {
        // Load new config
        let new_config = match load_config() {
            Ok(c) => c,
            Err(e) => {
                tracing::error!("Failed to load config: {}", e);
                tracing::warn!("Keeping current configuration");
                return Err(e);
            }
        };
        
        // Validate
        let validation = validate_config(&new_config);
        if !validation.is_valid() {
            tracing::error!("Invalid config, keeping current");
            for err in validation.errors {
                tracing::error!("  {}: {}", err.field, err.message);
            }
            anyhow::bail!("Configuration validation failed");
        }
        
        // Check non-reloadable settings
        let current = self.config.read().await;
        check_non_reloadable_changes(&current, &new_config);
        drop(current);
        
        // Apply new config
        let mut config = self.config.write().await;
        *config = new_config;
        
        tracing::info!("Configuration reloaded");
        Ok(())
    }
}

/// Non-reloadable settings that require restart
const NON_RELOADABLE: &[&str] = &[
    "daemon.pid_file",
    "daemon.socket_path",
    "daemon.http_bind",
    "daemon.http_port",
];

fn check_non_reloadable_changes(old: &Config, new: &Config) {
    if old.daemon.pid_file != new.daemon.pid_file {
        tracing::warn!("daemon.pid_file changed - requires restart");
    }
    if old.daemon.socket_path != new.daemon.socket_path {
        tracing::warn!("daemon.socket_path changed - requires restart");
    }
    if old.daemon.http_bind != new.daemon.http_bind {
        tracing::warn!("daemon.http_bind changed - requires restart");
    }
    if old.daemon.http_port != new.daemon.http_port {
        tracing::warn!("daemon.http_port changed - requires restart");
    }
}
```

**IPC RELOAD Handler:**

```rust
// src/ipc/protocol.rs
pub enum IpcCommand {
    Status,
    Pause,
    Resume,
    Reload,  // NEW
    NewSession,
}

// src/ipc/socket.rs
IpcCommand::Reload => {
    match daemon.reload_config().await {
        Ok(_) => "OK\n".to_string(),
        Err(e) => format!("ERR {}\n", e),
    }
}
```

### Reloadable vs Non-Reloadable Settings

| Setting | Reloadable | Notes |
|---------|------------|-------|
| `daemon.log_level` | Yes | Can update tracing subscriber |
| `daemon.log_file` | No | File handles already open |
| `daemon.pid_file` | No | Already created |
| `daemon.socket_path` | No | Already bound |
| `daemon.http_port` | No | Already bound |
| `monitoring.debounce_ms` | Yes | Apply to watcher |
| `monitoring.auto_detect` | Yes | Re-scan on reload |
| `resume.*` | Yes | All timing settings |
| `notifications.*` | Yes | All notification settings |
| `otel.*` | No | Telemetry pipeline complex |

### Dependencies

Uses existing dependencies:
- `nix` for signal handling (already in Cargo.toml)
- `tokio::signal` for async signal handling
- Config loader and validator from previous stories

### Testing Strategy

**Unit Tests:**
- Test check_non_reloadable_changes detection
- Test reload with valid config
- Test reload with invalid config

**Integration Tests:**
- Test CLI reload command sends SIGHUP
- Test daemon handles SIGHUP
- Test IPC RELOAD command
- Test non-reloadable warnings

### References

- [Source: _bmad-output/planning-artifacts/architecture.md#Daemon Module]
- [Source: _bmad-output/planning-artifacts/architecture.md#IPC Protocol]
- [Source: _bmad-output/planning-artifacts/epics.md#Story 4.6: Hot Reload via SIGHUP]

## File List

**Files to create:**
- None

**Files to modify:**
- `src/daemon/signals.rs`
- `src/daemon/core.rs`
- `src/daemon/state.rs`
- `src/cli/commands/daemon.rs`
- `_bmad-output/implementation-artifacts/sprint-status.yaml`

## Change Log

- 2026-02-05: Story created and marked ready-for-dev
- 2026-02-05: Implemented SIGHUP reload handling, config reload logic, and tests
