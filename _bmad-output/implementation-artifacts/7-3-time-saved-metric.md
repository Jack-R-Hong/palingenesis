# Story 7.3: Time Saved Metric

## Story Overview

**Epic:** 7 - Observability & Metrics
**Story ID:** 7-3
**Title:** Time Saved Metric
**Status:** ready-for-dev
**Priority:** Growth
**Story Points:** 3

## User Story

As a user,
I want to see how much time palingenesis has saved me,
So that I can quantify its value.

## Background

This story builds on Story 7.2 (Core Metrics Implementation) to add a "time saved" metric that estimates the cumulative time savings from automatic session resumption. This is a key value metric that helps users understand the ROI of running palingenesis. It implements FR39 (Time saved metric) by tracking each successful resume and calculating estimated time that would have been spent manually detecting and restarting sessions.

## Acceptance Criteria

### AC1: Time saved is calculated on successful resume
**Given** a successful resume operation completes
**When** time saved is calculated
**Then** it estimates: `actual_wait_duration + manual_restart_time`
**And** `manual_restart_time` defaults to 5 minutes (300 seconds)
**And** the calculated value is added to cumulative time saved

### AC2: Manual restart time is configurable
**Given** configuration file has `[metrics]` section
**When** `manual_restart_time_seconds` is specified
**Then** that value is used instead of default 300 seconds
**And** valid range is 60-1800 seconds (1-30 minutes)

### AC3: Time saved metric is exposed via Prometheus
**Given** the metrics endpoint is queried
**When** GET `/api/v1/metrics` is called
**Then** response includes:
- `palingenesis_time_saved_seconds_total` (counter) - Total seconds saved
- `palingenesis_time_saved_per_resume_seconds` (histogram) - Time saved per individual resume

### AC4: Time saved is persisted across daemon restarts
**Given** cumulative time saved is tracked
**When** daemon restarts
**Then** previous cumulative value is loaded from state file
**And** new saves are added to the existing total

### AC5: Status command displays time saved
**Given** the daemon is running with successful resumes
**When** I run `palingenesis status`
**Then** output includes "Time saved: X.X hours"
**And** format automatically selects appropriate unit (minutes/hours/days)

### AC6: Status JSON includes time saved
**Given** the daemon is running
**When** I run `palingenesis status --json`
**Then** JSON output includes `"time_saved_seconds": <number>`
**And** JSON output includes `"time_saved_human": "<formatted string>"`

### AC7: Time saved is included in IPC STATUS response
**Given** the daemon is running
**When** CLI sends STATUS command via IPC
**Then** response JSON includes `time_saved_seconds` field

### AC8: Time saved calculation is accurate
**Given** a resume after 60 second wait
**When** time saved is calculated with default 5 minute manual restart
**Then** time saved for that resume = 60 + 300 = 360 seconds
**And** this is added to cumulative total

## Technical Notes

### Implementation Location
- `src/telemetry/metrics.rs` - Add time_saved_seconds_total counter and histogram
- `src/state/schema.rs` - Add time_saved_seconds field to state
- `src/resume/mod.rs` - Calculate time saved after successful resume
- `src/config/schema.rs` - Add manual_restart_time_seconds config option
- `src/cli/commands/status.rs` - Display time saved in status output
- `src/ipc/protocol.rs` - Include time saved in STATUS response

### Dependencies
- Story 7.1 (Prometheus Metrics Endpoint) - provides metrics infrastructure
- Story 7.2 (Core Metrics Implementation) - provides metrics patterns
- Story 1.4 (State Persistence Layer) - for persisting cumulative time saved
- `prometheus` crate (0.13.x) - for metrics
- `humantime` crate - for human-readable duration formatting

### Architecture Alignment
- Implements: FR39 (Time saved metric)
- Extends: ARCH10 (State persistence) with time_saved_seconds field
- Uses: ARCH23 (Response format) for API responses

### Implementation Approach

1. **Add config option** in `src/config/schema.rs`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsConfig {
    /// Enable Prometheus metrics endpoint
    #[serde(default = "default_metrics_enabled")]
    pub enabled: bool,
    
    /// Estimated time for manual session restart (seconds)
    /// Used in time_saved calculation. Default: 300 (5 minutes)
    #[serde(default = "default_manual_restart_time")]
    pub manual_restart_time_seconds: u64,
}

