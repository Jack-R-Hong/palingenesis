# Story 7.5: OTEL Traces Export

## Story Overview

**Epic:** 7 - Observability & Metrics
**Story ID:** 7-5
**Title:** OTEL Traces Export
**Status:** done
**Priority:** Growth
**Story Points:** 3

## User Story

As an operator,
I want OpenTelemetry traces,
So that I can see detailed request flows in Jaeger.

## Background

This story adds OpenTelemetry (OTEL) distributed tracing support to palingenesis, enabling operators to visualize request flows and diagnose issues using standard observability tools like Jaeger, Zipkin, or any OTLP-compatible backend. Building on the tracing infrastructure from Story 1.12 and the metrics foundation from Epic 7, this story integrates the `opentelemetry` crate to export trace spans via OTLP (OpenTelemetry Protocol). The feature is opt-in to avoid runtime overhead when not needed. It implements FR36 (OTLP traces export).

## Acceptance Criteria

### AC1: OTEL tracing enabled via config
**Given** OTEL is enabled in config with `[otel] enabled = true`
**When** the daemon starts
**Then** OpenTelemetry tracer is initialized
**And** trace spans are created for daemon operations
**And** logs indicate "OpenTelemetry tracing enabled"

### AC2: Resume operations create trace spans
**Given** OTEL tracing is enabled
**When** a resume operation executes
**Then** a trace span is created for the operation
**And** span includes attributes: `stop_reason`, `wait_duration_ms`, `outcome`
**And** span includes operation name like `resume.same_session` or `resume.new_session`

### AC3: Traces exported via OTLP endpoint
**Given** OTLP endpoint is configured (`otel.endpoint = "http://localhost:4317"`)
**When** traces are exported
**Then** they are sent via gRPC or HTTP to the configured collector
**And** export errors are logged but don't crash the daemon

### AC4: OTEL disabled by default
**Given** OTEL is not configured or `enabled = false`
**When** the daemon runs
**Then** no OpenTelemetry tracer is initialized
**And** no trace export overhead occurs
**And** standard tracing-subscriber still works

### AC5: Trace context propagation
**Given** OTEL tracing is enabled
**When** a traced operation spawns child operations
**Then** parent-child span relationships are maintained
**And** trace context is propagated through async boundaries

### AC6: Daemon lifecycle spans
**Given** OTEL tracing is enabled
**When** daemon starts
**Then** a root span `daemon.run` is created
**And** child spans for major operations (monitor, resume, ipc) are created

### AC7: HTTP request tracing
**Given** OTEL tracing is enabled and HTTP API is running
**When** HTTP requests are received
**Then** each request creates a span with path, method, status_code
**And** spans are linked to the daemon root span

### AC8: Graceful degradation on export failure
**Given** OTLP endpoint is unreachable
**When** trace export fails
**Then** errors are logged at warn level
**And** daemon continues operating normally
**And** traces are dropped (not queued indefinitely)

## Technical Notes

### Implementation Location
- `src/telemetry/otel.rs` - OpenTelemetry initialization and configuration
- `src/telemetry/mod.rs` - Integrate OTEL with existing tracing setup
- `src/config/schema.rs` - Add OTEL configuration section
- `src/resume/mod.rs` - Add trace spans to resume operations
- `src/daemon/mod.rs` - Add root daemon span
- `src/http/server.rs` - Add HTTP tracing middleware

### Dependencies
- Story 1.12 (Tracing and Structured Logging Setup) - provides tracing foundation
- Story 7.1 (Prometheus Metrics Endpoint) - provides observability patterns
- `opentelemetry` crate (0.22.x) - core OTEL API
- `opentelemetry-otlp` crate (0.15.x) - OTLP exporter
- `tracing-opentelemetry` crate (0.23.x) - bridge between tracing and OTEL
- `opentelemetry_sdk` crate - runtime SDK

### Architecture Alignment
- Implements: FR36 (OTLP traces export)
- Extends: ARCH9 (Structured logging with tracing)
- Patterns: Uses feature flags for optional compilation

### Cargo.toml Configuration

```toml
[features]
default = []
otel = ["dep:opentelemetry", "dep:opentelemetry-otlp", "dep:opentelemetry_sdk", "dep:tracing-opentelemetry"]

[dependencies]
opentelemetry = { version = "0.22", optional = true }
opentelemetry-otlp = { version = "0.15", optional = true, features = ["tonic"] }
opentelemetry_sdk = { version = "0.22", optional = true, features = ["rt-tokio"] }
tracing-opentelemetry = { version = "0.23", optional = true }
```

