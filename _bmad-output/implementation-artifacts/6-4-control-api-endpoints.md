# Story 6.4: Control API Endpoints

## Story Information

| Field | Value |
|-------|-------|
| Story ID | 6-4 |
| Epic | Epic 6: Remote Control & External API |
| Status | ready-for-dev |
| Priority | High |
| Estimate | 3 story points |

## User Story

**As** an external tool,
**I want** control endpoints,
**So that** I can pause/resume/control the daemon remotely.

## Acceptance Criteria

### AC1: Pause Endpoint

**Given** the daemon is monitoring
**When** POST /api/v1/pause is called
**Then** daemon pauses and responds:
```json
{ "success": true }
```

### AC2: Resume Endpoint

**Given** the daemon is paused
**When** POST /api/v1/resume is called
**Then** daemon resumes and responds:
```json
{ "success": true }
```

### AC3: New Session Endpoint

**Given** POST /api/v1/new-session is called
**When** a session exists
**Then** new session is started and responds with session_id:
```json
{ "success": true, "session_id": "..." }
```

### AC4: Error Response Format

**Given** any control endpoint
**When** action fails
**Then** response is 400/500 with error details:
```json
{
  "success": false,
  "error": {
    "code": "ALREADY_PAUSED",
    "message": "Daemon already paused"
  }
}
```

## Technical Notes

- Implements: ARCH23 (response format)
- Create `src/http/handlers/control.rs`
- Same functionality as CLI commands
- Reference existing `DaemonStateAccess` trait methods:
  - `pause()` -> returns `Result<(), String>`
  - `resume()` -> returns `Result<(), String>`
  - `new_session()` -> returns `Result<(), String>`
- IPC protocol already has `PAUSE`, `RESUME`, `NEW_SESSION` commands in `src/ipc/protocol.rs`

## Technical Tasks

### Task 1: Create Response Structs

**File:** `src/http/handlers/control.rs`

- [ ] Create `ControlResponse` struct with `success: bool` field
- [ ] Create `ControlResponseWithId` struct adding optional `session_id: Option<String>`
- [ ] Create `ErrorDetail` struct with `code: String` and `message: String`
- [ ] Create `ControlErrorResponse` struct with `success: bool` and `error: ErrorDetail`
- [ ] Add serde derive macros for JSON serialization

### Task 2: Implement Pause Handler

**File:** `src/http/handlers/control.rs`

- [ ] Implement `pause_handler` async function
- [ ] Extract `Arc<DaemonState>` from axum state
- [ ] Call `state.pause()` method
- [ ] On success: return 200 with `{ "success": true }`
- [ ] On error "Daemon already paused": return 400 with `{ "success": false, "error": { "code": "ALREADY_PAUSED", "message": "..." } }`

### Task 3: Implement Resume Handler

**File:** `src/http/handlers/control.rs`

- [ ] Implement `resume_handler` async function
- [ ] Extract `Arc<DaemonState>` from axum state
- [ ] Call `state.resume()` method
- [ ] On success: return 200 with `{ "success": true }`
- [ ] On error "Daemon is not paused": return 400 with `{ "success": false, "error": { "code": "NOT_PAUSED", "message": "..." } }`

### Task 4: Implement New Session Handler

**File:** `src/http/handlers/control.rs`

- [ ] Implement `new_session_handler` async function
- [ ] Extract `Arc<DaemonState>` from axum state
- [ ] Call `state.new_session()` method
- [ ] On success: return 200 with `{ "success": true, "session_id": "..." }`
- [ ] On error: return 500 with `{ "success": false, "error": { "code": "SESSION_ERROR", "message": "..." } }`

### Task 5: Wire Control Routes to Router

**File:** `src/http/server.rs`

- [ ] Import control handler module
- [ ] Add `POST /api/v1/pause` route
- [ ] Add `POST /api/v1/resume` route
- [ ] Add `POST /api/v1/new-session` route
- [ ] Ensure routes follow existing routing patterns

### Task 6: Update Handlers Module

**File:** `src/http/handlers/mod.rs`

- [ ] Add `pub mod control;`
- [ ] Export control handlers

### Task 7: Write Tests

**File:** `src/http/handlers/control.rs` (tests module)

- [ ] Test pause returns 200 with `{ "success": true }` when monitoring
- [ ] Test pause returns 400 with error when already paused
- [ ] Test resume returns 200 with `{ "success": true }` when paused
- [ ] Test resume returns 400 with error when not paused
- [ ] Test new-session returns 200 with success and session_id
- [ ] Test error response format matches specification

## Dependencies

- Story 6-1 (HTTP API Server Setup) - **DONE**
- Story 6-2 (Health Endpoint) - **DONE** - pattern reference
- Story 6-3 (Status API Endpoint) - **DONE** - pattern reference
- `src/ipc/protocol.rs` - IpcCommand enum reference
- `src/daemon/state.rs` - DaemonState and DaemonStateAccess trait
- `src/ipc/socket.rs` - DaemonStateAccess trait definition

## Definition of Done

- [ ] `POST /api/v1/pause` returns 200 with success JSON when monitoring
- [ ] `POST /api/v1/resume` returns 200 with success JSON when paused
- [ ] `POST /api/v1/new-session` returns 200 with success and session_id
- [ ] All error cases return 400/500 with proper error JSON format
- [ ] All tests pass
- [ ] Code follows project conventions (clippy, fmt)
- [ ] Handlers are documented with rustdoc comments
- [ ] Functionality matches CLI command behavior

## Out of Scope

- Authentication on control endpoints
- Rate limiting
- Audit logging of control actions
- Bulk operations

## Notes

- Control endpoints provide remote daemon management
- Error codes should be consistent and documented
- Consider idempotency: pause when paused could be 200 (idempotent) or 400 (strict)
- Current implementation uses strict mode (returns error if already in target state)
- Follow ARCH23 response format conventions