fn default_manual_restart_time() -> u64 {
    300 // 5 minutes
}
```

2. **Add state field** in `src/state/schema.rs`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Stats {
    pub total_resumes: u64,
    pub successful_resumes: u64,
    pub failed_resumes: u64,
    /// Cumulative time saved in seconds
    pub time_saved_seconds: f64,
    // ... existing fields
}
```

3. **Add metrics** in `src/telemetry/metrics.rs`:

```rust
impl MetricsCollector {
    // ... existing fields
    
    /// Total time saved in seconds (counter)
    pub time_saved_seconds_total: Counter,
    
    /// Time saved per individual resume (histogram)
    pub time_saved_per_resume_seconds: Histogram,
}

impl MetricsCollector {
    pub fn new() -> Self {
        // ... existing initialization
        
        let time_saved_seconds_total = Counter::new(
            "palingenesis_time_saved_seconds_total",
            "Total estimated time saved by automatic resumption"
        ).unwrap();
        
        let time_saved_per_resume_seconds = Histogram::with_opts(
            HistogramOpts::new(
                "palingenesis_time_saved_per_resume_seconds",
                "Time saved per individual resume operation"
            ).buckets(vec![60.0, 120.0, 180.0, 300.0, 600.0, 900.0, 1800.0])
        ).unwrap();
        
        // Register metrics
        // ...
        
        Self {
            // ... existing fields
            time_saved_seconds_total,
            time_saved_per_resume_seconds,
        }
    }
}
```

4. **Calculate time saved in resume** (`src/resume/mod.rs`):

```rust
pub struct TimeSavedCalculation {
    pub wait_duration_seconds: f64,
    pub manual_restart_seconds: f64,
    pub total_saved_seconds: f64,
}

impl TimeSavedCalculation {
    pub fn calculate(
        wait_duration: Duration,
        config: &MetricsConfig,
    ) -> Self {
        let wait_seconds = wait_duration.as_secs_f64();
        let manual_restart = config.manual_restart_time_seconds as f64;
        
        Self {
            wait_duration_seconds: wait_seconds,
            manual_restart_seconds: manual_restart,
            total_saved_seconds: wait_seconds + manual_restart,
        }
    }
}

// After successful resume:
pub fn record_time_saved(
    metrics: &MetricsCollector,
    state: &mut DaemonState,
    wait_duration: Duration,
    config: &MetricsConfig,
) {
    let calc = TimeSavedCalculation::calculate(wait_duration, config);
    
    // Update Prometheus metrics
    metrics.time_saved_seconds_total.inc_by(calc.total_saved_seconds);
    metrics.time_saved_per_resume_seconds.observe(calc.total_saved_seconds);
    
    // Update persistent state
    state.stats.time_saved_seconds += calc.total_saved_seconds;
    
    tracing::info!(
        wait_seconds = calc.wait_duration_seconds,
        manual_restart_seconds = calc.manual_restart_seconds,
        total_saved = calc.total_saved_seconds,
        cumulative_saved = state.stats.time_saved_seconds,
        "Time saved by resume"
    );
}
```

5. **Format time saved for display** (`src/cli/commands/status.rs`):

```rust
fn format_time_saved(seconds: f64) -> String {
    if seconds < 60.0 {
        format!("{:.0} seconds", seconds)
    } else if seconds < 3600.0 {
        format!("{:.1} minutes", seconds / 60.0)
    } else if seconds < 86400.0 {
        format!("{:.1} hours", seconds / 3600.0)
    } else {
        format!("{:.1} days", seconds / 86400.0)
    }
}

// In status display:
println!("Time saved: {}", format_time_saved(stats.time_saved_seconds));
```

6. **Update IPC protocol** (`src/ipc/protocol.rs`):

```rust
#[derive(Debug, Serialize, Deserialize)]
pub struct StatusResponse {
    pub state: DaemonStateKind,
    pub uptime_seconds: u64,
    pub current_session: Option<SessionInfo>,
    pub stats: StatsResponse,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StatsResponse {
    pub total_resumes: u64,
    pub successful_resumes: u64,
    pub time_saved_seconds: f64,
    pub time_saved_human: String,
}
```

