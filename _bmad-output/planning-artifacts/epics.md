---
stepsCompleted: ['step-01-validate-prerequisites', 'step-02-design-epics', 'step-03-create-stories', 'step-04-final-validation']
workflow_completed: true
inputDocuments:
  - '_bmad-output/planning-artifacts/prd.md'
  - '_bmad-output/planning-artifacts/architecture.md'
workflowType: 'epics-and-stories'
project_name: 'palingenesis'
user_name: 'Jack'
date: '2026-02-06'
newEpicsAdded: ['Epic 8: MCP Server Interface', 'Epic 9: OpenCode Process Management']
---

# palingenesis - Epic Breakdown

## Overview

This document provides the complete epic and story breakdown for palingenesis, decomposing the requirements from the PRD and Architecture into implementable stories.

## Requirements Inventory

### Functional Requirements

**Session Monitoring (FR1-FR7)**
- FR1: Daemon can detect when opencode process starts
- FR2: Daemon can detect when opencode process stops
- FR3: Daemon can detect when opencode session hits rate limit (HTTP 429)
- FR4: Daemon can detect when opencode session exhausts context window
- FR5: Daemon can detect when user explicitly exits session
- FR6: Daemon can read session state from markdown frontmatter
- FR7: Daemon can parse `stepsCompleted` array from session files

**Session Resumption (FR8-FR13)**
- FR8: Daemon can resume same session after rate limit clears
- FR9: Daemon can start new session from `Next-step.md` after context exhaustion
- FR10: Daemon can backup session file before starting new session
- FR11: Daemon can respect `Retry-After` headers when waiting
- FR12: Daemon can implement exponential backoff for retries
- FR13: Daemon can track resumption history for audit trail

**CLI Control (FR14-FR20)**
- FR14: User can start daemon via CLI (`palingenesis daemon start`)
- FR15: User can stop daemon via CLI (`palingenesis daemon stop`)
- FR16: User can check daemon status via CLI (`palingenesis status`)
- FR17: User can view daemon logs via CLI (`palingenesis logs`)
- FR18: User can pause monitoring via CLI (`palingenesis pause`)
- FR19: User can resume monitoring via CLI (`palingenesis resume`)
- FR20: User can force new session via CLI (`palingenesis new-session`)

**Configuration (FR21-FR25)**
- FR21: User can initialize config file via CLI (`palingenesis config init`)
- FR22: User can validate config file via CLI (`palingenesis config validate`)
- FR23: User can edit config file via CLI (`palingenesis config edit`)
- FR24: Daemon can reload config without restart (SIGHUP)
- FR25: Daemon can auto-detect AI assistants if not configured

**Notifications - Growth (FR26-FR30)**
- FR26: Daemon can send webhook notifications on events
- FR27: Daemon can send Discord notifications on events
- FR28: Daemon can send Slack notifications on events
- FR29: Daemon can send ntfy.sh notifications on events
- FR30: User can configure notification channels via config file

**External Control - Growth (FR31-FR34)**
- FR31: User can check status via Discord/Slack command
- FR32: User can pause daemon via Discord/Slack command
- FR33: User can resume daemon via Discord/Slack command
- FR34: User can view logs via Discord/Slack command

**Observability - Growth (FR35-FR40)**
- FR35: Daemon can export metrics in Prometheus format
- FR36: Daemon can export traces via OTLP
- FR37: Daemon can export structured logs via OTLP
- FR38: User can view metrics dashboard in Grafana
- FR39: Daemon can calculate and report "time saved" metric
- FR40: Daemon can calculate and report "saves count" metric

**MCP Server Interface - Growth (FR41-FR44)**
- FR41: Daemon supports MCP stdio transport interface
- FR42: MCP interface uses JSON-RPC 2.0 protocol
- FR43: Daemon exposes control functions as MCP tools (status, pause, resume, new-session, logs)
- FR44: Supports OpenCode `type: "local"` MCP configuration format

**OpenCode Process Management - Growth (FR45-FR48)**
- FR45: Daemon detects OpenCode process crash/exit
- FR46: Daemon automatically restarts OpenCode via `opencode serve`
- FR47: Daemon manages sessions via OpenCode HTTP API (`/session/*` endpoints)
- FR48: User can configure OpenCode serve port/hostname via config file

**Telegram Integration - Growth (FR49-FR54)**
- FR49: Daemon can send Telegram notifications on events (via Bot API sendMessage)
- FR50: Daemon can receive incoming commands via Telegram Bot API
- FR51: User can check status via Telegram command
- FR52: User can pause/resume daemon via Telegram command
- FR53: User can execute control commands (skip/abort/new-session/config) via Telegram
- FR54: User can view logs via Telegram command

### Non-Functional Requirements

**Performance**
- NFR1: Stop detection latency <5 seconds
- NFR2: Resume execution time <2 seconds
- NFR3: CLI command response <500ms
- NFR4: Memory usage (idle) <50MB
- NFR5: CPU usage (idle) <1%

**Reliability**
- NFR6: Resume success rate >95%
- NFR7: Daemon uptime >99.9%
- NFR8: Graceful degradation on failure
- NFR9: State persistence survives restart

**Security**
- NFR10: No credential storage in daemon
- NFR11: Secure webhook URLs (config file permissions 600)
- NFR12: No network by default (external features opt-in)
- NFR13: Audit logging for all actions

**Compatibility**
- NFR14: Rust version 1.75+ (stable)
- NFR15: Linux support (Ubuntu 20.04+, Fedora 38+)
- NFR16: macOS support (12.0+ Monterey)
- NFR17: Binary size <10MB

**Maintainability**
- NFR18: Code coverage >80% for core logic
- NFR19: Documentation (README, man pages, --help)
- NFR20: Semantic versioning release cadence
- NFR21: Config file versioning for backward compatibility

### Additional Requirements

**From Architecture - Starter Template**
- ARCH1: Use `skanehira/rust-cli-template` via `cargo generate` for project initialization
- ARCH2: Initialize with CI/CD foundation (GitHub Actions)

**From Architecture - Project Structure**
- ARCH3: Follow defined module structure: `cli/`, `daemon/`, `monitor/`, `resume/`, `http/`, `ipc/`, `notify/`, `config/`, `state/`, `telemetry/`
- ARCH4: Use step-file architecture with 60+ files as specified

**From Architecture - Technology Stack**
- ARCH5: tokio 1.49 for async runtime
- ARCH6: axum 0.8.8 for HTTP server (control API)
- ARCH7: clap 4.5.50 for CLI parsing
- ARCH8: notify 8.2.0 for file system watching
- ARCH9: tracing 0.1.44 for structured logging

**From Architecture - Data Architecture**
- ARCH10: State persistence via JSON file at `~/.local/state/palingenesis/state.json`
- ARCH11: Audit trail via append-only JSONL at `~/.local/state/palingenesis/audit.jsonl`
- ARCH12: Session parsing via YAML frontmatter only

**From Architecture - API & Communication**
- ARCH13: CLI-Daemon IPC via Unix socket at `/run/user/{uid}/palingenesis.sock`
- ARCH14: HTTP API at `127.0.0.1:7654` (configurable) for external integrations
- ARCH15: Unix socket protocol: STATUS, PAUSE, RESUME, RELOAD commands

**From Architecture - Platform Specifics**
- ARCH16: Linux config at `~/.config/palingenesis/`
- ARCH17: macOS config at `~/Library/Application Support/palingenesis/`
- ARCH18: PID file at `/run/user/{uid}/palingenesis.pid`
- ARCH19: Support both foreground (systemd/launchd) and daemonize modes

**From Architecture - Implementation Patterns**
- ARCH20: Use `thiserror` for domain errors, `anyhow` for application code
- ARCH21: Use `CancellationToken` for graceful shutdown
- ARCH22: Structured logging with tracing macros
- ARCH23: Consistent API response format: `{ "success": bool, "data/error": {...} }`

### FR Coverage Map

| FR | Epic | Description |
|----|------|-------------|
| FR1 | Epic 2 | Detect opencode process start |
| FR2 | Epic 2 | Detect opencode process stop |
| FR3 | Epic 2 | Detect rate limit (HTTP 429) |
| FR4 | Epic 2 | Detect context window exhaustion |
| FR5 | Epic 2 | Detect user explicit exit |
| FR6 | Epic 2 | Read session state from frontmatter |
| FR7 | Epic 2 | Parse stepsCompleted array |
| FR8 | Epic 3 | Resume same session after rate limit |
| FR9 | Epic 3 | Start new session from Next-step.md |
| FR10 | Epic 3 | Backup session before new session |
| FR11 | Epic 3 | Respect Retry-After headers |
| FR12 | Epic 3 | Exponential backoff for retries |
| FR13 | Epic 3 | Track resumption history (audit) |
| FR14 | Epic 1 | CLI: daemon start |
| FR15 | Epic 1 | CLI: daemon stop |
| FR16 | Epic 1 | CLI: status |
| FR17 | Epic 1 | CLI: logs |
| FR18 | Epic 3 | CLI: pause |
| FR19 | Epic 3 | CLI: resume |
| FR20 | Epic 3 | CLI: new-session |
| FR21 | Epic 4 | CLI: config init |
| FR22 | Epic 4 | CLI: config validate |
| FR23 | Epic 4 | CLI: config edit |
| FR24 | Epic 4 | Hot reload via SIGHUP |
| FR25 | Epic 4 | Auto-detect AI assistants |
| FR26 | Epic 5 | Webhook notifications |
| FR27 | Epic 5 | Discord notifications |
| FR28 | Epic 5 | Slack notifications |
| FR29 | Epic 5 | ntfy.sh notifications |
| FR30 | Epic 5 | Configure notification channels |
| FR31 | Epic 6 | Status via Discord/Slack |
| FR32 | Epic 6 | Pause via Discord/Slack |
| FR33 | Epic 6 | Resume via Discord/Slack |
| FR34 | Epic 6 | Logs via Discord/Slack |
| FR35 | Epic 7 | Prometheus metrics export |
| FR36 | Epic 7 | OTLP traces export |
| FR37 | Epic 7 | OTLP logs export |
| FR38 | Epic 7 | Grafana dashboard |
| FR39 | Epic 7 | Time saved metric |
| FR40 | Epic 7 | Saves count metric |
| FR41 | Epic 8 | MCP stdio transport interface |
| FR42 | Epic 8 | JSON-RPC 2.0 protocol |
| FR43 | Epic 8 | MCP tools (status, pause, resume, etc.) |
| FR44 | Epic 8 | OpenCode local MCP config format |
| FR45 | Epic 9 | Detect OpenCode process crash/exit |
| FR46 | Epic 9 | Auto-restart OpenCode via serve |
| FR47 | Epic 9 | Manage sessions via HTTP API |
| FR48 | Epic 9 | Configure OpenCode serve port/hostname |
| FR49 | Epic 10 | Telegram outbound notifications |
| FR50 | Epic 10 | Telegram incoming command reception |
| FR51 | Epic 10 | Status via Telegram command |
| FR52 | Epic 10 | Pause/resume via Telegram command |
| FR53 | Epic 10 | Control commands via Telegram |
| FR54 | Epic 10 | Logs via Telegram command |

