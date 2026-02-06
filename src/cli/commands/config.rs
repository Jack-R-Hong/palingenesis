use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::{self, Command};

use anyhow::Context;
use serde::Serialize;

use crate::config::Paths;
use crate::config::schema::{Config, DiscordConfig, NtfyConfig, SlackConfig, WebhookConfig};
use crate::config::validation::validate_config;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ValidationStatus {
    Valid,
    Missing,
    Invalid,
}

pub async fn handle_init(force: bool, custom_path: Option<PathBuf>) -> anyhow::Result<()> {
    let config_path = custom_path.unwrap_or_else(Paths::config_file);

    if config_path.exists() && !force && !confirm_overwrite(&config_path)? {
        println!("Aborted.");
        return Ok(());
    }

    if let Some(parent) = config_path.parent() {
        fs::create_dir_all(parent)?;
        set_dir_permissions(parent);
    }

    let config_content = generate_default_config_toml();
    fs::write(&config_path, config_content)?;
    set_file_permissions(&config_path);

    println!("\x1b[32mConfig created at {}\x1b[0m", config_path.display());
    println!("Edit with: palingenesis config edit");

    Ok(())
}

pub async fn handle_show(
    json: bool,
    section: Option<String>,
    effective: bool,
) -> anyhow::Result<()> {
    let config_path = Paths::config_file();
    let using_defaults = !config_path.exists();

    let mut config = if using_defaults {
        Config::default()
    } else {
        load_config_from_path(&config_path)?
    };

    if effective {
        let overrides = apply_env_overrides(&mut config)?;
        if !overrides.is_empty() {
            eprintln!("Using environment overrides:");
            for (key, value) in overrides {
                eprintln!("  {key}={value}");
            }
            eprintln!();
        }
    }

    if using_defaults {
        eprintln!("Using default configuration (no config file found)");
        eprintln!("Run `palingenesis config init` to create one\n");
    }

    let output = if let Some(section_name) = section {
        format_section(&config, &section_name, json)?
    } else {
        format_config(&config, json)?
    };

    println!("{output}");
    Ok(())
}

pub async fn handle_validate(custom_path: Option<PathBuf>) -> anyhow::Result<()> {
    let config_path = custom_path.unwrap_or_else(Paths::config_file);
    match validate_config_at_path(&config_path)? {
        ValidationStatus::Valid | ValidationStatus::Missing => Ok(()),
        ValidationStatus::Invalid => {
            process::exit(1);
        }
    }
}

pub async fn handle_edit(custom_path: Option<PathBuf>, no_validate: bool) -> anyhow::Result<()> {
    let config_path = custom_path.unwrap_or_else(Paths::config_file);

    if !config_path.exists() {
        println!("No config file found. Creating default config...");
        handle_init(false, Some(config_path.clone())).await?;
    }

    let editor = find_editor()?;
    println!("Opening {} with {}...", config_path.display(), editor);

    let status = Command::new(&editor)
        .arg(&config_path)
        .status()
        .with_context(|| format!("Failed to launch editor: {editor}"))?;

    if !status.success() {
        anyhow::bail!("Editor exited with non-zero status");
    }

    if no_validate {
        println!("Validation skipped (--no-validate)");
        return Ok(());
    }

    println!("Validating configuration...");
    if let ValidationStatus::Invalid = validate_config_at_path(&config_path)? {
        eprintln!("Validation failed. You may want to re-run the editor.");
    }

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
# Auto-detect re-scan interval (seconds)
auto_detect_interval_secs = 300
# Explicit list of assistants to monitor (optional)
# assistants = ["opencode"]
# Debounce time for file events (milliseconds)
debounce_ms = 100
# Optional: Session directory override
# session_dir = "~/.opencode"
# Optional: Polling interval fallback (seconds)
# poll_interval_secs = 5

# OpenCode process monitoring configuration
[opencode]
# Enable OpenCode process monitoring
enabled = false
# OpenCode serve port
serve_port = 4096
# OpenCode serve hostname
serve_hostname = "localhost"
# Automatically restart OpenCode on crash
auto_restart = true
# Delay before restart (milliseconds)
restart_delay_ms = 1000
# Health check interval (milliseconds)
health_check_interval = 1000

# MCP server configuration
[mcp]
# Enable MCP server support
enabled = true
# MCP protocol version to advertise
protocol_version = "2024-11-05"
# Optional MCP instructions for clients
# instructions = "palingenesis MCP server"

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
# protocol = "http"  # http or grpc
# sampling_ratio = 1.0
# traces = true
# logs = false
# metrics = true
# metrics_enabled = true
"#
    .to_string()
}

