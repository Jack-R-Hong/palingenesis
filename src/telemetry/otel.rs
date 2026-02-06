#[cfg(feature = "otel")]
use std::sync::Once;

#[cfg(feature = "otel")]
use opentelemetry_otlp::WithExportConfig;

#[cfg(feature = "otel")]
use tracing::info;
use tracing::warn;

use crate::config::Paths;
use crate::config::schema::{Config, OtelConfig};
use crate::config::validation::validate_config;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OtelProtocol {
    Http,
    Grpc,
}

impl OtelProtocol {
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "http" => Some(Self::Http),
            "grpc" => Some(Self::Grpc),
            _ => None,
        }
    }
}

pub fn load_otel_config() -> Option<OtelConfig> {
    let path = Paths::config_file();
    if !path.exists() {
        return None;
    }

    let contents = match std::fs::read_to_string(&path) {
        Ok(contents) => contents,
        Err(err) => {
            warn!(error = %err, "Failed to read config for otel; using defaults");
            return None;
        }
    };

    let config: Config = match toml::from_str(&contents) {
        Ok(config) => config,
        Err(err) => {
            warn!(error = %err, "Failed to parse config for otel; using defaults");
            return None;
        }
    };

    let validation = validate_config(&config);
    if !validation.is_valid() {
        warn!("Config validation failed for otel; using defaults");
        return None;
    }

    config.otel
}

pub fn shutdown_otel() {
    #[cfg(feature = "otel")]
    {
        opentelemetry::global::shutdown_tracer_provider();
        opentelemetry::global::shutdown_logger_provider();
        info!("OpenTelemetry tracer and logger shut down");
    }
}

#[cfg(feature = "otel")]
pub type OtelLayer = tracing_opentelemetry::OpenTelemetryLayer<
    tracing_subscriber::Registry,
    opentelemetry_sdk::trace::Tracer,
>;

#[cfg(not(feature = "otel"))]
pub type OtelLayer = ();

/// Builds the OpenTelemetry tracing layer for trace export.
///
/// Returns `None` if OTEL is disabled, traces are disabled, or the endpoint is empty.
/// On success, installs a batch tracer with the configured endpoint and sampling ratio.
pub fn build_otel_layer(config: &OtelConfig) -> Option<OtelLayer> {
    if !config.enabled || !config.traces {
        return None;
    }

    let endpoint = config.endpoint.trim();
    if endpoint.is_empty() {
        warn!("OpenTelemetry endpoint is empty; skipping tracer setup");
        return None;
    }

    let protocol = OtelProtocol::parse(&config.protocol).unwrap_or_else(|| {
        warn!(protocol = %config.protocol, "Unknown OpenTelemetry protocol; defaulting to http");
        OtelProtocol::Http
    });

    let sampling_ratio = if (0.0..=1.0).contains(&config.sampling_ratio) {
        config.sampling_ratio
    } else {
        warn!(
            ratio = config.sampling_ratio,
            "Invalid sampling ratio; defaulting to 1.0"
        );
        1.0
    };

    #[cfg(feature = "otel")]
    {
        let _ = set_error_handler();
        opentelemetry::global::set_text_map_propagator(
            opentelemetry_sdk::propagation::TraceContextPropagator::new(),
        );

        let trace_config = || {
            opentelemetry_sdk::trace::Config::default()
                .with_resource(opentelemetry_sdk::Resource::new(vec![
                    opentelemetry::KeyValue::new("service.name", config.service_name.clone()),
                ]))
                .with_sampler(opentelemetry_sdk::trace::Sampler::TraceIdRatioBased(
                    sampling_ratio,
                ))
        };

        let tracer = match protocol {
            OtelProtocol::Http => opentelemetry_otlp::new_pipeline()
                .tracing()
                .with_exporter(
                    opentelemetry_otlp::new_exporter()
                        .http()
                        .with_endpoint(endpoint.to_string()),
                )
                .with_trace_config(trace_config())
                .install_batch(opentelemetry_sdk::runtime::Tokio),
            OtelProtocol::Grpc => opentelemetry_otlp::new_pipeline()
                .tracing()
                .with_exporter(
                    opentelemetry_otlp::new_exporter()
                        .tonic()
                        .with_endpoint(endpoint.to_string()),
                )
                .with_trace_config(trace_config())
                .install_batch(opentelemetry_sdk::runtime::Tokio),
        };

        let tracer = match tracer {
            Ok(tracer) => tracer,
            Err(err) => {
                warn!(error = %err, "OpenTelemetry tracing initialization failed");
                return None;
            }
        };

        info!("OpenTelemetry tracing enabled");
        Some(tracing_opentelemetry::layer().with_tracer(tracer))
    }

    #[cfg(not(feature = "otel"))]
    {
        warn!("OpenTelemetry feature not enabled; skipping tracer setup");
        let _ = (protocol, sampling_ratio);
        None
    }
}

#[cfg(feature = "otel")]
fn set_error_handler() -> Result<(), opentelemetry::global::Error> {
    static HANDLER: Once = Once::new();
    let mut result = Ok(());
    HANDLER.call_once(|| {
        result = opentelemetry::global::set_error_handler(|err| {
            warn!(error = %err, "OpenTelemetry export error");
        });
        if result.is_err() {
            warn!("Failed to set OpenTelemetry error handler");
        }
    });
    result
}