## Epic List

### Epic 1: Installable CLI with Daemon Lifecycle
User can install palingenesis, start/stop the daemon, check status, and view logs.
**FRs covered:** FR14, FR15, FR16, FR17
**ARCHs covered:** ARCH1-ARCH11, ARCH13, ARCH16-ARCH23
**Priority:** MVP

### Epic 2: Session Detection & Classification
Daemon monitors opencode sessions and correctly identifies when/why they stop (rate limit vs context exhaustion vs user exit).
**FRs covered:** FR1, FR2, FR3, FR4, FR5, FR6, FR7
**ARCHs covered:** ARCH12
**Priority:** MVP

### Epic 3: Automatic Session Resumption
Daemon automatically resumes work after rate limits or starts new session after context exhaustion - the "first save" moment.
**FRs covered:** FR8, FR9, FR10, FR11, FR12, FR13, FR18, FR19, FR20
**Priority:** MVP

### Epic 4: Configuration Management
User can customize daemon behavior via TOML config file, validate config, and daemon reloads dynamically without restart.
**FRs covered:** FR21, FR22, FR23, FR24, FR25
**Priority:** MVP

### Epic 5: Event Notifications
User receives push notifications on their preferred channel (webhook, Discord, Slack, ntfy.sh) when events occur.
**FRs covered:** FR26, FR27, FR28, FR29, FR30
**Priority:** Growth

### Epic 6: Remote Control & External API
User can monitor and control daemon remotely via Discord/Slack commands or HTTP API.
**FRs covered:** FR31, FR32, FR33, FR34
**ARCHs covered:** ARCH14, ARCH15
**Priority:** Growth

### Epic 7: Observability & Metrics
User can view metrics in Prometheus/Grafana, see traces in Jaeger, and track "time saved" and "saves count".
**FRs covered:** FR35, FR36, FR37, FR38, FR39, FR40
**Priority:** Growth

### Epic 10: Bi-Directional Telegram Bot
User can receive notifications AND send control commands to palingenesis via Telegram Bot, providing full mobile control without SSH access.
**FRs covered:** FR49, FR50, FR51, FR52, FR53, FR54
**Priority:** Growth

---

## Epic 1: Installable CLI with Daemon Lifecycle

User can install palingenesis, start/stop the daemon, check status, and view logs. This epic establishes the foundation: project structure, CLI framework, daemon process management, and IPC communication.

### Story 1.1: Project Initialization from Starter Template

As a developer,
I want to initialize the palingenesis project from a proven Rust CLI template,
So that I have a solid foundation with CI/CD, proper structure, and best practices.

**Acceptance Criteria:**

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

**Technical Notes:**
- Implements: ARCH1, ARCH2
- Must customize Cargo.toml with dependencies from Architecture doc (tokio, clap, serde, etc.)
- Create module directories: `cli/`, `daemon/`, `monitor/`, `resume/`, `http/`, `ipc/`, `notify/`, `config/`, `state/`, `telemetry/`

---

### Story 1.2: CLI Framework with Clap Subcommands

As a user,
I want a well-structured CLI with subcommands and help text,
So that I can easily discover and use all palingenesis features.

**Acceptance Criteria:**

**Given** the palingenesis binary
**When** I run `palingenesis --help`
**Then** I see the main help text with available subcommands
**And** the output includes: daemon, status, logs, config, pause, resume, new-session

**Given** the palingenesis binary
**When** I run `palingenesis --version`
**Then** I see the version number in semver format

**Given** the palingenesis binary
**When** I run `palingenesis daemon --help`
**Then** I see help for daemon subcommands: start, stop, restart, reload, status

**Given** the palingenesis binary
**When** I run `palingenesis logs --help`
**Then** I see options: --follow, --tail N, --since TIME

**Technical Notes:**
- Implements: ARCH7 (clap 4.5.50)
- Use clap derive macros for clean code
- Structure: `src/cli/app.rs` for Clap App, `src/cli/commands/` for handlers

---

### Story 1.3: Platform-Specific Path Resolution

As a user,
I want palingenesis to use platform-appropriate paths for config and state,
So that it integrates cleanly with my system conventions.

**Acceptance Criteria:**

**Given** a Linux system
**When** palingenesis resolves config path
**Then** it uses `~/.config/palingenesis/config.toml`

**Given** a macOS system
**When** palingenesis resolves config path
**Then** it uses `~/Library/Application Support/palingenesis/config.toml`

**Given** a Linux system
**When** palingenesis resolves state path
**Then** it uses `~/.local/state/palingenesis/`

**Given** a macOS system
**When** palingenesis resolves state path
**Then** it uses `~/Library/Application Support/palingenesis/`

**Given** the environment variable `PALINGENESIS_CONFIG` is set
**When** palingenesis resolves config path
**Then** it uses the path from the environment variable

**Technical Notes:**
- Implements: ARCH16, ARCH17
- Create `src/config/paths.rs` for platform-specific logic
- Use `dirs` crate or manual detection via `cfg(target_os)`

---

### Story 1.4: State Persistence Layer

As a daemon,
I want to persist my state to disk,
So that I can survive restarts and maintain session context.

**Acceptance Criteria:**

**Given** the daemon starts
**When** it initializes state
**Then** it reads from `{state_dir}/state.json` if exists
**And** creates the directory and file if not exists

**Given** the daemon has state to persist
**When** it writes state
**Then** the JSON file contains: version, daemon_state, current_session, stats
**And** file permissions are set to 600

**Given** a corrupted state file
**When** the daemon attempts to read it
**Then** it logs a warning and starts with default state
**And** backs up the corrupted file to `state.json.bak`

**Given** concurrent access attempts
**When** multiple processes try to write state
**Then** file locking prevents corruption

**Technical Notes:**
- Implements: ARCH10
- Schema from Architecture doc
- Create `src/state/store.rs` and `src/state/schema.rs`
- Use `fs2` crate for file locking

---

### Story 1.5: PID File Management

As a user,
I want palingenesis to track the daemon process via PID file,
So that I can ensure only one daemon runs and CLI commands can find it.

**Acceptance Criteria:**

**Given** no daemon is running
**When** I start the daemon
**Then** a PID file is created at `/run/user/{uid}/palingenesis.pid`
**And** the file contains the daemon's process ID

**Given** a daemon is already running (PID file exists with valid process)
**When** I try to start another daemon
**Then** it fails with error "Daemon already running (PID: N)"

**Given** a stale PID file exists (process not running)
**When** I start the daemon
**Then** it removes the stale PID file
**And** creates a new PID file with the new process ID

**Given** a running daemon
**When** it shuts down gracefully
**Then** the PID file is removed

**Technical Notes:**
- Implements: ARCH18
- Create `src/daemon/pid.rs`
- Check process existence via `/proc/{pid}` on Linux or `kill(pid, 0)` on Unix

---

### Story 1.6: Unix Socket IPC Server

As a daemon,
I want to listen on a Unix socket for CLI commands,
So that CLI tools can communicate with me efficiently.

**Acceptance Criteria:**

**Given** the daemon starts
**When** it initializes IPC
**Then** it creates a Unix socket at `/run/user/{uid}/palingenesis.sock`
**And** the socket accepts connections

**Given** a CLI client connects to the socket
**When** it sends `STATUS\n`
**Then** the daemon responds with JSON status
**And** the connection closes cleanly

**Given** a CLI client sends `PAUSE\n`
**When** the daemon receives the command
**Then** it transitions to paused state
**And** responds with `OK\n`

**Given** the socket path already exists
**When** the daemon starts
**Then** it removes the stale socket file
**And** creates a new socket

**Given** the daemon shuts down
**When** cleanup runs
**Then** the socket file is removed

**Technical Notes:**
- Implements: ARCH13, ARCH15
- Create `src/ipc/socket.rs`, `src/ipc/protocol.rs`
- Use tokio's UnixListener
- Protocol: Simple text commands with JSON responses

---

### Story 1.7: Unix Socket IPC Client

