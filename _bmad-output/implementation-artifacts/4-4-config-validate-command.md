# Story 4.4: Config Validate Command

Status: done

## Story

As a user,
I want to validate my config file,
So that I catch errors before starting the daemon.

## Acceptance Criteria

**AC1: Valid Config**
**Given** a valid config file
**When** I run `palingenesis config validate`
**Then** CLI displays "Configuration valid"
**And** exits with code 0

**AC2: Syntax Errors**
**Given** a config file with syntax errors
**When** I run `palingenesis config validate`
**Then** CLI displays the parse error with line number
**And** exits with code 1

**AC3: Invalid Values**
**Given** a config file with invalid values
**When** I run `palingenesis config validate`
**Then** CLI displays which value is invalid and why
**And** exits with code 1

**AC4: No Config File**
**Given** no config file exists
**When** I run `palingenesis config validate`
**Then** CLI displays "No config file found, will use defaults"
**And** exits with code 0

**AC5: Custom Path**
**Given** I run `palingenesis config validate --path /custom/config.toml`
**When** the specified file is validated
**Then** validation runs on that file instead of default path

**AC6: Semantic Validation**
**Given** a config with semantically invalid values (e.g., port > 65535)
**When** validation runs
**Then** it reports the semantic error with helpful message

## Tasks / Subtasks

- [x] Add config validate subcommand to CLI (AC: 1, 5)
  - [x] Add `validate` subcommand to `ConfigCmd` enum
  - [x] Add `--path` option for custom config path

- [x] Implement syntax validation (AC: 2)
  - [x] Attempt to parse config as TOML
  - [x] Catch parse errors
  - [x] Extract line/column from error
  - [x] Format helpful error message

- [x] Implement schema validation (AC: 3)
  - [x] Attempt to deserialize into Config struct
  - [x] Catch type mismatch errors
  - [x] Format field-specific error messages

- [x] Implement semantic validation (AC: 6)
  - [x] Create `validate_config()` function in `src/config/validation.rs`
  - [x] Validate port ranges (0-65535)
  - [x] Validate paths exist or can be created
  - [x] Validate log levels are valid
  - [x] Validate URL formats
  - [x] Validate duration values are positive

- [x] Handle missing config (AC: 4)
  - [x] Check if config file exists
  - [x] Print appropriate message
  - [x] Exit with code 0

- [x] Implement result formatting (AC: 1, 2, 3)
  - [x] Green "Configuration valid" on success
  - [x] Red error messages on failure
  - [x] Include suggestions for fixing errors

- [x] Add exit codes (AC: 1, 2, 3, 4)
  - [x] Exit 0 on valid/no config
  - [x] Exit 1 on invalid config

- [x] Add unit/integration tests (AC: 1, 2, 3, 4, 5, 6)
  - [x] Test valid config
  - [x] Test syntax error
  - [x] Test type mismatch
  - [x] Test semantic errors
  - [x] Test no config file
  - [x] Test custom path

## Dev Notes

### Architecture Requirements

**From architecture.md - Config Module:**

```
src/config/
    validation.rs             # Config validation (THIS STORY)
```

**Implements:** FR22 (User can validate config file via CLI)

### Technical Implementation

**CLI Command Definition:**

```rust
// src/cli/app.rs
#[derive(Subcommand)]
pub enum ConfigCmd {
    /// Validate configuration file
    Validate {
        /// Custom path to config file
        #[arg(long)]
        path: Option<PathBuf>,
    },
    // ... other config commands
}
```

**Validation Module:**

