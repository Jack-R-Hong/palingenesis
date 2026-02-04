# Story 1.1: Project Initialization from Starter Template

Status: ready-for-dev

## Story

As a developer,
I want to initialize the palingenesis project from a proven Rust CLI template,
So that I have a solid foundation with CI/CD, proper structure, and best practices.

## Acceptance Criteria

**Given** the template URL `https://github.com/skanehira/rust-cli-template.git`
**When** I run `cargo generate --git {url} --name palingenesis`
**Then** a new project directory is created with the template structure
**And** `.github/workflows/` contains CI/CD configuration
**And** `Cargo.toml` exists with initial dependencies
**And** `src/main.rs` exists with basic entry point

**Given** the generated project
**When** I run `cargo build`
**Then** the project compiles without errors
**And** the binary is created at `target/debug/palingenesis`

## Tasks / Subtasks

- [ ] Initialize project from starter template (AC: 1)
  - [ ] Run `cargo generate` with template URL
  - [ ] Verify directory structure created
  - [ ] Verify CI/CD workflows present
- [ ] Customize Cargo.toml with project dependencies (AC: 2)
  - [ ] Update package metadata (name, description, license)
  - [ ] Add tokio 1.49 with full features
  - [ ] Add axum 0.8.8 for HTTP server
  - [ ] Add clap 4.5.50 with derive features
  - [ ] Add serde 1.0.228 with derive features
  - [ ] Add notify 8.2.0 for file watching
  - [ ] Add reqwest 0.13.1 for HTTP client
  - [ ] Add tracing 0.1.44 and tracing-subscriber 0.3.22
  - [ ] Add thiserror 2.0.17 and anyhow 1.0.100
  - [ ] Add platform-specific dependencies (nix, systemd)
- [ ] Create module directory structure (AC: 2)
  - [ ] Create `src/cli/` directory
  - [ ] Create `src/daemon/` directory
  - [ ] Create `src/monitor/` directory
  - [ ] Create `src/resume/` directory
  - [ ] Create `src/http/` directory
  - [ ] Create `src/ipc/` directory
  - [ ] Create `src/notify/` directory
  - [ ] Create `src/config/` directory
  - [ ] Create `src/state/` directory
  - [ ] Create `src/telemetry/` directory
- [ ] Verify build succeeds (AC: 2)
  - [ ] Run `cargo build`
  - [ ] Confirm binary created at `target/debug/palingenesis`
  - [ ] Run `cargo test` to verify test infrastructure

## Dev Notes

### Architecture Context

This story implements **ARCH1** and **ARCH2** from the Architecture document:
- ARCH1: Use `skanehira/rust-cli-template` via `cargo generate` for project initialization
- ARCH2: Initialize with CI/CD foundation (GitHub Actions)

### Project Structure Requirements

The final directory structure must match the architecture specification:

```
palingenesis/
├── .github/workflows/     # CI/CD from template
├── src/
│   ├── main.rs           # Entry point
│   ├── lib.rs            # Library interface
│   ├── cli/              # CLI commands module
│   ├── daemon/           # Daemon orchestration
│   ├── monitor/          # File watcher & session parsing
│   ├── resume/           # Resume strategies
│   ├── http/             # Axum HTTP server
│   ├── ipc/              # Unix socket IPC
│   ├── notify/           # Notification dispatcher
│   ├── config/           # Configuration management
│   ├── state/            # State persistence
│   └── telemetry/        # Tracing & OTEL
├── config/               # Default config templates
├── systemd/              # Linux systemd unit
├── launchd/              # macOS launchd plist
├── tests/                # Integration tests
│   ├── fixtures/         # Test data
│   └── integration/      # Integration test files
├── Cargo.toml
└── README.md
```

### Cargo.toml Dependencies

**Complete dependency specification from Architecture doc:**

```toml
[package]
name = "palingenesis"
version = "0.1.0"
edition = "2021"
rust-version = "1.75"
description = "Agent resurrection system for continuous AI workflow execution"
license = "MIT"

[dependencies]
# Async runtime
tokio = { version = "1.49", features = ["full"] }

# HTTP server
axum = "0.8.8"
tower = "0.5.3"
tower-http = { version = "0.6.8", features = ["trace", "timeout", "cors"] }

# CLI parsing
clap = { version = "4.5.50", features = ["derive", "env"] }

# Serialization & config
serde = { version = "1.0.228", features = ["derive"] }
toml = "0.9.11"

# File watching
notify = "8.2.0"

# HTTP client (for webhooks)
reqwest = { version = "0.13.1", default-features = false, features = ["json", "rustls-tls"] }

# Logging & tracing
tracing = "0.1.44"
tracing-subscriber = { version = "0.3.22", features = ["json", "env-filter"] }

# Error handling
thiserror = "2.0.17"
anyhow = "1.0.100"

# Optional: OTEL (Growth feature)
opentelemetry = { version = "0.31.0", optional = true }

[target.'cfg(unix)'.dependencies]
nix = { version = "0.29", features = ["signal"] }

[target.'cfg(target_os = "linux")'.dependencies]
systemd = "0.10"

[features]
default = []
otel = ["opentelemetry"]
```

