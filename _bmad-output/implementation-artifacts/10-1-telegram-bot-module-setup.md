# Story 8.1: Telegram Bot Module Setup

Status: ready-for-dev

## Story

As a daemon,
I want a dedicated Telegram bot module,
So that I can handle both inbound commands and outbound notifications via Telegram.

## Acceptance Criteria

**AC1: Bot Initialization With Valid Config**
**Given** the daemon starts with Telegram configured (`bot_token` + `chat_id` in `[notifications.telegram]`)
**When** initialization completes
**Then** the Telegram bot polling loop starts as a dedicated tokio task
**And** logs "Telegram bot started, polling for commands" at INFO level

**AC2: Bot Skipped Without Config**
**Given** Telegram is not configured (no `bot_token`)
**When** the daemon starts
**Then** the Telegram bot module is not initialized
**And** no Telegram-related tasks are spawned
**And** no warnings or errors are logged about Telegram

**AC3: Graceful Shutdown**
**Given** the Telegram bot is running
**When** the daemon shuts down (SIGTERM/SIGINT)
**Then** the polling loop is cancelled via `CancellationToken`
**And** cleanup completes gracefully within the 10s shutdown timeout
**And** no error is logged from the polling loop

**AC4: Module Structure**
**Given** the `src/telegram/` module
**When** the code is compiled
**Then** `src/telegram/mod.rs` re-exports public types: `TelegramBot`
**And** `src/telegram/bot.rs` contains the bot lifecycle struct and `run()` method
**And** `src/lib.rs` declares `pub mod telegram`

**AC5: reqwest Client Reuse**
**Given** the bot needs an HTTP client for Telegram Bot API calls
**When** the `TelegramBot` is constructed
**Then** it creates a single shared `reqwest::Client` with a 35s timeout
**And** the client uses `rustls` TLS backend (matching existing `reqwest` features)

## Tasks / Subtasks

- [ ] Task 1: Create telegram module structure (AC: 4)
  - [ ] Create `src/telegram/mod.rs` with `pub mod bot;` and `pub use bot::TelegramBot;`
  - [ ] Create `src/telegram/bot.rs` with `TelegramBot` struct
  - [ ] Add `pub mod telegram;` to `src/lib.rs`

- [ ] Task 2: Implement TelegramBot struct (AC: 1, 2, 5)
  - [ ] Define `TelegramBot` struct with fields: `bot_token: String`, `chat_id: String`, `client: reqwest::Client`, `cancel: CancellationToken`
  - [ ] Implement `TelegramBot::new(config: &TelegramConfig, cancel: CancellationToken) -> Self`
  - [ ] Build `reqwest::Client` with 35s timeout in constructor
  - [ ] Implement `TelegramBot::is_configured(config: &Config) -> bool` as a static helper that checks `config.notifications.telegram.is_some()` AND `bot_token` is non-empty

- [ ] Task 3: Implement bot run loop placeholder (AC: 1, 3)
  - [ ] Implement `pub async fn run(&self)` method with a `tokio::select!` loop
  - [ ] The select branches: `cancel.cancelled()` to break, and a placeholder `tokio::time::sleep(Duration::from_secs(30))` that will be replaced by real `getUpdates` polling in Story 8.2
  - [ ] Log `info!("Telegram bot started, polling for commands")` on entry
  - [ ] Log `info!("Telegram bot shutting down")` on cancellation

- [ ] Task 4: Integrate bot into daemon core (AC: 1, 2, 3)
  - [ ] In `src/daemon/core.rs` `Daemon::run()`, after existing task spawns:
    - Check if Telegram is configured via `TelegramBot::is_configured(&config)`
    - If yes, construct `TelegramBot::new(&telegram_config, cancel.clone())`
    - Spawn `tokio::spawn(async move { bot.run().await })` and register with `ShutdownCoordinator`
  - [ ] Import `TelegramConfig` from `crate::config::schema` and `TelegramBot` from `crate::telegram`

- [ ] Task 5: Add unit tests (AC: 1, 2, 3, 4, 5)
  - [ ] Test `TelegramBot::is_configured` returns `false` when `telegram` is `None`
  - [ ] Test `TelegramBot::is_configured` returns `false` when `bot_token` is empty
  - [ ] Test `TelegramBot::is_configured` returns `true` when both `bot_token` and `chat_id` are set
  - [ ] Test `TelegramBot::new` constructs without panic
  - [ ] Test `TelegramBot::run` exits when `CancellationToken` is cancelled (tokio test)

## Dev Notes

### Architecture Requirements

**From architecture.md - Telegram Bot Architecture:**

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Command Ingestion | Long-polling (`getUpdates`) | No public URL needed, works behind NAT |
| Bot Framework | Direct Bot API via `reqwest` | No heavy framework; avoids `teloxide` dependency bloat |
| Command Dispatch | Shared `CommandHandler` trait | Same dispatch as IPC and HTTP |
| Module Location | `src/telegram/` | Separate from `src/notify/` because Telegram is bi-directional |
| Polling Loop | Dedicated tokio task | `tokio::spawn` with `CancellationToken` |

**From architecture.md - Integration Points:**

