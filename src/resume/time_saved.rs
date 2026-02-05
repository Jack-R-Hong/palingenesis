use std::time::Duration;

use tracing::warn;

use crate::config::schema::{Config, MetricsConfig};
use crate::config::validation::validate_config;
use crate::config::Paths;

/// Result of calculating time saved by automatic session resumption.
///
/// Time saved is estimated as: `wait_duration + manual_restart_time`
/// where:
/// - `wait_duration` is the actual time spent waiting (e.g., for rate limit to expire)
/// - `manual_restart_time` is the estimated time a user would spend manually detecting
///   and restarting a stopped session (configurable, default 5 minutes)
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TimeSavedCalculation {
    /// Duration the daemon waited before resuming (seconds).
    pub wait_duration_seconds: f64,
    /// Estimated manual restart time from config (seconds).
    pub manual_restart_seconds: f64,
    /// Total time saved: wait_duration + manual_restart (seconds).
    pub total_saved_seconds: f64,
}

/// Calculate time saved by a successful resume operation.
///
/// # Arguments
/// * `wait_duration` - Time spent waiting before resume (e.g., rate limit backoff)
/// * `config` - Metrics configuration containing manual_restart_time_seconds
///
/// # Returns
/// A [`TimeSavedCalculation`] with the breakdown and total time saved.
pub fn calculate_time_saved(
    wait_duration: Duration,
    config: &MetricsConfig,
) -> TimeSavedCalculation {
    let wait_seconds = wait_duration.as_secs_f64();
    let manual_restart = config.manual_restart_time_seconds as f64;
    TimeSavedCalculation {
        wait_duration_seconds: wait_seconds,
        manual_restart_seconds: manual_restart,
        total_saved_seconds: wait_seconds + manual_restart,
    }
}

pub fn load_metrics_config() -> MetricsConfig {
    let path = Paths::config_file();
    if !path.exists() {
        return MetricsConfig::default();
    }

    let contents = match std::fs::read_to_string(&path) {
        Ok(contents) => contents,
        Err(err) => {
            warn!(error = %err, "Failed to read config for metrics; using defaults");
            return MetricsConfig::default();
        }
    };

    let config: Config = match toml::from_str(&contents) {
        Ok(config) => config,
        Err(err) => {
            warn!(error = %err, "Failed to parse config for metrics; using defaults");
            return MetricsConfig::default();
        }
    };

    let validation = validate_config(&config);
    if !validation.is_valid() {
        warn!("Config validation failed for metrics; using defaults");
        return MetricsConfig::default();
    }

    config.metrics
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_time_saved_default() {
        let config = MetricsConfig::default();
        let calculation = calculate_time_saved(Duration::from_secs(60), &config);
        assert_eq!(calculation.wait_duration_seconds, 60.0);
        assert_eq!(calculation.manual_restart_seconds, 300.0);
        assert_eq!(calculation.total_saved_seconds, 360.0);
    }

    #[test]
    fn test_calculate_time_saved_custom_manual_restart() {
        let mut config = MetricsConfig::default();
        config.manual_restart_time_seconds = 1200;
        let calculation = calculate_time_saved(Duration::from_secs(90), &config);
        assert_eq!(calculation.wait_duration_seconds, 90.0);
        assert_eq!(calculation.manual_restart_seconds, 1200.0);
        assert_eq!(calculation.total_saved_seconds, 1290.0);
    }
}
