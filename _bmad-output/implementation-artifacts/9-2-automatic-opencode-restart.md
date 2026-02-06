# Story 9.2: Automatic OpenCode Restart

Status: ready-for-dev

## Story

As a daemon,
I want to automatically restart OpenCode when it crashes or exits unexpectedly,
So that AI workflow execution continues without manual intervention.

## Acceptance Criteria

**AC1: Restart on Crash**
**Given** OpenCode process crashes (non-zero exit code)
**When** the daemon detects the crash
**Then** it waits `restart_delay_ms` (default 1000ms)
**And** restarts OpenCode via `opencode serve`

**AC2: Restart on Unexpected Exit**
**Given** OpenCode exits normally but unexpectedly
**When** `auto_restart` is enabled in config
**Then** daemon restarts OpenCode automatically
**And** logs "Restarting OpenCode after unexpected exit"

**AC3: No Restart on User Stop**
**Given** user explicitly stops OpenCode (SIGTERM/SIGINT)
**When** daemon detects the signal
**Then** it does NOT auto-restart
**And** logs "OpenCode stopped by user, not restarting"

**AC4: Configurable Restart Behavior**
**Given** config has `opencode.auto_restart = false`
**When** OpenCode crashes
**Then** daemon only logs the crash
**And** does NOT attempt restart

**AC5: Restart Backoff**
**Given** OpenCode keeps crashing (3+ times in 60 seconds)
**When** rapid crash loop detected
**Then** daemon backs off restart attempts
**And** logs warning "OpenCode crash loop detected, backing off"
**And** waits progressively longer between attempts

**AC6: Command Customization**
**Given** config has `opencode.serve_command`
**When** restart is triggered
**Then** it uses the configured command instead of default
**And** passes configured port/hostname arguments

## Tasks / Subtasks

- [ ] Create restart manager module (AC: 1, 2, 3, 4, 5)
  - [ ] Create `src/opencode/restart.rs` module
  - [ ] Define `RestartManager` struct with config reference
  - [ ] Define `RestartPolicy` enum (Always, OnCrash, Never)
  - [ ] Implement crash loop detection with sliding window counter

- [ ] Implement restart trigger logic (AC: 1, 2, 3)
  - [ ] Add `should_restart()` method to RestartManager
  - [ ] Check exit reason: crash vs signal vs normal
  - [ ] Respect `auto_restart` config setting
  - [ ] Track consecutive crash count for backoff

- [ ] Implement OpenCode process spawning (AC: 1, 6)
  - [ ] Add `spawn_opencode()` async method
  - [ ] Build command: `opencode serve --port {port} --hostname {hostname}`
  - [ ] Handle custom `serve_command` override
  - [ ] Capture process handle for monitoring

- [ ] Implement restart backoff strategy (AC: 5)
  - [ ] Define `RestartBackoff` struct with exponential delay
  - [ ] Base delay: `restart_delay_ms` (default 1000ms)
  - [ ] Max delay: 60 seconds
  - [ ] Reset backoff on successful run (>60s uptime)

- [ ] Integrate with OpenCodeMonitor (AC: 1, 2, 3, 4)
  - [ ] Extend event handler in daemon core
  - [ ] On `OpenCodeStopped` event, invoke RestartManager
  - [ ] Pass exit reason for restart decision
  - [ ] Emit `OpenCodeRestarting` event before restart

- [ ] Add configuration options (AC: 4, 6)
  - [ ] Add `opencode.auto_restart` config option (default: true)
  - [ ] Add `opencode.restart_delay_ms` config option (default: 1000)
  - [ ] Add `opencode.max_restart_attempts` config option (default: 5)
  - [ ] Add `opencode.serve_command` config option (default: "opencode")

- [ ] Add unit/integration tests (AC: 1, 2, 3, 4, 5, 6)
  - [ ] Test restart on crash exit code
  - [ ] Test no restart on signal termination
  - [ ] Test crash loop backoff behavior
  - [ ] Test config respects auto_restart=false
  - [ ] Test custom serve_command usage
  - [ ] Test restart delay is honored

## Dev Notes

### Architecture Requirements

**From architecture.md - FR46 (OpenCode Auto-Restart):**
> Daemon automatically restarts OpenCode via `opencode serve`

