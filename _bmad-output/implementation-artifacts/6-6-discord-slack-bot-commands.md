# Story 6.6: Discord/Slack Bot Commands

## Story Information

| Field | Value |
|-------|-------|
| Story ID | 6-6 |
| Epic | Epic 6: Remote Control & External API |
| Status | review |
| Priority | Medium |
| Estimate | 5 story points |

## User Story

**As** a user,
**I want** to control palingenesis via Discord/Slack commands,
**So that** I can manage it from my phone without SSH.

## Acceptance Criteria

### AC1: Discord Bot Status Command

**Given** Discord bot is configured
**When** I type `/palin status` in Discord
**Then** bot responds with daemon status including:
- Running state (running/stopped/paused)
- Uptime
- Current session path (if any)
- Last event timestamp

### AC2: Slack Bot Pause Command

**Given** Slack bot is configured
**When** I type `/palin pause` in Slack
**Then** bot pauses daemon and confirms with message:
```
Daemon paused successfully.
```

### AC3: Logs Tail Command

**Given** command `/palin logs --tail 5`
**When** executed via chat (Discord or Slack)
**Then** last 5 log lines are returned formatted for chat readability

### AC4: Authorization Enforcement

**Given** unauthorized user (not in allowed list)
**When** they try to use commands
**Then** command is rejected with message:
```
Unauthorized: You don't have permission to use this command.
```

## Technical Notes

- Implements: FR31, FR32, FR33, FR34
- Bot commands receive webhooks from Discord/Slack and call existing HTTP API endpoints
- Existing infrastructure to leverage:
  - HTTP API endpoints: `/api/v1/status`, `/api/v1/control/pause`, `/api/v1/control/resume`
  - Discord notification channel: `src/notify/discord.rs`
  - Slack notification channel: `src/notify/slack.rs`
- Consider: Slack slash commands vs Slack app with event subscriptions
- Consider: Discord interactions (slash commands) require registering with Discord API

## Architecture Decision

### Webhook Receiver Approach (Recommended)

Instead of running a persistent bot connection, use webhook receivers:

1. **Discord**: Register slash commands via Discord API, receive interactions at webhook endpoint
2. **Slack**: Configure slash commands in Slack app, receive POST at webhook endpoint

Both approaches:
- Reuse existing HTTP server (`src/http/server.rs`)
- Add new routes under `/api/v1/bot/discord` and `/api/v1/bot/slack`
- Parse platform-specific payloads
- Call existing control handlers internally
- Format responses for respective platforms

## Technical Tasks

### Task 1: Define Bot Command Types

**File:** `src/bot/commands.rs` (new)

- [x] Create `BotCommand` enum: `Status`, `Pause`, `Resume`, `Logs { tail: usize }`, `NewSession`, `Help`
- [x] Implement `FromStr` for parsing command strings like `/palin status`
- [x] Create `BotCommandResult` struct for command responses
- [x] Implement `BotCommandResult::to_discord_response()` and `to_slack_response()`
- [x] Add error variants for unknown command, missing args, etc.

### Task 2: Create Command Executor

**File:** `src/bot/executor.rs` (new)

- [x] Create `CommandExecutor` struct holding reference to `DaemonState`
- [x] Implement `execute(cmd: BotCommand) -> BotCommandResult`
- [x] Status command: call internal status logic (reuse from status handler)
- [x] Pause command: call `DaemonState::pause()`
- [x] Resume command: call `DaemonState::resume()`
- [x] Logs command: read recent log entries (implement or stub)
- [x] NewSession command: trigger new session
- [x] Help command: return available commands list

### Task 3: Create Authorization Layer

**File:** `src/bot/auth.rs` (new)

- [x] Create `BotAuth` struct with `allowed_users: HashSet<String>`
- [x] Implement `is_authorized(user_id: &str) -> bool`
- [x] Load allowed users from config (Discord user IDs, Slack user IDs)
- [x] Add `bot` section to config schema with `authorized_users`
- [x] If no authorized_users configured, allow all (or deny all based on config flag)

### Task 4: Discord Webhook Handler

**File:** `src/http/handlers/bot_discord.rs` (new)

- [x] Create `discord_webhook_handler` accepting Discord interaction payload
- [x] Verify Discord signature (Ed25519) for security
- [x] Parse interaction type (PING vs APPLICATION_COMMAND)
- [x] For PING: respond with type 1 (required for Discord verification)
- [x] For APPLICATION_COMMAND: extract command name and options
- [x] Check authorization using Discord user ID
- [x] Execute command via `CommandExecutor`
- [x] Format response as Discord interaction response (type 4)

### Task 5: Slack Webhook Handler

**File:** `src/http/handlers/bot_slack.rs` (new)

- [x] Create `slack_webhook_handler` accepting Slack slash command payload
- [x] Verify Slack signature (HMAC-SHA256) for security
- [x] Parse command text (e.g., "status" from `/palin status`)
- [x] Check authorization using Slack user ID
- [x] Execute command via `CommandExecutor`
- [x] Format response as Slack block kit message or plain text
- [x] Support both immediate response and delayed response (response_url)

### Task 6: Add Bot Routes to HTTP Server

**File:** `src/http/server.rs`

- [x] Add `POST /api/v1/bot/discord` route for Discord interactions
- [x] Add `POST /api/v1/bot/slack` route for Slack slash commands
- [x] Inject `CommandExecutor` and `BotAuth` into handler state
- [x] Ensure routes work without breaking existing endpoints

### Task 7: Update Configuration Schema

**File:** `src/config/schema.rs`