As a CLI command,
I want to communicate with the running daemon via Unix socket,
So that I can send commands and receive responses.

**Acceptance Criteria:**

**Given** a daemon is running
**When** CLI runs `palingenesis status`
**Then** it connects to the Unix socket
**And** sends `STATUS\n`
**And** receives JSON status
**And** displays formatted output

**Given** no daemon is running (socket doesn't exist)
**When** CLI runs `palingenesis status`
**Then** it displays "Daemon not running"
**And** exits with code 1

**Given** the daemon is unresponsive (timeout)
**When** CLI waits for response
**Then** it times out after 5 seconds
**And** displays "Daemon unresponsive"

**Technical Notes:**
- Implements: ARCH13
- Create `src/ipc/client.rs`
- NFR3: CLI response <500ms

---

### Story 1.8: Daemon Start Command

As a user,
I want to start the palingenesis daemon,
So that it begins monitoring my AI assistant sessions.

**Acceptance Criteria:**

**Given** no daemon is running
**When** I run `palingenesis daemon start`
**Then** the daemon starts in background (daemonizes)
**And** PID file is created
**And** Unix socket is created
**And** CLI displays "Daemon started (PID: N)"

**Given** no daemon is running
**When** I run `palingenesis daemon start --foreground`
**Then** the daemon runs in foreground (no daemonize)
**And** logs are printed to stderr
**And** Ctrl+C triggers graceful shutdown

**Given** a daemon is already running
**When** I run `palingenesis daemon start`
**Then** CLI displays "Daemon already running (PID: N)"
**And** exits with code 1

**Given** the daemon starts
**When** initialization completes
**Then** it logs "palingenesis daemon started" at INFO level
**And** enters monitoring state

**Technical Notes:**
- Implements: FR14, ARCH19
- Create `src/cli/commands/daemon.rs`
- Use `daemonize` crate or fork pattern for background mode
- `--foreground` is useful for systemd/launchd

---

### Story 1.9: Daemon Stop Command

As a user,
I want to stop the palingenesis daemon gracefully,
So that it saves state and cleans up resources.

**Acceptance Criteria:**

**Given** a daemon is running
**When** I run `palingenesis daemon stop`
**Then** it sends SIGTERM to the daemon process
**And** waits for graceful shutdown (up to 10s)
**And** displays "Daemon stopped"

**Given** a daemon is running but unresponsive
**When** graceful shutdown times out
**Then** it sends SIGKILL
**And** displays "Daemon killed (did not respond to SIGTERM)"

**Given** no daemon is running
**When** I run `palingenesis daemon stop`
**Then** it displays "Daemon not running"
**And** exits with code 1

**Given** the daemon receives SIGTERM
**When** it begins shutdown
**Then** it uses CancellationToken to stop all tasks
**And** persists current state to disk
**And** removes PID file and socket
**And** exits with code 0

**Technical Notes:**
- Implements: FR15, ARCH21
- Use nix crate for signal handling
- Graceful shutdown pattern with CancellationToken

---

### Story 1.10: Daemon Status Command

As a user,
I want to check the daemon status,
So that I can see if it's running and what it's monitoring.

**Acceptance Criteria:**

**Given** a daemon is running
**When** I run `palingenesis status`
**Then** I see formatted output:
```
palingenesis daemon: running (PID: 12345)
State: monitoring
Uptime: 2h 30m
Current session: /path/to/session.md
Steps completed: 5/12
Saves: 42
```

**Given** a daemon is running
**When** I run `palingenesis status --json`
**Then** I see JSON output with all status fields

**Given** no daemon is running
**When** I run `palingenesis status`
**Then** I see "Daemon not running"
**And** exit code is 1

**Technical Notes:**
- Implements: FR16
- Human-readable output by default, JSON with --json flag

---

### Story 1.11: Daemon Logs Command

As a user,
I want to view daemon logs,
So that I can troubleshoot issues and monitor activity.

**Acceptance Criteria:**

**Given** a daemon is running with log file
**When** I run `palingenesis logs`
**Then** I see the last 20 lines of logs

**Given** a daemon is running
**When** I run `palingenesis logs --tail 50`
**Then** I see the last 50 lines of logs

**Given** a daemon is running
**When** I run `palingenesis logs --follow`
**Then** logs stream in real-time
**And** Ctrl+C stops the stream

**Given** a daemon is running
**When** I run `palingenesis logs --since "1h"`
**Then** I see logs from the last hour only

**Given** no log file exists
**When** I run `palingenesis logs`
**Then** I see "No logs available"

**Technical Notes:**
- Implements: FR17
- Log file at `{state_dir}/daemon.log` when file logging enabled
- For --follow, use file watching or request log stream from daemon

---

### Story 1.12: Tracing and Structured Logging Setup

As a developer,
I want structured logging throughout the daemon,
So that I can debug issues and integrate with observability tools.

**Acceptance Criteria:**

**Given** the daemon starts
**When** logging is initialized
**Then** tracing subscriber is configured
**And** log level is set from config (default: info)

**Given** the daemon runs with `--debug` flag
**When** logging is initialized
**Then** log level is set to debug

**Given** any log statement in the code
**When** it executes
**Then** output includes: timestamp, level, target, message, structured fields

**Given** config specifies file logging
**When** the daemon runs
**Then** logs are written to `{state_dir}/daemon.log`
**And** logs are also written to stderr (unless daemonized)

**Technical Notes:**
- Implements: ARCH9, ARCH22
- Create `src/telemetry/tracing.rs`
- Use tracing-subscriber with json and env-filter features

---

### Story 1.13: Graceful Shutdown Coordination

As a daemon,
I want coordinated shutdown of all components,
So that no data is lost and resources are cleaned up properly.

**Acceptance Criteria:**

**Given** multiple async tasks are running (IPC, file watcher, etc.)
**When** shutdown is triggered (SIGTERM or SIGINT)
**Then** CancellationToken is triggered
**And** all tasks observe cancellation and stop gracefully

**Given** shutdown is in progress
**When** a task has in-progress work
**Then** it completes the current unit of work
**And** then exits cleanly

**Given** shutdown is triggered
**When** all tasks have stopped
**Then** state is persisted to disk
**And** PID file is removed
**And** Unix socket is removed
**And** process exits with code 0

**Given** a task hangs during shutdown
**When** shutdown timeout (10s) is reached
**Then** remaining tasks are forcefully cancelled
**And** a warning is logged

**Technical Notes:**
- Implements: ARCH21
- Create `src/daemon/shutdown.rs`
- Use tokio_util::sync::CancellationToken
- Pattern: broadcast shutdown signal to all spawned tasks

---

## Epic 2: Session Detection & Classification

Daemon monitors opencode sessions and correctly identifies when/why they stop (rate limit vs context exhaustion vs user exit).

### Story 2.1: File System Watcher Setup

As a daemon,
I want to watch the opencode session directory for changes,
So that I can detect session file modifications in real-time.

**Acceptance Criteria:**

**Given** the daemon starts monitoring
**When** it initializes the file watcher
**Then** it watches `~/.opencode/` directory recursively
**And** the watcher uses inotify on Linux, FSEvents on macOS

**Given** the session directory doesn't exist
**When** the daemon starts
**Then** it logs a warning and waits for the directory to appear
**And** starts watching once the directory is created

**Given** a file is modified in the session directory
**When** the watcher detects the change
**Then** it emits a `FileChanged` event with the file path
**And** event is sent via channel to the monitor

**Given** high-frequency file changes occur
**When** the watcher receives events
**Then** it debounces events (100ms window)
**And** processes only the latest state

**Technical Notes:**
- Implements: NFR1 (<5s detection)
- Use notify 8.2.0 crate
- Create `src/monitor/watcher.rs`
- Event-driven, not polling (NFR5: <1% CPU idle)

---

### Story 2.2: Session File Parser (Frontmatter Extraction)

As a monitor,
I want to parse session file frontmatter,
So that I can extract workflow state and metadata.

**Acceptance Criteria:**

**Given** a session file with YAML frontmatter
**When** the parser reads the file
**Then** it extracts frontmatter between `---` delimiters
**And** parses it as YAML
**And** returns a Session struct

**Given** a session file without frontmatter
**When** the parser reads the file
**Then** it returns an error `NoFrontmatter`

**Given** a session file with invalid YAML frontmatter
**When** the parser reads the file
**Then** it returns an error `InvalidFrontmatter` with details

**Given** frontmatter contains `stepsCompleted` array
**When** parsed successfully
**Then** Session struct contains the array values

**Given** frontmatter contains `lastStep` field
**When** parsed successfully
**Then** Session struct contains the step number

**Technical Notes:**
- Implements: FR6, FR7, ARCH12
- Create `src/monitor/frontmatter.rs`, `src/monitor/session.rs`
- Only parse frontmatter, ignore body content (efficiency)

---

### Story 2.3: Process Detection (opencode Start/Stop)

As a monitor,
I want to detect when opencode processes start and stop,
So that I can track active sessions.

**Acceptance Criteria:**

**Given** the daemon is monitoring
**When** an opencode process starts
**Then** a `ProcessStarted` event is emitted
**And** the event includes PID and command line

**Given** an opencode process is running
**When** the process terminates
**Then** a `ProcessStopped` event is emitted
**And** the event includes exit code if available

**Given** multiple opencode processes are running
**When** any one terminates
**Then** only that specific process stop is detected
**And** other processes continue to be monitored

**Given** the daemon starts
**When** opencode is already running
**Then** it detects the existing process
**And** begins monitoring it

**Technical Notes:**
- Implements: FR1, FR2
- Use `/proc` scanning on Linux or `sysctl`/`ps` on macOS
- Alternatively, watch for specific files opencode creates
- Consider: file-based detection may be simpler than process detection

---

### Story 2.4: Stop Reason Classification - Rate Limit

As a classifier,
I want to identify when a session stopped due to rate limiting,
So that the daemon knows to wait and resume the same session.

**Acceptance Criteria:**

**Given** a session file or log contains "rate_limit_error"
**When** the classifier analyzes the stop
**Then** it returns `StopReason::RateLimit`
**And** extracts `retry_after` duration if present

**Given** HTTP 429 status code in session output
**When** the classifier analyzes the stop
**Then** it returns `StopReason::RateLimit`

**Given** a `Retry-After` header value is present
**When** the classifier extracts it
**Then** the duration is included in the classification result

**Given** no retry information is available
**When** the classifier returns RateLimit
**Then** it uses a default wait time from config

**Technical Notes:**
- Implements: FR3, FR11
- Create `src/monitor/classifier.rs`
- Pattern matching on error strings and HTTP codes

---

### Story 2.5: Stop Reason Classification - Context Exhaustion

As a classifier,
I want to identify when a session stopped due to context window exhaustion,
So that the daemon knows to start a new session.

**Acceptance Criteria:**

**Given** a session file or log contains "context_length_exceeded"
**When** the classifier analyzes the stop
**Then** it returns `StopReason::ContextExhausted`

**Given** token count exceeds threshold (>80% of context window)
**When** the classifier analyzes the session
**Then** it considers this a context exhaustion risk

**Given** session frontmatter shows `stepsCompleted` at final step
**When** the classifier analyzes the stop
**Then** it returns `StopReason::Completed` (not context exhaustion)

**Technical Notes:**
- Implements: FR4
- Distinguish from rate limit (different resume strategy)
- May need to read recent log output for classification

---

### Story 2.6: Stop Reason Classification - User Exit

As a classifier,
I want to identify when a user explicitly exited a session,
So that the daemon respects their intent and doesn't auto-resume.

**Acceptance Criteria:**

**Given** the user pressed Ctrl+C in the opencode session
**When** the classifier analyzes the stop
**Then** it returns `StopReason::UserExit`

**Given** the session exited with a clean exit code (0) without errors
**When** the classifier analyzes the stop
**Then** it returns `StopReason::UserExit` or `StopReason::Completed`

**Given** a user exit is detected
**When** the daemon considers resumption
**Then** it does NOT auto-resume (respects user intent)
**And** logs "Session ended by user, not auto-resuming"

**Technical Notes:**
- Implements: FR5
- Key: don't auto-resume when user intentionally stopped
- May use exit code or signal detection

---

### Story 2.7: Monitor Event Channel

As a monitor,
I want to emit events via a channel to the daemon core,
So that monitoring is decoupled from resume logic.

**Acceptance Criteria:**

**Given** the monitor detects a session file change
**When** it processes the change
**Then** it sends a `MonitorEvent::SessionChanged` to the channel

**Given** the monitor classifies a stop reason
**When** classification completes
**Then** it sends a `MonitorEvent::SessionStopped { reason, session }` to the channel

**Given** the daemon core receives a `SessionStopped` event
**When** it processes the event
**Then** it routes to the appropriate resume strategy

**Given** the monitor encounters an error
**When** the error is transient
**Then** it logs the error and continues monitoring
**And** does not crash the daemon

**Technical Notes:**
- Use `tokio::sync::mpsc` channel
- MonitorEvent enum: SessionChanged, SessionStopped, Error
- Decoupled architecture for testability

---

## Epic 3: Automatic Session Resumption

Daemon automatically resumes work after rate limits or starts new session after context exhaustion - the "first save" moment.

### Story 3.1: Resume Strategy Trait

As a developer,
I want a common trait for resume strategies,
So that different resume approaches are interchangeable.

**Acceptance Criteria:**

**Given** the ResumeStrategy trait definition
**When** implemented
**Then** it has an async `execute` method
**And** the method takes session context and returns Result

**Given** a rate limit stop
**When** the strategy selector chooses
**Then** it selects `SameSessionStrategy`

**Given** a context exhaustion stop
**When** the strategy selector chooses
**Then** it selects `NewSessionStrategy`

**Technical Notes:**
- Create `src/resume/strategy.rs`
- Trait: `async fn execute(&self, ctx: &ResumeContext) -> Result<ResumeOutcome>`
- Strategy pattern for extensibility

---

### Story 3.2: Same-Session Resume Strategy

As a daemon,
I want to resume the same session after rate limit clears,
So that I preserve context and continue where I left off.

**Acceptance Criteria:**

**Given** a rate limit stop with Retry-After: 60s
**When** the same-session strategy executes
**Then** it waits for 60 seconds
**And** then triggers opencode to continue

**Given** no Retry-After header
**When** the same-session strategy executes
**Then** it uses exponential backoff starting at configured base (default 30s)

**Given** the wait period completes
**When** resume is triggered
**Then** it sends the appropriate signal/command to opencode
**And** logs "Resuming session after rate limit"

**Given** resume succeeds
**When** the session continues
**Then** stats.total_resumes is incremented
**And** state is persisted

**Technical Notes:**
- Implements: FR8
- Create `src/resume/same_session.rs`
- Integration point: how to signal opencode to continue

---

### Story 3.3: New-Session Resume Strategy

As a daemon,
I want to start a new session from Next-step.md after context exhaustion,
So that I can continue the workflow with fresh context.

**Acceptance Criteria:**

**Given** a context exhaustion stop
**When** the new-session strategy executes
**Then** it reads the `Next-step.md` file to determine continuation point

**Given** the new session starts
**When** it begins execution
**Then** it starts from the correct step (from Next-step.md or stepsCompleted)
**And** logs "Starting new session from step N"

**Given** no Next-step.md exists
**When** the strategy looks for continuation point
**Then** it uses stepsCompleted from frontmatter to determine next step
**And** creates appropriate prompt to continue

**Given** new session creation succeeds
**When** the session starts
**Then** stats.total_resumes is incremented
**And** audit trail records the transition

**Technical Notes:**
- Implements: FR9
- Create `src/resume/new_session.rs`
- Must integrate with opencode session creation API/mechanism

---

### Story 3.4: Session Backup Before New Session

As a daemon,
I want to backup the session file before starting a new session,
So that I can recover if something goes wrong.

**Acceptance Criteria:**

**Given** a context exhaustion triggers new session
**When** backup runs before new session creation
**Then** the session file is copied to `session-backup-{timestamp}.md`
**And** the backup is in the same directory as the original

**Given** backup succeeds
**When** the new session starts
**Then** the original session file may be modified
**And** the backup remains unchanged

**Given** backup fails (disk full, permissions)
**When** the error is caught
**Then** it logs error "Failed to backup session: {reason}"
**And** proceeds with new session anyway (warn, don't block)

**Given** backups accumulate
**When** more than N backups exist (configurable, default 10)
**Then** oldest backups are pruned

**Technical Notes:**
- Implements: FR10
- Create backup logic in `src/resume/new_session.rs`
- Timestamp format: `YYYYMMDD-HHMMSS`

---

### Story 3.5: Exponential Backoff Implementation

As a daemon,
I want exponential backoff for retry attempts,
So that I don't overwhelm services and respect rate limits.

**Acceptance Criteria:**

**Given** an initial retry
**When** backoff calculates delay
**Then** delay = base_delay (default 30s)

**Given** a second retry
**When** backoff calculates delay
**Then** delay = base_delay * 2 = 60s

**Given** subsequent retries
**When** backoff calculates delay
**Then** delay = min(base_delay * 2^(attempt-1), max_delay)
**And** max_delay is configurable (default 5 minutes)

**Given** jitter is enabled (default)
**When** backoff calculates delay
**Then** delay is randomized +/- 10%

**Given** max_retries is reached
**When** another retry would be attempted
**Then** it gives up and logs error
**And** sends notification if configured

**Technical Notes:**
- Implements: FR12
- Create `src/resume/backoff.rs`
- Configurable: base_delay, max_delay, max_retries, jitter

---

### Story 3.6: Audit Trail Logging

As a daemon,
I want to log all resume events to an audit trail,
So that I have a history of actions for debugging and metrics.

**Acceptance Criteria:**

**Given** any resume action occurs
**When** audit logging runs
**Then** a JSON line is appended to `{state_dir}/audit.jsonl`

**Given** an audit entry
**When** it is written
**Then** it includes: timestamp, event_type, session_path, stop_reason, action_taken, outcome

**Given** the audit file doesn't exist
**When** the first audit entry is written
**Then** the file is created with mode 600

**Given** the audit file grows large
**When** rotation is triggered (configurable size, default 10MB)
**Then** the file is rotated to `audit.jsonl.1`
**And** a new `audit.jsonl` is created

**Technical Notes:**
- Implements: FR13, ARCH11
- Create `src/state/audit.rs`
- Append-only, one JSON object per line

---

### Story 3.7: Pause Command Implementation

As a user,
I want to pause daemon monitoring,
So that I can work without auto-resume interference.

**Acceptance Criteria:**

**Given** the daemon is in monitoring state
**When** I run `palingenesis pause`
**Then** the daemon transitions to paused state
**And** CLI displays "Monitoring paused"

**Given** the daemon is paused
**When** a session stops
**Then** the daemon does NOT auto-resume
**And** logs "Session stopped but monitoring is paused"

**Given** the daemon is already paused
**When** I run `palingenesis pause`
**Then** CLI displays "Already paused"

**Given** the daemon is paused
**When** I check status
**Then** status shows "State: paused"

**Technical Notes:**
- Implements: FR18
- IPC command: PAUSE
- State machine transition: Monitoring -> Paused

---

### Story 3.8: Resume Command Implementation

As a user,
I want to resume daemon monitoring after pausing,
So that auto-resume functionality is restored.

**Acceptance Criteria:**

**Given** the daemon is in paused state
**When** I run `palingenesis resume`
**Then** the daemon transitions to monitoring state
**And** CLI displays "Monitoring resumed"

**Given** the daemon is already monitoring
**When** I run `palingenesis resume`
**Then** CLI displays "Already monitoring"

**Given** monitoring resumes
**When** a session stops (rate limit)
**Then** normal auto-resume behavior occurs

**Technical Notes:**
- Implements: FR19
- IPC command: RESUME
- State machine transition: Paused -> Monitoring

---

### Story 3.9: Force New Session Command

As a user,
I want to force a new session manually,
So that I can recover from stuck states or start fresh.

**Acceptance Criteria:**

**Given** a session is active or stopped
**When** I run `palingenesis new-session`
**Then** the daemon backs up the current session
**And** starts a new session from Next-step.md
**And** CLI displays "New session started"

**Given** no session exists
**When** I run `palingenesis new-session`
**Then** CLI displays "No active session to replace"
**And** exits with code 1

**Given** backup fails during new-session
**When** the error occurs
**Then** it warns but proceeds anyway

**Technical Notes:**
- Implements: FR20
- IPC command: NEW-SESSION
- Forces the new-session strategy regardless of stop reason

---

## Epic 4: Configuration Management

User can customize daemon behavior via TOML config file, validate config, and daemon reloads dynamically without restart.

### Story 4.1: Config Schema Definition

As a developer,
I want a well-defined config schema,
So that configuration is type-safe and documented.

**Acceptance Criteria:**

**Given** the config schema
**When** deserialized from TOML
**Then** it maps to Rust structs with serde

**Given** the config schema
**When** documented
**Then** each field has a comment explaining its purpose

**Given** a config file
**When** it contains all sections
**Then** sections include: daemon, monitoring, resume, notifications, otel

**Given** the default config
**When** generated
**Then** all fields have sensible defaults
**And** it is valid and usable immediately

**Technical Notes:**
- Implements: ARCH configuration schema
- Create `src/config/schema.rs`
- Use serde with default values

---

### Story 4.2: Config Init Command

As a user,
I want to initialize a config file with defaults,
So that I have a starting point for customization.

**Acceptance Criteria:**

**Given** no config file exists
**When** I run `palingenesis config init`
**Then** a default config file is created at the platform-specific path
**And** CLI displays "Config created at {path}"

**Given** a config file already exists
**When** I run `palingenesis config init`
**Then** CLI asks for confirmation before overwriting
**And** respects the user's choice

**Given** the default config is generated
**When** written to file
**Then** it includes comments documenting each option
**And** file permissions are set to 600

**Given** I run `palingenesis config init --force`
**When** a config file exists
**Then** it overwrites without asking

**Technical Notes:**
- Implements: FR21
- Generate commented TOML for user-friendliness

---

### Story 4.3: Config Show Command

As a user,
I want to view the current configuration,
So that I can verify settings without opening the file.

**Acceptance Criteria:**

**Given** a config file exists
**When** I run `palingenesis config show`
**Then** the current config is displayed in TOML format

**Given** no config file exists
**When** I run `palingenesis config show`
**Then** the default config is displayed
**And** CLI notes "Using default configuration"

**Given** I run `palingenesis config show --json`
**When** config is displayed
**Then** output is JSON format instead of TOML

**Technical Notes:**
- Implements: part of FR23
- Read and display, optionally merge with defaults

---

### Story 4.4: Config Validate Command

As a user,
I want to validate my config file,
So that I catch errors before starting the daemon.

**Acceptance Criteria:**

**Given** a valid config file
**When** I run `palingenesis config validate`
**Then** CLI displays "Configuration valid"
**And** exits with code 0

**Given** a config file with syntax errors
**When** I run `palingenesis config validate`
**Then** CLI displays the parse error with line number
**And** exits with code 1

**Given** a config file with invalid values
**When** I run `palingenesis config validate`
**Then** CLI displays which value is invalid and why
**And** exits with code 1

**Given** no config file exists
**When** I run `palingenesis config validate`
**Then** CLI displays "No config file found, will use defaults"
**And** exits with code 0

**Technical Notes:**
- Implements: FR22
- Validate syntax (TOML parsing) and semantics (value ranges, paths exist)

---

### Story 4.5: Config Edit Command

As a user,
I want to open my config in my preferred editor,
So that I can make changes easily.

**Acceptance Criteria:**

**Given** a config file exists
**When** I run `palingenesis config edit`
**Then** the file opens in $EDITOR (or vi/nano fallback)

**Given** $EDITOR is not set
**When** I run `palingenesis config edit`
**Then** it tries `vi`, then `nano`, then fails with helpful message

**Given** no config file exists
**When** I run `palingenesis config edit`
**Then** it creates the default config first
**And** then opens it in the editor

**Given** I edit and save the config
**When** the editor closes
**Then** validation runs automatically
**And** displays result

**Technical Notes:**
- Implements: FR23
- Standard pattern: spawn $EDITOR, wait for exit

---

### Story 4.6: Hot Reload via SIGHUP

As a user,
I want the daemon to reload config without restarting,
So that I can update settings without interrupting monitoring.

**Acceptance Criteria:**

**Given** the daemon is running
**When** I run `palingenesis daemon reload`
**Then** SIGHUP is sent to the daemon process

**Given** the daemon receives SIGHUP
**When** it handles the signal
**Then** it re-reads the config file
**And** applies new settings
**And** logs "Configuration reloaded"

**Given** the new config is invalid
**When** reload is attempted
**Then** the daemon logs error "Invalid config, keeping current"
**And** continues with the old config

**Given** certain settings change (e.g., check_interval)
**When** config is reloaded
**Then** the new value takes effect immediately

**Technical Notes:**
- Implements: FR24
- Use nix crate for SIGHUP handling
- Some settings may require restart (document which)

---

### Story 4.7: Auto-Detect AI Assistants

As a user,
I want palingenesis to auto-detect running AI assistants,
So that I don't need to configure them manually.

**Acceptance Criteria:**

**Given** config has empty `monitoring.assistants` list
**When** the daemon starts
**Then** it auto-detects supported assistants

**Given** opencode is running
**When** auto-detection runs
**Then** it finds opencode and adds it to monitored list

**Given** auto-detection finds assistants
**When** logging the result
**Then** it logs "Auto-detected assistants: [list]"

**Given** explicit assistants are configured
**When** auto-detection is skipped
**Then** only configured assistants are monitored

**Technical Notes:**
- Implements: FR25
- Detection methods: process list, known directories, file patterns
- Initially support: opencode (claude-code)

---

## Epic 5: Event Notifications

User receives push notifications on their preferred channel (webhook, Discord, Slack, ntfy.sh) when events occur.

### Story 5.1: Notification Dispatcher

As a daemon,
I want a central notification dispatcher,
So that events are routed to all configured channels.

**Acceptance Criteria:**

**Given** a notification-worthy event occurs
**When** dispatcher receives it
**Then** it routes to all enabled notification channels

**Given** multiple channels are configured
**When** notification is sent
**Then** all channels receive the notification in parallel

**Given** one channel fails
**When** sending notification
**Then** other channels still receive their notifications
**And** the failure is logged

**Given** no notification channels are configured
**When** an event occurs
**Then** no notifications are sent (silent operation)

**Technical Notes:**
- Implements: FR30
- Create `src/notify/dispatcher.rs`
- Channel trait for polymorphism

---

### Story 5.2: Webhook Notifications

As a user,
I want to receive webhook notifications,
So that I can integrate with any system that accepts HTTP POST.

**Acceptance Criteria:**

**Given** webhook_url is configured
**When** a notification event occurs
**Then** an HTTP POST is sent to the URL
**And** body contains JSON with event details

**Given** the webhook returns 2xx
**When** response is received
**Then** notification is considered successful

**Given** the webhook returns error or times out
**When** retry is enabled
**Then** it retries up to 3 times with backoff

**Given** all retries fail
**When** giving up
**Then** it logs error and continues

**Technical Notes:**
- Implements: FR26
- Create `src/notify/webhook.rs`
- Use reqwest for HTTP client

---

### Story 5.3: ntfy.sh Integration

As a user,
I want to receive ntfy.sh push notifications,
So that I get alerts on my phone without complex setup.

**Acceptance Criteria:**

**Given** ntfy_topic is configured
**When** a notification event occurs
**Then** POST is sent to `https://ntfy.sh/{topic}`

**Given** ntfy notification
**When** sent successfully
**Then** my phone receives the push notification

**Given** custom ntfy server is configured
**When** notification is sent
**Then** it uses the custom server URL instead of ntfy.sh

**Given** notification has priority
**When** sent to ntfy
**Then** priority header is included

**Technical Notes:**
- Implements: FR29
- Create `src/notify/ntfy.rs`
- ntfy.sh is simple HTTP POST with headers

---

### Story 5.4: Discord Webhook Integration

As a user,
I want to receive Discord notifications,
So that I see alerts in my Discord server.

**Acceptance Criteria:**

**Given** discord_webhook is configured
**When** a notification event occurs
**Then** POST is sent to Discord webhook URL
**And** message is formatted as Discord embed

**Given** Discord notification
**When** sent successfully
**Then** message appears in the configured channel

**Given** notification includes event type
**When** formatted for Discord
**Then** embed color reflects severity (green=info, yellow=warn, red=error)

**Technical Notes:**
- Implements: FR27
- Create `src/notify/discord.rs`
- Discord webhook format: `{ "embeds": [...] }`

---

### Story 5.5: Slack Webhook Integration

As a user,
I want to receive Slack notifications,
So that I see alerts in my Slack workspace.

**Acceptance Criteria:**

**Given** slack_webhook is configured
**When** a notification event occurs
**Then** POST is sent to Slack webhook URL
**And** message is formatted as Slack blocks

**Given** Slack notification
**When** sent successfully
**Then** message appears in the configured channel

**Given** notification includes event details
**When** formatted for Slack
**Then** it uses structured blocks with fields

**Technical Notes:**
- Implements: FR28
- Create `src/notify/slack.rs`
- Slack webhook format: `{ "blocks": [...] }` or simple `{ "text": "..." }`

---

### Story 5.6: Notification Events Definition

As a developer,
I want clearly defined notification events,
So that users know what notifications they'll receive.

**Acceptance Criteria:**

**Given** the notification event types
**When** documented
**Then** events include: SessionStopped, ResumeAttempted, ResumeSucceeded, ResumeFailed, DaemonStarted, DaemonStopped

**Given** each event type
**When** notification is created
**Then** it includes: timestamp, event_type, session_path, details

**Given** user configures notification filters
**When** an event occurs
**Then** only events matching filter are sent

**Technical Notes:**
- Create `src/notify/events.rs`
- NotificationEvent enum with to_json() for serialization

---

## Epic 6: Remote Control & External API

User can monitor and control daemon remotely via Discord/Slack commands or HTTP API.

### Story 6.1: HTTP API Server Setup

As a daemon,
I want to run an HTTP API server,
So that external tools can monitor and control me.

**Acceptance Criteria:**

**Given** the daemon starts with HTTP API enabled
**When** initialization completes
**Then** axum server listens on `127.0.0.1:7654`

**Given** config specifies different port
**When** server starts
**Then** it uses the configured port

**Given** config specifies bind address `0.0.0.0`
**When** server starts
**Then** it binds to all interfaces (warns about security)

**Given** the server is running
**When** any request is received
**Then** it is logged with tracing middleware

**Technical Notes:**
- Implements: ARCH6, ARCH14
- Create `src/http/server.rs`
- Use tower-http for middleware

---

### Story 6.2: Health Endpoint

As an external tool,
I want a health check endpoint,
So that I can monitor if the daemon is healthy.

**Acceptance Criteria:**

**Given** the daemon is running
**When** GET /health is called
**Then** response is 200 with `{ "status": "ok", "uptime": "2h30m" }`

**Given** the daemon has issues
**When** GET /health is called
**Then** response includes degraded status and reason

**Given** load balancer or monitoring tool
**When** it polls /health
**Then** response time is <100ms

**Technical Notes:**
- Create `src/http/handlers/health.rs`
- Simple, fast endpoint for health checks

---

### Story 6.3: Status API Endpoint

As an external tool,
I want a status endpoint,
So that I can get detailed daemon state.

**Acceptance Criteria:**

**Given** the daemon is running
**When** GET /api/v1/status is called
**Then** response is 200 with full status JSON

**Given** status response
**When** parsed
**Then** it includes: state, current_session, stats, config_summary

**Technical Notes:**
- Implements: ARCH23 (response format)
- Create `src/http/handlers/status.rs`
- Returns same data as CLI status --json

---

### Story 6.4: Control API Endpoints

As an external tool,
I want control endpoints,
So that I can pause/resume/control the daemon remotely.

**Acceptance Criteria:**

**Given** the daemon is monitoring
**When** POST /api/v1/pause is called
**Then** daemon pauses and responds `{ "success": true }`

**Given** the daemon is paused
**When** POST /api/v1/resume is called
**Then** daemon resumes and responds `{ "success": true }`

**Given** POST /api/v1/new-session is called
**When** a session exists
**Then** new session is started and responds with session_id

**Given** any control endpoint
**When** action fails
**Then** response is 400/500 with `{ "success": false, "error": { "code": "...", "message": "..." } }`

**Technical Notes:**
- Implements: ARCH23
- Create `src/http/handlers/control.rs`
- Same functionality as CLI commands

---

### Story 6.5: Events SSE Stream

As an external tool,
I want to stream events in real-time,
So that I can react to daemon events without polling.

**Acceptance Criteria:**

**Given** a client connects to GET /api/v1/events
**When** connection is established
**Then** Server-Sent Events stream begins

**Given** a daemon event occurs
**When** the SSE stream is active
**Then** event is pushed to all connected clients

**Given** client disconnects
**When** cleanup runs
**Then** that client is removed from broadcast list

**Given** no events occur
**When** stream is idle
**Then** keep-alive is sent every 30s

**Technical Notes:**
- Create `src/http/handlers/events.rs`
- Use axum's SSE support
- Broadcast pattern for multiple clients

---

### Story 6.6: Discord/Slack Bot Commands

As a user,
I want to control palingenesis via Discord/Slack commands,
So that I can manage it from my phone without SSH.

**Acceptance Criteria:**

**Given** Discord bot is configured
**When** I type `/palin status` in Discord
**Then** bot responds with daemon status

**Given** Slack bot is configured
**When** I type `/palin pause` in Slack
**Then** bot pauses daemon and confirms

**Given** command `/palin logs --tail 5`
**When** executed via chat
**Then** last 5 log lines are returned

**Given** unauthorized user
**When** they try to use commands
**Then** command is rejected (if auth configured)

**Technical Notes:**
- Implements: FR31, FR32, FR33, FR34
- This requires webhook receiver or bot framework
- Consider: Slack app vs incoming webhook limitations

---

## Epic 7: Observability & Metrics

User can view metrics in Prometheus/Grafana, see traces in Jaeger, and track "time saved" and "saves count".

### Story 7.1: Prometheus Metrics Endpoint

As an operator,
I want a Prometheus metrics endpoint,
So that I can scrape metrics into my monitoring stack.

**Acceptance Criteria:**

**Given** metrics endpoint is enabled
**When** GET /api/v1/metrics is called
**Then** response is Prometheus text format

**Given** metrics are scraped
**When** parsed by Prometheus
**Then** all metrics have proper labels and types

**Given** metrics endpoint
**When** called frequently
**Then** response time is <50ms

**Technical Notes:**
- Implements: FR35
- Create `src/http/handlers/metrics.rs`
- Use prometheus crate or manual formatting

---

### Story 7.2: Core Metrics Implementation

As an operator,
I want core daemon metrics,
So that I can monitor daemon health and activity.

**Acceptance Criteria:**

**Given** the metrics system
**When** metrics are collected
**Then** counters include: resumes_total, failures_total, sessions_started_total

**Given** the metrics system
**When** metrics are collected
**Then** gauges include: daemon_state (1=monitoring, 2=paused, etc.), current_session_steps

**Given** the metrics system
**When** metrics are collected
**Then** histograms include: resume_duration_seconds, detection_latency_seconds

**Technical Notes:**
- Implements: FR35
- Create `src/telemetry/metrics.rs`
- Follow Prometheus naming conventions

---

### Story 7.3: Time Saved Metric

As a user,
I want to see how much time palingenesis has saved me,
So that I can quantify its value.

**Acceptance Criteria:**

**Given** a successful resume
**When** time saved is calculated
**Then** it estimates: time_waiting + time_to_manually_restart

**Given** configuration for time estimates
**When** metrics are calculated
**Then** manual_restart_time is configurable (default 5 minutes)

**Given** time saved metric
**When** queried
**Then** returns total hours saved since daemon started

**Given** the status command
**When** it displays stats
**Then** it includes "Time saved: 4.2 hours"

**Technical Notes:**
- Implements: FR39
- Estimation: each resume saves ~5 minutes of manual intervention
- Track cumulative in state file

---

### Story 7.4: Saves Count Metric

As a user,
I want to see how many times palingenesis has saved my work,
So that I can see its impact at a glance.

**Acceptance Criteria:**

**Given** a successful resume
**When** saves count is updated
**Then** stats.saves_count is incremented

**Given** the status command
**When** it displays stats
**Then** it includes "Saves: 42"

**Given** the metrics endpoint
**When** scraped
**Then** it includes `palingenesis_saves_total` counter

**Given** the weekly summary (if notifications enabled)
**When** sent
**Then** it includes saves count for the week

**Technical Notes:**
- Implements: FR40
- Simple counter in state file
- Persisted across restarts

---

### Story 7.5: OTEL Traces Export

As an operator,
I want OpenTelemetry traces,
So that I can see detailed request flows in Jaeger.

**Acceptance Criteria:**

**Given** OTEL is enabled in config
**When** the daemon performs actions
**Then** trace spans are created

**Given** a resume operation
**When** traced
**Then** span includes: stop_reason, wait_duration, outcome

**Given** OTLP endpoint is configured
**When** traces are exported
**Then** they are sent via gRPC or HTTP to the collector

**Given** OTEL is disabled (default)
**When** the daemon runs
**Then** no trace overhead occurs

**Technical Notes:**
- Implements: FR36
- Create `src/telemetry/otel.rs`
- Use opentelemetry crate with optional feature

---

### Story 7.6: OTEL Logs Export

As an operator,
I want structured logs exported via OTLP,
So that I have unified observability.

**Acceptance Criteria:**

**Given** OTEL logs are enabled
**When** the daemon logs
**Then** logs are also sent to OTLP collector

**Given** log export
**When** logs are sent
**Then** they include trace context if available

**Given** OTEL endpoint is unreachable
**When** export fails
**Then** logs are still written locally
**And** export errors don't crash daemon

**Technical Notes:**
- Implements: FR37
- Use tracing-opentelemetry for integration
- Logs, metrics, traces all to same collector

---

### Story 7.7: Grafana Dashboard Template

As an operator,
I want a pre-built Grafana dashboard,
So that I can visualize palingenesis metrics immediately.

**Acceptance Criteria:**

**Given** the dashboard JSON file
**When** imported into Grafana
**Then** it displays all palingenesis metrics

**Given** the dashboard
**When** viewed
**Then** panels include: saves over time, resume success rate, time saved, daemon state

**Given** the dashboard
**When** exported from repo
**Then** it works with Prometheus data source

**Technical Notes:**
- Implements: FR38
- Create `grafana/palingenesis-dashboard.json`
- Document import process in README

---

## Epic 8: MCP Server Interface

OpenCode AI Agent can control palingenesis daemon via MCP protocol, enabling AI-driven workflow orchestration.

### Story 8.1: MCP Server stdio Transport Setup

As a daemon,
I want to support MCP stdio transport,
So that OpenCode can communicate with me via the MCP protocol.

**Acceptance Criteria:**

**Given** the daemon is started with `palingenesis mcp serve`
**When** it initializes MCP mode
**Then** it reads JSON-RPC messages from stdin
**And** writes responses to stdout

**Given** the MCP server is running
**When** it receives a valid JSON-RPC request
**Then** it parses and processes the request
**And** returns a properly formatted JSON-RPC response

**Given** the MCP server receives malformed JSON
**When** parsing fails
**Then** it returns a JSON-RPC error response with code -32700 (Parse error)
**And** continues running (doesn't crash)

**Given** the MCP server is running
**When** stdin is closed (EOF)
**Then** it shuts down gracefully

**Technical Notes:**
- Implements: FR41
- Create `src/mcp/server.rs`
- Use `rmcp` crate or implement JSON-RPC 2.0 manually
- stdio transport: line-delimited JSON on stdin/stdout

---

### Story 8.2: JSON-RPC 2.0 Protocol Implementation

As an MCP server,
I want full JSON-RPC 2.0 compliance,
So that any MCP client can communicate reliably.

**Acceptance Criteria:**

**Given** a JSON-RPC request with `method` and `id`
**When** processed successfully
**Then** response includes matching `id` and `result`

**Given** a JSON-RPC request with invalid method
**When** processed
**Then** response includes error with code -32601 (Method not found)

**Given** a JSON-RPC notification (no `id`)
**When** processed
**Then** no response is sent

**Given** a batch request (array of requests)
**When** processed
**Then** batch response is returned with results in same order

**Given** JSON-RPC 2.0 spec
**When** any request is processed
**Then** response always includes `"jsonrpc": "2.0"`

**Technical Notes:**
- Implements: FR42
- Create `src/mcp/protocol.rs`
- Support: requests, notifications, batch requests
- Error codes per JSON-RPC 2.0 spec

---

### Story 8.3: MCP Tool Definitions

As an MCP server,
I want to expose palingenesis control functions as MCP tools,
So that AI agents can discover and invoke them.

**Acceptance Criteria:**

**Given** MCP client sends `tools/list` request
**When** server processes it
**Then** response includes tool definitions for: `status`, `pause`, `resume`, `new_session`, `logs`

**Given** each tool definition
**When** listed
**Then** it includes: name, description, inputSchema (JSON Schema)

**Given** MCP client invokes `tools/call` with tool `status`
**When** executed
**Then** response includes current daemon status as JSON

**Given** MCP client invokes `tools/call` with tool `pause`
**When** daemon is monitoring
**Then** daemon pauses and response confirms success

**Given** MCP client invokes `tools/call` with tool `resume`
**When** daemon is paused
**Then** daemon resumes and response confirms success

**Given** MCP client invokes `tools/call` with tool `new_session`
**When** a session exists
**Then** new session is started and response includes session info

**Given** MCP client invokes `tools/call` with tool `logs` and `lines: 10`
**When** executed
**Then** response includes last 10 log lines

**Technical Notes:**
- Implements: FR43
- Create `src/mcp/tools.rs` and `src/mcp/handlers.rs`
- Tool input schemas use JSON Schema format
- Tools delegate to same logic as CLI/HTTP commands

---

### Story 8.4: OpenCode Local MCP Configuration Support

As a user,
I want to configure palingenesis as a local MCP server in OpenCode,
So that OpenCode can control the daemon automatically.

**Acceptance Criteria:**

**Given** OpenCode's MCP config format
**When** user adds palingenesis
**Then** config looks like:
```json
{
  "mcpServers": {
    "palingenesis": {
      "type": "local",
      "command": "palingenesis",
      "args": ["mcp", "serve"]
    }
  }
}
```

**Given** the MCP server starts via OpenCode
**When** initialization completes
**Then** it sends proper MCP initialization response

**Given** OpenCode sends `initialize` request
**When** server responds
**Then** response includes server info and capabilities

**Given** OpenCode sends `initialized` notification
**When** server receives it
**Then** server is ready to accept tool calls

**Given** config documentation
**When** user reads it
**Then** they can set up palingenesis MCP in <2 minutes

**Technical Notes:**
- Implements: FR44
- Document in README: OpenCode MCP configuration
- Support MCP protocol initialization handshake
- Test with actual OpenCode integration

---

## Epic 9: OpenCode Process Management

Daemon monitors OpenCode process, automatically restarts it when crashed, and manages sessions via HTTP API for seamless recovery.

### Story 9.1: OpenCode Process Detection

As a daemon,
I want to detect when OpenCode process starts, stops, or crashes,
So that I can respond appropriately.

**Acceptance Criteria:**

**Given** the daemon is monitoring
**When** OpenCode process starts
**Then** event `OpenCodeStarted` is logged with PID

**Given** OpenCode is running
**When** the process exits normally (exit code 0)
**Then** event `OpenCodeStopped` is logged
**And** reason is classified as `NormalExit`

**Given** OpenCode is running
**When** the process crashes (non-zero exit)
**Then** event `OpenCodeCrashed` is logged with exit code
**And** reason is classified as `Crash`

**Given** OpenCode is running
**When** the process is killed (SIGKILL/SIGTERM)
**Then** event `OpenCodeKilled` is logged with signal

**Given** daemon starts
**When** OpenCode is already running
**Then** it detects the existing process and begins monitoring

**Technical Notes:**
- Implements: FR45
- Create `src/opencode/process.rs`
- Detection methods: PID file, process list scan, or socket check
- OpenCode may create a PID file or socket we can watch

---

### Story 9.2: Automatic OpenCode Restart

As a daemon,
I want to automatically restart OpenCode when it crashes,
So that monitoring can continue without manual intervention.

**Acceptance Criteria:**

**Given** OpenCode crashes
**When** auto_restart is enabled (default: true)
**Then** daemon waits `restart_delay_ms` (default: 1000ms)
**And** spawns `opencode serve` with configured options

**Given** OpenCode is restarted
**When** the process starts successfully
**Then** daemon logs "OpenCode restarted (PID: N)"
**And** begins monitoring the new process

**Given** OpenCode restart fails
**When** spawn returns error
**Then** daemon logs error and retries with backoff
**And** sends notification if configured

**Given** OpenCode crashes repeatedly (3+ times in 5 minutes)
**When** crash loop detected
**Then** daemon pauses auto-restart
**And** sends alert notification
**And** logs "Crash loop detected, pausing auto-restart"

**Given** auto_restart is disabled in config
**When** OpenCode crashes
**Then** daemon does NOT restart it
**And** logs "OpenCode stopped, auto-restart disabled"

**Technical Notes:**
- Implements: FR46
- Spawn command: `opencode serve --port {port} --hostname {hostname}`
- Crash loop detection prevents infinite restart loops
- Use `tokio::process::Command` for spawning

---

### Story 9.3: OpenCode HTTP API Client

As a daemon,
I want to communicate with OpenCode via its HTTP API,
So that I can manage sessions programmatically.

**Acceptance Criteria:**

**Given** OpenCode server is running
**When** daemon calls `GET /api/health`
**Then** it receives health status
**And** knows OpenCode is ready

**Given** daemon needs to resume a session
**When** it calls `POST /api/session/{id}/resume`
**Then** session resumes and response confirms

**Given** daemon needs to start new session
**When** it calls `POST /api/session` with prompt
**Then** new session starts and response includes session_id

**Given** daemon needs session status
**When** it calls `GET /api/session/{id}`
**Then** it receives session state, messages, status

**Given** OpenCode API is unreachable
**When** request times out (5s default)
**Then** error is logged and retry is scheduled

**Given** OpenCode returns error response
**When** status is 4xx/5xx
**Then** error is parsed and logged with details

**Technical Notes:**
- Implements: FR47
- Create `src/opencode/client.rs`
- Use reqwest with configured base URL
- Endpoints based on OpenCode HTTP API spec

---

### Story 9.4: OpenCode Configuration Options

As a user,
I want to configure OpenCode connection settings,
So that palingenesis works with my OpenCode setup.

**Acceptance Criteria:**

**Given** config section `[opencode]`
**When** user specifies `serve_port = 4096`
**Then** daemon uses port 4096 for OpenCode API calls
**And** passes `--port 4096` when restarting OpenCode

**Given** config section `[opencode]`
**When** user specifies `serve_hostname = "127.0.0.1"`
**Then** daemon connects to that hostname
**And** passes `--hostname 127.0.0.1` when restarting

**Given** config section `[opencode]`
**When** user specifies `auto_restart = false`
**Then** daemon does NOT auto-restart crashed OpenCode

**Given** config section `[opencode]`
**When** user specifies `restart_delay_ms = 2000`
**Then** daemon waits 2 seconds before restarting

**Given** config section `[opencode]`
**When** user specifies `health_check_interval = "10s"`
**Then** daemon polls OpenCode health every 10 seconds

**Given** default config
**When** no `[opencode]` section exists
**Then** defaults are used: port 4096, hostname 127.0.0.1, auto_restart true

**Technical Notes:**
- Implements: FR48
- Add to `src/config/schema.rs`
- Config structure matches PRD specification
- Document all options in default config template

---

## Epic 10: Bi-Directional Telegram Bot

User can receive notifications AND send control commands to palingenesis via Telegram Bot, providing full mobile control without SSH access.

### Story 10.1: Telegram Bot Module Setup

As a daemon,
I want a dedicated Telegram bot module,
So that I can handle both inbound commands and outbound notifications via Telegram.

**Acceptance Criteria:**

**Given** the daemon starts with Telegram configured (bot_token + chat_id)
**When** initialization completes
**Then** the Telegram bot polling loop starts as a dedicated tokio task
**And** logs "Telegram bot started, polling for commands"

**Given** Telegram is not configured (no bot_token)
**When** the daemon starts
**Then** the Telegram bot module is not initialized
**And** no Telegram-related tasks are spawned

**Given** the Telegram bot is running
**When** the daemon shuts down
**Then** the polling loop is cancelled via CancellationToken
**And** cleanup completes gracefully

**Technical Notes:**
- Implements: FR50
- Create `src/telegram/mod.rs`, `src/telegram/bot.rs`
- Uses CancellationToken from daemon shutdown system
- No external bot framework needed — direct Bot API via reqwest

---

### Story 10.2: Telegram getUpdates Long-Polling

As a Telegram bot,
I want to poll for incoming messages via getUpdates,
So that I can receive user commands without a public webhook endpoint.

**Acceptance Criteria:**

**Given** the bot is initialized
**When** it starts polling
**Then** it calls `getUpdates` with `timeout=30` (long-poll)
**And** tracks `offset` to avoid processing duplicate updates

**Given** no new messages
**When** the long-poll times out (30s)
**Then** it immediately starts a new poll
**And** CPU usage remains <1%

**Given** new messages arrive
**When** `getUpdates` returns updates
**Then** each update is processed sequentially
**And** `offset` is updated to `last_update_id + 1`

**Given** the Telegram API returns an error
**When** the error is transient (network, 500)
**Then** it retries with exponential backoff (5s, 10s, 20s, max 60s)
**And** logs the error at warn level

**Given** the bot token is invalid
**When** Telegram returns 401 Unauthorized
**Then** it logs error "Invalid Telegram bot token"
**And** stops polling (does not retry — permanent error)

**Technical Notes:**
- Implements: FR50
- Create `src/telegram/polling.rs`
- Long-poll timeout of 30s = ~2 API calls/minute during idle
- Use reqwest client shared with notification sender

---

### Story 10.3: Telegram Command Parser

As a Telegram bot,
I want to parse incoming messages as daemon commands,
So that users can control palingenesis from Telegram.

**Acceptance Criteria:**

**Given** a message `/status` from the configured chat_id
**When** the parser processes it
**Then** it returns `Command::Status`

**Given** a message `/pause` from the configured chat_id
**When** the parser processes it
**Then** it returns `Command::Pause`

**Given** a message `/resume` from the configured chat_id
**When** the parser processes it
**Then** it returns `Command::Resume`

**Given** a message `/skip` from the configured chat_id
**When** the parser processes it
**Then** it returns `Command::Skip`

**Given** a message `/abort` from the configured chat_id
**When** the parser processes it
**Then** it returns `Command::Abort`

**Given** a message `/config` from the configured chat_id
**When** the parser processes it
**Then** it returns `Command::Config`

**Given** a message `/new_session` from the configured chat_id
**When** the parser processes it
**Then** it returns `Command::NewSession`

**Given** a message `/logs` or `/logs 20` from the configured chat_id
**When** the parser processes it
**Then** it returns `Command::Logs { tail: N }` (default 10)

**Given** a message `/help` from the configured chat_id
**When** the parser processes it
**Then** it returns `Command::Help`

**Given** a message from a DIFFERENT chat_id
**When** the parser processes it
**Then** it rejects the message silently (security: only configured chat_id)
**And** logs warn "Rejected command from unauthorized chat: {chat_id}"

**Given** an unrecognized command
**When** the parser processes it
**Then** it returns `Command::Unknown` with the original text
**And** bot responds with "Unknown command. Type /help for available commands."

**Technical Notes:**
- Implements: FR51, FR52, FR53, FR54
- Create `src/telegram/commands.rs`
- Commands route through the same `CommandHandler` trait used by IPC and HTTP
- Security: validate chat_id matches config before processing

---

### Story 10.4: Telegram Command Dispatch & Response

As a Telegram bot,
I want to execute parsed commands and send responses back,
So that users get feedback on their control actions.

**Acceptance Criteria:**

**Given** a valid `/status` command
**When** dispatched to the daemon
**Then** bot responds with formatted status message:
```
ℹ️ Daemon Status
State: monitoring
Uptime: 2h 30m
Session: /path/to/session.md
Steps: 5/12
Saves: 42
```

**Given** a valid `/pause` command
**When** dispatched to the daemon
**Then** daemon transitions to paused state
**And** bot responds "⏸ Monitoring paused"

**Given** a valid `/resume` command
**When** dispatched to the daemon
**Then** daemon transitions to monitoring state
**And** bot responds "▶️ Monitoring resumed"

**Given** a valid `/logs 5` command
**When** dispatched to the daemon
**Then** bot responds with last 5 log lines
**And** message uses monospace formatting for readability

**Given** a `/help` command
**When** processed
**Then** bot responds with list of available commands and descriptions

**Given** any command fails
**When** error occurs
**Then** bot responds with "❌ Error: {reason}"

**Given** response text exceeds Telegram's 4096 char limit
**When** sending response
**Then** it truncates with "... (truncated, showing last N lines)"

**Technical Notes:**
- Implements: FR51, FR52, FR53, FR54
- Use the shared CommandHandler trait for dispatch
- Responses sent via `sendMessage` with HTML parse_mode
- Reuses existing `notify/telegram.rs` client for sending

---

### Story 10.5: Telegram Outbound Notifications

As a user,
I want to receive daemon event notifications in Telegram,
So that I'm alerted about session stops, resumes, and errors on my phone.

**Acceptance Criteria:**

**Given** Telegram is configured with bot_token and chat_id
**When** a notification event occurs (SessionStopped, ResumeSucceeded, etc.)
**Then** a formatted message is sent to the configured Telegram chat

**Given** a SessionStopped event
**When** notification is sent
**Then** message includes emoji, event title, timestamp, session path, stop reason

**Given** a ResumeSucceeded event
**When** notification is sent
**Then** message includes strategy used and wait time

**Given** the Telegram API is unavailable
**When** notification fails
**Then** it is retried up to 3 times with backoff
**And** failure is logged but doesn't crash the daemon

**Technical Notes:**
- Implements: FR49
- Already partially implemented in `src/notify/telegram.rs`
- Ensure existing TelegramChannel is registered in the notification dispatcher
- Verify it follows the same NotificationChannel trait pattern as Discord/Slack/webhook

---

### Story 10.6: Telegram Integration Tests

As a developer,
I want integration tests for the Telegram bot module,
So that I can verify command parsing, dispatch, and error handling.

**Acceptance Criteria:**

**Given** test fixtures with sample Telegram updates (JSON)
**When** the command parser processes them
**Then** all commands are correctly parsed

**Given** an unauthorized chat_id in test
**When** the security check runs
**Then** the message is rejected

**Given** mock Telegram API responses
**When** polling loop processes them
**Then** offset tracking works correctly across batches

**Given** a long-poll timeout scenario
**When** the polling loop handles it
**Then** it re-polls without error

**Technical Notes:**
- Create `tests/integration/telegram_test.rs`
- Use wiremock or similar for mocking Telegram Bot API
- Test both happy paths and error paths (invalid token, network errors)

---

## Summary

| Epic | Stories | FRs Covered | Priority |
|------|---------|-------------|----------|
| Epic 1: Installable CLI with Daemon Lifecycle | 13 | FR14-17 + ARCH | MVP |
| Epic 2: Session Detection & Classification | 7 | FR1-7 | MVP |
| Epic 3: Automatic Session Resumption | 9 | FR8-13, FR18-20 | MVP |
| Epic 4: Configuration Management | 7 | FR21-25 | MVP |
| Epic 5: Event Notifications | 6 | FR26-30 | Growth |
| Epic 6: Remote Control & External API | 6 | FR31-34 | Growth |
| Epic 7: Observability & Metrics | 7 | FR35-40 | Growth |
| Epic 8: MCP Server Interface | 4 | FR41-44 | Growth |
| Epic 9: OpenCode Process Management | 4 | FR45-48 | Growth |
| Epic 10: Bi-Directional Telegram Bot | 6 | FR49-54 | Growth |
| **Total** | **69 stories** | **54 FRs** | ✅ Complete |

All functional requirements are covered. Stories are sized for single dev agent completion with clear acceptance criteria in Given/When/Then format.
