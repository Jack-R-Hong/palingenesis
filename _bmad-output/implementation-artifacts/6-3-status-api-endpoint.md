# Story 6.3: Status API Endpoint

## Story Information

| Field | Value |
|-------|-------|
| Story ID | 6-3 |
| Epic | Epic 6: Remote Control & External API |
| Status | ready-for-dev |
| Priority | High |
| Estimate | 3 story points |

## User Story

**As** an external tool,
**I want** a status endpoint,
**So that** I can get detailed daemon state.

## Acceptance Criteria

### AC1: Status Endpoint Returns Full JSON

**Given** the daemon is running
**When** GET /api/v1/status is called
**Then** response is HTTP 200 with full status JSON:
```json
{
  "state": "monitoring",
  "current_session": "/path/to/session.md",
  "stats": {
    "uptime_secs": 3600,
    "saves_count": 7,
    "total_resumes": 3
  },
  "config_summary": {
    "http_enabled": true,
    "http_port": 8080,
    "auto_detect": true,
    "assistants": ["opencode", "aider"]
  }
}
```

### AC2: Response Includes Required Fields

**Given** status response
**When** parsed
**Then** it includes all required fields:
- `state`: Current daemon state ("monitoring" | "paused")
- `current_session`: Path to current session file (nullable)
- `stats`: Object with uptime_secs, saves_count, total_resumes
- `config_summary`: Object with key configuration values

### AC3: Same Data as CLI Status --json

**Given** the daemon is running
**When** GET /api/v1/status is called
**Then** response contains equivalent data to `palingenesis status --json` CLI output

## Technical Notes

- Implements: ARCH23 (response format)
- Create `src/http/handlers/status.rs`
- Returns same data as CLI status --json
- Reference existing `DaemonStatus` struct in `src/ipc/protocol.rs`
- Follow health endpoint pattern from `src/http/handlers/health.rs`

## Technical Tasks

### Task 1: Create StatusResponse Struct

**File:** `src/http/handlers/status.rs`

- [ ] Create `StatusResponse` struct with serde serialization
- [ ] Include `state: String` field
- [ ] Include `current_session: Option<String>` field
- [ ] Create nested `StatsResponse` struct with uptime_secs, saves_count, total_resumes
- [ ] Create nested `ConfigSummary` struct with http_enabled, http_port, auto_detect, assistants
- [ ] Implement conversion from `DaemonState` to `StatusResponse`

### Task 2: Implement Status Handler

**File:** `src/http/handlers/status.rs`

- [ ] Implement `status_handler` async function
- [ ] Extract state from `Arc<DaemonState>`
- [ ] Build `StatusResponse` from daemon state
- [ ] Extract config summary from daemon config
- [ ] Return `(StatusCode::OK, Json<StatusResponse>)`

### Task 3: Wire Status Route to Router

**File:** `src/http/server.rs`

- [ ] Import status handler module
- [ ] Add `GET /api/v1/status` route to router
- [ ] Ensure route follows existing routing patterns

### Task 4: Update Handlers Module

**File:** `src/http/handlers/mod.rs`

- [ ] Add `pub mod status;`
- [ ] Export status handler

### Task 5: Write Tests

**File:** `src/http/handlers/status.rs` (tests module)

- [ ] Test status response returns 200 with correct JSON structure
- [ ] Test state field reflects daemon state (monitoring vs paused)
- [ ] Test stats contain valid numeric values
- [ ] Test config_summary contains expected fields
- [ ] Test current_session is None when no active session

## Dependencies

- Story 6-1 (HTTP API Server Setup) - **DONE**
- Story 6-2 (Health Endpoint) - **DONE** - pattern reference
- `src/ipc/protocol.rs` - `DaemonStatus` struct reference
- `src/daemon/state.rs` - `DaemonState` for data extraction

## Definition of Done

- [ ] `GET /api/v1/status` returns 200 with full status JSON
- [ ] Response includes state, current_session, stats, config_summary
- [ ] All tests pass
- [ ] Code follows project conventions (clippy, fmt)
- [ ] Handler is documented with rustdoc comments
- [ ] Response format matches CLI `status --json` output

## Out of Scope

- Authentication on status endpoint
- Rate limiting
- Caching status responses
- Historical status data

## Notes

- The status endpoint provides detailed operational information
- Consider whether sensitive config values should be redacted
- This enables external monitoring tools and dashboards
- Follow ARCH23 response format conventions
