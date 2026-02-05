# Story 7.1: Prometheus Metrics Endpoint

## Story Overview

**Epic:** 7 - Observability & Metrics
**Story ID:** 7-1
**Title:** Prometheus Metrics Endpoint
**Status:** ready-for-dev
**Priority:** Growth
**Story Points:** 3

## User Story

As an operator,
I want a Prometheus metrics endpoint,
So that I can scrape metrics into my monitoring stack.

## Background

This story implements FR35 (Prometheus metrics export) from the PRD. It provides the foundation for the observability epic by exposing a standard `/api/v1/metrics` endpoint that Prometheus can scrape. This enables integration with existing monitoring infrastructure and paves the way for Grafana dashboards (Story 7.7).

## Acceptance Criteria

### AC1: Metrics endpoint returns Prometheus text format
**Given** metrics endpoint is enabled in configuration
**When** GET `/api/v1/metrics` is called
**Then** response has Content-Type `text/plain; version=0.0.4; charset=utf-8`
**And** response body is valid Prometheus exposition format
**And** response status is 200 OK

### AC2: Metrics are parseable by Prometheus
**Given** metrics are scraped from the endpoint
**When** parsed by Prometheus server
**Then** all metrics have proper TYPE declarations (counter, gauge, histogram)
**And** all metrics have proper HELP documentation strings
**And** metrics follow Prometheus naming conventions (`palingenesis_*`)

### AC3: Metrics endpoint performance
**Given** the metrics endpoint
**When** called frequently (10 req/s)
**Then** response time is consistently <50ms
**And** endpoint does not cause memory leaks
**And** endpoint does not block other daemon operations

### AC4: Configuration controls metrics exposure
**Given** configuration file with `[otel] metrics_enabled = false`
**When** the daemon starts
**Then** the `/api/v1/metrics` endpoint returns 404 Not Found

**Given** configuration file with `[otel] metrics_enabled = true` (default)
**When** the daemon starts
**Then** the `/api/v1/metrics` endpoint is available

### AC5: Basic daemon metrics are exposed
**Given** the metrics endpoint is enabled
**When** GET `/api/v1/metrics` is called
**Then** response includes at minimum:
- `palingenesis_info` (gauge, value=1, with version label)
- `palingenesis_daemon_state` (gauge, 1=monitoring, 2=paused, 3=waiting, 4=resuming)
- `palingenesis_uptime_seconds` (gauge)
- `palingenesis_build_info` (gauge with version, commit labels)

## Technical Notes

### Implementation Location
- `src/http/handlers/metrics.rs` - Metrics endpoint handler
- `src/telemetry/metrics.rs` - Metrics registry and collection (may exist from Story 1.12)

### Dependencies
- `prometheus` crate (0.13.x) for metrics collection and encoding
- Existing HTTP server from Epic 6 (`src/http/server.rs`)
- Existing configuration system from Epic 4

### Architecture Alignment
- Implements: FR35 (Prometheus metrics export)
- Uses: ARCH6 (axum HTTP server), ARCH23 (response format)
- HTTP API port: 127.0.0.1:7654 (configurable per ARCH14)

### Implementation Approach

1. **Add prometheus dependency** to Cargo.toml:
```toml
prometheus = { version = "0.13", features = ["process"] }
```

2. **Create metrics registry** in `src/telemetry/metrics.rs`:
- Define static `Registry` for all metrics
- Create `MetricsCollector` struct to hold metric handles
- Implement lazy initialization pattern

3. **Define basic metrics**:
```rust
// Gauges
palingenesis_info{version="x.y.z"} 1
palingenesis_daemon_state 1
palingenesis_uptime_seconds <value>

// Info metric for build metadata
palingenesis_build_info{version="x.y.z",commit="abc123"} 1
```

4. **Create HTTP handler** in `src/http/handlers/metrics.rs`:
```rust
pub async fn metrics_handler(
    State(app_state): State<AppState>,
) -> impl IntoResponse {
    let encoder = TextEncoder::new();
    let metric_families = prometheus::gather();
    let mut buffer = Vec::new();
    encoder.encode(&metric_families, &mut buffer).unwrap();
    
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/plain; version=0.0.4; charset=utf-8")],
        buffer,
    )
}
```

5. **Register route** in existing HTTP router:
```rust
.route("/api/v1/metrics", get(metrics_handler))
```

6. **Add configuration** in `src/config/schema.rs`:
```rust
#[derive(Debug, Deserialize, Default)]
pub struct OtelConfig {
    #[serde(default = "default_metrics_enabled")]
    pub metrics_enabled: bool,
    // ... other fields
}

fn default_metrics_enabled() -> bool { true }
```

### Prometheus Naming Conventions
- Prefix all metrics with `palingenesis_`
- Use snake_case for metric names
- Use `_total` suffix for counters
- Use `_seconds` suffix for time durations
- Use `_bytes` suffix for byte sizes

### Example Output
```
# HELP palingenesis_info Information about the palingenesis daemon
# TYPE palingenesis_info gauge
palingenesis_info{version="0.1.0"} 1

# HELP palingenesis_daemon_state Current state of the daemon (1=monitoring, 2=paused, 3=waiting, 4=resuming)
# TYPE palingenesis_daemon_state gauge
palingenesis_daemon_state 1

# HELP palingenesis_uptime_seconds Total uptime of the daemon in seconds
# TYPE palingenesis_uptime_seconds gauge
palingenesis_uptime_seconds 3600.5

# HELP palingenesis_build_info Build information
# TYPE palingenesis_build_info gauge
palingenesis_build_info{version="0.1.0",commit="abc1234"} 1
```

## Definition of Done

- [ ] `prometheus` crate added to Cargo.toml
- [ ] `src/telemetry/metrics.rs` created with MetricsCollector
- [ ] `src/http/handlers/metrics.rs` created with endpoint handler
- [ ] Route `/api/v1/metrics` registered in HTTP server
- [ ] Configuration option `otel.metrics_enabled` implemented
- [ ] Basic metrics (info, state, uptime, build_info) exposed
- [ ] Unit tests for metrics encoding
- [ ] Integration test for metrics endpoint
- [ ] All tests pass (`cargo nextest run`)
- [ ] No clippy warnings (`cargo clippy`)
- [ ] Code formatted (`cargo fmt`)

## Test Scenarios

### Unit Tests
1. Test metrics registry initialization
2. Test metrics encoding to Prometheus format
3. Test state gauge value mapping
4. Test configuration parsing for metrics_enabled

### Integration Tests
1. GET `/api/v1/metrics` returns 200 with valid format
2. GET `/api/v1/metrics` with metrics disabled returns 404
3. Verify metrics can be parsed by prometheus client library
4. Performance test: 100 requests complete in <5 seconds total

## Related Stories

- **Story 7.2**: Core Metrics Implementation (adds counters/histograms)
- **Story 7.3**: Time Saved Metric
- **Story 7.4**: Saves Count Metric
- **Story 7.7**: Grafana Dashboard Template (consumes these metrics)
- **Story 6.1**: HTTP API Server Setup (provides the HTTP infrastructure)

## Dependencies

### Upstream Dependencies
- Story 6.1 (HTTP API Server Setup) - provides axum server infrastructure

### Downstream Dependencies
- Story 7.2 (Core Metrics Implementation) - builds on this foundation
- Story 7.7 (Grafana Dashboard Template) - requires metrics endpoint

---
*Generated: 2026-02-06*
*Epic Reference: _bmad-output/planning-artifacts/epics.md#story-71-prometheus-metrics-endpoint*
