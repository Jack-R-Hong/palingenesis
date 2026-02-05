use std::sync::{Arc, Mutex};

use prometheus_client::encoding::EncodeLabelSet;
use prometheus_client::encoding::text::encode;
use prometheus_client::metrics::family::Family;
use prometheus_client::metrics::gauge::Gauge;
use prometheus_client::registry::Registry;

use crate::daemon::state::DaemonState;
use crate::ipc::socket::DaemonStateAccess;

const METRICS_NAMESPACE: &str = "palingenesis";
const BUILD_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Clone, Debug, Hash, PartialEq, Eq, EncodeLabelSet)]
struct InfoLabels {
    version: String,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, EncodeLabelSet)]
struct BuildInfoLabels {
    version: String,
    commit: String,
}

#[derive(Clone)]
pub struct Metrics {
    registry: Arc<Mutex<Registry>>,
    info: Family<InfoLabels, Gauge>,
    build_info: Family<BuildInfoLabels, Gauge>,
    daemon_state: Gauge,
    uptime_seconds: Gauge,
}

impl Metrics {
    pub fn new() -> Self {
        let mut registry = Registry::default();

        let info = Family::<InfoLabels, Gauge>::default();
        registry.register(
            format!("{METRICS_NAMESPACE}_info"),
            "Information about the palingenesis daemon",
            info.clone(),
        );

        let build_info = Family::<BuildInfoLabels, Gauge>::default();
        registry.register(
            format!("{METRICS_NAMESPACE}_build_info"),
            "Build information",
            build_info.clone(),
        );

        let daemon_state = Gauge::default();
        registry.register(
            format!("{METRICS_NAMESPACE}_daemon_state"),
            "Current state of the daemon (1=monitoring, 2=paused, 3=waiting, 4=resuming)",
            daemon_state.clone(),
        );

        let uptime_seconds = Gauge::default();
        registry.register(
            format!("{METRICS_NAMESPACE}_uptime_seconds"),
            "Total uptime of the daemon in seconds",
            uptime_seconds.clone(),
        );

        let metrics = Self {
            registry: Arc::new(Mutex::new(registry)),
            info,
            build_info,
            daemon_state,
            uptime_seconds,
        };

        metrics.set_static_info();
        metrics
    }

    pub fn update_from_state(&self, state: &DaemonState) {
        let status = state.get_status();
        let state_value = match status.state.as_str() {
            "monitoring" => 1,
            "paused" => 2,
            "waiting" => 3,
            "resuming" => 4,
            _ => 0,
        };
        self.daemon_state.set(state_value);
        self.uptime_seconds.set(state.uptime().as_secs() as i64);
    }

    pub fn encode(&self) -> Result<String, std::fmt::Error> {
        let registry = self
            .registry
            .lock()
            .expect("metrics registry lock poisoned");
        let mut buffer = String::new();
        encode(&mut buffer, &registry)?;
        Ok(buffer)
    }

    fn set_static_info(&self) {
        let version = BUILD_VERSION.to_string();
        let commit = build_commit().to_string();
        self.info
            .get_or_create(&InfoLabels {
                version: version.clone(),
            })
            .set(1);
        self.build_info
            .get_or_create(&BuildInfoLabels { version, commit })
            .set(1);
    }
}

fn build_commit() -> &'static str {
    option_env!("GIT_COMMIT_SHA").unwrap_or("unknown")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_encode_contains_expected_families() {
        let metrics = Metrics::new();
        let state = DaemonState::new();
        metrics.update_from_state(&state);
        let output = metrics.encode().expect("encode metrics");

        assert!(output.contains("# HELP palingenesis_info"));
        assert!(output.contains("# TYPE palingenesis_info gauge"));
        assert!(output.contains("palingenesis_info{version=\""));
        assert!(output.contains("# HELP palingenesis_daemon_state"));
        assert!(output.contains("# TYPE palingenesis_daemon_state gauge"));
        assert!(output.contains("# HELP palingenesis_uptime_seconds"));
        assert!(output.contains("# TYPE palingenesis_uptime_seconds gauge"));
        assert!(output.contains("# HELP palingenesis_build_info"));
        assert!(output.contains("# TYPE palingenesis_build_info gauge"));
    }

    #[test]
    fn test_metrics_state_mapping_paused() {
        let metrics = Metrics::new();
        let state = DaemonState::new();
        state.pause().expect("pause daemon");
        metrics.update_from_state(&state);
        let output = metrics.encode().expect("encode metrics");
        assert!(output.contains("palingenesis_daemon_state 2"));
    }
}
