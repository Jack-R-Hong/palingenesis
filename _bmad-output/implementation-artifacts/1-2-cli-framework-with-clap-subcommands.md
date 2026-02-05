# Story 1.2: CLI Framework with Clap Subcommands

Status: done

## Story

As a user,
I want a well-structured CLI with subcommands and help text,
So that I can easily discover and use all palingenesis features.

## Acceptance Criteria

**AC1: Main Help**
**Given** the palingenesis binary
**When** I run `palingenesis --help`
**Then** I see the main help text with available subcommands
**And** the output includes: daemon, status, logs, config, pause, resume, new-session

**AC2: Version**
**Given** the palingenesis binary
**When** I run `palingenesis --version`
**Then** I see the version number in semver format

**AC3: Daemon Subcommands**
**Given** the palingenesis binary
**When** I run `palingenesis daemon --help`
**Then** I see help for daemon subcommands: start, stop, restart, reload, status

**AC4: Logs Options**
**Given** the palingenesis binary
**When** I run `palingenesis logs --help`
**Then** I see options: --follow, --tail N, --since TIME

## Tasks / Subtasks

- [x] Add missing commands to CLI structure (AC: 1, 3)
  - [x] Add `NewSession` command to Commands enum
  - [x] Add `Status` variant to DaemonAction enum
- [x] Add missing logs option (AC: 4)
  - [x] Add `--since` option to Logs command (accepts duration string like "1h", "30m")
- [x] Refactor CLI into app.rs and commands/ structure
  - [x] Create `src/cli/app.rs` with Clap App definition
  - [x] Create `src/cli/commands/mod.rs` for command handler re-exports
  - [x] Create `src/cli/commands/daemon.rs` with handler stubs
  - [x] Create `src/cli/commands/status.rs` with handler stub
  - [x] Create `src/cli/commands/logs.rs` with handler stub
  - [x] Create `src/cli/commands/config.rs` with handler stubs
  - [x] Create `src/cli/commands/session.rs` with handler stubs (pause, resume, new-session)
  - [x] Update `src/cli/mod.rs` to re-export from app.rs and commands/
- [x] Implement main.rs command dispatch (AC: 1, 2, 3, 4)
  - [x] Add match arms for all commands
  - [x] Return appropriate exit codes
  - [x] Print "not implemented" messages for now (actual implementation in later stories)
- [x] Add unit tests for new commands
  - [x] Test new-session command parsing
  - [x] Test daemon status subcommand parsing
  - [x] Test logs --since option parsing
- [x] Add integration tests for CLI behavior (AC: 1, 2, 3, 4)
  - [x] Test `--help` output contains all required subcommands
  - [x] Test `--version` outputs semver format
  - [x] Test `daemon --help` lists all subcommands
  - [x] Test `logs --help` shows all options

## Dev Notes

### Current State Analysis

Story 1-1 already established the CLI structure in `src/cli/mod.rs` with:
- `Cli` struct with clap Parser derive
- `Commands` enum with: Daemon, Status, Logs, Pause, Resume, Config
- `DaemonAction`: Start, Stop, Restart, Reload
- `ConfigAction`: Init, Show, Validate, Edit
- Logs with `--follow` and `--tail` flags
- 20 unit tests for CLI parsing

**Gaps to address in this story:**
1. Missing `NewSession` command in Commands enum (per AC1)
2. Missing `Status` variant in DaemonAction enum (per AC3)
3. Missing `--since TIME` option for Logs command (per AC4)
4. CLI structure needs refactoring to match architecture (app.rs + commands/)

### Architecture Requirements

**From architecture.md - Project Structure:**
```
src/cli/
├── mod.rs           # CLI module root
├── app.rs           # Clap App definition
└── commands/
    ├── mod.rs
    ├── daemon.rs    # start, stop, restart, reload, status
    ├── status.rs    # status, health
    ├── config.rs    # config init, validate, show, edit
    └── logs.rs      # logs --follow --tail --since
```

**From architecture.md - Implementation Patterns:**
- Use clap derive macros for clean code (ARCH7)
- clap version: 4.5.50 with features ["derive", "env"]
- Module re-export pattern: expose public API through mod.rs

### Technical Requirements

**CLI Structure Pattern:**
```rust
// src/cli/app.rs
use clap::Parser;

#[derive(Parser, Debug)]
#[command(name = "palingenesis")]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(clap::Subcommand, Debug)]
pub enum Commands {
    Daemon { ... },
    Status,
    Logs { ... },
    Config { ... },
    Pause,
    Resume,
    NewSession,  // NEW
}
```

**DaemonAction (updated):**
```rust
#[derive(clap::Subcommand, Debug)]
pub enum DaemonAction {
    Start { foreground: bool },
    Stop,
    Restart,
    Reload,
    Status,  // NEW
}
```