```rust
// src/config/validation.rs
use std::path::PathBuf;

use crate::config::schema::Config;

/// Result of config validation
#[derive(Debug)]
pub struct ValidationResult {
    pub errors: Vec<ValidationError>,
    pub warnings: Vec<ValidationWarning>,
}

impl ValidationResult {
    pub fn is_valid(&self) -> bool {
        self.errors.is_empty()
    }
}

#[derive(Debug)]
pub struct ValidationError {
    pub field: String,
    pub message: String,
    pub suggestion: Option<String>,
}

#[derive(Debug)]
pub struct ValidationWarning {
    pub field: String,
    pub message: String,
}

/// Validate config semantically (values make sense)
pub fn validate_config(config: &Config) -> ValidationResult {
    let mut errors = Vec::new();
    let mut warnings = Vec::new();
    
    // Validate daemon config
    if config.daemon.http_port > 65535 {
        errors.push(ValidationError {
            field: "daemon.http_port".to_string(),
            message: format!("Port {} exceeds maximum 65535", config.daemon.http_port),
            suggestion: Some("Use a port between 1 and 65535".to_string()),
        });
    }
    
    if config.daemon.http_port < 1024 && config.daemon.http_enabled {
        warnings.push(ValidationWarning {
            field: "daemon.http_port".to_string(),
            message: format!("Port {} requires root privileges", config.daemon.http_port),
        });
    }
    
    let valid_log_levels = ["trace", "debug", "info", "warn", "error"];
    if !valid_log_levels.contains(&config.daemon.log_level.to_lowercase().as_str()) {
        errors.push(ValidationError {
            field: "daemon.log_level".to_string(),
            message: format!("Invalid log level: {}", config.daemon.log_level),
            suggestion: Some(format!("Valid levels: {}", valid_log_levels.join(", "))),
        });
    }
    
    // Validate resume config
    if config.resume.base_delay_secs == 0 {
        errors.push(ValidationError {
            field: "resume.base_delay_secs".to_string(),
            message: "Base delay cannot be zero".to_string(),
            suggestion: Some("Use a value of at least 1 second".to_string()),
        });
    }
    
    if config.resume.max_delay_secs < config.resume.base_delay_secs {
        errors.push(ValidationError {
            field: "resume.max_delay_secs".to_string(),
            message: "Max delay cannot be less than base delay".to_string(),
            suggestion: None,
        });
    }
    
    if config.resume.max_retries == 0 && config.resume.enabled {
        warnings.push(ValidationWarning {
            field: "resume.max_retries".to_string(),
            message: "Resume enabled but max_retries is 0 (will never retry)".to_string(),
        });
    }
    
    // Validate notification URLs if present
    if let Some(ref webhook) = config.notifications.webhook {
        if !webhook.url.starts_with("http://") && !webhook.url.starts_with("https://") {
            errors.push(ValidationError {
                field: "notifications.webhook.url".to_string(),
                message: "Webhook URL must start with http:// or https://".to_string(),
                suggestion: None,
            });
        }
    }
    
    if let Some(ref ntfy) = config.notifications.ntfy {
        if ntfy.topic.is_empty() {
            errors.push(ValidationError {
                field: "notifications.ntfy.topic".to_string(),
                message: "ntfy topic cannot be empty".to_string(),
                suggestion: None,
            });
        }
    }
    
    ValidationResult { errors, warnings }
}
```

**Config Validate Handler:**

```rust
// src/cli/commands/config.rs
use std::fs;
use std::process;

use crate::config::paths::get_config_path;
use crate::config::schema::Config;
use crate::config::validation::validate_config;

pub fn handle_validate(custom_path: Option<PathBuf>) -> anyhow::Result<()> {
    let config_path = custom_path.unwrap_or_else(|| get_config_path());
    
    // Check if file exists
    if !config_path.exists() {
        println!("No config file found at {}", config_path.display());
        println!("Will use default configuration");
        return Ok(());
    }
    
    // Read file content
    let content = fs::read_to_string(&config_path)?;
    
    // Parse TOML (syntax check)
    let config: Config = match toml::from_str(&content) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Configuration syntax error:");
            eprintln!("  {}", e);
            if let Some((line, col)) = e.span().map(|s| (s.start, s.end)) {
                eprintln!("  at line {}", line);
            }
            process::exit(1);
        }
    };
    
    // Semantic validation
    let result = validate_config(&config);
    
    // Print warnings
    for warning in &result.warnings {
        eprintln!("Warning: {}: {}", warning.field, warning.message);
    }
    
    // Print errors and exit
    if !result.is_valid() {
        eprintln!("\nConfiguration errors:");
        for error in &result.errors {
            eprintln!("  {}: {}", error.field, error.message);
            if let Some(ref suggestion) = error.suggestion {
                eprintln!("    Suggestion: {}", suggestion);
            }
        }
        process::exit(1);
    }
    
    println!("Configuration valid");
    Ok(())
}
```

### Dependencies

Uses existing dependencies:
- `toml` for TOML parsing
- Config schema from Story 4.1

### Testing Strategy

**Unit Tests:**
- Test validate_config with valid config
- Test validate_config with invalid port
- Test validate_config with invalid log level
- Test validate_config with invalid delay values
- Test validate_config with invalid URLs

**Integration Tests:**
- Test validate command with valid file
- Test validate command with syntax error file
- Test validate command with semantic error file
- Test validate command with no file

### References

- [Source: _bmad-output/planning-artifacts/architecture.md#Config Module]
- [Source: _bmad-output/planning-artifacts/epics.md#Story 4.4: Config Validate Command]

## File List

**Files to create:**
- `src/config/validation.rs`
- `tests/config_validate_test.rs`

**Files to modify:**
- `src/config/mod.rs`
- `src/cli/app.rs`
- `src/cli/commands/config.rs`
- `src/main.rs`
- `_bmad-output/implementation-artifacts/sprint-status.yaml`

## Change Log

- 2026-02-05: Story created and marked ready-for-dev
- 2026-02-05: Implemented config validation command, semantic checks, and tests
