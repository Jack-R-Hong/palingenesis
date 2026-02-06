# Story 9.4: OpenCode Configuration Options

Status: ready-for-dev

## Story

As a user,
I want to configure OpenCode integration settings,
So that I can customize how palingenesis manages OpenCode processes.

## Acceptance Criteria

**AC1: Configure Serve Port**
**Given** config file has `opencode.serve_port = 8080`
**When** daemon restarts OpenCode
**Then** it starts with `--port 8080`
**And** health checks use port 8080

**AC2: Configure Serve Hostname**
**Given** config file has `opencode.serve_hostname = "0.0.0.0"`
**When** daemon restarts OpenCode
**Then** it starts with `--hostname 0.0.0.0`

**AC3: Configure Auto-Restart**
**Given** config file has `opencode.auto_restart = false`
**When** OpenCode crashes
**Then** daemon logs the crash but does NOT restart

**AC4: Configure Restart Delay**
**Given** config file has `opencode.restart_delay_ms = 5000`
**When** daemon decides to restart OpenCode
**Then** it waits 5 seconds before spawning

**AC5: Hot Reload Configuration**
**Given** daemon receives SIGHUP
**When** OpenCode config section changed
**Then** new settings apply to next restart
**And** logs "OpenCode configuration reloaded"

**AC6: Config Validation**
**Given** `opencode.serve_port = 0` (invalid)
**When** config is validated
**Then** validation fails with "serve_port must be 1-65535"

**AC7: Config Show Includes OpenCode**
**Given** OpenCode is configured
**When** user runs `palingenesis config show`
**Then** output includes `[opencode]` section with all options

## Tasks / Subtasks

- [ ] Extend config schema (AC: 1, 2, 3, 4)
  - [ ] Add complete `OpenCodeConfig` struct to schema
  - [ ] Add `serve_port` field with validation (1-65535)
  - [ ] Add `serve_hostname` field (default: "localhost")
  - [ ] Add `auto_restart` field (default: true)
  - [ ] Add `restart_delay_ms` field (default: 1000)

- [ ] Add advanced configuration options (AC: extended)
  - [ ] Add `enabled` field (default: false)
  - [ ] Add `health_timeout_ms` field (default: 2000)
  - [ ] Add `poll_interval_ms` field (default: 1000)
  - [ ] Add `max_restart_attempts` field (default: 5)
  - [ ] Add `serve_command` field (optional override)

- [ ] Implement config validation (AC: 6)
  - [ ] Validate port range 1-65535
  - [ ] Validate hostname is non-empty
  - [ ] Validate delay_ms is positive
  - [ ] Validate timeout_ms is reasonable (100-30000)

- [ ] Implement default config generation (AC: 7)
  - [ ] Add `[opencode]` section to default config template
  - [ ] Include comments explaining each option
  - [ ] Generate sensible defaults

- [ ] Implement hot reload support (AC: 5)
  - [ ] Ensure `OpenCodeConfig` is read on SIGHUP
  - [ ] Update RestartManager with new config
  - [ ] Update OpenCodeClient with new base URL if port changes
  - [ ] Log configuration changes

- [ ] Update config show command (AC: 7)
  - [ ] Include OpenCode section in `config show` output
  - [ ] Show current effective values
  - [ ] Indicate which values are defaults vs explicit

- [ ] Add documentation (AC: all)
  - [ ] Document all config options in generated config file
  - [ ] Add examples for common configurations
  - [ ] Document port requirements (must match actual OpenCode)

- [ ] Add unit/integration tests (AC: 1, 2, 3, 4, 5, 6)
  - [ ] Test config parsing with all fields
  - [ ] Test validation error messages
  - [ ] Test hot reload updates config
  - [ ] Test default values are sensible

## Dev Notes

### Architecture Requirements

**From architecture.md - FR48 (OpenCode Configuration):**
> User can configure OpenCode serve port/hostname via config file

**From architecture.md - Configuration Pattern:**
> TOML configuration at platform-specific paths
> Hot reload via SIGHUP

### Technical Implementation

**Complete Configuration Schema:**

