use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use prometheus_client::encoding::text::encode;
use prometheus_client::encoding::EncodeLabelSet;
use prometheus_client::metrics::counter::Counter;
use prometheus_client::metrics::family::Family;
use prometheus_client::metrics::gauge::Gauge;
use prometheus_client::metrics::histogram::Histogram;
use prometheus_client::registry::Registry;
use tracing::warn;

use crate::daemon::state::DaemonState;
use crate::ipc::socket::DaemonStateAccess;
use crate::state::StateStore;

const METRICS_NAMESPACE: &str = "palingenesis";
const BUILD_VERSION: &str = env!("CARGO_PKG_VERSION");

static GLOBAL_METRICS: OnceLock<Arc<Metrics>> = OnceLock::new();

#[derive(Clone, Debug, Hash, PartialEq, Eq, EncodeLabelSet)]
struct InfoLabels {
    version: String,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, EncodeLabelSet)]
struct BuildInfoLabels {
    version: String,
    commit: String,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, EncodeLabelSet)]
struct ResumeReasonLabels {
    reason: String,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, EncodeLabelSet)]
struct ResumeFailureLabels {
    error_type: String,
}

#[derive(Clone)]
pub struct Metrics {
    registry: Arc<Mutex<Registry>>,
    info: Family<InfoLabels, Gauge>,
    build_info: Family<BuildInfoLabels, Gauge>,
    daemon_state: Gauge,
    uptime_seconds: Gauge,
    resumes_total: Family<ResumeReasonLabels, Counter>,
    resumes_success_total: Counter,
    resumes_failure_total: Family<ResumeFailureLabels, Counter>,
    sessions_started_total: Counter,
    rate_limits_total: Counter,
    context_exhaustions_total: Counter,
    current_session_steps_completed: Gauge,
    current_session_steps_total: Gauge,
    active_sessions: Gauge,
    retry_attempts: Gauge,
    resume_duration_seconds: Histogram,
    detection_latency_seconds: Histogram,
    wait_duration_seconds: Histogram,
    time_saved_seconds_total: Counter<f64>,
    time_saved_per_resume_seconds: Histogram,
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

        let resumes_total = Family::<ResumeReasonLabels, Counter>::default();
        for reason in ["rate_limit", "context_exhausted", "manual"] {
            let _ = resumes_total.get_or_create(&ResumeReasonLabels {
                reason: reason.to_string(),
            });
        }
        registry.register(
            format!("{METRICS_NAMESPACE}_resumes_total"),
            "Total number of resume operations attempted",
            resumes_total.clone(),
        );

        let resumes_success_total = Counter::default();
        registry.register(
            format!("{METRICS_NAMESPACE}_resumes_success_total"),
            "Total number of successful resumes",
            resumes_success_total.clone(),
        );

        let resumes_failure_total = Family::<ResumeFailureLabels, Counter>::default();
        for error_type in [
            "timeout",
            "spawn_failed",
            "command_failed",
            "session_not_found",
            "retry_exceeded",
            "config",
            "io",
            "unknown",
        ] {
            let _ = resumes_failure_total.get_or_create(&ResumeFailureLabels {
                error_type: error_type.to_string(),
            });
        }
        registry.register(
            format!("{METRICS_NAMESPACE}_resumes_failure_total"),
            "Total number of failed resume attempts",
            resumes_failure_total.clone(),
        );

        let sessions_started_total = Counter::default();
        registry.register(
            format!("{METRICS_NAMESPACE}_sessions_started_total"),
            "Total number of sessions started",
            sessions_started_total.clone(),
        );

        let rate_limits_total = Counter::default();
        registry.register(
            format!("{METRICS_NAMESPACE}_rate_limits_total"),
            "Total number of rate limit events detected",
            rate_limits_total.clone(),
        );

        let context_exhaustions_total = Counter::default();
        registry.register(
            format!("{METRICS_NAMESPACE}_context_exhaustions_total"),
            "Total number of context exhaustion events",
            context_exhaustions_total.clone(),
        );

        let current_session_steps_completed = Gauge::default();
        registry.register(
            format!("{METRICS_NAMESPACE}_current_session_steps_completed"),
            "Steps completed in current session",
            current_session_steps_completed.clone(),
        );

        let current_session_steps_total = Gauge::default();
        registry.register(
            format!("{METRICS_NAMESPACE}_current_session_steps_total"),
            "Total steps in current session (if known)",
            current_session_steps_total.clone(),
        );

        let active_sessions = Gauge::default();
        registry.register(
            format!("{METRICS_NAMESPACE}_active_sessions"),
            "Number of currently monitored sessions (0 or 1)",
            active_sessions.clone(),
        );

        let retry_attempts = Gauge::default();
        registry.register(
            format!("{METRICS_NAMESPACE}_retry_attempts"),
            "Current retry attempt number (0 if not retrying)",
            retry_attempts.clone(),
        );

        let resume_duration_seconds = Histogram::new([0.1, 0.5, 1.0, 2.0, 5.0, 10.0, 30.0, 60.0]);
        registry.register(
            format!("{METRICS_NAMESPACE}_resume_duration_seconds"),
            "Time taken for resume operations",
            resume_duration_seconds.clone(),
        );

