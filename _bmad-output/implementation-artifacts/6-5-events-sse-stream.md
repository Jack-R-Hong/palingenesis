# Story 6.5: Events SSE Stream

## Story Information

| Field | Value |
|-------|-------|
| Story ID | 6-5 |
| Epic | Epic 6: Remote Control & External API |
| Status | ready-for-dev |
| Priority | High |
| Estimate | 3 story points |

## User Story

**As** an external tool,
**I want** to stream events in real-time,
**So that** I can react to daemon events without polling.

## Acceptance Criteria

### AC1: SSE Connection Establishment

**Given** a client connects to GET /api/v1/events
**When** connection is established
**Then** Server-Sent Events stream begins with initial connection event:
```
event: connected
data: {"status":"connected","timestamp":"..."}

```

### AC2: Event Broadcasting

**Given** a daemon event occurs (session_stopped, resume_attempted, resume_succeeded, resume_failed, daemon_started, daemon_stopped)
**When** the SSE stream is active
**Then** event is pushed to all connected clients in SSE format:
```
event: session_stopped
data: {"event":"session_stopped","timestamp":"...","session_path":"...","stop_reason":"...","details":null}

```

### AC3: Client Disconnection Cleanup

**Given** client disconnects
**When** cleanup runs
**Then** that client is removed from broadcast list without affecting other clients

### AC4: Keep-Alive Heartbeat

**Given** no events occur
**When** stream is idle for 30 seconds
**Then** keep-alive comment is sent:
```
: heartbeat

```

## Technical Notes

- Create `src/http/handlers/events.rs`
- Use axum's SSE support via `axum::response::sse::{Event, Sse}`
- Broadcast pattern for multiple clients using `tokio::sync::broadcast`
- Reference `NotificationEvent` types from `src/notify/events.rs`:
  - `SessionStopped`
  - `ResumeAttempted`
  - `ResumeSucceeded`
  - `ResumeFailed`
  - `DaemonStarted`
  - `DaemonStopped`
- Events already implement `Serialize` with `#[serde(tag = "event", rename_all = "snake_case")]`

## Technical Tasks

### Task 1: Create Event Broadcaster

**File:** `src/http/events.rs` (new module at http level)

- [ ] Create `EventBroadcaster` struct wrapping `tokio::sync::broadcast::Sender<NotificationEvent>`
- [ ] Implement `new(capacity: usize)` constructor (default capacity: 1024)
- [ ] Implement `subscribe()` returning `broadcast::Receiver<NotificationEvent>`
- [ ] Implement `send(event: NotificationEvent)` method
- [ ] Make `EventBroadcaster` `Clone` for sharing across handlers
- [ ] Add to `src/http/mod.rs` exports

### Task 2: Create SSE Event Handler

**File:** `src/http/handlers/events.rs`

- [ ] Import `axum::response::sse::{Event, Sse, KeepAlive}`
- [ ] Import `futures::stream::Stream`
- [ ] Create `events_handler` async function returning `Sse<impl Stream<Item = Result<Event, Infallible>>>`
- [ ] Extract `EventBroadcaster` from axum state
- [ ] Call `broadcaster.subscribe()` to get receiver
- [ ] Convert receiver to stream using `tokio_stream::wrappers::BroadcastStream`

### Task 3: Implement Event Stream Mapping

**File:** `src/http/handlers/events.rs`

- [ ] Map `NotificationEvent` to `Event` using:
  - `Event::default().event(event.event_type()).json_data(&event)`
- [ ] Handle `BroadcastStreamRecvError::Lagged` by logging and continuing
- [ ] Filter out lagged errors from stream output
- [ ] Send initial "connected" event when stream starts

### Task 4: Configure Keep-Alive

**File:** `src/http/handlers/events.rs`

- [ ] Configure `Sse` response with `KeepAlive::new().interval(Duration::from_secs(30)).text(": heartbeat")`
- [ ] Ensure keep-alive uses SSE comment format (line starting with `:`)

### Task 5: Wire Events Route to Router

**File:** `src/http/server.rs`

- [ ] Add `EventBroadcaster` to app state alongside `DaemonState`
- [ ] Create `AppState` struct combining `Arc<DaemonState>` and `EventBroadcaster`
- [ ] Add `GET /api/v1/events` route pointing to `events_handler`
- [ ] Import events handler module

### Task 6: Update Handlers Module

**File:** `src/http/handlers/mod.rs`

- [ ] Add `pub mod events;`
- [ ] Export events handler

### Task 7: Integrate Broadcaster with Daemon Events

**File:** `src/daemon/mod.rs` or appropriate daemon coordination point

- [ ] Pass `EventBroadcaster` to daemon components that emit events
- [ ] When `NotificationEvent` is dispatched to notifiers, also send to broadcaster
- [ ] Ensure broadcaster integration doesn't block notification dispatch

### Task 8: Write Tests

**File:** `src/http/handlers/events.rs` (tests module)

- [ ] Test SSE stream returns correct content-type: `text/event-stream`
- [ ] Test initial "connected" event is sent on connection
- [ ] Test event is correctly formatted as SSE (event: + data: lines)
- [ ] Test multiple clients receive same event
- [ ] Test client removal doesn't affect other clients
- [ ] Test keep-alive is sent after timeout (may require mock time)

**File:** `src/http/events.rs` (tests module)

- [ ] Test `EventBroadcaster::new()` creates broadcaster
- [ ] Test `subscribe()` returns working receiver
- [ ] Test `send()` delivers to all subscribers
- [ ] Test lagged subscribers don't block sender

## Dependencies

- Story 6-1 (HTTP API Server Setup) - **DONE** - server infrastructure
- Story 6-2 (Health Endpoint) - **DONE** - pattern reference
- Story 5-6 (Notification Events Definition) - **DONE** - `NotificationEvent` types
- `src/notify/events.rs` - event type definitions
- `axum::response::sse` - SSE support
- `tokio::sync::broadcast` - multi-consumer channel
- `tokio_stream::wrappers::BroadcastStream` - stream adapter

## Definition of Done

- [ ] `GET /api/v1/events` returns SSE stream with `text/event-stream` content type
- [ ] Initial "connected" event sent on connection
- [ ] All `NotificationEvent` types broadcast correctly to clients
- [ ] Multiple clients can connect and receive events simultaneously
- [ ] Disconnected clients are cleaned up (no resource leak)
- [ ] Keep-alive heartbeat sent every 30s during idle periods
- [ ] All tests pass
- [ ] Code follows project conventions (clippy, fmt)
- [ ] Handlers are documented with rustdoc comments

## Out of Scope

- Authentication on SSE endpoint
- Event filtering per client
- Event replay/history
- Websocket alternative
- Rate limiting SSE connections
- Per-event acknowledgment

## Notes

- SSE is HTTP/1.1 chunked transfer encoding; ensure proxy compatibility
- Broadcast channel drops old messages if receiver is slow (acceptable)
- `BroadcastStream` handles the `Lagged` error gracefully
- Keep-alive prevents proxy/load balancer connection timeout
- Event names match `NotificationEvent::event_type()` output (snake_case)
- JSON payload in `data:` line must be single-line (no embedded newlines)
