# Story 7.2: Core Metrics Implementation

## Story Overview

**Epic:** 7 - Observability & Metrics
**Story ID:** 7-2
**Title:** Core Metrics Implementation
**Status:** ready-for-dev
**Priority:** Growth
**Story Points:** 5

## User Story

As an operator,
I want core daemon metrics,
So that I can monitor daemon health and activity.

## Background

This story builds on Story 7.1 (Prometheus Metrics Endpoint) to implement the comprehensive set of counters, gauges, and histograms needed for full daemon observability. It implements FR35 (Prometheus metrics export) with the complete metrics suite that enables operators to monitor resume operations, failure rates, session activity, and latency characteristics.

## Acceptance Criteria

### AC1: Counter metrics are exposed
**Given** the metrics system is initialized
**When** metrics are collected via GET `/api/v1/metrics`
**Then** response includes counters:
- `palingenesis_resumes_total` - Total number of resume operations attempted
- `palingenesis_resumes_success_total` - Total number of successful resumes
- `palingenesis_resumes_failure_total` - Total number of failed resume attempts
- `palingenesis_sessions_started_total` - Total number of sessions started
- `palingenesis_rate_limits_total` - Total number of rate limit events detected
- `palingenesis_context_exhaustions_total` - Total number of context exhaustion events

### AC2: Gauge metrics are exposed
**Given** the metrics system is initialized
**When** metrics are collected via GET `/api/v1/metrics`
**Then** response includes gauges:
- `palingenesis_daemon_state` - Current daemon state (1=monitoring, 2=paused, 3=waiting, 4=resuming)
- `palingenesis_current_session_steps_completed` - Steps completed in current session
- `palingenesis_current_session_steps_total` - Total steps in current session (if known)
- `palingenesis_active_sessions` - Number of currently monitored sessions (0 or 1)
- `palingenesis_retry_attempts` - Current retry attempt number (0 if not retrying)

### AC3: Histogram metrics are exposed
**Given** the metrics system is initialized
**When** metrics are collected via GET `/api/v1/metrics`
**Then** response includes histograms:
- `palingenesis_resume_duration_seconds` - Time taken for resume operations
- `palingenesis_detection_latency_seconds` - Time from session stop to detection
- `palingenesis_wait_duration_seconds` - Time spent waiting (rate limit backoff)

### AC4: Histograms have appropriate buckets
**Given** histogram metrics are defined
**When** buckets are configured
**Then** `resume_duration_seconds` uses buckets: [0.1, 0.5, 1.0, 2.0, 5.0, 10.0, 30.0, 60.0]
**And** `detection_latency_seconds` uses buckets: [0.01, 0.05, 0.1, 0.5, 1.0, 2.0, 5.0]
**And** `wait_duration_seconds` uses buckets: [1.0, 5.0, 10.0, 30.0, 60.0, 120.0, 300.0, 600.0]

### AC5: Metrics are updated in real-time
**Given** a resume operation completes successfully
**When** metrics are scraped immediately after
**Then** `palingenesis_resumes_total` has incremented by 1
**And** `palingenesis_resumes_success_total` has incremented by 1
**And** `palingenesis_resume_duration_seconds` histogram has a new observation

**Given** a session stop is detected
**When** metrics are scraped
**Then** `palingenesis_detection_latency_seconds` has a new observation
**And** appropriate counter is incremented based on stop reason

### AC6: Metrics have proper labels
**Given** counters track different types of events
**When** events occur
**Then** `palingenesis_resumes_total` includes label `reason` (rate_limit, context_exhausted, manual)
**And** `palingenesis_resumes_failure_total` includes label `error_type` (timeout, spawn_failed, etc.)

### AC7: Metrics are thread-safe
**Given** multiple async tasks update metrics concurrently
**When** the daemon is under load
**Then** no data races occur
**And** metrics values are consistent
**And** no panics occur from metric operations

## Technical Notes

### Implementation Location
- `src/telemetry/metrics.rs` - Expand existing metrics module from Story 7.1
- `src/daemon/mod.rs` - Add metric instrumentation to daemon operations
- `src/resume/mod.rs` - Add metric instrumentation to resume strategies
- `src/monitor/mod.rs` - Add metric instrumentation to session detection