### Config Schema Addition

```rust
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct OtelConfig {
    /// Enable OpenTelemetry tracing (default: false)
    #[serde(default)]
    pub enabled: bool,
    
    /// OTLP endpoint URL (default: "http://localhost:4317")
    #[serde(default = "default_otel_endpoint")]
    pub endpoint: String,
    
    /// Service name for traces (default: "palingenesis")
    #[serde(default = "default_service_name")]
    pub service_name: String,
    
    /// Export protocol: "grpc" or "http" (default: "grpc")
    #[serde(default = "default_protocol")]
    pub protocol: String,
    
    /// Sampling ratio 0.0-1.0 (default: 1.0 = all traces)
    #[serde(default = "default_sampling_ratio")]
    pub sampling_ratio: f64,
}

fn default_otel_endpoint() -> String { "http://localhost:4317".to_string() }
fn default_service_name() -> String { "palingenesis".to_string() }
fn default_protocol() -> String { "grpc".to_string() }
fn default_sampling_ratio() -> f64 { 1.0 }
```

### Implementation Approach

1. **Create OTEL module** (`src/telemetry/otel.rs`):

```rust
use opentelemetry::trace::TracerProvider;
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::{runtime, trace as sdktrace};
use tracing_opentelemetry::OpenTelemetryLayer;
use tracing_subscriber::layer::SubscriberExt;

#[cfg(feature = "otel")]
pub fn init_otel(config: &OtelConfig) -> Result<sdktrace::TracerProvider, OtelError> {
    let exporter = opentelemetry_otlp::new_exporter()
        .tonic()
        .with_endpoint(&config.endpoint);
    
    let tracer_provider = opentelemetry_otlp::new_pipeline()
        .tracing()
        .with_exporter(exporter)
        .with_trace_config(
            sdktrace::Config::default()
                .with_resource(Resource::new(vec![
                    KeyValue::new("service.name", config.service_name.clone()),
                ]))
                .with_sampler(sdktrace::Sampler::TraceIdRatioBased(config.sampling_ratio)),
        )
        .install_batch(runtime::Tokio)?;
    
    Ok(tracer_provider)
}

#[cfg(feature = "otel")]
pub fn otel_layer(provider: &sdktrace::TracerProvider) -> OpenTelemetryLayer<...> {
    let tracer = provider.tracer("palingenesis");
    tracing_opentelemetry::layer().with_tracer(tracer)
}

#[cfg(not(feature = "otel"))]
pub fn init_otel(_config: &OtelConfig) -> Result<(), OtelError> {
    // No-op when feature is disabled
    Ok(())
}
```

2. **Integrate with tracing subscriber** (`src/telemetry/mod.rs`):

```rust
pub fn init_tracing(config: &Config) -> Result<()> {
    let subscriber = tracing_subscriber::registry()
        .with(fmt_layer)
        .with(env_filter);
    
    #[cfg(feature = "otel")]
    let subscriber = if config.otel.enabled {
        let provider = otel::init_otel(&config.otel)?;
        let otel_layer = otel::otel_layer(&provider);
        tracing::info!("OpenTelemetry tracing enabled");
        subscriber.with(otel_layer)
    } else {
        subscriber.with(None::<OpenTelemetryLayer<_>>)
    };
    
    tracing::subscriber::set_global_default(subscriber)?;
    Ok(())
}
```

3. **Add spans to resume operations** (`src/resume/mod.rs`):

```rust
use tracing::{instrument, Span};

#[instrument(
    name = "resume.execute",
    skip(self, ctx),
    fields(
        stop_reason = %ctx.stop_reason,
        session_path = %ctx.session.path.display(),
    )
)]
pub async fn execute_resume(&self, ctx: &ResumeContext) -> Result<ResumeOutcome> {
    let start = Instant::now();
    
    let outcome = match ctx.stop_reason {
        StopReason::RateLimit { retry_after } => {
            self.same_session_resume(ctx, retry_after).await
        }
        StopReason::ContextExhausted => {
            self.new_session_resume(ctx).await
        }
        _ => return Ok(ResumeOutcome::Skipped),
    };
    
    // Record span attributes for the outcome
    Span::current().record("outcome", outcome.as_str());
    Span::current().record("duration_ms", start.elapsed().as_millis() as i64);
    
    outcome
}
```