### Technical Requirements

**From Architecture Document:**

1. **Rust Version**: 1.75+ (stable)
2. **Binary Size Target**: <10MB
3. **Platform Support**: Linux (Ubuntu 20.04+, Fedora 38+), macOS (12.0+ Monterey)
4. **Build System**: Cargo with workspace support

### Library & Framework Requirements

**Core Dependencies (verified Feb 2026):**

| Crate | Version | Purpose |
|-------|---------|---------|
| tokio | 1.49.0 | Async runtime |
| axum | 0.8.8 | HTTP server for control API |
| clap | 4.5.50 | CLI argument parsing |
| serde | 1.0.228 | Serialization |
| toml | 0.9.11 | Config file parsing |
| notify | 8.2.0 | File system watching |
| reqwest | 0.13.1 | HTTP client (outbound webhooks) |
| tracing | 0.1.44 | Structured logging |
| tracing-subscriber | 0.3.22 | Log formatters |
| tower | 0.5.3 | Middleware |
| tower-http | 0.6.8 | HTTP middleware |
| thiserror | 2.0.17 | Domain error types |
| anyhow | 1.0.100 | Application error handling |

**Platform-Specific:**
- `nix` 0.29: Unix signal handling (Linux/macOS)
- `systemd` 0.10: systemd integration (Linux only)

### File Structure Requirements

**Module Organization Pattern:**

Each module must have:
- `mod.rs` - Module root with public API re-exports
- Submodules for specific functionality
- `error.rs` for module-specific error types (where applicable)

**Example:**
```rust
// src/monitor/mod.rs
mod classifier;
mod session;
mod watcher;

pub use classifier::StopReason;
pub use session::{Session, SessionState};
pub use watcher::SessionWatcher;
```

### Testing Requirements

**Test Infrastructure:**

1. **Unit Tests**: Inline `#[cfg(test)]` modules in each source file
2. **Integration Tests**: `tests/integration/` directory
3. **Test Fixtures**: `tests/fixtures/` for test data
4. **Coverage Target**: >80% for core logic (NFR18)

**Initial Test Structure:**
```
tests/
├── fixtures/
│   ├── session_rate_limit.md
│   ├── session_context_exhausted.md
│   └── config_valid.toml
└── integration/
    ├── daemon_test.rs
    ├── monitor_test.rs
    ├── resume_test.rs
    └── http_test.rs
```

### Project Context Reference

**From PRD:**

- **Project Type**: CLI tool + API backend hybrid
- **Domain**: Developer Tools
- **Complexity**: Medium
- **Context**: Greenfield

**Core Concept**: A lightweight Rust daemon that monitors Sisyphus/opencode agent sessions and automatically resumes work when the agent stops due to rate limits, context limits, or other interruptions.

**Key Capabilities**:
1. Monitors opencode/Sisyphus sessions for stop signals
2. Classifies stop reason (rate limit vs context exhaustion vs completion)
3. Waits intelligently (respects `Retry-After`, polls quota endpoints)
4. Resumes work automatically (same session or new session from `Next-step.md`)
5. Minimizes tokens via step-file architecture
6. Observes via OpenTelemetry (traces, metrics, logs)
7. Notifies via external channels (webhook, Slack, Discord, Telegram, ntfy)
8. Controlled via external channels (pause/resume/skip/abort/status/config)

### References

- [Source: _bmad-output/planning-artifacts/architecture.md#Starter Template Evaluation]
- [Source: _bmad-output/planning-artifacts/architecture.md#Verified Dependencies]
- [Source: _bmad-output/planning-artifacts/architecture.md#Project Structure & Boundaries]
- [Source: _bmad-output/planning-artifacts/prd.md#Product Requirements Document]
- [Source: _bmad-output/planning-artifacts/epics.md#Epic 1: Installable CLI with Daemon Lifecycle]

## Dev Agent Record

### Agent Model Used

_To be filled by dev agent_

### Debug Log References

_To be filled by dev agent_

### Completion Notes List

_To be filled by dev agent_

### File List

_To be filled by dev agent_
