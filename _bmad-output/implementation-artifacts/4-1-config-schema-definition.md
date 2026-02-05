# Story 4.1: Config Schema Definition

Status: ready-for-dev

## Story

As a developer,
I want a well-defined config schema,
So that configuration is type-safe and documented.

## Acceptance Criteria

**AC1: TOML Deserialization**
**Given** the config schema
**When** deserialized from TOML
**Then** it maps to Rust structs with serde

**AC2: Documentation**
**Given** the config schema
**When** documented
**Then** each field has a comment explaining its purpose

**AC3: Config Sections**
**Given** a config file
**When** it contains all sections
**Then** sections include: daemon, monitoring, resume, notifications, otel

**AC4: Default Values**
**Given** the default config
**When** generated
**Then** all fields have sensible defaults
**And** it is valid and usable immediately

**AC5: Nested Structs**
**Given** complex config sections (e.g., notifications with multiple channels)
**When** deserialized
**Then** nested structs are properly typed and validated

**AC6: Optional Fields**
**Given** a minimal config file
**When** parsed
**Then** missing optional fields use defaults without errors

## Tasks / Subtasks

- [ ] Create config module structure (AC: 1)
  - [ ] Create `src/config/mod.rs` with module exports
  - [ ] Create `src/config/schema.rs` for config structs

- [ ] Define root Config struct (AC: 1, 3)
  - [ ] Add `daemon: DaemonConfig` section
  - [ ] Add `monitoring: MonitoringConfig` section
  - [ ] Add `resume: ResumeConfig` section
  - [ ] Add `notifications: NotificationsConfig` section
  - [ ] Add `otel: Option<OtelConfig>` section (optional)

- [ ] Define DaemonConfig struct (AC: 1, 2, 4)
  - [ ] Add `pid_file: Option<PathBuf>` with platform default
  - [ ] Add `socket_path: Option<PathBuf>` with platform default
  - [ ] Add `http_enabled: bool` default false
  - [ ] Add `http_port: u16` default 7654
  - [ ] Add `http_bind: String` default "127.0.0.1"
  - [ ] Add `log_level: String` default "info"
  - [ ] Add `log_file: Option<PathBuf>`

- [ ] Define MonitoringConfig struct (AC: 1, 2, 4)
  - [ ] Add `session_dir: PathBuf` with platform default
  - [ ] Add `assistants: Vec<String>` for explicit list
  - [ ] Add `auto_detect: bool` default true
  - [ ] Add `debounce_ms: u64` default 100
  - [ ] Add `poll_interval_secs: Option<u64>` for polling fallback

- [ ] Define ResumeConfig struct (AC: 1, 2, 4)
  - [ ] Add `enabled: bool` default true
  - [ ] Add `base_delay_secs: u64` default 30
  - [ ] Add `max_delay_secs: u64` default 300
  - [ ] Add `max_retries: u32` default 10
  - [ ] Add `jitter: bool` default true
  - [ ] Add `backup_count: u32` default 10

- [ ] Define NotificationsConfig struct (AC: 1, 2, 4, 5)
  - [ ] Add `enabled: bool` default false
  - [ ] Add `webhook: Option<WebhookConfig>`
  - [ ] Add `ntfy: Option<NtfyConfig>`
  - [ ] Add `discord: Option<DiscordConfig>`
  - [ ] Add `slack: Option<SlackConfig>`

- [ ] Define notification channel sub-structs (AC: 5)
  - [ ] Define `WebhookConfig { url: String, headers: Option<HashMap<String, String>> }`
  - [ ] Define `NtfyConfig { topic: String, server: Option<String>, priority: Option<String> }`
  - [ ] Define `DiscordConfig { webhook_url: String }`
  - [ ] Define `SlackConfig { webhook_url: String }`

- [ ] Define OtelConfig struct (AC: 1, 6)
  - [ ] Add `enabled: bool` default false
  - [ ] Add `endpoint: Option<String>`
  - [ ] Add `service_name: String` default "palingenesis"
  - [ ] Add `traces: bool` default true
  - [ ] Add `metrics: bool` default true

- [ ] Implement Default trait (AC: 4)
  - [ ] Implement `Default` for all config structs
  - [ ] Use platform-specific paths in defaults
  - [ ] Ensure default config is immediately usable

- [ ] Add serde attributes (AC: 1, 6)
  - [ ] Add `#[serde(default)]` to all structs
  - [ ] Add `#[serde(skip_serializing_if = "Option::is_none")]` to optional fields
  - [ ] Add `#[serde(rename = "...")]` where TOML naming differs

- [ ] Add documentation comments (AC: 2)
  - [ ] Add doc comments to all structs
  - [ ] Add doc comments to all fields
  - [ ] Include examples in doc comments

- [ ] Add unit tests (AC: 1, 3, 4, 5, 6)
  - [ ] Test deserialize full config
  - [ ] Test deserialize minimal config
  - [ ] Test default values applied
  - [ ] Test nested struct deserialization
  - [ ] Test invalid config errors

## Dev Notes

### Architecture Requirements

**From architecture.md - Config Module:**

```
src/config/
    mod.rs                    # Config module root
    schema.rs                 # Config struct definitions (THIS STORY)
    loader.rs                 # File/env/CLI config loading (Story 4.2)
    paths.rs                  # Platform-specific paths (Story 1.3 - exists)
    validation.rs             # Config validation (Story 4.4)
```

**From architecture.md - Project Structure:**

```toml
# config/default.toml
[daemon]
log_level = "info"
http_enabled = false
http_port = 7654

[monitoring]
auto_detect = true
debounce_ms = 100

[resume]
enabled = true
base_delay_secs = 30
max_delay_secs = 300
max_retries = 10
jitter = true

[notifications]
enabled = false
```