### Prometheus Output Example

```
# HELP palingenesis_time_saved_seconds_total Total estimated time saved by automatic resumption
# TYPE palingenesis_time_saved_seconds_total counter
palingenesis_time_saved_seconds_total 15120

# HELP palingenesis_time_saved_per_resume_seconds Time saved per individual resume operation
# TYPE palingenesis_time_saved_per_resume_seconds histogram
palingenesis_time_saved_per_resume_seconds_bucket{le="60"} 0
palingenesis_time_saved_per_resume_seconds_bucket{le="120"} 0
palingenesis_time_saved_per_resume_seconds_bucket{le="180"} 0
palingenesis_time_saved_per_resume_seconds_bucket{le="300"} 2
palingenesis_time_saved_per_resume_seconds_bucket{le="600"} 35
palingenesis_time_saved_per_resume_seconds_bucket{le="900"} 40
palingenesis_time_saved_per_resume_seconds_bucket{le="1800"} 42
palingenesis_time_saved_per_resume_seconds_bucket{le="+Inf"} 42
palingenesis_time_saved_per_resume_seconds_sum 15120
palingenesis_time_saved_per_resume_seconds_count 42
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
    "total_resumes": 42,
    "successful_resumes": 42,
    "failed_resumes": 0,
    "time_saved_seconds": 15120,
    "time_saved_human": "4.2 hours"
  }
}
```

## Definition of Done

- [ ] Config option `manual_restart_time_seconds` added with validation (60-1800)
- [ ] State schema updated with `time_saved_seconds` field
- [ ] `palingenesis_time_saved_seconds_total` counter metric added
- [ ] `palingenesis_time_saved_per_resume_seconds` histogram metric added
- [ ] Time saved calculated and recorded after each successful resume
- [ ] Cumulative time saved persisted to state file
- [ ] Cumulative time saved loaded on daemon startup
- [ ] Status command displays time saved in human-readable format
- [ ] Status --json includes time_saved_seconds and time_saved_human
- [ ] IPC STATUS response includes time saved
- [ ] Unit tests for time saved calculation
- [ ] Unit tests for duration formatting
- [ ] Integration test verifying metric in Prometheus output
- [ ] Integration test verifying persistence across restart
- [ ] All tests pass (`cargo nextest run`)
- [ ] No clippy warnings (`cargo clippy`)
- [ ] Code formatted (`cargo fmt`)

## Test Scenarios

### Unit Tests
1. Test time saved calculation with default config (wait + 300s)
2. Test time saved calculation with custom manual_restart_time
3. Test config validation rejects values outside 60-1800 range
4. Test format_time_saved produces correct units (seconds/minutes/hours/days)
5. Test state serialization/deserialization with time_saved_seconds

### Integration Tests
1. After simulated successful resume, time_saved_seconds_total increments
2. After daemon restart, cumulative time_saved persists from state file
3. GET `/api/v1/metrics` includes time_saved metrics
4. `palingenesis status` output includes "Time saved: X.X hours"
5. `palingenesis status --json` includes time_saved_seconds field
6. IPC STATUS response includes time_saved_seconds

### Edge Cases
1. First resume (no prior time saved) - starts from 0
2. Very long wait duration (>30 minutes) - correctly calculated
3. Config reload with new manual_restart_time - new value used for future resumes
4. Corrupted state file - time_saved_seconds defaults to 0

## Related Stories

- **Story 7.1**: Prometheus Metrics Endpoint (provides metrics infrastructure)
- **Story 7.2**: Core Metrics Implementation (provides metrics patterns)
- **Story 7.4**: Saves Count Metric (companion value metric)
- **Story 7.7**: Grafana Dashboard Template (will visualize time saved)

## Dependencies

### Upstream Dependencies
- Story 7.1 (Prometheus Metrics Endpoint) - provides metrics infrastructure
- Story 7.2 (Core Metrics Implementation) - provides counter/histogram patterns

### Downstream Dependencies
- Story 7.7 (Grafana Dashboard Template) - will include time saved visualization

---
*Generated: 2026-02-06*
*Epic Reference: _bmad-output/planning-artifacts/epics.md#story-73-time-saved-metric*
