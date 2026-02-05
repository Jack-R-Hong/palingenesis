# Story 6.2: Health Endpoint

## Story Information

| Field | Value |
|-------|-------|
| Story ID | 6-2 |
| Epic | Epic 6: Remote Control & External API |
| Status | ready-for-dev |
| Priority | High |
| Estimate | 2 story points |

## User Story

**As** an external tool,
**I want** a health check endpoint,
**So that** I can monitor if the daemon is healthy.

## Acceptance Criteria

### AC1: Healthy Daemon Returns OK Status

**Given** the daemon is running normally
**When** GET /health is called
**Then** response is HTTP 200 with body:
```json
{
  "status": "ok",
  "uptime": "2h30m"
}
```

### AC2: Degraded State Returns Details

**Given** the daemon has issues (e.g., watcher disconnected, IPC unavailable)
**When** GET /health is called
**Then** response includes degraded status and reason:
```json
{
  "status": "degraded",
  "uptime": "1h15m",
  "issues": ["file_watcher_disconnected"]
}
```

### AC3: Response Time Under 100ms

**Given** a load balancer or monitoring tool
**When** it polls /health
**Then** response time is consistently <100ms

## Technical Notes

- Create `src/http/handlers/health.rs`
- Simple, fast endpoint for health checks
- No database or expensive operations in health check
- Use daemon start time for uptime calculation

## Technical Tasks

### Task 1: Create Health Handler Module

**File:** `src/http/handlers/health.rs`

- [ ] Create `src/http/handlers/` directory structure
- [ ] Create `mod.rs` to export handlers
- [ ] Implement `HealthResponse` struct with serde serialization
- [ ] Implement `health_handler` async function returning JSON
- [ ] Track daemon start time via shared state or lazy_static

### Task 2: Implement Uptime Calculation

- [ ] Store daemon start time at server initialization
- [ ] Create helper function to format duration as human-readable (e.g., "2h30m")
- [ ] Ensure atomic access to start time (no race conditions)

### Task 3: Wire Health Route to Router

**File:** `src/http/server.rs`

- [ ] Import health handler module
- [ ] Add `GET /health` route to router
- [ ] Ensure route is registered before fallback handler

### Task 4: Implement Degraded Status Detection (Stretch)

- [ ] Define health check interface for subsystems
- [ ] Poll critical subsystems (watcher, IPC) if available
- [ ] Aggregate issues into response when degraded
- [ ] Return "ok" if all subsystems healthy, "degraded" otherwise

### Task 5: Write Tests

**File:** `src/http/handlers/health.rs` (tests module)

- [ ] Test healthy response returns 200 with correct JSON structure
- [ ] Test uptime format is valid duration string
- [ ] Test response time is under 100ms (benchmark test)
- [ ] Test degraded status includes issues array

## Dependencies

- Story 6-1 (HTTP API Server Setup) - **DONE**
- `src/http/server.rs` exists with router infrastructure

## Definition of Done

- [ ] `GET /health` returns 200 with `{"status": "ok", "uptime": "..."}` when healthy
- [ ] Uptime reflects actual daemon running time
- [ ] Response time consistently under 100ms
- [ ] All tests pass
- [ ] Code follows project conventions (clippy, fmt)
- [ ] Handler is documented with rustdoc comments

## Out of Scope

- Kubernetes liveness/readiness probes (separate endpoints)
- Detailed metrics (handled by Epic 7)
- Authentication on health endpoint (should be open)

## Notes

- Health endpoint is critical for production deployments
- Keep it simple and fast - no blocking operations
- Consider future expansion for readiness vs liveness distinction
