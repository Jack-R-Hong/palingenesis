# Story 4.3: Config Show Command

Status: ready-for-dev

## Story

As a user,
I want to view the current configuration,
So that I can verify settings without opening the file.

## Acceptance Criteria

**AC1: Show Existing Config**
**Given** a config file exists
**When** I run `palingenesis config show`
**Then** the current config is displayed in TOML format

**AC2: Show Defaults**
**Given** no config file exists
**When** I run `palingenesis config show`
**Then** the default config is displayed
**And** CLI notes "Using default configuration"

**AC3: JSON Output**
**Given** I run `palingenesis config show --json`
**When** config is displayed
**Then** output is JSON format instead of TOML

**AC4: Merged View**
**Given** a config file with partial settings
**When** I run `palingenesis config show`
**Then** output shows merged config (file values + defaults for missing)

**AC5: Section Filter**
**Given** I run `palingenesis config show --section daemon`
**When** output is displayed
**Then** only the daemon section is shown

**AC6: Effective Config**
**Given** environment variables override config values
**When** I run `palingenesis config show --effective`
**Then** the actually-used values are displayed (including env overrides)

## Tasks / Subtasks

- [ ] Add config show subcommand to CLI (AC: 1, 3, 5, 6)
  - [ ] Add `show` subcommand to `ConfigCmd` enum
  - [ ] Add `--json` flag for JSON output
  - [ ] Add `--section` option for filtering
  - [ ] Add `--effective` flag for runtime values

- [ ] Implement config loading (AC: 1, 2, 4)
  - [ ] Load config from file if exists
  - [ ] Fall back to defaults if no file
  - [ ] Merge file config with defaults

- [ ] Implement TOML output (AC: 1)
  - [ ] Serialize config to TOML string
  - [ ] Use pretty formatting
  - [ ] Print to stdout

- [ ] Implement JSON output (AC: 3)
  - [ ] Serialize config to JSON when --json flag
  - [ ] Use pretty-printed JSON
  - [ ] Print to stdout

- [ ] Implement section filtering (AC: 5)
  - [ ] Parse --section argument
  - [ ] Extract only requested section
  - [ ] Handle invalid section names

- [ ] Implement effective config (AC: 6)
  - [ ] Apply environment variable overrides
  - [ ] Show actually-used values
  - [ ] Indicate which values came from env

- [ ] Add default config notice (AC: 2)
  - [ ] Detect when using defaults
  - [ ] Print notice to stderr before config output
  - [ ] Suggest running `config init`

- [ ] Add unit/integration tests (AC: 1, 2, 3, 4, 5, 6)
  - [ ] Test show with existing config
  - [ ] Test show with no config
  - [ ] Test JSON output
  - [ ] Test section filter
  - [ ] Test effective config with env vars

## Dev Notes

### Architecture Requirements

**From architecture.md - CLI Module:**

```
src/cli/commands/
    config.rs             # config init, validate, show
```

**Implements:** Part of FR23 (config management)

### Technical Implementation

**CLI Command Definition:**

```rust
// src/cli/app.rs
#[derive(Subcommand)]
pub enum ConfigCmd {
    /// Show current configuration
    Show {
        /// Output in JSON format
        #[arg(long)]
        json: bool,
        
        /// Show only a specific section
        #[arg(long)]
        section: Option<String>,
        
        /// Show effective config (including env overrides)
        #[arg(long)]
        effective: bool,
    },
    // ... other config commands
}
```

**Config Show Handler:**

```rust
// src/cli/commands/config.rs
use crate::config::loader::load_config;
use crate::config::paths::get_config_path;
use crate::config::schema::Config;

pub fn handle_show(json: bool, section: Option<String>, effective: bool) -> anyhow::Result<()> {
    let config_path = get_config_path();
    let using_defaults = !config_path.exists();
    
    // Load config (file or defaults)
    let config = if effective {
        load_config_effective()?
    } else {
        load_config()?
    };
    
    // Note if using defaults
    if using_defaults {
        eprintln!("Using default configuration (no config file found)");
        eprintln!("Run `palingenesis config init` to create one\n");
    }
    
    // Filter to section if requested
    let output = if let Some(ref section_name) = section {
        format_section(&config, section_name, json)?
    } else {
        format_config(&config, json)?
    };
    
    println!("{}", output);
    Ok(())
}

fn format_config(config: &Config, json: bool) -> anyhow::Result<String> {
    if json {
        Ok(serde_json::to_string_pretty(config)?)
    } else {
        Ok(toml::to_string_pretty(config)?)
    }
}

fn format_section(config: &Config, section: &str, json: bool) -> anyhow::Result<String> {
    match section {
        "daemon" => {
            if json {
                Ok(serde_json::to_string_pretty(&config.daemon)?)
            } else {
                Ok(toml::to_string_pretty(&config.daemon)?)
            }
        }
        "monitoring" => {
            if json {
                Ok(serde_json::to_string_pretty(&config.monitoring)?)
            } else {
                Ok(toml::to_string_pretty(&config.monitoring)?)
            }
        }
        "resume" => {
            if json {
                Ok(serde_json::to_string_pretty(&config.resume)?)
            } else {
                Ok(toml::to_string_pretty(&config.resume)?)
            }
        }
        "notifications" => {
            if json {
                Ok(serde_json::to_string_pretty(&config.notifications)?)
            } else {
                Ok(toml::to_string_pretty(&config.notifications)?)
            }
        }
        "otel" => {
            if json {
                Ok(serde_json::to_string_pretty(&config.otel)?)
            } else {
                Ok(toml::to_string_pretty(&config.otel)?)
            }
        }
        _ => anyhow::bail!("Unknown section: {}. Valid sections: daemon, monitoring, resume, notifications, otel", section),
    }
}

fn load_config_effective() -> anyhow::Result<Config> {
    // Load base config
    let mut config = load_config()?;
    
    // Apply environment overrides
    if let Ok(level) = std::env::var("PALINGENESIS_LOG_LEVEL") {
        config.daemon.log_level = level;
    }
    if let Ok(port) = std::env::var("PALINGENESIS_HTTP_PORT") {
        config.daemon.http_port = port.parse()?;
    }
    // ... more env overrides
    
    Ok(config)
}
```

### Dependencies

Uses existing dependencies:
- `serde_json` for JSON output
- `toml` for TOML output
- Config loader from Story 4.2

### Testing Strategy

**Unit Tests:**
- Test format_config TOML output
- Test format_config JSON output
- Test format_section with valid sections
- Test format_section with invalid section

**Integration Tests:**
- Test show command with real config file
- Test show command with no config file
- Test --json flag
- Test --section flag
- Test --effective with env vars

### References

- [Source: _bmad-output/planning-artifacts/architecture.md#CLI Module]
- [Source: _bmad-output/planning-artifacts/epics.md#Story 4.3: Config Show Command]

## File List

**Files to create:**
- `tests/config_show_test.rs`

**Files to modify:**
- `src/cli/app.rs`
- `src/cli/commands/config.rs`
- `_bmad-output/sprint-status.yaml`

## Change Log

- 2026-02-05: Story created and marked ready-for-dev