**Logs command (updated):**
```rust
/// View daemon logs
Logs {
    /// Follow log output
    #[arg(short, long)]
    follow: bool,
    /// Number of lines to show
    #[arg(short, long, default_value = "20")]
    tail: u32,
    /// Show logs since duration (e.g., "1h", "30m", "1d")  // NEW
    #[arg(short, long)]
    since: Option<String>,
}
```

### Command Handler Pattern

Each command handler should be an async function that returns `anyhow::Result<()>`:

```rust
// src/cli/commands/daemon.rs
pub async fn handle_start(foreground: bool) -> anyhow::Result<()> {
    println!("daemon start not implemented (Story 1.8)");
    Ok(())
}

pub async fn handle_stop() -> anyhow::Result<()> {
    println!("daemon stop not implemented (Story 1.9)");
    Ok(())
}
// ... etc
```

### Exit Code Convention

| Scenario | Exit Code |
|----------|-----------|
| Success | 0 |
| User error (bad args) | 1 |
| Daemon not running | 1 |
| Internal error | 2 |

### Testing Requirements

**Unit Tests (inline in modules):**
- Test command parsing for all variants
- Test flag combinations
- Test invalid input rejection

**Integration Tests (tests/cli_test.rs):**
```rust
use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn test_help_shows_all_subcommands() {
    Command::cargo_bin("palingenesis")
        .unwrap()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("daemon"))
        .stdout(predicate::str::contains("status"))
        .stdout(predicate::str::contains("logs"))
        .stdout(predicate::str::contains("config"))
        .stdout(predicate::str::contains("pause"))
        .stdout(predicate::str::contains("resume"))
        .stdout(predicate::str::contains("new-session"));
}

#[test]
fn test_version_is_semver() {
    Command::cargo_bin("palingenesis")
        .unwrap()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::is_match(r"\d+\.\d+\.\d+").unwrap());
}
```

### File Structure After This Story

```
src/cli/
├── mod.rs              # Re-exports Cli, Commands, run()
├── app.rs              # Clap App definition (Cli, Commands, Actions)
└── commands/
    ├── mod.rs          # Re-exports all handlers
    ├── daemon.rs       # start, stop, restart, reload, status handlers
    ├── status.rs       # status handler
    ├── logs.rs         # logs handler
    ├── config.rs       # config init/show/validate/edit handlers
    └── session.rs      # pause, resume, new-session handlers
```

### Previous Story Learnings

From Story 1-1 Dev Notes:
1. **Rust Edition**: Using Rust 2024 edition (rust-version 1.85+)
2. **Test Infrastructure**: Unit tests inline with `#[cfg(test)]`, integration tests in `tests/`
3. **CLI moved to library**: CLI types already in `src/cli/mod.rs`, main.rs imports from library
4. **20 existing tests**: Must not break existing CLI tests when refactoring

### References

- [Source: _bmad-output/planning-artifacts/architecture.md#Project Structure & Boundaries]
- [Source: _bmad-output/planning-artifacts/architecture.md#Implementation Patterns & Consistency Rules]
- [Source: _bmad-output/planning-artifacts/epics.md#Story 1.2: CLI Framework with Clap Subcommands]
- [Source: _bmad-output/implementation-artifacts/1-1-project-initialization-from-starter-template.md#Completion Notes]

## Dev Agent Record

### Agent Model Used

openai/gpt-5.2-codex

### Debug Log References

- cargo build
- cargo test
- rust-analyzer LSP diagnostics unavailable (tool error: unknown binary)

### Completion Notes List

- Added new-session command, daemon status action, and logs --since option.
- Refactored CLI definitions into app.rs and command handler modules with stubbed handlers.
- Implemented main dispatch with exit code handling and added unit/integration tests for help/version and parsing.

### File List

**Files to create:**
- `src/cli/app.rs`
- `src/cli/commands/mod.rs`
- `src/cli/commands/daemon.rs`
- `src/cli/commands/status.rs`
- `src/cli/commands/logs.rs`
- `src/cli/commands/config.rs`
- `src/cli/commands/session.rs`
- `tests/cli_test.rs`

**Files to modify:**
- `_bmad-output/implementation-artifacts/1-2-cli-framework-with-clap-subcommands.md`
- `_bmad-output/implementation-artifacts/sprint-status.yaml`
- `src/cli/mod.rs` - Refactor to re-export from app.rs
- `src/main.rs` - Update command dispatch

## Change Log

| Date | Change |
|------|--------|
| 2026-02-05 | Story created from epics.md, status: ready-for-dev |
| 2026-02-05 | Implemented clap refactor, command stubs, and CLI tests; marked story ready for review |