### Dependencies
- Story 7.1 (Prometheus Metrics Endpoint) - provides metrics infrastructure
- `prometheus` crate (0.13.x) - already added in Story 7.1
- Existing daemon state machine from Epic 1
- Existing monitor/resume modules from Epic 2-3

### Architecture Alignment
- Implements: FR35 (Prometheus metrics export)
- Uses: ARCH9 (tracing for structured logging alongside metrics)
- Follows Prometheus naming conventions from Story 7.1

### Implementation Approach

1. **Expand MetricsCollector** in `src/telemetry/metrics.rs`:

```rust
pub struct MetricsCollector {
    // Existing from 7.1
    pub info: IntGauge,
    pub daemon_state: IntGauge,
    pub uptime_seconds: Gauge,
    
    // New counters
    pub resumes_total: IntCounterVec,
    pub resumes_success_total: IntCounter,
    pub resumes_failure_total: IntCounterVec,
    pub sessions_started_total: IntCounter,
    pub rate_limits_total: IntCounter,
    pub context_exhaustions_total: IntCounter,
    
    // New gauges
    pub current_session_steps_completed: IntGauge,
    pub current_session_steps_total: IntGauge,
    pub active_sessions: IntGauge,
    pub retry_attempts: IntGauge,
    
    // New histograms
    pub resume_duration_seconds: Histogram,
    pub detection_latency_seconds: Histogram,
    pub wait_duration_seconds: Histogram,
}
```

2. **Define histogram buckets**:

```rust
use prometheus::{exponential_buckets, linear_buckets, Histogram, HistogramOpts};

fn create_resume_duration_histogram() -> Histogram {
    let opts = HistogramOpts::new(
        "palingenesis_resume_duration_seconds",
        "Time taken for resume operations"
    ).buckets(vec![0.1, 0.5, 1.0, 2.0, 5.0, 10.0, 30.0, 60.0]);
    Histogram::with_opts(opts).unwrap()
}

fn create_detection_latency_histogram() -> Histogram {
    let opts = HistogramOpts::new(
        "palingenesis_detection_latency_seconds",
        "Time from session stop to detection"
    ).buckets(vec![0.01, 0.05, 0.1, 0.5, 1.0, 2.0, 5.0]);
    Histogram::with_opts(opts).unwrap()
}

fn create_wait_duration_histogram() -> Histogram {
    let opts = HistogramOpts::new(
        "palingenesis_wait_duration_seconds",
        "Time spent waiting for rate limit backoff"
    ).buckets(vec![1.0, 5.0, 10.0, 30.0, 60.0, 120.0, 300.0, 600.0]);
    Histogram::with_opts(opts).unwrap()
}
```

3. **Create labeled counters**:

```rust
use prometheus::{IntCounterVec, Opts};

fn create_resumes_total() -> IntCounterVec {
    let opts = Opts::new(
        "palingenesis_resumes_total",
        "Total number of resume operations attempted"
    );
    IntCounterVec::new(opts, &["reason"]).unwrap()
}

fn create_resumes_failure_total() -> IntCounterVec {
    let opts = Opts::new(
        "palingenesis_resumes_failure_total", 
        "Total number of failed resume attempts"
    );
    IntCounterVec::new(opts, &["error_type"]).unwrap()
}
```

4. **Add instrumentation helper trait**:

```rust
pub trait Instrumented {
    fn record_resume_started(&self, reason: &str);
    fn record_resume_completed(&self, duration: Duration, success: bool, error: Option<&str>);
    fn record_detection(&self, latency: Duration, stop_reason: &str);
    fn record_wait(&self, duration: Duration);
}

impl Instrumented for MetricsCollector {
    fn record_resume_started(&self, reason: &str) {
        self.resumes_total.with_label_values(&[reason]).inc();
    }
    
    fn record_resume_completed(&self, duration: Duration, success: bool, error: Option<&str>) {
        self.resume_duration_seconds.observe(duration.as_secs_f64());
        if success {
            self.resumes_success_total.inc();
        } else if let Some(err) = error {
            self.resumes_failure_total.with_label_values(&[err]).inc();
        }
    }
    
    fn record_detection(&self, latency: Duration, stop_reason: &str) {
        self.detection_latency_seconds.observe(latency.as_secs_f64());
        match stop_reason {
            "rate_limit" => self.rate_limits_total.inc(),
            "context_exhausted" => self.context_exhaustions_total.inc(),
            _ => {}
        }
    }
    
    fn record_wait(&self, duration: Duration) {
        self.wait_duration_seconds.observe(duration.as_secs_f64());
    }
}
```