        let detection_latency_seconds = Histogram::new([0.01, 0.05, 0.1, 0.5, 1.0, 2.0, 5.0]);
        registry.register(
            format!("{METRICS_NAMESPACE}_detection_latency_seconds"),
            "Time from session stop to detection",
            detection_latency_seconds.clone(),
        );

        let wait_duration_seconds =
            Histogram::new([1.0, 5.0, 10.0, 30.0, 60.0, 120.0, 300.0, 600.0]);
        registry.register(
            format!("{METRICS_NAMESPACE}_wait_duration_seconds"),
            "Time spent waiting for rate limit backoff",
            wait_duration_seconds.clone(),
        );

        let time_saved_seconds_total = Counter::<f64>::default();
        registry.register(
            format!("{METRICS_NAMESPACE}_time_saved_seconds_total"),
            "Total estimated time saved by automatic resumption",
            time_saved_seconds_total.clone(),
        );

        let time_saved_per_resume_seconds =
            Histogram::new([60.0, 120.0, 180.0, 300.0, 600.0, 900.0, 1800.0]);
        registry.register(
            format!("{METRICS_NAMESPACE}_time_saved_per_resume_seconds"),
            "Time saved per individual resume operation",
            time_saved_per_resume_seconds.clone(),
        );

        let metrics = Self {
            registry: Arc::new(Mutex::new(registry)),
            info,
            build_info,
            daemon_state,
            uptime_seconds,
            resumes_total,
            resumes_success_total,
            resumes_failure_total,
            sessions_started_total,
            rate_limits_total,
            context_exhaustions_total,
            current_session_steps_completed,
            current_session_steps_total,
            active_sessions,
            retry_attempts,
            resume_duration_seconds,
            detection_latency_seconds,
            wait_duration_seconds,
            time_saved_seconds_total,
            time_saved_per_resume_seconds,
        };

        metrics.set_static_info();
        metrics.initialize_time_saved_total();
        metrics
    }

    pub fn set_global(metrics: Arc<Metrics>) -> bool {
        GLOBAL_METRICS.set(metrics).is_ok()
    }

    pub fn global() -> Option<Arc<Metrics>> {
        GLOBAL_METRICS.get().cloned()
    }

    pub fn update_from_state(&self, state: &DaemonState) {
        let status = state.get_status();
        let state_value = match status.state.as_str() {
            "monitoring" => 1,
            "paused" => 2,
            "waiting" => 3,
            "resuming" => 4,
            unknown => {
                warn!(state = %unknown, "Unknown daemon state encountered, reporting as 0");
                0
            }
        };
        self.daemon_state.set(state_value);
        self.uptime_seconds.set(state.uptime().as_secs() as i64);
        self.update_session_gauges();
    }

    /// Records the start of a resume operation.
    ///
    /// # Arguments
    /// * `reason` - The reason for the resume: "rate_limit", "context_exhausted", or "manual"
    pub fn record_resume_started(&self, reason: &str) {
        self.resumes_total
            .get_or_create(&ResumeReasonLabels {
                reason: reason.to_string(),
            })
            .inc();
    }

    /// Records the completion of a resume operation.
    ///
    /// # Arguments
    /// * `duration` - Time taken for the resume operation
    /// * `success` - Whether the resume succeeded
    /// * `error_type` - Error type label if failed: "timeout", "spawn_failed", "command_failed", etc.
    pub fn record_resume_completed(
        &self,
        duration: Duration,
        success: bool,
        error_type: Option<&str>,
    ) {
        self.resume_duration_seconds.observe(duration.as_secs_f64());
        if success {
            self.resumes_success_total.inc();
            self.retry_attempts.set(0);
        } else {
            let label = error_type.unwrap_or("unknown");
            self.resumes_failure_total
                .get_or_create(&ResumeFailureLabels {
                    error_type: label.to_string(),
                })
                .inc();
        }
    }

    pub fn record_session_started(&self) {
        self.sessions_started_total.inc();
    }

    pub fn record_detection(&self, latency: Duration, stop_reason: &str) {
        self.detection_latency_seconds
            .observe(latency.as_secs_f64());
        match stop_reason {
            "rate_limit" => {
                self.rate_limits_total.inc();
            }
            "context_exhausted" => {
                self.context_exhaustions_total.inc();
            }
            _ => {}
        }
    }

    pub fn record_wait(&self, duration: Duration) {
        self.wait_duration_seconds.observe(duration.as_secs_f64());
    }

    pub fn record_time_saved(&self, total_saved_seconds: f64) {
        if !total_saved_seconds.is_finite() || total_saved_seconds <= 0.0 {
            return;
        }
        self.time_saved_seconds_total.inc_by(total_saved_seconds);
        self.time_saved_per_resume_seconds
            .observe(total_saved_seconds);
    }

    pub fn set_retry_attempts(&self, attempt: u32) {
        self.retry_attempts.set(i64::from(attempt));
    }

    fn update_session_gauges(&self) {
        let store = StateStore::new();
        let state = store.load();
        if let Some(session) = state.current_session {
            let completed = session.steps_completed.len() as i64;
            let total_steps = if session.total_steps > 0 {
                session.total_steps
            } else {
                session.last_step
            };
            self.current_session_steps_completed.set(completed);
            self.current_session_steps_total.set(total_steps as i64);
            self.active_sessions.set(1);
        } else {
            self.current_session_steps_completed.set(0);
            self.current_session_steps_total.set(0);
            self.active_sessions.set(0);
        }
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

    fn initialize_time_saved_total(&self) {
        let store = StateStore::new();
        let state = store.load();
        let total_saved = state.stats.time_saved_seconds;
        if total_saved.is_finite() && total_saved > 0.0 {
            self.time_saved_seconds_total.inc_by(total_saved);
        }
    }
}