**Implements:** FR21-FR25 foundation, ARCH configuration schema

### Technical Implementation

**Config Schema (src/config/schema.rs):**

```rust
use std::collections::HashMap;
use std::path::PathBuf;
use serde::{Deserialize, Serialize};

/// Root configuration structure for palingenesis.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Config {
    /// Daemon configuration
    pub daemon: DaemonConfig,
    /// Session monitoring configuration
    pub monitoring: MonitoringConfig,
    /// Resume strategy configuration
    pub resume: ResumeConfig,
    /// Notification channel configuration
    pub notifications: NotificationsConfig,
    /// OpenTelemetry configuration (optional)
    pub otel: Option<OtelConfig>,
}

/// Daemon process configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct DaemonConfig {
    /// Path to PID file (platform default if not set)
    pub pid_file: Option<PathBuf>,
    /// Path to Unix socket (platform default if not set)
    pub socket_path: Option<PathBuf>,
    /// Enable HTTP control API
    pub http_enabled: bool,
    /// HTTP server port
    pub http_port: u16,
    /// HTTP server bind address
    pub http_bind: String,
    /// Log level (trace, debug, info, warn, error)
    pub log_level: String,
    /// Optional log file path
    pub log_file: Option<PathBuf>,
}

impl Default for DaemonConfig {
    fn default() -> Self {
        Self {
            pid_file: None,
            socket_path: None,
            http_enabled: false,
            http_port: 7654,
            http_bind: "127.0.0.1".to_string(),
            log_level: "info".to_string(),
            log_file: None,
        }
    }
}

/// Session monitoring configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct MonitoringConfig {
    /// Directory to watch for session files
    pub session_dir: Option<PathBuf>,
    /// Explicit list of assistants to monitor
    pub assistants: Vec<String>,
    /// Auto-detect running assistants
    pub auto_detect: bool,
    /// Debounce time for file events (milliseconds)
    pub debounce_ms: u64,
    /// Polling interval fallback (seconds)
    pub poll_interval_secs: Option<u64>,
}

impl Default for MonitoringConfig {
    fn default() -> Self {
        Self {
            session_dir: None,
            assistants: Vec::new(),
            auto_detect: true,
            debounce_ms: 100,
            poll_interval_secs: None,
        }
    }
}

/// Resume strategy configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ResumeConfig {
    /// Enable automatic resume
    pub enabled: bool,
    /// Base delay for exponential backoff (seconds)
    pub base_delay_secs: u64,
    /// Maximum delay cap (seconds)
    pub max_delay_secs: u64,
    /// Maximum retry attempts
    pub max_retries: u32,
    /// Add jitter to delays
    pub jitter: bool,
    /// Number of session backups to keep
    pub backup_count: u32,
}

impl Default for ResumeConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            base_delay_secs: 30,
            max_delay_secs: 300,
            max_retries: 10,
            jitter: true,
            backup_count: 10,
        }
    }
}

/// Notification channel configuration.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct NotificationsConfig {
    /// Enable notifications globally
    pub enabled: bool,
    /// Webhook notification config
    pub webhook: Option<WebhookConfig>,
    /// ntfy.sh notification config
    pub ntfy: Option<NtfyConfig>,
    /// Discord notification config
    pub discord: Option<DiscordConfig>,
    /// Slack notification config
    pub slack: Option<SlackConfig>,
}

/// Webhook notification configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookConfig {
    /// Webhook URL
    pub url: String,
    /// Optional custom headers
    #[serde(skip_serializing_if = "Option::is_none")]
    pub headers: Option<HashMap<String, String>>,
}

/// ntfy.sh notification configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NtfyConfig {
    /// ntfy topic name
    pub topic: String,
    /// Custom ntfy server (default: ntfy.sh)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub server: Option<String>,
    /// Notification priority
    #[serde(skip_serializing_if = "Option::is_none")]
    pub priority: Option<String>,
}

/// Discord webhook notification configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscordConfig {
    /// Discord webhook URL
    pub webhook_url: String,
}

/// Slack webhook notification configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlackConfig {
    /// Slack webhook URL
    pub webhook_url: String,
}

/// OpenTelemetry configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct OtelConfig {
    /// Enable OTEL export
    pub enabled: bool,
    /// OTLP endpoint
    pub endpoint: Option<String>,
    /// Service name for telemetry
    pub service_name: String,
    /// Enable trace export
    pub traces: bool,
    /// Enable metrics export
    pub metrics: bool,
}

impl Default for OtelConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            endpoint: None,
            service_name: "palingenesis".to_string(),
            traces: true,
            metrics: true,
        }
    }
}
```

### Dependencies

Uses existing dependencies:
- `serde = { version = "1.0", features = ["derive"] }` (already in Cargo.toml)
- `toml = "0.9"` (already in Cargo.toml)

### Testing Strategy

**Unit Tests:**
- Parse full config file
- Parse minimal config (all defaults)
- Parse config with only some sections
- Verify default values
- Test invalid TOML syntax errors
- Test type mismatch errors

### References

- [Source: _bmad-output/planning-artifacts/architecture.md#Config Module]
- [Source: _bmad-output/planning-artifacts/architecture.md#Project Structure]
- [Source: _bmad-output/planning-artifacts/epics.md#Story 4.1: Config Schema Definition]

## File List

**Files to create:**
- `src/config/schema.rs`
- `tests/config_schema_test.rs`

**Files to modify:**
- `src/config/mod.rs`
- `_bmad-output/sprint-status.yaml`

## Change Log

- 2026-02-05: Story created and marked ready-for-dev