#[cfg(feature = "otel")]
pub type OtelLogsLayer = opentelemetry_appender_tracing::layer::OpenTelemetryTracingBridge<
    opentelemetry_sdk::logs::LoggerProvider,
    opentelemetry_sdk::logs::Logger,
>;

#[cfg(not(feature = "otel"))]
pub type OtelLogsLayer = ();

#[cfg(feature = "otel")]
fn build_log_exporter(
    endpoint: &str,
    protocol: OtelProtocol,
) -> Result<opentelemetry_otlp::LogExporter, opentelemetry::logs::LogError> {
    match protocol {
        OtelProtocol::Http => opentelemetry_otlp::new_exporter()
            .http()
            .with_endpoint(endpoint.to_string())
            .build_log_exporter(),
        OtelProtocol::Grpc => opentelemetry_otlp::new_exporter()
            .tonic()
            .with_endpoint(endpoint.to_string())
            .build_log_exporter(),
    }
}

/// Builds the OpenTelemetry logs layer for log export via OTLP.
///
/// Returns `None` if OTEL is disabled, logs are disabled, or the endpoint is empty.
/// On success, installs a batch log exporter and bridges tracing events to OTLP logs.
/// Logs include trace context (trace_id, span_id) when emitted within a traced span.
pub fn build_otel_logs_layer(config: &OtelConfig) -> Option<OtelLogsLayer> {
    if !config.enabled || !config.logs {
        return None;
    }

    let endpoint = config.endpoint.trim();
    if endpoint.is_empty() {
        warn!("OpenTelemetry endpoint is empty; skipping logs setup");
        return None;
    }

    let protocol = OtelProtocol::parse(&config.protocol).unwrap_or_else(|| {
        warn!(protocol = %config.protocol, "Unknown OpenTelemetry protocol; defaulting to http");
        OtelProtocol::Http
    });

    #[cfg(feature = "otel")]
    {
        let _ = set_error_handler();

        let exporter = match build_log_exporter(endpoint, protocol) {
            Ok(exp) => exp,
            Err(err) => {
                warn!(error = %err, "OpenTelemetry log exporter creation failed");
                return None;
            }
        };

        let logger_provider = opentelemetry_sdk::logs::LoggerProvider::builder()
            .with_batch_exporter(exporter, opentelemetry_sdk::runtime::Tokio)
            .with_config(opentelemetry_sdk::logs::Config::default().with_resource(
                opentelemetry_sdk::Resource::new(vec![opentelemetry::KeyValue::new(
                    "service.name",
                    config.service_name.clone(),
                )]),
            ))
            .build();

        opentelemetry::global::set_logger_provider(logger_provider.clone());
        info!("OpenTelemetry logs enabled");

        Some(
            opentelemetry_appender_tracing::layer::OpenTelemetryTracingBridge::new(
                &logger_provider,
            ),
        )
    }

    #[cfg(not(feature = "otel"))]
    {
        warn!("OpenTelemetry feature not enabled; skipping logs setup");
        let _ = protocol;
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::schema::OtelConfig;

    #[test]
    fn test_protocol_parse() {
        assert_eq!(OtelProtocol::parse("http"), Some(OtelProtocol::Http));
        assert_eq!(OtelProtocol::parse("grpc"), Some(OtelProtocol::Grpc));
        assert_eq!(OtelProtocol::parse("unknown"), None);
    }

    #[test]
    fn test_build_otel_layer_disabled_returns_none() {
        let config = OtelConfig {
            enabled: false,
            traces: true,
            ..Default::default()
        };
        assert!(build_otel_layer(&config).is_none());
    }

    #[test]
    fn test_build_otel_layer_traces_false_returns_none() {
        let config = OtelConfig {
            enabled: true,
            traces: false,
            ..Default::default()
        };
        assert!(build_otel_layer(&config).is_none());
    }

    #[test]
    fn test_build_otel_layer_empty_endpoint_returns_none() {
        let config = OtelConfig {
            enabled: true,
            traces: true,
            endpoint: "  ".to_string(),
            ..Default::default()
        };
        assert!(build_otel_layer(&config).is_none());
    }

    #[test]
    fn test_build_otel_logs_layer_disabled_returns_none() {
        let config = OtelConfig {
            enabled: false,
            logs: true,
            ..Default::default()
        };
        assert!(build_otel_logs_layer(&config).is_none());
    }

    #[test]
    fn test_build_otel_logs_layer_logs_false_returns_none() {
        let config = OtelConfig {
            enabled: true,
            logs: false,
            ..Default::default()
        };
        assert!(build_otel_logs_layer(&config).is_none());
    }

    #[test]
    fn test_build_otel_logs_layer_empty_endpoint_returns_none() {
        let config = OtelConfig {
            enabled: true,
            logs: true,
            endpoint: "".to_string(),
            ..Default::default()
        };
        assert!(build_otel_logs_layer(&config).is_none());
    }
}