- [x] Add `BotConfig` struct with:
  - `enabled: bool`
  - `discord_application_id: Option<String>`
  - `discord_public_key: Option<String>` (for signature verification)
  - `slack_signing_secret: Option<String>` (for signature verification)
  - `authorized_users: Vec<AuthorizedUser>`
- [x] Add `AuthorizedUser` struct: `{ platform: Platform, user_id: String }`
- [x] Add `bot` section to main `Config` struct
- [x] Update config validation to check bot config consistency

### Task 8: Create Bot Module

**File:** `src/bot/mod.rs` (new)

- [x] Export `commands`, `executor`, `auth` modules
- [x] Add module to `src/lib.rs`

### Task 9: Update Handlers Module

**File:** `src/http/handlers/mod.rs`

- [x] Add `pub mod bot_discord;`
- [x] Add `pub mod bot_slack;`
- [x] Export bot handlers

### Task 10: Write Unit Tests

**File:** `src/bot/commands.rs` (tests module)

- [x] Test `BotCommand` parsing from string
- [x] Test `/palin status` parses to `Status`
- [x] Test `/palin logs --tail 5` parses to `Logs { tail: 5 }`
- [x] Test invalid command returns error

**File:** `src/bot/executor.rs` (tests module)

- [x] Test status command returns current state
- [x] Test pause command updates state
- [x] Test resume command updates state

**File:** `src/bot/auth.rs` (tests module)

- [x] Test authorized user passes check
- [x] Test unauthorized user fails check
- [x] Test empty authorized list behavior

### Task 11: Write Integration Tests

**File:** `tests/bot_integration.rs` (new)

- [x] Test Discord webhook endpoint accepts valid request
- [x] Test Discord webhook rejects invalid signature
- [x] Test Slack webhook endpoint accepts valid request
- [x] Test Slack webhook rejects invalid signature
- [x] Test unauthorized user receives rejection message

## Dependencies

- Story 6-1 (HTTP API Server Setup) - **DONE** - server infrastructure
- Story 6-3 (Status API Endpoint) - **DONE** - status logic to reuse
- Story 6-4 (Control API Endpoints) - **DONE** - pause/resume logic to reuse
- `src/notify/discord.rs` - reference for Discord API patterns
- `src/notify/slack.rs` - reference for Slack API patterns
- `ed25519-dalek` crate - Discord signature verification
- `hmac` + `sha2` crates - Slack signature verification

## Definition of Done

- [x] Discord interactions endpoint responds to slash commands
- [x] Slack slash command endpoint responds to commands
- [x] Status command returns daemon status on both platforms
- [x] Pause command pauses daemon on both platforms
- [x] Resume command resumes daemon on both platforms
- [x] Logs command returns recent log lines
- [x] Unauthorized users receive rejection message
- [x] Signature verification protects both endpoints
- [x] All tests pass
- [x] Code follows project conventions (clippy, fmt)
- [x] Handlers are documented with rustdoc comments

## Out of Scope

- Persistent bot connections (WebSocket to Discord Gateway)
- Rich interactive components (buttons, modals)
- Slash command registration CLI tool
- Multi-guild/workspace support
- Rate limiting bot commands
- Audit logging of bot commands
- Discord ephemeral responses

## Setup Instructions (for users)

### Discord Setup

1. Create Discord application at https://discord.com/developers/applications
2. Create slash command `/palin` with options: `status`, `pause`, `resume`, `logs`, `new-session`
3. Set Interactions Endpoint URL to `https://your-domain/api/v1/bot/discord`
4. Copy Application ID and Public Key to config
5. Add bot to server with `applications.commands` scope

### Slack Setup

1. Create Slack app at https://api.slack.com/apps
2. Create slash command `/palin` pointing to `https://your-domain/api/v1/bot/slack`
3. Copy Signing Secret to config
4. Install app to workspace

## Notes

- Discord requires responding within 3 seconds; long operations may need deferred responses
- Slack allows 3 second initial response, then 30 minutes via response_url
- Signature verification is critical - without it, anyone could control your daemon
- Consider adding `/palin help` command listing available commands
- Log lines should be truncated to fit platform message limits (Discord: 2000 chars, Slack: 3000 chars)
- User IDs are platform-specific: Discord snowflakes vs Slack member IDs

## Dev Agent Record

### Implementation Plan
- Add bot command types, executor, and authorization helpers.
- Implement Discord and Slack webhook handlers with signature verification.
- Wire routes, config schema, and validation updates.
- Add unit/integration tests and verify with build/tests.

### Completion Notes
- Added bot command parsing, execution, and authorization flows with formatted Discord/Slack responses.
- Implemented Discord/Slack webhook handlers with signature verification and router wiring.
- Updated config schema/validation, event tracking, and added integration tests.
- Verified with `cargo build` and `cargo test`.

## File List
- Cargo.toml
- src/bot/auth.rs
- src/bot/commands.rs
- src/bot/discord.rs
- src/bot/executor.rs
- src/bot/mod.rs
- src/bot/slack.rs
- src/config/schema.rs
- src/config/validation.rs
- src/daemon/state.rs
- src/http/events.rs
- src/http/handlers/bot_discord.rs
- src/http/handlers/bot_slack.rs
- src/http/handlers/control.rs
- src/http/handlers/mod.rs
- src/http/handlers/status.rs
- src/http/server.rs
- src/lib.rs
- src/notify/events.rs
- tests/bot_integration.rs

## Change Log
- 2026-02-06: Added bot webhook handling, authorization, config schema updates, and tests.