5. **Integrate into resume strategies** (`src/resume/same_session.rs`):

```rust
impl ResumeStrategy for SameSessionStrategy {
    async fn execute(&self, ctx: &ResumeContext) -> Result<ResumeOutcome> {
        let start = Instant::now();
        ctx.metrics.record_resume_started("rate_limit");
        
        // Wait for rate limit
        let wait_start = Instant::now();
        self.wait_for_rate_limit(ctx).await?;
        ctx.metrics.record_wait(wait_start.elapsed());
        
        // Execute resume
        let result = self.do_resume(ctx).await;
        ctx.metrics.record_resume_completed(
            start.elapsed(),
            result.is_ok(),
            result.as_ref().err().map(|e| e.error_type())
        );
        
        result
    }
}
```

6. **Integrate into monitor** (`src/monitor/classifier.rs`):

```rust
impl StopClassifier {
    pub fn classify(&self, event: &SessionEvent, metrics: &MetricsCollector) -> StopReason {
        let detection_latency = event.detected_at - event.stopped_at;
        let reason = self.do_classify(event);
        metrics.record_detection(detection_latency, reason.as_str());
        reason
    }
}
```

### Prometheus Output Example

```
# HELP palingenesis_resumes_total Total number of resume operations attempted
# TYPE palingenesis_resumes_total counter
palingenesis_resumes_total{reason="rate_limit"} 15
palingenesis_resumes_total{reason="context_exhausted"} 3
palingenesis_resumes_total{reason="manual"} 2

# HELP palingenesis_resumes_success_total Total number of successful resumes
# TYPE palingenesis_resumes_success_total counter
palingenesis_resumes_success_total 18

# HELP palingenesis_resumes_failure_total Total number of failed resume attempts
# TYPE palingenesis_resumes_failure_total counter
palingenesis_resumes_failure_total{error_type="timeout"} 1
palingenesis_resumes_failure_total{error_type="spawn_failed"} 1

# HELP palingenesis_sessions_started_total Total number of sessions started
# TYPE palingenesis_sessions_started_total counter
palingenesis_sessions_started_total 20

# HELP palingenesis_rate_limits_total Total number of rate limit events detected
# TYPE palingenesis_rate_limits_total counter
palingenesis_rate_limits_total 15

# HELP palingenesis_context_exhaustions_total Total number of context exhaustion events
# TYPE palingenesis_context_exhaustions_total counter
palingenesis_context_exhaustions_total 3

# HELP palingenesis_daemon_state Current state of the daemon
# TYPE palingenesis_daemon_state gauge
palingenesis_daemon_state 1

# HELP palingenesis_current_session_steps_completed Steps completed in current session
# TYPE palingenesis_current_session_steps_completed gauge
palingenesis_current_session_steps_completed 7

# HELP palingenesis_current_session_steps_total Total steps in current session
# TYPE palingenesis_current_session_steps_total gauge
palingenesis_current_session_steps_total 12

# HELP palingenesis_active_sessions Number of currently monitored sessions
# TYPE palingenesis_active_sessions gauge
palingenesis_active_sessions 1

# HELP palingenesis_retry_attempts Current retry attempt number
# TYPE palingenesis_retry_attempts gauge
palingenesis_retry_attempts 0

# HELP palingenesis_resume_duration_seconds Time taken for resume operations
# TYPE palingenesis_resume_duration_seconds histogram
palingenesis_resume_duration_seconds_bucket{le="0.1"} 5
palingenesis_resume_duration_seconds_bucket{le="0.5"} 10
palingenesis_resume_duration_seconds_bucket{le="1"} 15
palingenesis_resume_duration_seconds_bucket{le="2"} 18
palingenesis_resume_duration_seconds_bucket{le="5"} 19
palingenesis_resume_duration_seconds_bucket{le="10"} 20
palingenesis_resume_duration_seconds_bucket{le="30"} 20
palingenesis_resume_duration_seconds_bucket{le="60"} 20
palingenesis_resume_duration_seconds_bucket{le="+Inf"} 20
palingenesis_resume_duration_seconds_sum 25.5
palingenesis_resume_duration_seconds_count 20

# HELP palingenesis_detection_latency_seconds Time from session stop to detection
# TYPE palingenesis_detection_latency_seconds histogram
palingenesis_detection_latency_seconds_bucket{le="0.01"} 0
palingenesis_detection_latency_seconds_bucket{le="0.05"} 2
palingenesis_detection_latency_seconds_bucket{le="0.1"} 10
palingenesis_detection_latency_seconds_bucket{le="0.5"} 18
palingenesis_detection_latency_seconds_bucket{le="1"} 20
palingenesis_detection_latency_seconds_bucket{le="2"} 20
palingenesis_detection_latency_seconds_bucket{le="5"} 20
palingenesis_detection_latency_seconds_bucket{le="+Inf"} 20
palingenesis_detection_latency_seconds_sum 4.2
palingenesis_detection_latency_seconds_count 20

# HELP palingenesis_wait_duration_seconds Time spent waiting for rate limit backoff
# TYPE palingenesis_wait_duration_seconds histogram
palingenesis_wait_duration_seconds_bucket{le="1"} 0
palingenesis_wait_duration_seconds_bucket{le="5"} 0
palingenesis_wait_duration_seconds_bucket{le="10"} 2
palingenesis_wait_duration_seconds_bucket{le="30"} 5
palingenesis_wait_duration_seconds_bucket{le="60"} 12
palingenesis_wait_duration_seconds_bucket{le="120"} 15
palingenesis_wait_duration_seconds_bucket{le="300"} 15
palingenesis_wait_duration_seconds_bucket{le="600"} 15
palingenesis_wait_duration_seconds_bucket{le="+Inf"} 15
palingenesis_wait_duration_seconds_sum 750.0
palingenesis_wait_duration_seconds_count 15
```