**From architecture.md - Module Location:**
```
src/opencode/
    mod.rs                    # OpenCode integration root
    process.rs                # Process monitoring (Story 9.1)
    restart.rs                # Restart logic (THIS STORY)
    client.rs                 # HTTP client (Story 9.3)
```

### Technical Implementation

**Restart Manager:**

```rust
// src/opencode/restart.rs
use std::time::{Duration, Instant};
use tokio::process::Command;

/// Manages OpenCode restart logic
pub struct RestartManager {
    config: OpenCodeConfig,
    crash_times: Vec<Instant>,
    restart_count: u32,
    last_restart: Option<Instant>,
}

impl RestartManager {
    pub fn new(config: OpenCodeConfig) -> Self {
        Self {
            config,
            crash_times: Vec::new(),
            restart_count: 0,
            last_restart: None,
        }
    }

    /// Determine if we should restart based on exit reason
    pub fn should_restart(&mut self, reason: &ExitReason) -> bool {
        if !self.config.auto_restart {
            return false;
        }

        match reason {
            ExitReason::NormalExit => {
                // Normal exit might still warrant restart
                // (e.g., opencode finished but we want it running)
                self.config.restart_on_normal_exit
            }
            ExitReason::Crash { .. } => {
                // Always restart on crash if enabled
                !self.is_crash_loop()
            }
            ExitReason::Signal { signal } => {
                // Don't restart if user sent SIGTERM/SIGINT
                !matches!(*signal, 2 | 15) // SIGINT=2, SIGTERM=15
            }
            ExitReason::Unknown => true,
        }
    }

    /// Check if we're in a crash loop (3+ crashes in 60s)
    fn is_crash_loop(&mut self) -> bool {
        let now = Instant::now();
        let window = Duration::from_secs(60);
        
        // Remove old entries
        self.crash_times.retain(|t| now.duration_since(*t) < window);
        
        // Add current crash
        self.crash_times.push(now);
        
        // Check if too many crashes
        self.crash_times.len() >= 3
    }

    /// Calculate restart delay with backoff
    pub fn restart_delay(&self) -> Duration {
        let base = Duration::from_millis(self.config.restart_delay_ms);
        let multiplier = 2u32.pow(self.restart_count.min(5));
        let delay = base * multiplier;
        
        // Cap at 60 seconds
        delay.min(Duration::from_secs(60))
    }

    /// Spawn OpenCode process
    pub async fn spawn_opencode(&mut self) -> anyhow::Result<std::process::Child> {
        let cmd = self.config.serve_command.as_deref().unwrap_or("opencode");
        
        tracing::info!(
            command = cmd,
            port = self.config.serve_port,
            hostname = %self.config.serve_hostname,
            "Spawning OpenCode"
        );

        let child = Command::new(cmd)
            .arg("serve")
            .arg("--port")
            .arg(self.config.serve_port.to_string())
            .arg("--hostname")
            .arg(&self.config.serve_hostname)
            .spawn()?;

        self.restart_count += 1;
        self.last_restart = Some(Instant::now());
        
        Ok(child)
    }

    /// Reset backoff after successful run
    pub fn reset_backoff(&mut self) {
        if let Some(last) = self.last_restart {
            if last.elapsed() > Duration::from_secs(60) {
                self.restart_count = 0;
                self.crash_times.clear();
            }
        }
    }
}
```

**Daemon Integration:**

