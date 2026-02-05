use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::config::Paths;

/// Root configuration for palingenesis.
///
/// Example:
/// ```toml
/// [daemon]
/// log_level = "info"
///
/// [monitoring]
/// auto_detect = true
///
/// [resume]
/// enabled = true
///
/// [notifications]
/// enabled = false
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(default)]
pub struct Config {
    /// Daemon configuration section.
    /// Example: [daemon]
    pub daemon: DaemonConfig,
    /// Session monitoring configuration section.
    /// Example: [monitoring]
    pub monitoring: MonitoringConfig,
    /// Resume strategy configuration section.
    /// Example: [resume]
    pub resume: ResumeConfig,
    /// Notification channel configuration section.
    /// Example: [notifications]
    pub notifications: NotificationsConfig,
    /// Bot command configuration section.
    /// Example: [bot]
    pub bot: BotConfig,
    /// Metrics configuration section.
    /// Example: [metrics]
    pub metrics: MetricsConfig,
    /// Optional OpenTelemetry configuration section.
    /// Example: [otel]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub otel: Option<OtelConfig>,
}

/// Metrics configuration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct MetricsConfig {
    /// Estimated time for manual session restart (seconds).
    /// Default: 300 (5 minutes)
    pub manual_restart_time_seconds: u64,
}

impl Default for MetricsConfig {
    fn default() -> Self {
        Self {
            manual_restart_time_seconds: default_manual_restart_time_seconds(),
        }
    }
}

/// Daemon process configuration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct DaemonConfig {
    /// Path to PID file (platform default if not set).
    /// Example: pid_file = "/run/user/1000/palingenesis/palingenesis.pid"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pid_file: Option<PathBuf>,
    /// Path to Unix socket (platform default if not set).
    /// Example: socket_path = "/run/user/1000/palingenesis/palingenesis.sock"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub socket_path: Option<PathBuf>,
    /// Enable the HTTP control API.
    /// Example: http_enabled = false
    pub http_enabled: bool,
    /// HTTP server port.
    /// Example: http_port = 7654
    pub http_port: u16,
    /// HTTP server bind address.
    /// Example: http_bind = "127.0.0.1"
    pub http_bind: String,
    /// Log level (trace, debug, info, warn, error).
    /// Example: log_level = "info"
    pub log_level: String,
    /// Optional log file path.
    /// Example: log_file = "/var/log/palingenesis.log"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub log_file: Option<PathBuf>,
}

impl Default for DaemonConfig {
    fn default() -> Self {
        let runtime_dir = Paths::runtime_dir();
        Self {
            pid_file: Some(runtime_dir.join("palingenesis.pid")),
            socket_path: Some(runtime_dir.join("palingenesis.sock")),
            http_enabled: false,
            http_port: 7654,
            http_bind: "127.0.0.1".to_string(),
            log_level: "info".to_string(),
            log_file: None,
        }
    }
}

/// Session monitoring configuration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct MonitoringConfig {
    /// Directory to watch for session files.
    /// Example: session_dir = "~/.opencode"
    pub session_dir: PathBuf,
    /// Explicit list of assistants to monitor.
    /// Example: assistants = ["sisyphus", "opencode"]
    pub assistants: Vec<String>,
    /// Auto-detect running assistants.
    /// Example: auto_detect = true
    pub auto_detect: bool,
    /// Interval for auto-detection re-scan (seconds).
    /// Example: auto_detect_interval_secs = 300
    pub auto_detect_interval_secs: u64,
    /// Debounce time for file events (milliseconds).
    /// Example: debounce_ms = 100
    pub debounce_ms: u64,
    /// Polling interval fallback (seconds).
    /// Example: poll_interval_secs = 5
    #[serde(skip_serializing_if = "Option::is_none")]
    pub poll_interval_secs: Option<u64>,
}

impl Default for MonitoringConfig {
    fn default() -> Self {
        let session_dir = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".opencode");
        Self {
            session_dir,
            assistants: Vec::new(),
            auto_detect: true,
            auto_detect_interval_secs: 300,
            debounce_ms: 100,
            poll_interval_secs: None,
        }
    }
}

/// Resume strategy configuration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct ResumeConfig {
    /// Enable automatic resume.
    /// Example: enabled = true
    pub enabled: bool,
    /// Base delay for exponential backoff (seconds).
    /// Example: base_delay_secs = 30
    pub base_delay_secs: u64,
    /// Maximum delay cap (seconds).
    /// Example: max_delay_secs = 300
    pub max_delay_secs: u64,
    /// Maximum retry attempts.
    /// Example: max_retries = 10
    pub max_retries: u32,
    /// Add jitter to delays.
    /// Example: jitter = true
    pub jitter: bool,
    /// Number of session backups to keep.
    /// Example: backup_count = 10
    pub backup_count: u32,
}

impl Default for ResumeConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            base_delay_secs: 30,
            max_delay_secs: 300,
            max_retries: 10,
            jitter: true,
            backup_count: 10,
        }
    }
}

/// Notification channel configuration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(default)]
pub struct NotificationsConfig {
    /// Enable notifications globally.
    /// Example: enabled = false
    pub enabled: bool,
    /// Webhook notification configuration.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub webhook: Option<WebhookConfig>,
    /// ntfy.sh notification configuration.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ntfy: Option<NtfyConfig>,
    /// Discord notification configuration.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub discord: Option<DiscordConfig>,
    /// Slack notification configuration.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slack: Option<SlackConfig>,
}

