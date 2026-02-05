# Story 4.2: Config Init Command

Status: ready-for-dev

## Story

As a user,
I want to initialize a config file with defaults,
So that I have a starting point for customization.

## Acceptance Criteria

**AC1: Create Default Config**
**Given** no config file exists
**When** I run `palingenesis config init`
**Then** a default config file is created at the platform-specific path
**And** CLI displays "Config created at {path}"

**AC2: Overwrite Protection**
**Given** a config file already exists
**When** I run `palingenesis config init`
**Then** CLI asks for confirmation before overwriting
**And** respects the user's choice

**AC3: Commented Config**
**Given** the default config is generated
**When** written to file
**Then** it includes comments documenting each option
**And** file permissions are set to 600

**AC4: Force Overwrite**
**Given** I run `palingenesis config init --force`
**When** a config file exists
**Then** it overwrites without asking

**AC5: Custom Path**
**Given** I run `palingenesis config init --path /custom/path.toml`
**When** the command completes
**Then** config is created at the specified path

**AC6: Directory Creation**
**Given** the config directory does not exist
**When** `palingenesis config init` runs
**Then** parent directories are created with appropriate permissions

## Tasks / Subtasks

- [ ] Add config init subcommand to CLI (AC: 1, 4, 5)
  - [ ] Add `init` subcommand to `ConfigCmd` enum in CLI
  - [ ] Add `--force` flag for overwrite without prompt
  - [ ] Add `--path` option for custom config path

- [ ] Implement config init handler (AC: 1, 6)
  - [ ] Create `src/cli/commands/config.rs` init handler
  - [ ] Resolve platform-specific default path
  - [ ] Create parent directories if needed
  - [ ] Generate default config content

- [ ] Generate commented TOML (AC: 3)
  - [ ] Create `generate_default_config_toml()` function
  - [ ] Include header comments explaining the file
  - [ ] Include section comments for each config block
  - [ ] Include inline comments for each field
  - [ ] Use TOML formatting with proper indentation

- [ ] Implement overwrite protection (AC: 2)
  - [ ] Check if config file already exists
  - [ ] Prompt user for confirmation (y/n)
  - [ ] Read stdin for user response
  - [ ] Abort if user declines

- [ ] Handle force flag (AC: 4)
  - [ ] Skip existence check when --force is set
  - [ ] Overwrite existing file directly

- [ ] Set file permissions (AC: 3)
  - [ ] Set permissions to 600 (owner read/write only)
  - [ ] Use platform-specific permission APIs
  - [ ] Log warning if permissions cannot be set

- [ ] Add success output (AC: 1)
  - [ ] Print "Config created at {path}" on success
  - [ ] Use colored output for visibility
  - [ ] Include hint about editing the config

- [ ] Add unit/integration tests (AC: 1, 2, 3, 4, 5, 6)
  - [ ] Test config init creates file
  - [ ] Test config init with --force
  - [ ] Test config init with --path
  - [ ] Test overwrite prompt
  - [ ] Test directory creation
  - [ ] Test file permissions

## Dev Notes

### Architecture Requirements

**From architecture.md - CLI Module:**

```
src/cli/commands/
    config.rs             # config init, validate, show
```

**From architecture.md - Platform Paths:**

| Resource | Linux | macOS |
|----------|-------|-------|
| Config | `~/.config/palingenesis/` | `~/Library/Application Support/palingenesis/` |

**Implements:** FR21 (User can initialize config file via CLI)

### Technical Implementation

**CLI Command Definition:**

```rust
// src/cli/app.rs
#[derive(Subcommand)]
pub enum ConfigCmd {
    /// Initialize a new config file with defaults
    Init {
        /// Overwrite existing config without asking
        #[arg(long)]
        force: bool,
        
        /// Custom path for config file
        #[arg(long)]
        path: Option<PathBuf>,
    },
    // ... other config commands
}
```

**Config Init Handler:**

