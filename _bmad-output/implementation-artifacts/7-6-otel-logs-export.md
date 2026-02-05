# Story 7.6: OTEL Logs Export

## Story Overview

**Epic:** 7 - Observability & Metrics
**Story ID:** 7-6
**Title:** OTEL Logs Export
**Status:** done
**Priority:** Growth
**Story Points:** 2

## User Story

As an operator,
I want structured logs exported via OTLP,
So that I have unified observability with traces and logs correlated.

## Background

This story extends the OpenTelemetry integration from Story 7-5 to include log export via OTLP. When enabled, logs emitted via the `tracing` macros are also sent to the OTLP collector alongside traces, enabling correlation between logs and traces in observability backends like Grafana Loki, Elastic, or any OTLP-compatible system. The feature is opt-in via the existing `otel` feature flag and `[otel]` config section. It implements FR37 (OTLP logs export).

## Acceptance Criteria

### AC1: OTEL logs enabled via config
**Given** OTEL is enabled with `[otel] enabled = true` and `logs = true`
**When** the daemon logs messages
**Then** logs are sent to the OTLP collector
**And** logs are also written locally (file/stderr as configured)

### AC2: Logs include trace context
**Given** OTEL logs and traces are both enabled
**When** a log is emitted within a traced span
**Then** the log includes the trace_id and span_id
**And** logs and traces are correlated in the observability backend

### AC3: Graceful degradation on export failure
**Given** OTLP endpoint is unreachable
**When** log export fails
**Then** logs are still written locally
**And** export errors don't crash the daemon
**And** a warning is logged (locally) about the export failure

### AC4: OTEL logs disabled by default
**Given** OTEL is not configured or `logs = false`
**When** the daemon runs
**Then** no log export to OTLP occurs
**And** standard local logging still works

### AC5: Log level filtering
**Given** OTEL logs are enabled
**When** logs are exported
**Then** the log level (trace, debug, info, warn, error) is preserved
**And** filtering configured via RUST_LOG still applies

## Technical Notes

### Implementation Location
- `src/telemetry/otel.rs` - Add logs layer builder
- `src/telemetry/tracing.rs` - Integrate logs layer with subscriber
- `src/config/schema.rs` - Add `logs` field to OtelConfig

### Dependencies
- Story 7-5 (OTEL Traces Export) - provides OTEL infrastructure
- `opentelemetry-appender-tracing` crate for log bridging
- Existing OTEL dependencies from Story 7-5

### Architecture Alignment
- Implements: FR37 (OTLP logs export)
- Extends: Story 7-5 OTEL infrastructure

### Config Schema Addition

Add `logs` field to existing OtelConfig:

```rust
pub struct OtelConfig {
    // ... existing fields ...
    /// Enable log export via OTLP.
    /// Example: logs = true
    #[serde(default)]
    pub logs: bool,
}
```

### Implementation Approach

1. **Add logs field to OtelConfig** with default `false`

2. **Add opentelemetry-appender-tracing dependency**:
```toml
opentelemetry-appender-tracing = { version = "0.3", optional = true }
```
Update `otel` feature to include this dependency.

3. **Create logs layer builder** in `src/telemetry/otel.rs`:
```rust
#[cfg(feature = "otel")]
pub fn build_otel_logs_layer(config: &OtelConfig) -> Option<OpenTelemetryTracingBridge<...>> {
    if !config.enabled || !config.logs {
        return None;
    }
    // Build and return the logs layer
}
```

4. **Integrate with tracing subscriber** alongside existing layers

5. **Add shutdown for logs provider**

### Config File Example

```toml
[otel]
enabled = true
endpoint = "http://localhost:4317"
service_name = "palingenesis"
traces = true
logs = true
```

## Definition of Done

- [ ] `logs` field added to OtelConfig (default: false)
- [ ] `opentelemetry-appender-tracing` added to Cargo.toml under otel feature
- [ ] Logs layer builder implemented in otel.rs
- [ ] Logs layer integrated with tracing subscriber
- [ ] Logs include trace context (trace_id, span_id)
- [ ] Graceful degradation on export failure
- [ ] Local logging unaffected by OTEL logs
- [ ] All tests pass (`cargo nextest run`)
- [ ] All tests pass with feature (`cargo nextest run --features otel`)
- [ ] No clippy warnings (`cargo clippy --features otel`)
- [ ] Code formatted (`cargo fmt`)

## Test Scenarios

### Unit Tests
1. Test OtelConfig parsing with logs = true
2. Test OtelConfig parsing with logs = false (default)
3. Test logs disabled when otel.enabled = false

### Integration Tests
1. Daemon starts with OTEL logs enabled
2. Logs are still written locally when OTEL logs enabled
3. OTEL logs disabled doesn't affect local logging

## Related Stories

- **Story 7-5**: OTEL Traces Export (provides OTEL infrastructure)
- **Story 7-7**: Grafana Dashboard Template (visualization)

---
*Generated: 2026-02-06*
*Epic Reference: _bmad-output/planning-artifacts/epics.md#story-76-otel-logs-export*