/// Bot command configuration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct BotConfig {
    /// Enable bot command endpoints.
    /// Example: enabled = true
    pub enabled: bool,
    /// Allow all users if the authorized list is empty.
    /// Example: allow_all_users = true
    pub allow_all_users: bool,
    /// Discord application ID.
    /// Example: discord_application_id = "1234567890"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub discord_application_id: Option<String>,
    /// Discord public key for signature verification.
    /// Example: discord_public_key = "a1b2..."
    #[serde(skip_serializing_if = "Option::is_none")]
    pub discord_public_key: Option<String>,
    /// Slack signing secret for signature verification.
    /// Example: slack_signing_secret = "abcd1234"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slack_signing_secret: Option<String>,
    /// Authorized user list across platforms.
    pub authorized_users: Vec<AuthorizedUser>,
}

impl Default for BotConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            allow_all_users: true,
            discord_application_id: None,
            discord_public_key: None,
            slack_signing_secret: None,
            authorized_users: Vec::new(),
        }
    }
}

/// Authorized user entry for bot commands.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct AuthorizedUser {
    pub platform: BotPlatform,
    pub user_id: String,
}

/// Supported bot platforms.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum BotPlatform {
    Discord,
    Slack,
}

/// Webhook notification configuration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WebhookConfig {
    /// Webhook URL.
    /// Example: url = "https://example.com/hooks"
    pub url: String,
    /// Optional custom headers.
    /// Example: headers = { Authorization = "Bearer token" }
    #[serde(skip_serializing_if = "Option::is_none")]
    pub headers: Option<HashMap<String, String>>,
}

/// ntfy.sh notification configuration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NtfyConfig {
    /// ntfy topic name.
    /// Example: topic = "palingenesis"
    pub topic: String,
    /// Custom ntfy server (default: ntfy.sh).
    /// Example: server = "https://ntfy.sh"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub server: Option<String>,
    /// Notification priority.
    /// Example: priority = "high"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub priority: Option<String>,
}

/// Discord webhook notification configuration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DiscordConfig {
    /// Discord webhook URL.
    /// Example: webhook_url = "https://discord.com/api/webhooks/..."
    pub webhook_url: String,
}

/// Slack webhook notification configuration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SlackConfig {
    /// Slack webhook URL.
    /// Example: webhook_url = "https://hooks.slack.com/services/..."
    pub webhook_url: String,
}

/// OpenTelemetry configuration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct OtelConfig {
    /// Enable OpenTelemetry export.
    /// Example: enabled = false
    pub enabled: bool,
    /// OTLP endpoint.
    /// Example: endpoint = "http://localhost:4317"
    #[serde(default = "default_otel_endpoint")]
    pub endpoint: String,
    /// Service name for telemetry.
    /// Example: service_name = "palingenesis"
    pub service_name: String,
    /// Export protocol: "grpc" or "http".
    /// Example: protocol = "http"
    #[serde(default = "default_otel_protocol")]
    pub protocol: String,
    /// Sampling ratio 0.0-1.0.
    /// Example: sampling_ratio = 1.0
    #[serde(default = "default_otel_sampling_ratio")]
    pub sampling_ratio: f64,
    /// Enable trace export.
    /// Example: traces = true
    pub traces: bool,
    /// Enable log export via OTLP.
    /// Example: logs = true
    #[serde(default)]
    pub logs: bool,
    /// Enable metrics export.
    /// Example: metrics = true
    pub metrics: bool,
    /// Enable Prometheus metrics endpoint exposure.
    /// Example: metrics_enabled = true
    #[serde(default = "default_metrics_enabled")]
    pub metrics_enabled: bool,
}

impl Default for OtelConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            endpoint: default_otel_endpoint(),
            service_name: "palingenesis".to_string(),
            protocol: default_otel_protocol(),
            sampling_ratio: default_otel_sampling_ratio(),
            traces: true,
            logs: false,
            metrics: true,
            metrics_enabled: default_metrics_enabled(),
        }
    }
}

fn default_otel_endpoint() -> String {
    "http://localhost:4317".to_string()
}

fn default_otel_protocol() -> String {
    "http".to_string()
}

fn default_otel_sampling_ratio() -> f64 {
    1.0
}

fn default_metrics_enabled() -> bool {
    true
}

fn default_manual_restart_time_seconds() -> u64 {
    300
}

#[cfg(test)]
mod tests {
    use super::Config;

    #[test]
    fn test_metrics_enabled_parsing() {
        let config: Config = toml::from_str("[otel]\nmetrics_enabled = false\n").unwrap();
        let otel = config.otel.expect("otel config");
        assert!(!otel.metrics_enabled);
    }

    #[test]
    fn test_metrics_manual_restart_time_parsing() {
        let config: Config =
            toml::from_str("[metrics]\nmanual_restart_time_seconds = 900\n").unwrap();
        assert_eq!(config.metrics.manual_restart_time_seconds, 900);
    }

    #[test]
    fn test_otel_defaults_apply() {
        let config: Config = toml::from_str("[otel]\nenabled = true\n").unwrap();
        let otel = config.otel.expect("otel config");
        assert_eq!(otel.endpoint, "http://localhost:4317");
        assert_eq!(otel.protocol, "http");
        assert_eq!(otel.sampling_ratio, 1.0);
    }

    #[test]
    fn test_otel_sampling_ratio_parsing() {
        let config: Config = toml::from_str("[otel]\nsampling_ratio = 0.4\n").unwrap();
        let otel = config.otel.expect("otel config");
        assert_eq!(otel.sampling_ratio, 0.4);
    }
}