## Definition of Done

- [ ] MetricsCollector expanded with all counter metrics
- [ ] MetricsCollector expanded with all gauge metrics  
- [ ] MetricsCollector expanded with all histogram metrics
- [ ] Histogram buckets configured appropriately
- [ ] Labels defined for resumes_total and resumes_failure_total
- [ ] Instrumented trait implemented for metric recording
- [ ] Resume strategies instrumented with metric calls
- [ ] Monitor/classifier instrumented with metric calls
- [ ] Session state changes update gauge metrics
- [ ] Unit tests for metric registration and incrementing
- [ ] Integration test verifying all metrics appear in output
- [ ] Thread-safety verified under concurrent load
- [ ] All tests pass (`cargo nextest run`)
- [ ] No clippy warnings (`cargo clippy`)
- [ ] Code formatted (`cargo fmt`)

## Test Scenarios

### Unit Tests
1. Test counter increments correctly for each event type
2. Test gauge updates reflect correct state values
3. Test histogram observations are recorded in correct buckets
4. Test labeled counters with different label values
5. Test concurrent metric updates don't cause races

### Integration Tests
1. GET `/api/v1/metrics` includes all defined metrics
2. After simulated resume, counters are incremented
3. After simulated wait, histogram has observations
4. State transitions update daemon_state gauge
5. Session changes update session-related gauges
6. Verify prometheus crate can parse all metric output

### Performance Tests
1. 1000 metric updates complete in <100ms
2. Metric collection doesn't block daemon operations
3. Memory usage stable under continuous metric updates

## Related Stories

- **Story 7.1**: Prometheus Metrics Endpoint (provides foundation)
- **Story 7.3**: Time Saved Metric (adds time_saved_seconds metric)
- **Story 7.4**: Saves Count Metric (adds saves_total counter)
- **Story 7.7**: Grafana Dashboard Template (visualizes these metrics)

## Dependencies

### Upstream Dependencies
- Story 7.1 (Prometheus Metrics Endpoint) - provides metrics infrastructure and registry

### Downstream Dependencies
- Story 7.3 (Time Saved Metric) - extends metrics with time_saved
- Story 7.4 (Saves Count Metric) - extends metrics with saves_total
- Story 7.7 (Grafana Dashboard Template) - requires these metrics for visualization

---
*Generated: 2026-02-06*
*Epic Reference: _bmad-output/planning-artifacts/epics.md#story-72-core-metrics-implementation*
