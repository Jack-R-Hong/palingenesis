# Story 6.6: Discord/Slack Bot Commands

## Story Information

| Field | Value |
|-------|-------|
| Story ID | 6-6 |
| Epic | Epic 6: Remote Control & External API |
| Status | ready-for-dev |
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

- [ ] Create `BotCommand` enum: `Status`, `Pause`, `Resume`, `Logs { tail: usize }`, `NewSession`, `Help`
- [ ] Implement `FromStr` for parsing command strings like `/palin status`
- [ ] Create `BotCommandResult` struct for command responses
- [ ] Implement `BotCommandResult::to_discord_response()` and `to_slack_response()`
- [ ] Add error variants for unknown command, missing args, etc.

### Task 2: Create Command Executor

**File:** `src/bot/executor.rs` (new)

- [ ] Create `CommandExecutor` struct holding reference to `DaemonState`
- [ ] Implement `execute(cmd: BotCommand) -> BotCommandResult`
- [ ] Status command: call internal status logic (reuse from status handler)
- [ ] Pause command: call `DaemonState::pause()`
- [ ] Resume command: call `DaemonState::resume()`
- [ ] Logs command: read recent log entries (implement or stub)
- [ ] NewSession command: trigger new session
- [ ] Help command: return available commands list

### Task 3: Create Authorization Layer

**File:** `src/bot/auth.rs` (new)

- [ ] Create `BotAuth` struct with `allowed_users: HashSet<String>`
- [ ] Implement `is_authorized(user_id: &str) -> bool`
- [ ] Load allowed users from config (Discord user IDs, Slack user IDs)
- [ ] Add `bot` section to config schema with `authorized_users`
- [ ] If no authorized_users configured, allow all (or deny all based on config flag)

### Task 4: Discord Webhook Handler

**File:** `src/http/handlers/bot_discord.rs` (new)

- [ ] Create `discord_webhook_handler` accepting Discord interaction payload
- [ ] Verify Discord signature (Ed25519) for security
- [ ] Parse interaction type (PING vs APPLICATION_COMMAND)
- [ ] For PING: respond with type 1 (required for Discord verification)
- [ ] For APPLICATION_COMMAND: extract command name and options
- [ ] Check authorization using Discord user ID
- [ ] Execute command via `CommandExecutor`
- [ ] Format response as Discord interaction response (type 4)

### Task 5: Slack Webhook Handler

**File:** `src/http/handlers/bot_slack.rs` (new)

- [ ] Create `slack_webhook_handler` accepting Slack slash command payload
- [ ] Verify Slack signature (HMAC-SHA256) for security
- [ ] Parse command text (e.g., "status" from `/palin status`)
- [ ] Check authorization using Slack user ID
- [ ] Execute command via `CommandExecutor`
- [ ] Format response as Slack block kit message or plain text
- [ ] Support both immediate response and delayed response (response_url)

### Task 6: Add Bot Routes to HTTP Server

**File:** `src/http/server.rs`

- [ ] Add `POST /api/v1/bot/discord` route for Discord interactions
- [ ] Add `POST /api/v1/bot/slack` route for Slack slash commands
- [ ] Inject `CommandExecutor` and `BotAuth` into handler state
- [ ] Ensure routes work without breaking existing endpoints

### Task 7: Update Configuration Schema

**File:** `src/config/schema.rs`

- [ ] Add `BotConfig` struct with:
  - `enabled: bool`
  - `discord_application_id: Option<String>`
  - `discord_public_key: Option<String>` (for signature verification)
  - `slack_signing_secret: Option<String>` (for signature verification)
  - `authorized_users: Vec<AuthorizedUser>`
- [ ] Add `AuthorizedUser` struct: `{ platform: Platform, user_id: String }`
- [ ] Add `bot` section to main `Config` struct
- [ ] Update config validation to check bot config consistency

### Task 8: Create Bot Module

**File:** `src/bot/mod.rs` (new)

- [ ] Export `commands`, `executor`, `auth` modules
- [ ] Add module to `src/lib.rs`

### Task 9: Update Handlers Module

**File:** `src/http/handlers/mod.rs`

- [ ] Add `pub mod bot_discord;`
- [ ] Add `pub mod bot_slack;`
- [ ] Export bot handlers

### Task 10: Write Unit Tests

**File:** `src/bot/commands.rs` (tests module)

- [ ] Test `BotCommand` parsing from string
- [ ] Test `/palin status` parses to `Status`
- [ ] Test `/palin logs --tail 5` parses to `Logs { tail: 5 }`
- [ ] Test invalid command returns error

**File:** `src/bot/executor.rs` (tests module)

- [ ] Test status command returns current state
- [ ] Test pause command updates state
- [ ] Test resume command updates state

**File:** `src/bot/auth.rs` (tests module)

- [ ] Test authorized user passes check
- [ ] Test unauthorized user fails check
- [ ] Test empty authorized list behavior

### Task 11: Write Integration Tests

**File:** `tests/bot_integration.rs` (new)

- [ ] Test Discord webhook endpoint accepts valid request
- [ ] Test Discord webhook rejects invalid signature
- [ ] Test Slack webhook endpoint accepts valid request
- [ ] Test Slack webhook rejects invalid signature
- [ ] Test unauthorized user receives rejection message

## Dependencies

- Story 6-1 (HTTP API Server Setup) - **DONE** - server infrastructure
- Story 6-3 (Status API Endpoint) - **DONE** - status logic to reuse
- Story 6-4 (Control API Endpoints) - **DONE** - pause/resume logic to reuse
- `src/notify/discord.rs` - reference for Discord API patterns
- `src/notify/slack.rs` - reference for Slack API patterns
- `ed25519-dalek` crate - Discord signature verification
- `hmac` + `sha2` crates - Slack signature verification

## Definition of Done

- [ ] Discord interactions endpoint responds to slash commands
- [ ] Slack slash command endpoint responds to commands
- [ ] Status command returns daemon status on both platforms
- [ ] Pause command pauses daemon on both platforms
- [ ] Resume command resumes daemon on both platforms
- [ ] Logs command returns recent log lines
- [ ] Unauthorized users receive rejection message
- [ ] Signature verification protects both endpoints
- [ ] All tests pass
- [ ] Code follows project conventions (clippy, fmt)
- [ ] Handlers are documented with rustdoc comments

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