```rust
// src/config/schema.rs
use serde::{Deserialize, Serialize};

/// OpenCode integration configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct OpenCodeConfig {
    /// Enable OpenCode process management
    /// When false, daemon ignores OpenCode entirely
    pub enabled: bool,
    
    /// Port for OpenCode serve command and health checks
    /// Must match the port OpenCode is configured to use
    /// Range: 1-65535, Default: 4096
    pub serve_port: u16,
    
    /// Hostname for OpenCode serve command
    /// Use "localhost" for local-only, "0.0.0.0" for all interfaces
    /// Default: "localhost"
    pub serve_hostname: String,
    
    /// Automatically restart OpenCode when it crashes
    /// Set to false if using external process manager (systemd, etc.)
    /// Default: true
    pub auto_restart: bool,
    
    /// Delay in milliseconds before restarting OpenCode
    /// Allows time for cleanup and prevents tight crash loops
    /// Default: 1000
    pub restart_delay_ms: u64,
    
    /// Maximum restart attempts before giving up
    /// Resets after successful run of 60+ seconds
    /// Default: 5
    pub max_restart_attempts: u32,
    
    /// Timeout for health check HTTP requests in milliseconds
    /// Default: 2000
    pub health_timeout_ms: u64,
    
    /// Interval for polling OpenCode process state in milliseconds
    /// Lower values = faster detection, higher CPU
    /// Default: 1000
    pub poll_interval_ms: u64,
    
    /// Custom command to start OpenCode (default: "opencode")
    /// Useful for custom installations or wrappers
    pub serve_command: Option<String>,
    
    /// Also restart on normal exit (not just crashes)
    /// Enable if you want OpenCode always running
    /// Default: false
    pub restart_on_normal_exit: bool,
    
    /// Maximum API request retries
    /// Default: 3
    pub max_api_retries: Option<u32>,
}

impl Default for OpenCodeConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            serve_port: 4096,
            serve_hostname: "localhost".to_string(),
            auto_restart: true,
            restart_delay_ms: 1000,
            max_restart_attempts: 5,
            health_timeout_ms: 2000,
            poll_interval_ms: 1000,
            serve_command: None,
            restart_on_normal_exit: false,
            max_api_retries: Some(3),
        }
    }
}

impl OpenCodeConfig {
    /// Validate configuration values
    pub fn validate(&self) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();
        
        if self.serve_port == 0 {
            errors.push("opencode.serve_port must be 1-65535".to_string());
        }
        
        if self.serve_hostname.is_empty() {
            errors.push("opencode.serve_hostname cannot be empty".to_string());
        }
        
        if self.restart_delay_ms == 0 {
            errors.push("opencode.restart_delay_ms must be positive".to_string());
        }
        
        if self.health_timeout_ms < 100 || self.health_timeout_ms > 30000 {
            errors.push("opencode.health_timeout_ms must be 100-30000".to_string());
        }
        
        if self.poll_interval_ms < 100 {
            errors.push("opencode.poll_interval_ms must be >= 100".to_string());
        }
        
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
    
    /// Get base URL for API requests
    pub fn base_url(&self) -> String {
        format!("http://{}:{}", self.serve_hostname, self.serve_port)
    }
}
```

**Default Config File Template:**

```toml
# OpenCode Integration
# Enables palingenesis to manage OpenCode process lifecycle
[opencode]
# Enable OpenCode management (default: false)
# Set to true to enable process monitoring and auto-restart
enabled = false

# Port for OpenCode serve command (default: 4096)
# Must match OpenCode's configured port
serve_port = 4096

# Hostname for OpenCode serve (default: "localhost")
# Use "0.0.0.0" to listen on all interfaces
serve_hostname = "localhost"

# Auto-restart OpenCode on crash (default: true)
# Disable if using external process manager
auto_restart = true

# Delay before restart in milliseconds (default: 1000)
restart_delay_ms = 1000

# Maximum restart attempts before giving up (default: 5)
max_restart_attempts = 5

# Health check timeout in milliseconds (default: 2000)
health_timeout_ms = 2000

# Process polling interval in milliseconds (default: 1000)
poll_interval_ms = 1000

# Custom serve command (optional)
# serve_command = "/usr/local/bin/opencode"

# Restart even on normal exit (default: false)
restart_on_normal_exit = false
```

**Hot Reload Integration:**

```rust
// src/daemon/reload.rs (additions)
impl Daemon {
    pub async fn reload_config(&self) -> anyhow::Result<()> {
        let new_config = Config::load()?;
        new_config.validate()?;
        
        // Check what changed
        let old_config = self.config.read().await;
        
        if old_config.opencode != new_config.opencode {
            tracing::info!("OpenCode configuration changed");
            
            // Update OpenCode components if running
            if let Some(monitor) = &self.opencode_monitor {
                monitor.update_config(new_config.opencode.clone()).await;
            }
        }
        
        // Apply new config
        *self.config.write().await = new_config;
        
        tracing::info!("Configuration reloaded successfully");
        Ok(())
    }
}
```

### Configuration Examples

**Minimal (enable with defaults):**
```toml
[opencode]
enabled = true
```

**Custom port:**
```toml
[opencode]
enabled = true
serve_port = 8080
```

**Disable auto-restart (systemd manages):**
```toml
[opencode]
enabled = true
auto_restart = false
```

**Custom command with different binary:**
```toml
[opencode]
enabled = true
serve_command = "/opt/opencode/bin/opencode"
serve_port = 4096
```

**High-availability settings:**
```toml
[opencode]
enabled = true
auto_restart = true
restart_delay_ms = 500
max_restart_attempts = 10
health_timeout_ms = 1000
poll_interval_ms = 500
restart_on_normal_exit = true
```

### Dependencies

No new dependencies. Uses existing:
- `serde` for serialization
- `toml` for config parsing

### Testing Strategy

**Unit Tests:**
- Test default values
- Test validation for all fields
- Test error messages are helpful

**Integration Tests:**
- Test config parsing from TOML string
- Test hot reload updates config
- Test config show includes OpenCode section

**Manual Testing:**
1. Run `palingenesis config init`
2. Verify `[opencode]` section present with comments
3. Modify values and run `palingenesis config validate`
4. Test invalid values produce clear errors

### References

- [Source: _bmad-output/planning-artifacts/architecture.md - Configuration section]
- [Source: _bmad-output/planning-artifacts/epics.md#Story 9.4]
- [Source: _bmad-output/planning-artifacts/prd.md - FR48]
- [Existing: src/config/schema.rs - Config patterns]
- [Existing: Story 4.1 - Config Schema Definition]

## File List

**Files to modify:**
- `src/config/schema.rs` (complete OpenCodeConfig)
- `src/config/template.rs` (add OpenCode section to default template)
- `src/config/validation.rs` (add OpenCode validation)
- `src/daemon/reload.rs` (hot reload for OpenCode config)
- `_bmad-output/implementation-artifacts/sprint-status.yaml`

## Change Log

- 2026-02-06: Story created and marked ready-for-dev