fn load_config_from_path(path: &Path) -> anyhow::Result<Config> {
    let contents = fs::read_to_string(path)
        .with_context(|| format!("Failed to read config file: {}", path.display()))?;
    let config = toml::from_str(&contents)
        .with_context(|| format!("Failed to parse config file: {}", path.display()))?;
    Ok(config)
}

fn validate_config_at_path(path: &Path) -> anyhow::Result<ValidationStatus> {
    if !path.exists() {
        println!("No config file found, will use defaults");
        return Ok(ValidationStatus::Missing);
    }

    let contents = fs::read_to_string(path)
        .with_context(|| format!("Failed to read config file: {}", path.display()))?;

    if let Err(err) = toml::from_str::<toml::Value>(&contents) {
        eprintln!("\x1b[31mConfiguration syntax error:\x1b[0m");
        eprintln!("  {err}");
        if let Some((line, column)) = toml_error_location(&contents, &err) {
            eprintln!("  at line {line}, column {column}");
            eprintln!("  Suggestion: check syntax near line {line}");
        }
        return Ok(ValidationStatus::Invalid);
    }

    let config: Config = match toml::from_str(&contents) {
        Ok(config) => config,
        Err(err) => {
            eprintln!("\x1b[31mConfiguration value error:\x1b[0m");
            eprintln!("  {err}");
            eprintln!("  Suggestion: ensure values match the expected types");
            return Ok(ValidationStatus::Invalid);
        }
    };

    let result = validate_config(&config);

    for warning in &result.warnings {
        eprintln!("Warning: {}: {}", warning.field, warning.message);
    }

    if !result.is_valid() {
        eprintln!("\x1b[31mConfiguration errors:\x1b[0m");
        for error in &result.errors {
            eprintln!("  {}: {}", error.field, error.message);
            if let Some(ref suggestion) = error.suggestion {
                eprintln!("    Suggestion: {suggestion}");
            }
        }
        return Ok(ValidationStatus::Invalid);
    }

    println!("\x1b[32mConfiguration valid\x1b[0m");
    Ok(ValidationStatus::Valid)
}

fn toml_error_location(contents: &str, err: &toml::de::Error) -> Option<(usize, usize)> {
    let span = err.span()?;
    Some(line_col_from_offset(contents, span.start))
}

fn line_col_from_offset(contents: &str, offset: usize) -> (usize, usize) {
    let mut line = 1;
    let mut column = 1;
    for (idx, ch) in contents.char_indices() {
        if idx >= offset {
            break;
        }
        if ch == '\n' {
            line += 1;
            column = 1;
        } else {
            column += 1;
        }
    }
    (line, column)
}

fn find_editor() -> anyhow::Result<String> {
    if let Some(editor) = env::var("EDITOR")
        .ok()
        .filter(|value| !value.trim().is_empty())
    {
        return Ok(editor);
    }

    if let Some(visual) = env::var("VISUAL")
        .ok()
        .filter(|value| !value.trim().is_empty())
    {
        return Ok(visual);
    }

    #[cfg(unix)]
    {
        if command_exists("vi") {
            return Ok("vi".to_string());
        }

        if command_exists("nano") {
            return Ok("nano".to_string());
        }
    }

    #[cfg(windows)]
    {
        return Ok("notepad".to_string());
    }

    anyhow::bail!("No editor found. Set the EDITOR environment variable (e.g., export EDITOR=vim).")
}