```rust
// src/daemon/core.rs (additions to event handler)
fn spawn_opencode_event_handler(&self, mut rx: mpsc::Receiver<OpenCodeEvent>) {
    let config = self.config.clone();
    let cancel = self.cancel_token.clone();
    let event_tx = self.opencode_event_tx.clone();
    
    tokio::spawn(async move {
        let mut restart_mgr = RestartManager::new(config.read().await.opencode.clone());
        
        loop {
            tokio::select! {
                _ = cancel.cancelled() => break,
                Some(event) = rx.recv() => {
                    match event {
                        OpenCodeEvent::Started { pid } => {
                            tracing::info!(pid, "OpenCode started");
                            restart_mgr.reset_backoff();
                        }
                        OpenCodeEvent::Stopped { reason } => {
                            tracing::warn!(?reason, "OpenCode stopped");
                            
                            if restart_mgr.should_restart(&reason) {
                                let delay = restart_mgr.restart_delay();
                                tracing::info!(?delay, "Scheduling OpenCode restart");
                                
                                tokio::time::sleep(delay).await;
                                
                                match restart_mgr.spawn_opencode().await {
                                    Ok(_) => {
                                        tracing::info!("OpenCode restarted");
                                    }
                                    Err(e) => {
                                        tracing::error!(error = %e, "Failed to restart OpenCode");
                                    }
                                }
                            } else {
                                tracing::info!("Not restarting OpenCode (policy/crash-loop)");
                            }
                        }
                        OpenCodeEvent::HealthCheckFailed { error } => {
                            tracing::warn!(error, "OpenCode health check failed");
                        }
                    }
                }
            }
        }
    });
}
```

**Configuration Schema Update:**

```rust
// src/config/schema.rs (additions to OpenCodeConfig)
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OpenCodeConfig {
    /// Enable OpenCode process management
    #[serde(default)]
    pub enabled: bool,
    
    /// Port for OpenCode health checks and serve
    #[serde(default = "default_serve_port")]
    pub serve_port: u16,
    
    /// Hostname for OpenCode serve
    #[serde(default = "default_serve_hostname")]
    pub serve_hostname: String,
    
    /// Auto-restart OpenCode on crash
    #[serde(default = "default_auto_restart")]
    pub auto_restart: bool,
    
    /// Delay before restart in milliseconds
    #[serde(default = "default_restart_delay")]
    pub restart_delay_ms: u64,
    
    /// Maximum restart attempts before giving up
    #[serde(default = "default_max_restart_attempts")]
    pub max_restart_attempts: u32,
    
    /// Custom serve command (default: "opencode")
    #[serde(default)]
    pub serve_command: Option<String>,
    
    /// Restart on normal exit (not just crash)
    #[serde(default)]
    pub restart_on_normal_exit: bool,
    
    // ... existing fields
}

fn default_serve_port() -> u16 { 4096 }
fn default_serve_hostname() -> String { "localhost".to_string() }
fn default_auto_restart() -> bool { true }
fn default_restart_delay() -> u64 { 1000 }
fn default_max_restart_attempts() -> u32 { 5 }
```

### OpenCode Context

**Default OpenCode Configuration:**
- Port: 4096
- Hostname: localhost
- Command: `opencode serve`

**Restart Triggers:**
1. Process crash (non-zero exit) -> Always restart (if enabled)
2. Signal kill (SIGKILL) -> Restart (unexpected termination)
3. User stop (SIGTERM/SIGINT) -> Do NOT restart
4. Normal exit (code 0) -> Configurable

### Dependencies

No new dependencies needed. Uses existing:
- `tokio::process::Command` for spawning
- `tracing` for logging

### Testing Strategy

**Unit Tests:**
- Test `should_restart()` for different exit reasons
- Test crash loop detection logic
- Test restart delay backoff calculation

**Integration Tests:**
- Mock process spawning
- Test event-driven restart flow
- Test config option effects

**Manual Testing:**
1. Start daemon with `opencode.enabled = true, auto_restart = true`
2. Kill OpenCode with `kill -9 <pid>`
3. Verify daemon restarts it within `restart_delay_ms`
4. Repeat 3 times quickly, verify backoff kicks in

### References

- [Source: _bmad-output/planning-artifacts/architecture.md - Module: src/opencode/]
- [Source: _bmad-output/planning-artifacts/epics.md#Story 9.2]
- [Source: _bmad-output/planning-artifacts/prd.md - FR46]
- [Depends: Story 9.1 - OpenCode Process Detection]
- [Existing: src/resume/backoff.rs - Backoff pattern reference]

## File List

**Files to create:**
- `src/opencode/restart.rs`

**Files to modify:**
- `src/opencode/mod.rs` (add restart module)
- `src/config/schema.rs` (extend OpenCodeConfig)
- `src/daemon/core.rs` (integrate restart manager)
- `_bmad-output/implementation-artifacts/sprint-status.yaml`

## Change Log

- 2026-02-06: Story created and marked ready-for-dev
