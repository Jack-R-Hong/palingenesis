use std::sync::Arc;

use crate::daemon::DaemonState;
use crate::daemon::core::run_mcp_server;
use crate::telemetry::otel::load_otel_config;
use crate::telemetry::tracing::{TracingConfig, init_tracing};

pub async fn handle_serve() -> anyhow::Result<()> {
    let otel_config = load_otel_config();
    let config = TracingConfig {
        log_to_file: false,
        log_to_stderr: true,
        ..TracingConfig::default()
    };
    let _guard = init_tracing(&config, otel_config.as_ref())?;

    let state = Arc::new(DaemonState::new_without_auto_detection());
    run_mcp_server(state).await?;
    Ok(())
}
