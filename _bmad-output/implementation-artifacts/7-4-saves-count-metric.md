# Story 7.4: Saves Count Metric

## Story Overview

**Epic:** 7 - Observability & Metrics
**Story ID:** 7-4
**Title:** Saves Count Metric
**Status:** done
**Priority:** Growth
**Story Points:** 2

## User Story

As a user,
I want to see how many times palingenesis has saved my work,
So that I can see its impact at a glance.

## Background

This story builds on Story 7.2 (Core Metrics Implementation) and complements Story 7.3 (Time Saved Metric) to provide a simple "saves count" metric. While time saved gives a duration-based value metric, saves count provides a discrete count that's easy to understand at a glance. Each successful resume operation represents one "save" - a moment when palingenesis automatically recovered work that would otherwise have required manual intervention. It implements FR40 (Saves count metric).

## Acceptance Criteria

### AC1: Saves count increments on successful resume
**Given** a successful resume operation completes
**When** saves count is updated
**Then** `stats.saves_count` is incremented by 1
**And** the increment is persisted to state file

### AC2: Status command displays saves count
**Given** the daemon is running with successful resumes
**When** I run `palingenesis status`
**Then** output includes "Saves: N" where N is the total saves count
**And** the format is consistent with other status output

### AC3: Prometheus metrics endpoint includes saves count
**Given** the metrics endpoint is queried
**When** GET `/api/v1/metrics` is called
**Then** response includes `palingenesis_saves_total` counter
**And** the counter value matches the state saves_count

### AC4: Saves count persists across daemon restarts
**Given** cumulative saves count is tracked
**When** daemon restarts
**Then** previous cumulative value is loaded from state file
**And** new saves are added to the existing total

### AC5: Weekly summary includes saves count (if notifications enabled)
**Given** notifications are configured with weekly summary enabled
**When** the weekly summary is sent
**Then** it includes "Saves this week: N"
**And** the weekly count is calculated from audit trail or separate weekly counter

### AC6: Status JSON includes saves count
**Given** the daemon is running
**When** I run `palingenesis status --json`
**Then** JSON output includes `"saves_count": <number>`

### AC7: IPC STATUS response includes saves count
**Given** the daemon is running
**When** CLI sends STATUS command via IPC
**Then** response JSON includes `saves_count` field

### AC8: Saves count is accurate across resume types
**Given** different resume strategies (same-session, new-session)
**When** any resume succeeds
**Then** saves_count is incremented regardless of strategy type

## Technical Notes

### Implementation Location
- `src/state/schema.rs` - Add/ensure saves_count field in Stats struct
- `src/telemetry/metrics.rs` - Add palingenesis_saves_total counter
- `src/resume/mod.rs` - Increment saves count after successful resume
- `src/cli/commands/status.rs` - Display saves count in status output
- `src/ipc/protocol.rs` - Include saves count in STATUS response
- `src/notify/dispatcher.rs` - Include saves count in weekly summary

### Dependencies
- Story 7.1 (Prometheus Metrics Endpoint) - provides metrics infrastructure
- Story 7.2 (Core Metrics Implementation) - provides metrics patterns
- Story 7.3 (Time Saved Metric) - related implementation patterns
- Story 1.4 (State Persistence Layer) - for persisting cumulative saves count
- `prometheus` crate (0.13.x) - for metrics

### Architecture Alignment
- Implements: FR40 (Saves count metric)
- Extends: ARCH10 (State persistence) with saves_count field
- Uses: ARCH23 (Response format) for API responses

### Implementation Approach

1. **Verify/add state field** in `src/state/schema.rs`:

```rust
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Stats {
    pub total_resumes: u64,
    pub successful_resumes: u64,
    pub failed_resumes: u64,
    pub time_saved_seconds: f64,
    /// Total number of successful saves (same as successful_resumes)
    pub saves_count: u64,
    // ... other fields
}
```

Note: `saves_count` may alias `successful_resumes` or be a distinct counter if we want to differentiate. For simplicity, we can use `successful_resumes` as the saves count, or maintain a separate field for clarity.

2. **Add Prometheus metric** in `src/telemetry/metrics.rs`:

```rust
impl MetricsCollector {
    // ... existing fields
    
    /// Total number of saves (successful resumes)
    pub saves_total: Counter,
}

impl MetricsCollector {
    pub fn new() -> Self {
        // ... existing initialization
        
        let saves_total = Counter::new(
            "palingenesis_saves_total",
            "Total number of times palingenesis saved work by automatically resuming"
        ).unwrap();
        
        // Register metric
        // ...
        
        Self {
            // ... existing fields
            saves_total,
        }
    }
}
```