impl Default for Metrics {
    fn default() -> Self {
        Self::new()
    }
}

fn build_commit() -> &'static str {
    option_env!("GIT_COMMIT_SHA").unwrap_or("unknown")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{CurrentSession, StateFile, StateStore, Stats};
    use crate::test_utils::ENV_LOCK;
    use std::env;
    use std::sync::Arc;
    use std::thread;
    use tempfile::tempdir;

    fn set_env_var(key: &str, value: impl AsRef<std::ffi::OsStr>) {
        unsafe {
            env::set_var(key, value);
        }
    }

    fn remove_env_var(key: &str) {
        unsafe {
            env::remove_var(key);
        }
    }

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
    fn test_metrics_encode_contains_core_metrics() {
        let metrics = Metrics::new();
        metrics.record_resume_started("rate_limit");
        metrics.record_resume_completed(Duration::from_millis(250), true, None);
        metrics.record_detection(Duration::from_millis(50), "rate_limit");
        metrics.record_wait(Duration::from_secs(2));
        metrics.record_session_started();
        metrics.record_time_saved(360.0);
        let output = metrics.encode().expect("encode metrics");

        assert!(output.contains("palingenesis_resumes_total"));
        assert!(output.contains("palingenesis_resumes_success_total"));
        assert!(output.contains("palingenesis_resumes_failure_total"));
        assert!(output.contains("palingenesis_sessions_started_total"));
        assert!(output.contains("palingenesis_rate_limits_total"));
        assert!(output.contains("palingenesis_context_exhaustions_total"));
        assert!(output.contains("palingenesis_current_session_steps_completed"));
        assert!(output.contains("palingenesis_current_session_steps_total"));
        assert!(output.contains("palingenesis_active_sessions"));
        assert!(output.contains("palingenesis_retry_attempts"));
        assert!(output.contains("palingenesis_resume_duration_seconds"));
        assert!(output.contains("palingenesis_detection_latency_seconds"));
        assert!(output.contains("palingenesis_wait_duration_seconds"));
        assert!(output.contains("palingenesis_time_saved_seconds_total"));
        assert!(output.contains("palingenesis_time_saved_per_resume_seconds"));
    }

    #[test]
    fn test_session_gauges_from_state_store() {
        let _lock = ENV_LOCK.lock().unwrap();
        let temp = tempdir().unwrap();
        let state_dir = temp.path().join("state");
        set_env_var("PALINGENESIS_STATE", &state_dir);

        let store = StateStore::new();
        let mut state = StateFile::default();
        state.current_session = Some(CurrentSession {
            path: temp.path().join("session.md"),
            steps_completed: vec![1, 2, 3],
            last_step: 5,
            total_steps: 8,
        });
        state.stats = Stats::default();
        store.save(&state).expect("save state");

        let metrics = Metrics::new();
        let daemon_state = DaemonState::new();
        metrics.update_from_state(&daemon_state);
        let output = metrics.encode().expect("encode metrics");

        assert!(output.contains("palingenesis_active_sessions 1"));
        assert!(output.contains("palingenesis_current_session_steps_completed 3"));
        assert!(output.contains("palingenesis_current_session_steps_total 8"));

        remove_env_var("PALINGENESIS_STATE");
    }

    #[test]
    fn test_metrics_concurrent_updates() {
        let metrics = Arc::new(Metrics::new());
        let mut handles = Vec::new();

        for _ in 0..8 {
            let metrics = Arc::clone(&metrics);
            handles.push(thread::spawn(move || {
                for _ in 0..100 {
                    metrics.record_resume_started("rate_limit");
                    metrics.record_wait(Duration::from_millis(5));
                    metrics.record_resume_completed(
                        Duration::from_millis(10),
                        false,
                        Some("timeout"),
                    );
                }
            }));
        }

        for handle in handles {
            handle.join().expect("thread join");
        }

        let count = metrics
            .resumes_total
            .get_or_create(&ResumeReasonLabels {
                reason: "rate_limit".to_string(),
            })
            .get();
        assert_eq!(count, 800);

        let failures = metrics
            .resumes_failure_total
            .get_or_create(&ResumeFailureLabels {
                error_type: "timeout".to_string(),
            })
            .get();
        assert_eq!(failures, 800);
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