```rust
// src/cli/commands/config.rs
use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;

use crate::config::paths::get_config_path;
use crate::config::schema::Config;

pub fn handle_init(force: bool, custom_path: Option<PathBuf>) -> anyhow::Result<()> {
    let config_path = custom_path.unwrap_or_else(|| get_config_path());
    
    // Check if exists and not forcing
    if config_path.exists() && !force {
        print!("Config already exists at {}. Overwrite? [y/N] ", config_path.display());
        io::stdout().flush()?;
        
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        
        if !input.trim().eq_ignore_ascii_case("y") {
            println!("Aborted.");
            return Ok(());
        }
    }
    
    // Create parent directories
    if let Some(parent) = config_path.parent() {
        fs::create_dir_all(parent)?;
    }
    
    // Generate commented config
    let config_content = generate_default_config_toml();
    
    // Write file
    fs::write(&config_path, config_content)?;
    
    // Set permissions (Unix only)
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = fs::Permissions::from_mode(0o600);
        fs::set_permissions(&config_path, perms)?;
    }
    
    println!("Config created at {}", config_path.display());
    println!("Edit with: palingenesis config edit");
    
    Ok(())
}

fn generate_default_config_toml() -> String {
    r#"# palingenesis configuration file
# https://github.com/Jack-R-Hong/palingenesis

# Daemon process configuration
[daemon]
# Log level: trace, debug, info, warn, error
log_level = "info"
# Enable HTTP control API (default: disabled)
http_enabled = false
# HTTP server port (when enabled)
http_port = 7654
# HTTP server bind address
http_bind = "127.0.0.1"
# Optional: Custom PID file path (uses platform default if not set)
# pid_file = "/run/user/1000/palingenesis.pid"
# Optional: Log to file instead of stderr
# log_file = "/path/to/daemon.log"

# Session monitoring configuration
[monitoring]
# Auto-detect running AI assistants
auto_detect = true
# Explicit list of assistants to monitor (optional)
# assistants = ["opencode"]
# Debounce time for file events (milliseconds)
debounce_ms = 100
# Optional: Session directory override
# session_dir = "~/.opencode"

# Resume strategy configuration
[resume]
# Enable automatic session resume
enabled = true
# Base delay for exponential backoff (seconds)
base_delay_secs = 30
# Maximum delay cap (seconds)
max_delay_secs = 300
# Maximum retry attempts before giving up
max_retries = 10
# Add random jitter to delays
jitter = true
# Number of session backups to keep
backup_count = 10

# Notification configuration (all optional)
[notifications]
# Enable notifications globally
enabled = false

# Webhook notifications
# [notifications.webhook]
# url = "https://your-webhook.example.com/hook"
# headers = { "Authorization" = "Bearer token" }

# ntfy.sh notifications
# [notifications.ntfy]
# topic = "your-topic"
# server = "https://ntfy.sh"  # optional, default is ntfy.sh
# priority = "default"  # min, low, default, high, max

# Discord notifications
# [notifications.discord]
# webhook_url = "https://discord.com/api/webhooks/..."

# Slack notifications
# [notifications.slack]
# webhook_url = "https://hooks.slack.com/services/..."

# OpenTelemetry configuration (optional, for observability)
# [otel]
# enabled = false
# endpoint = "http://localhost:4317"
# service_name = "palingenesis"
# traces = true
# metrics = true
"#.to_string()
}
```

### Dependencies

Uses existing dependencies:
- `std::fs` for file operations
- `std::io` for stdin prompts
- Platform path resolution from Story 1.3

### Testing Strategy

**Unit Tests:**
- Test generate_default_config_toml() produces valid TOML
- Test path resolution

**Integration Tests:**
- Test full init flow in temp directory
- Test --force flag behavior
- Test --path flag behavior
- Test overwrite prompt (mock stdin)

### References

- [Source: _bmad-output/planning-artifacts/architecture.md#CLI Module]
- [Source: _bmad-output/planning-artifacts/architecture.md#Platform Paths]
- [Source: _bmad-output/planning-artifacts/epics.md#Story 4.2: Config Init Command]
- [Source: _bmad-output/implementation-artifacts/1-3-platform-specific-path-resolution.md]

## File List

**Files to create:**
- `tests/config_init_test.rs`

**Files to modify:**
- `src/cli/app.rs`
- `src/cli/commands/config.rs`
- `_bmad-output/sprint-status.yaml`

## Change Log

- 2026-02-05: Story created and marked ready-for-dev