3. **Increment on successful resume** (`src/resume/mod.rs`):

```rust
pub fn record_save(
    metrics: &MetricsCollector,
    state: &mut DaemonState,
) {
    // Increment Prometheus counter
    metrics.saves_total.inc();
    
    // Increment persistent state
    state.stats.saves_count += 1;
    
    tracing::info!(
        saves_count = state.stats.saves_count,
        "Save recorded"
    );
}
```

4. **Display in status** (`src/cli/commands/status.rs`):

```rust
// In status display:
println!("Saves: {}", stats.saves_count);
```

5. **Update IPC protocol** (`src/ipc/protocol.rs`):

```rust
#[derive(Debug, Serialize, Deserialize)]
pub struct StatsResponse {
    pub total_resumes: u64,
    pub successful_resumes: u64,
    pub saves_count: u64,
    pub time_saved_seconds: f64,
    pub time_saved_human: String,
}
```

6. **Weekly summary** (if notifications enabled):

```rust
// In weekly summary notification:
format!(
    "Weekly Summary:\n- Saves this week: {}\n- Time saved: {}",
    weekly_saves,
    format_duration(weekly_time_saved)
)
```

### Prometheus Output Example

```
# HELP palingenesis_saves_total Total number of times palingenesis saved work by automatically resuming
# TYPE palingenesis_saves_total counter
palingenesis_saves_total 42
```

### CLI Output Example

```
$ palingenesis status
palingenesis daemon: running (PID: 12345)
State: monitoring
Uptime: 2h 30m
Current session: /home/user/.opencode/session.md
Steps completed: 5/12
Saves: 42
Time saved: 4.2 hours
```

### JSON Output Example

```json
{
  "state": "monitoring",
  "pid": 12345,
  "uptime_seconds": 9000,
  "current_session": {
    "path": "/home/user/.opencode/session.md",
    "steps_completed": 5,
    "steps_total": 12
  },
  "stats": {
    "total_resumes": 45,
    "successful_resumes": 42,
    "failed_resumes": 3,
    "saves_count": 42,
    "time_saved_seconds": 15120,
    "time_saved_human": "4.2 hours"
  }
}
```

## Definition of Done

- [ ] State schema has saves_count field (or verified existing field usage)
- [ ] `palingenesis_saves_total` counter metric added to metrics collector
- [ ] Saves count incremented after each successful resume
- [ ] Saves count persisted to state file
- [ ] Saves count loaded on daemon startup
- [ ] Status command displays "Saves: N"
- [ ] Status --json includes saves_count field
- [ ] IPC STATUS response includes saves_count
- [ ] GET /api/v1/metrics includes palingenesis_saves_total
- [ ] Weekly summary includes saves count (if notifications configured)
- [ ] Unit tests for saves count increment
- [ ] Integration test verifying metric in Prometheus output
- [ ] Integration test verifying persistence across restart
- [ ] All tests pass (`cargo nextest run`)
- [ ] No clippy warnings (`cargo clippy`)
- [ ] Code formatted (`cargo fmt`)

## Test Scenarios

### Unit Tests
1. Test saves_count increments on record_save() call
2. Test state serialization/deserialization with saves_count
3. Test saves_count starts at 0 for new state
4. Test saves_count accumulates correctly across multiple saves

### Integration Tests
1. After simulated successful resume, saves_total metric increments
2. After daemon restart, cumulative saves_count persists from state file
3. GET `/api/v1/metrics` includes palingenesis_saves_total counter
4. `palingenesis status` output includes "Saves: N"
5. `palingenesis status --json` includes saves_count field
6. IPC STATUS response includes saves_count

### Edge Cases
1. First save (no prior saves) - starts from 0, becomes 1
2. Failed resume - saves_count should NOT increment
3. Corrupted state file - saves_count defaults to 0
4. Metric value matches state value after restart

## Related Stories

- **Story 7.1**: Prometheus Metrics Endpoint (provides metrics infrastructure)
- **Story 7.2**: Core Metrics Implementation (provides metrics patterns)
- **Story 7.3**: Time Saved Metric (companion value metric)
- **Story 7.7**: Grafana Dashboard Template (will visualize saves count)

## Dependencies

### Upstream Dependencies
- Story 7.1 (Prometheus Metrics Endpoint) - provides metrics infrastructure
- Story 7.2 (Core Metrics Implementation) - provides counter patterns
- Story 7.3 (Time Saved Metric) - may share implementation patterns

### Downstream Dependencies
- Story 7.7 (Grafana Dashboard Template) - will include saves count visualization

---
*Generated: 2026-02-06*
*Epic Reference: _bmad-output/planning-artifacts/epics.md#story-74-saves-count-metric*
