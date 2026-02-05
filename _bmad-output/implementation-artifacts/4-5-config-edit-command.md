# Story 4.5: Config Edit Command

Status: done

## Story

As a user,
I want to open my config in my preferred editor,
So that I can make changes easily.

## Acceptance Criteria

**AC1: Open in Editor**
**Given** a config file exists
**When** I run `palingenesis config edit`
**Then** the file opens in $EDITOR (or vi/nano fallback)

**AC2: Editor Fallback**
**Given** $EDITOR is not set
**When** I run `palingenesis config edit`
**Then** it tries `vi`, then `nano`, then fails with helpful message

**AC3: Create Before Edit**
**Given** no config file exists
**When** I run `palingenesis config edit`
**Then** it creates the default config first
**And** then opens it in the editor

**AC4: Post-Edit Validation**
**Given** I edit and save the config
**When** the editor closes
**Then** validation runs automatically
**And** displays result

**AC5: Custom Path**
**Given** I run `palingenesis config edit --path /custom/config.toml`
**When** the editor opens
**Then** it opens the specified file

**AC6: Skip Validation**
**Given** I run `palingenesis config edit --no-validate`
**When** the editor closes
**Then** validation is skipped

## Tasks / Subtasks

- [x] Add config edit subcommand to CLI (AC: 1, 5, 6)
  - [x] Add `edit` subcommand to `ConfigCmd` enum
  - [x] Add `--path` option for custom config path
  - [x] Add `--no-validate` flag to skip validation

- [x] Implement editor detection (AC: 1, 2)
  - [x] Check `$EDITOR` environment variable
  - [x] Check `$VISUAL` environment variable
  - [x] Fall back to `vi` on Unix
  - [x] Fall back to `nano` if `vi` not found
  - [x] Fall back to `notepad` on Windows
  - [x] Return error if no editor found

- [x] Implement create-before-edit (AC: 3)
  - [x] Check if config file exists
  - [x] If not, call config init logic
  - [x] Then proceed to open editor

- [x] Implement editor invocation (AC: 1)
  - [x] Spawn editor process with config file path
  - [x] Wait for editor to exit
  - [x] Handle editor errors

- [x] Implement post-edit validation (AC: 4)
  - [x] After editor exits, call validate logic
  - [x] Display validation result
  - [x] Don't exit with error (just inform)

- [x] Handle skip validation flag (AC: 6)
  - [x] Skip validation when --no-validate is set
  - [x] Print message that validation was skipped

- [x] Add unit/integration tests (AC: 1, 2, 3, 4, 5, 6)
  - [x] Test editor detection
  - [x] Test editor fallback chain
  - [x] Test create before edit
  - [x] Test post-edit validation

## Dev Notes

### Architecture Requirements

**From architecture.md - CLI Module:**

```
src/cli/commands/
    config.rs             # config init, validate, show, edit
```

**Implements:** FR23 (User can edit config file via CLI)

### Technical Implementation

**CLI Command Definition:**

```rust
// src/cli/app.rs
#[derive(Subcommand)]
pub enum ConfigCmd {
    /// Edit configuration file in your preferred editor
    Edit {
        /// Custom path to config file
        #[arg(long)]
        path: Option<PathBuf>,
        
        /// Skip validation after editing
        #[arg(long)]
        no_validate: bool,
    },
    // ... other config commands
}
```

**Config Edit Handler:**

```rust
// src/cli/commands/config.rs
use std::env;
use std::path::PathBuf;
use std::process::Command;

use crate::config::paths::get_config_path;

pub fn handle_edit(custom_path: Option<PathBuf>, no_validate: bool) -> anyhow::Result<()> {
    let config_path = custom_path.unwrap_or_else(|| get_config_path());
    
    // Create config if doesn't exist
    if !config_path.exists() {
        println!("No config file found. Creating default config...");
        super::handle_init(false, Some(config_path.clone()))?;
    }
    
    // Find editor
    let editor = find_editor()?;
    
    // Open editor
    println!("Opening {} with {}...", config_path.display(), editor);
    let status = Command::new(&editor)
        .arg(&config_path)
        .status()?;
    
    if !status.success() {
        anyhow::bail!("Editor exited with non-zero status");
    }
    
    // Post-edit validation
    if !no_validate {
        println!("\nValidating configuration...");
        match super::handle_validate(Some(config_path.clone())) {
            Ok(_) => {}
            Err(e) => {
                eprintln!("Validation failed: {}", e);
                eprintln!("You may want to run `palingenesis config edit` again to fix issues.");
            }
        }
    } else {
        println!("Validation skipped (--no-validate)");
    }
    
    Ok(())
}

fn find_editor() -> anyhow::Result<String> {
    // Check environment variables first
    if let Ok(editor) = env::var("EDITOR") {
        if !editor.is_empty() {
            return Ok(editor);
        }
    }
    
    if let Ok(visual) = env::var("VISUAL") {
        if !visual.is_empty() {
            return Ok(visual);
        }
    }
    
    // Platform-specific fallbacks
    #[cfg(unix)]
    {
        // Try vi
        if Command::new("which").arg("vi").status().map(|s| s.success()).unwrap_or(false) {
            return Ok("vi".to_string());
        }
        
        // Try nano
        if Command::new("which").arg("nano").status().map(|s| s.success()).unwrap_or(false) {
            return Ok("nano".to_string());
        }
    }
    
    #[cfg(windows)]
    {
        return Ok("notepad".to_string());
    }
    
    anyhow::bail!(
        "No editor found. Set the EDITOR environment variable.\n\
         Example: export EDITOR=vim"
    )
}
```

### Dependencies

Uses existing dependencies:
- `std::process::Command` for spawning editor
- `std::env` for environment variables
- Config init from Story 4.2
- Config validate from Story 4.4

### Testing Strategy

**Unit Tests:**
- Test find_editor with $EDITOR set
- Test find_editor with $VISUAL set
- Test find_editor fallback chain

**Integration Tests:**
- Test edit command creates config if missing
- Test edit command with custom path
- Mock editor interaction for CI

### Notes

Editor testing is tricky in CI environments. Consider:
- Setting `EDITOR=cat` for basic tests
- Skipping interactive tests in CI
- Using mock editor scripts

### References

- [Source: _bmad-output/planning-artifacts/architecture.md#CLI Module]
- [Source: _bmad-output/planning-artifacts/epics.md#Story 4.5: Config Edit Command]

## File List

**Files to create:**
- `tests/config_edit_test.rs`

**Files to modify:**
- `src/cli/app.rs`
- `src/cli/commands/config.rs`
- `src/main.rs`
- `_bmad-output/implementation-artifacts/sprint-status.yaml`

## Change Log

- 2026-02-05: Story created and marked ready-for-dev
- 2026-02-05: Implemented config edit command, editor selection, and tests
