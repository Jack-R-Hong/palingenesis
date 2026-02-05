use std::path::PathBuf;

use palingenesis::config::schema::{
    Config, DaemonConfig, MonitoringConfig, NotificationsConfig, OtelConfig, ResumeConfig,
};

fn expected_session_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".opencode")
}

#[test]
fn test_deserialize_full_config() {
    let toml_str = r#"
[daemon]
pid_file = "/tmp/palingenesis.pid"
socket_path = "/tmp/palingenesis.sock"
http_enabled = true
http_port = 7777
http_bind = "0.0.0.0"
log_level = "debug"
log_file = "/tmp/palingenesis.log"

[monitoring]
session_dir = "/tmp/opencode"
assistants = ["sisyphus", "opencode"]
auto_detect = false
debounce_ms = 250
poll_interval_secs = 5

[resume]
enabled = false
base_delay_secs = 10
max_delay_secs = 60
max_retries = 3
jitter = false
backup_count = 2

[notifications]
enabled = true

[notifications.webhook]
url = "https://example.com/hook"

[notifications.webhook.headers]
Authorization = "Bearer token"

[notifications.ntfy]
topic = "palingenesis"
server = "https://ntfy.sh"
priority = "high"

[notifications.discord]
webhook_url = "https://discord.com/api/webhooks/1"

[notifications.slack]
webhook_url = "https://hooks.slack.com/services/1"

[otel]
enabled = true
endpoint = "http://localhost:4317"
service_name = "palingenesis-test"
traces = false
metrics = true
"#;

    let config: Config = toml::from_str(toml_str).expect("parse full config");

    assert_eq!(
        config.daemon,
        DaemonConfig {
            pid_file: Some(PathBuf::from("/tmp/palingenesis.pid")),
            socket_path: Some(PathBuf::from("/tmp/palingenesis.sock")),
            http_enabled: true,
            http_port: 7777,
            http_bind: "0.0.0.0".to_string(),
            log_level: "debug".to_string(),
            log_file: Some(PathBuf::from("/tmp/palingenesis.log")),
        }
    );

    assert_eq!(
        config.monitoring,
        MonitoringConfig {
            session_dir: PathBuf::from("/tmp/opencode"),
            assistants: vec!["sisyphus".to_string(), "opencode".to_string()],
            auto_detect: false,
            auto_detect_interval_secs: 300,
            debounce_ms: 250,
            poll_interval_secs: Some(5),
        }
    );

    assert_eq!(
        config.resume,
        ResumeConfig {
            enabled: false,
            base_delay_secs: 10,
            max_delay_secs: 60,
            max_retries: 3,
            jitter: false,
            backup_count: 2,
        }
    );

    assert_eq!(config.notifications.enabled, true);
    let webhook = config
        .notifications
        .webhook
        .as_ref()
        .expect("webhook config");
    assert_eq!(webhook.url, "https://example.com/hook");
    let headers = webhook.headers.as_ref().expect("headers");
    assert_eq!(
        headers.get("Authorization").map(String::as_str),
        Some("Bearer token")
    );

    let otel = config.otel.expect("otel config");
    assert_eq!(
        otel,
        OtelConfig {
            enabled: true,
            endpoint: Some("http://localhost:4317".to_string()),
            service_name: "palingenesis-test".to_string(),
            traces: false,
            metrics: true,
        }
    );
}

#[test]
fn test_deserialize_minimal_config_uses_defaults() {
    let config: Config = toml::from_str("").expect("parse minimal config");
    assert_eq!(config, Config::default());
    assert!(config.otel.is_none());
}

#[test]
fn test_default_values_applied() {
    let config = Config::default();

    assert_eq!(config.daemon.http_enabled, false);
    assert_eq!(config.daemon.http_port, 7654);
    assert_eq!(config.daemon.http_bind, "127.0.0.1");
    assert_eq!(config.daemon.log_level, "info");

    assert_eq!(config.monitoring.session_dir, expected_session_dir());
    assert!(config.monitoring.assistants.is_empty());
    assert_eq!(config.monitoring.auto_detect, true);
    assert_eq!(config.monitoring.auto_detect_interval_secs, 300);
    assert_eq!(config.monitoring.debounce_ms, 100);
    assert_eq!(config.monitoring.poll_interval_secs, None);

    assert_eq!(config.resume.base_delay_secs, 30);
    assert_eq!(config.resume.max_delay_secs, 300);
    assert_eq!(config.resume.max_retries, 10);
    assert_eq!(config.resume.jitter, true);
    assert_eq!(config.resume.backup_count, 10);

    assert_eq!(
        config.notifications,
        NotificationsConfig {
            enabled: false,
            webhook: None,
            ntfy: None,
            discord: None,
            slack: None,
        }
    );
}

#[test]
fn test_nested_struct_deserialization() {
    let toml_str = r#"
[notifications]
enabled = true

[notifications.webhook]
url = "https://example.com/hook"

[notifications.webhook.headers]
Authorization = "Bearer token"
"#;

    let config: Config = toml::from_str(toml_str).expect("parse nested config");
    let webhook = config
        .notifications
        .webhook
        .as_ref()
        .expect("webhook config");
    assert_eq!(webhook.url, "https://example.com/hook");
    let headers = webhook.headers.as_ref().expect("headers");
    assert_eq!(
        headers.get("Authorization").map(String::as_str),
        Some("Bearer token")
    );
}

#[test]
fn test_invalid_config_errors() {
    let toml_str = r#"
[daemon]
http_port = "not-a-number"
"#;

    let result: Result<Config, _> = toml::from_str(toml_str);
    assert!(result.is_err());
}