#[cfg(unix)]
fn command_exists(command: &str) -> bool {
    Command::new("which")
        .arg(command)
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn format_config(config: &Config, json: bool) -> anyhow::Result<String> {
    if json {
        Ok(serde_json::to_string_pretty(config)?)
    } else {
        Ok(toml::to_string_pretty(config)?)
    }
}

fn format_section(config: &Config, section: &str, json: bool) -> anyhow::Result<String> {
    let section = section.to_lowercase();
    match section.as_str() {
        "daemon" => format_value(&config.daemon, json),
        "monitoring" => format_value(&config.monitoring, json),
        "resume" => format_value(&config.resume, json),
        "notifications" => format_value(&config.notifications, json),
        "opencode" => format_value(&config.opencode, json),
        "mcp" => format_value(&config.mcp, json),
        "otel" => {
            let otel = config.otel.clone().unwrap_or_default();
            format_value(&otel, json)
        }
        _ => anyhow::bail!(
            "Unknown section: {section}. Valid sections: daemon, monitoring, resume, notifications, opencode, mcp, otel"
        ),
    }
}

fn format_value<T: Serialize>(value: &T, json: bool) -> anyhow::Result<String> {
    if json {
        Ok(serde_json::to_string_pretty(value)?)
    } else {
        Ok(toml::to_string_pretty(value)?)
    }
}

fn apply_env_overrides(config: &mut Config) -> anyhow::Result<Vec<(String, String)>> {
    let mut overrides = Vec::new();

    apply_string_env(
        "PALINGENESIS_LOG_LEVEL",
        &mut config.daemon.log_level,
        &mut overrides,
    );
    apply_bool_env(
        "PALINGENESIS_HTTP_ENABLED",
        &mut config.daemon.http_enabled,
        &mut overrides,
    )?;
    apply_parse_env(
        "PALINGENESIS_HTTP_PORT",
        &mut config.daemon.http_port,
        &mut overrides,
    )?;
    apply_string_env(
        "PALINGENESIS_HTTP_BIND",
        &mut config.daemon.http_bind,
        &mut overrides,
    );
    apply_path_env_option(
        "PALINGENESIS_PID_FILE",
        &mut config.daemon.pid_file,
        &mut overrides,
    );
    apply_path_env_option(
        "PALINGENESIS_SOCKET_PATH",
        &mut config.daemon.socket_path,
        &mut overrides,
    );
    apply_path_env_option(
        "PALINGENESIS_LOG_FILE",
        &mut config.daemon.log_file,
        &mut overrides,
    );

    apply_path_env_value(
        "PALINGENESIS_SESSION_DIR",
        &mut config.monitoring.session_dir,
        &mut overrides,
    );
    apply_list_env(
        "PALINGENESIS_ASSISTANTS",
        &mut config.monitoring.assistants,
        &mut overrides,
    );
    apply_bool_env(
        "PALINGENESIS_AUTO_DETECT",
        &mut config.monitoring.auto_detect,
        &mut overrides,
    )?;
    apply_parse_env(
        "PALINGENESIS_DEBOUNCE_MS",
        &mut config.monitoring.debounce_ms,
        &mut overrides,
    )?;
    apply_option_parse_env(
        "PALINGENESIS_POLL_INTERVAL_SECS",
        &mut config.monitoring.poll_interval_secs,
        &mut overrides,
    )?;

    apply_bool_env(
        "PALINGENESIS_OPENCODE_ENABLED",
        &mut config.opencode.enabled,
        &mut overrides,
    )?;
    apply_parse_env(
        "PALINGENESIS_OPENCODE_SERVE_PORT",
        &mut config.opencode.serve_port,
        &mut overrides,
    )?;
    apply_string_env(
        "PALINGENESIS_OPENCODE_SERVE_HOSTNAME",
        &mut config.opencode.serve_hostname,
        &mut overrides,
    );
    apply_bool_env(
        "PALINGENESIS_OPENCODE_AUTO_RESTART",
        &mut config.opencode.auto_restart,
        &mut overrides,
    )?;
    apply_parse_env(
        "PALINGENESIS_OPENCODE_RESTART_DELAY_MS",
        &mut config.opencode.restart_delay_ms,
        &mut overrides,
    )?;
    apply_parse_env(
        "PALINGENESIS_OPENCODE_HEALTH_CHECK_INTERVAL",
        &mut config.opencode.health_check_interval,
        &mut overrides,
    )?;

    apply_bool_env(
        "PALINGENESIS_RESUME_ENABLED",
        &mut config.resume.enabled,
        &mut overrides,
    )?;
    apply_parse_env(
        "PALINGENESIS_RESUME_BASE_DELAY_SECS",
        &mut config.resume.base_delay_secs,
        &mut overrides,
    )?;
    apply_parse_env(
        "PALINGENESIS_RESUME_MAX_DELAY_SECS",
        &mut config.resume.max_delay_secs,
        &mut overrides,
    )?;
    apply_parse_env(
        "PALINGENESIS_RESUME_MAX_RETRIES",
        &mut config.resume.max_retries,
        &mut overrides,
    )?;
    apply_bool_env(
        "PALINGENESIS_RESUME_JITTER",
        &mut config.resume.jitter,
        &mut overrides,
    )?;
    apply_parse_env(
        "PALINGENESIS_RESUME_BACKUP_COUNT",
        &mut config.resume.backup_count,
        &mut overrides,
    )?;

    apply_bool_env(
        "PALINGENESIS_NOTIFICATIONS_ENABLED",
        &mut config.notifications.enabled,
        &mut overrides,
    )?;

    if let Ok(url) = env::var("PALINGENESIS_WEBHOOK_URL") {
        config.notifications.webhook = Some(WebhookConfig {
            url: url.clone(),
            headers: None,
        });
        config.notifications.enabled = true;
        overrides.push(("PALINGENESIS_WEBHOOK_URL".to_string(), url));
    }

    if let Ok(topic) = env::var("PALINGENESIS_NTFY_TOPIC") {
        let mut ntfy = NtfyConfig {
            topic: topic.clone(),
            server: None,
            priority: None,
        };
        if let Ok(server) = env::var("PALINGENESIS_NTFY_SERVER") {
            ntfy.server = Some(server.clone());
            overrides.push(("PALINGENESIS_NTFY_SERVER".to_string(), server));
        }
        if let Ok(priority) = env::var("PALINGENESIS_NTFY_PRIORITY") {
            ntfy.priority = Some(priority.clone());
            overrides.push(("PALINGENESIS_NTFY_PRIORITY".to_string(), priority));
        }
        config.notifications.ntfy = Some(ntfy);
        config.notifications.enabled = true;
        overrides.push(("PALINGENESIS_NTFY_TOPIC".to_string(), topic));
    }

    if let Ok(url) = env::var("PALINGENESIS_DISCORD_WEBHOOK_URL") {
        config.notifications.discord = Some(DiscordConfig {
            webhook_url: url.clone(),
        });
        config.notifications.enabled = true;
        overrides.push(("PALINGENESIS_DISCORD_WEBHOOK_URL".to_string(), url));
    }

    if let Ok(url) = env::var("PALINGENESIS_SLACK_WEBHOOK_URL") {
        config.notifications.slack = Some(SlackConfig {
            webhook_url: url.clone(),
        });
        config.notifications.enabled = true;
        overrides.push(("PALINGENESIS_SLACK_WEBHOOK_URL".to_string(), url));
    }

    let mut otel_config = config.otel.clone();
    let mut otel_override = false;

    if let Ok(value) = env::var("PALINGENESIS_OTEL_ENABLED") {
        let parsed = value
            .parse::<bool>()
            .context("PALINGENESIS_OTEL_ENABLED must be true/false")?;
        otel_config = Some(otel_config.unwrap_or_default());
        if let Some(ref mut otel) = otel_config {
            otel.enabled = parsed;
        }
        overrides.push(("PALINGENESIS_OTEL_ENABLED".to_string(), value));
        otel_override = true;
    }

    if let Ok(endpoint) = env::var("PALINGENESIS_OTEL_ENDPOINT") {
        otel_config = Some(otel_config.unwrap_or_default());
        if let Some(ref mut otel) = otel_config {
            otel.endpoint = endpoint.clone();
        }
        overrides.push(("PALINGENESIS_OTEL_ENDPOINT".to_string(), endpoint));
        otel_override = true;
    }

    if let Ok(name) = env::var("PALINGENESIS_OTEL_SERVICE_NAME") {
        otel_config = Some(otel_config.unwrap_or_default());
        if let Some(ref mut otel) = otel_config {
            otel.service_name = name.clone();
        }
        overrides.push(("PALINGENESIS_OTEL_SERVICE_NAME".to_string(), name));
        otel_override = true;
    }

    if let Ok(value) = env::var("PALINGENESIS_OTEL_TRACES") {
        let parsed = value
            .parse::<bool>()
            .context("PALINGENESIS_OTEL_TRACES must be true/false")?;
        otel_config = Some(otel_config.unwrap_or_default());
        if let Some(ref mut otel) = otel_config {
            otel.traces = parsed;
        }
        overrides.push(("PALINGENESIS_OTEL_TRACES".to_string(), value));
        otel_override = true;
    }

    if let Ok(value) = env::var("PALINGENESIS_OTEL_METRICS") {
        let parsed = value
            .parse::<bool>()
            .context("PALINGENESIS_OTEL_METRICS must be true/false")?;
        otel_config = Some(otel_config.unwrap_or_default());
        if let Some(ref mut otel) = otel_config {
            otel.metrics = parsed;
        }
        overrides.push(("PALINGENESIS_OTEL_METRICS".to_string(), value));
        otel_override = true;
    }

    if let Ok(value) = env::var("PALINGENESIS_OTEL_METRICS_ENABLED") {
        let parsed = value
            .parse::<bool>()
            .context("PALINGENESIS_OTEL_METRICS_ENABLED must be true/false")?;
        otel_config = Some(otel_config.unwrap_or_default());
        if let Some(ref mut otel) = otel_config {
            otel.metrics_enabled = parsed;
        }
        overrides.push(("PALINGENESIS_OTEL_METRICS_ENABLED".to_string(), value));
        otel_override = true;
    }

    if let Ok(protocol) = env::var("PALINGENESIS_OTEL_PROTOCOL") {
        otel_config = Some(otel_config.unwrap_or_default());
        if let Some(ref mut otel) = otel_config {
            otel.protocol = protocol.clone();
        }
        overrides.push(("PALINGENESIS_OTEL_PROTOCOL".to_string(), protocol));
        otel_override = true;
    }

    if let Ok(value) = env::var("PALINGENESIS_OTEL_SAMPLING_RATIO") {
        let parsed = value
            .parse::<f64>()
            .context("PALINGENESIS_OTEL_SAMPLING_RATIO must be a float")?;
        otel_config = Some(otel_config.unwrap_or_default());
        if let Some(ref mut otel) = otel_config {
            otel.sampling_ratio = parsed;
        }
        overrides.push(("PALINGENESIS_OTEL_SAMPLING_RATIO".to_string(), value));
        otel_override = true;
    }

    if otel_override {
        config.otel = otel_config;
    }

    Ok(overrides)
}

fn apply_string_env(key: &str, target: &mut String, overrides: &mut Vec<(String, String)>) {
    if let Ok(value) = env::var(key) {
        *target = value.clone();
        overrides.push((key.to_string(), value));
    }
}

fn apply_parse_env<T>(
    key: &str,
    target: &mut T,
    overrides: &mut Vec<(String, String)>,
) -> anyhow::Result<()>
where
    T: std::str::FromStr,
    T::Err: std::fmt::Display,
{
    if let Ok(value) = env::var(key) {
        *target = value
            .parse()
            .map_err(|err| anyhow::anyhow!("{key} is invalid: {err}"))?;
        overrides.push((key.to_string(), value));
    }
    Ok(())
}

fn apply_option_parse_env<T>(
    key: &str,
    target: &mut Option<T>,
    overrides: &mut Vec<(String, String)>,
) -> anyhow::Result<()>
where
    T: std::str::FromStr,
    T::Err: std::fmt::Display,
{
    if let Ok(value) = env::var(key) {
        *target = Some(
            value
                .parse()
                .map_err(|err| anyhow::anyhow!("{key} is invalid: {err}"))?,
        );
        overrides.push((key.to_string(), value));
    }
    Ok(())
}

fn apply_bool_env(
    key: &str,
    target: &mut bool,
    overrides: &mut Vec<(String, String)>,
) -> anyhow::Result<()> {
    if let Ok(value) = env::var(key) {
        *target = value
            .parse()
            .with_context(|| format!("{key} must be true/false"))?;
        overrides.push((key.to_string(), value));
    }
    Ok(())
}

fn apply_path_env_option(
    key: &str,
    target: &mut Option<PathBuf>,
    overrides: &mut Vec<(String, String)>,
) {
    if let Ok(value) = env::var(key) {
        *target = Some(PathBuf::from(&value));
        overrides.push((key.to_string(), value));
    }
}

fn apply_list_env(key: &str, target: &mut Vec<String>, overrides: &mut Vec<(String, String)>) {
    if let Ok(value) = env::var(key) {
        let list = value
            .split(',')
            .map(|item| item.trim())
            .filter(|item| !item.is_empty())
            .map(String::from)
            .collect::<Vec<_>>();
        *target = list;
        overrides.push((key.to_string(), value));
    }
}

fn apply_path_env_value(key: &str, target: &mut PathBuf, overrides: &mut Vec<(String, String)>) {
    if let Ok(value) = env::var(key) {
        *target = PathBuf::from(&value);
        overrides.push((key.to_string(), value));
    }
}
