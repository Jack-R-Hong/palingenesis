use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use crate::config::Paths;

pub async fn handle_init(force: bool, custom_path: Option<PathBuf>) -> anyhow::Result<()> {
    let config_path = custom_path.unwrap_or_else(Paths::config_file);

    if config_path.exists() && !force {
        if !confirm_overwrite(&config_path)? {
            println!("Aborted.");
            return Ok(());
        }
    }

    if let Some(parent) = config_path.parent() {
        fs::create_dir_all(parent)?;
        set_dir_permissions(parent);
    }

    let config_content = generate_default_config_toml();
    fs::write(&config_path, config_content)?;
    set_file_permissions(&config_path);

    println!(
        "\x1b[32mConfig created at {}\x1b[0m",
        config_path.display()
    );
    println!("Edit with: palingenesis config edit");

    Ok(())
}

pub async fn handle_show() -> anyhow::Result<()> {
    println!("config show not implemented (Story 4.3)");
    Ok(())
}

pub async fn handle_validate() -> anyhow::Result<()> {
    println!("config validate not implemented (Story 4.4)");
    Ok(())
}

pub async fn handle_edit() -> anyhow::Result<()> {
    println!("config edit not implemented (Story 4.5)");
    Ok(())
}

fn confirm_overwrite(path: &Path) -> anyhow::Result<bool> {
    print!(
        "Config already exists at {}. Overwrite? [y/N] ",
        path.display()
    );
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let response = input.trim();
    Ok(response.eq_ignore_ascii_case("y") || response.eq_ignore_ascii_case("yes"))
}

fn set_dir_permissions(path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Err(err) = fs::set_permissions(path, fs::Permissions::from_mode(0o700)) {
            eprintln!("Warning: failed to set directory permissions: {err}");
        }
    }
}

fn set_file_permissions(path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Err(err) = fs::set_permissions(path, fs::Permissions::from_mode(0o600)) {
            eprintln!("Warning: failed to set config file permissions: {err}");
        }
    }
}

fn generate_default_config_toml() -> String {
    r#"# palingenesis configuration file
# https://github.com/Jack-R-Hong/palingenesis

# Daemon process configuration
[daemon]
# Log level: trace, debug, info, warn, error
log_level = "info"
# Enable HTTP control API (default: disabled)
http_enabled = false
# HTTP server port (when enabled)
http_port = 7654
# HTTP server bind address
http_bind = "127.0.0.1"
# Optional: Custom PID file path (uses platform default if not set)
# pid_file = "/run/user/1000/palingenesis/palingenesis.pid"
# Optional: Custom socket path (uses platform default if not set)
# socket_path = "/run/user/1000/palingenesis/palingenesis.sock"
# Optional: Log to file instead of stderr
# log_file = "/path/to/daemon.log"

# Session monitoring configuration
[monitoring]
# Auto-detect running AI assistants
auto_detect = true
# Explicit list of assistants to monitor (optional)
# assistants = ["opencode"]
# Debounce time for file events (milliseconds)
debounce_ms = 100
# Optional: Session directory override
# session_dir = "~/.opencode"
# Optional: Polling interval fallback (seconds)
# poll_interval_secs = 5

# Resume strategy configuration
[resume]
# Enable automatic session resume
enabled = true
# Base delay for exponential backoff (seconds)
base_delay_secs = 30
# Maximum delay cap (seconds)
max_delay_secs = 300
# Maximum retry attempts before giving up
max_retries = 10
# Add random jitter to delays
jitter = true
# Number of session backups to keep
backup_count = 10

# Notification configuration (all optional)
[notifications]
# Enable notifications globally
enabled = false

# Webhook notifications
# [notifications.webhook]
# url = "https://your-webhook.example.com/hook"
# headers = { "Authorization" = "Bearer token" }

# ntfy.sh notifications
# [notifications.ntfy]
# topic = "your-topic"
# server = "https://ntfy.sh"  # optional, default is ntfy.sh
# priority = "default"  # min, low, default, high, max

# Discord notifications
# [notifications.discord]
# webhook_url = "https://discord.com/api/webhooks/..."

# Slack notifications
# [notifications.slack]
# webhook_url = "https://hooks.slack.com/services/..."

# OpenTelemetry configuration (optional, for observability)
# [otel]
# enabled = false
# endpoint = "http://localhost:4317"
# service_name = "palingenesis"
# traces = true
# metrics = true
"#
    .to_string()
}