4. **HTTP tracing middleware** (`src/http/server.rs`):

```rust
use tower_http::trace::TraceLayer;

let app = Router::new()
    .route("/api/v1/status", get(status_handler))
    // ... other routes
    .layer(
        TraceLayer::new_for_http()
            .make_span_with(|request: &Request<_>| {
                tracing::info_span!(
                    "http.request",
                    method = %request.method(),
                    path = %request.uri().path(),
                    status_code = tracing::field::Empty,
                )
            })
            .on_response(|response: &Response<_>, latency: Duration, span: &Span| {
                span.record("status_code", response.status().as_u16());
            })
    );
```

5. **Shutdown handling**:

```rust
pub async fn shutdown_otel() {
    #[cfg(feature = "otel")]
    {
        opentelemetry::global::shutdown_tracer_provider();
        tracing::info!("OpenTelemetry tracer shut down");
    }
}
```

### Config File Example

```toml
[otel]
enabled = true
endpoint = "http://localhost:4317"
service_name = "palingenesis"
protocol = "grpc"
sampling_ratio = 1.0
```

### Trace Span Examples

```
daemon.run (root span)
  |-- monitor.watch
  |   |-- monitor.file_change
  |   +-- monitor.classify
  |-- resume.execute
  |   |-- resume.wait (wait_duration_ms=60000)
  |   +-- resume.same_session (outcome=success)
  +-- http.request (method=GET, path=/api/v1/status, status_code=200)
```

## Definition of Done

- [ ] `src/telemetry/otel.rs` created with OTEL initialization
- [ ] Feature flag `otel` added to Cargo.toml
- [ ] Config schema extended with `[otel]` section
- [ ] OTEL tracer integrates with existing tracing subscriber
- [ ] Resume operations instrumented with spans
- [ ] HTTP requests traced via middleware
- [ ] Daemon lifecycle spans created
- [ ] Graceful degradation on export failure
- [ ] OTEL disabled by default (no overhead)
- [ ] Config validation for OTEL settings
- [ ] Unit tests for OTEL initialization
- [ ] Integration test with mock OTLP collector
- [ ] Documentation in README for OTEL setup
- [ ] All tests pass (`cargo nextest run`)
- [ ] All tests pass with feature (`cargo nextest run --features otel`)
- [ ] No clippy warnings (`cargo clippy --features otel`)
- [ ] Code formatted (`cargo fmt`)

## Test Scenarios

### Unit Tests
1. Test OTEL config parsing with all fields
2. Test OTEL config with defaults
3. Test OTEL disabled when `enabled = false`
4. Test sampling ratio validation (0.0-1.0)

### Integration Tests
1. Daemon starts with OTEL enabled and logs confirmation
2. Daemon starts with OTEL disabled, no OTEL overhead
3. Resume operation creates span with correct attributes
4. HTTP request creates span with method/path/status
5. Export failure doesn't crash daemon
6. Shutdown properly flushes pending traces

### Feature Flag Tests
1. Build without `otel` feature compiles and runs
2. Build with `otel` feature includes OTEL deps
3. Runtime behavior matches feature flag

### Edge Cases
1. Invalid OTLP endpoint URL - daemon starts but logs warning
2. OTLP endpoint unreachable - traces dropped, daemon continues
3. Very high trace volume - sampling ratio reduces load
4. Daemon shutdown - pending traces flushed before exit

## Related Stories

- **Story 1.12**: Tracing and Structured Logging Setup (provides foundation)
- **Story 7.1**: Prometheus Metrics Endpoint (observability patterns)
- **Story 7.6**: OTEL Logs Export (related OTEL integration)
- **Story 7.7**: Grafana Dashboard Template (visualization)

## Dependencies

### Upstream Dependencies
- Story 1.12 (Tracing and Structured Logging Setup) - tracing subscriber
- Story 7.1 (Prometheus Metrics Endpoint) - observability patterns

### Downstream Dependencies
- Story 7.6 (OTEL Logs Export) - shares OTEL infrastructure
- Story 7.7 (Grafana Dashboard Template) - may reference traces

---
*Generated: 2026-02-06*
*Epic Reference: _bmad-output/planning-artifacts/epics.md#story-75-otel-traces-export*