| From | To | Mechanism |
|------|-----|-----------|
| Telegram -> Daemon | `tokio::sync::mpsc` | `TelegramCommand` channel |

> **Note for this story:** Story 8.1 sets up the module skeleton and daemon integration only. The actual `getUpdates` polling (Story 8.2), command parsing (Story 8.3), and command dispatch (Story 8.4) are separate stories. The `run()` loop here is a placeholder that sleeps and responds to cancellation.

### Existing Code to Reuse (DO NOT Reinvent)

1. **`src/notify/telegram.rs`** - Outbound-only `TelegramChannel` already exists with:
   - `TelegramPayload` struct for `sendMessage`
   - `format_event_message()` for notification formatting
   - `html_escape()` helper
   - **DO NOT** duplicate the outbound notification logic. The new `src/telegram/` module handles inbound commands; outbound stays in `src/notify/telegram.rs`.

2. **`src/config/schema.rs`** - `TelegramConfig` already defined:
   ```rust
   pub struct TelegramConfig {
       pub bot_token: String,
       pub chat_id: String,
   }
   ```
   Located at `config.notifications.telegram: Option<TelegramConfig>`.

3. **`src/daemon/core.rs`** - Daemon task spawn pattern:
   ```rust
   let cancel = self.shutdown.cancel_token();
   self.shutdown.register_task(tokio::spawn(async move {
       // task body with cancel.cancelled() select
   }));
   ```

4. **`src/daemon/shutdown.rs`** - `ShutdownCoordinator` with 10s timeout. All spawned tasks must be registered via `register_task()`.

5. **`src/ipc/socket.rs`** - `DaemonStateAccess` trait defines the command interface the bot will eventually dispatch to (Story 8.4):
   ```rust
   pub trait DaemonStateAccess: Send + Sync {
       fn get_status(&self) -> DaemonStatus;
       fn pause(&self) -> Result<(), String>;
       fn resume(&self) -> Result<(), String>;
       fn new_session(&self) -> Result<(), String>;
       fn reload_config(&self) -> Result<(), String>;
   }
   ```

### Project Structure (This Story Creates)

```
src/telegram/
    mod.rs          # pub mod bot; pub use bot::TelegramBot;
    bot.rs          # TelegramBot struct, new(), run(), is_configured()
```

Future stories (8.2-8.6) will add:
```
src/telegram/
    polling.rs      # getUpdates long-polling loop (Story 8.2)
    commands.rs     # Command parsing & dispatch (Story 8.3, 8.4)
```

### Dependencies

No new crate dependencies needed. Uses existing:
- `reqwest` 0.13.1 (already in Cargo.toml with `json`, `rustls` features)
- `tokio` 1.49 with `full` features
- `tokio-util` 0.7 for `CancellationToken`
- `tracing` 0.1.44 for structured logging

### Naming & Pattern Conventions

- Follow Rust standard: `PascalCase` for types, `snake_case` for functions/modules
- Use `thiserror` for any domain errors (add `TelegramError` if needed, or defer to Story 8.2)
- Re-export public types through `mod.rs`
- Structured logging: `info!(bot_token_len = config.bot_token.len(), "Telegram bot configured")`
- **Never log the actual `bot_token` value** — only log its length or a masked version

### Testing Strategy

**Unit Tests (inline `#[cfg(test)]`):**
- `is_configured` logic with various config states
- Constructor doesn't panic
- `run()` exits on cancellation (use `tokio::test` with short sleep + cancel)

**No integration tests needed for this story** — the bot doesn't make real API calls yet. Integration tests for Telegram API interaction come in Story 8.6.

### Security Considerations

- **Bot token must never be logged.** Use `bot_token_len` or `"***"` in any log output.
- **Config file permissions:** Already enforced at 600 by config module (Story 4.2).
- **chat_id validation** is deferred to Story 8.3 (command parser).

### References

- [Source: _bmad-output/planning-artifacts/architecture.md#Telegram Bot Architecture]
- [Source: _bmad-output/planning-artifacts/architecture.md#Project Structure & Boundaries]
- [Source: _bmad-output/planning-artifacts/architecture.md#Integration Points]
- [Source: _bmad-output/planning-artifacts/epics.md#Story 8.1: Telegram Bot Module Setup]
- [Source: _bmad-output/planning-artifacts/prd.md#Telegram Integration (Growth)]
- [Source: src/notify/telegram.rs - Existing outbound notification implementation]
- [Source: src/daemon/core.rs - Task spawn and shutdown pattern]
- [Source: src/config/schema.rs - TelegramConfig struct]
- [Source: src/ipc/socket.rs - DaemonStateAccess trait for command dispatch]

## Dev Agent Record

### Agent Model Used

### Debug Log References

### Completion Notes List

### File List

**Files to create:**
- `src/telegram/mod.rs`
- `src/telegram/bot.rs`

**Files to modify:**
- `src/lib.rs` (add `pub mod telegram;`)
- `src/daemon/core.rs` (spawn Telegram bot task when configured)

**Files NOT to modify:**
- `src/notify/telegram.rs` (outbound notifications — leave as-is)
- `src/config/schema.rs` (TelegramConfig already exists)
- `Cargo.toml` (no new dependencies)

## Change Log

- 2026-02-06: Story created and marked ready-for-dev
