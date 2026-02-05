use std::path::Path;

use crate::config::schema::Config;

#[derive(Debug, Default)]
pub struct ValidationResult {
    pub errors: Vec<ValidationError>,
    pub warnings: Vec<ValidationWarning>,
}

impl ValidationResult {
    pub fn is_valid(&self) -> bool {
        self.errors.is_empty()
    }
}

#[derive(Debug)]
pub struct ValidationError {
    pub field: String,
    pub message: String,
    pub suggestion: Option<String>,
}

#[derive(Debug)]
pub struct ValidationWarning {
    pub field: String,
    pub message: String,
}

pub fn validate_config(config: &Config) -> ValidationResult {
    let mut errors = Vec::new();
    let mut warnings = Vec::new();

    validate_log_level(&config.daemon.log_level, &mut errors);

    if config.daemon.http_port == 0 {
        errors.push(ValidationError {
            field: "daemon.http_port".to_string(),
            message: "HTTP port must be between 1 and 65535".to_string(),
            suggestion: Some("Use a port between 1 and 65535".to_string()),
        });
    }

    if config.daemon.http_enabled && config.daemon.http_port < 1024 {
        warnings.push(ValidationWarning {
            field: "daemon.http_port".to_string(),
            message: format!(
                "Port {} requires elevated privileges on most systems",
                config.daemon.http_port
            ),
        });
    }

    validate_file_parent_path(
        "daemon.pid_file",
        config.daemon.pid_file.as_ref(),
        &mut errors,
        &mut warnings,
    );
    validate_file_parent_path(
        "daemon.socket_path",
        config.daemon.socket_path.as_ref(),
        &mut errors,
        &mut warnings,
    );
    validate_file_parent_path(
        "daemon.log_file",
        config.daemon.log_file.as_ref(),
        &mut errors,
        &mut warnings,
    );

    validate_dir_path(
        "monitoring.session_dir",
        &config.monitoring.session_dir,
        &mut errors,
        &mut warnings,
    );

    if config.monitoring.debounce_ms == 0 {
        errors.push(ValidationError {
            field: "monitoring.debounce_ms".to_string(),
            message: "Debounce duration must be positive".to_string(),
            suggestion: Some("Use a value of at least 1 ms".to_string()),
        });
    }

    if config.monitoring.auto_detect_interval_secs == 0 {
        errors.push(ValidationError {
            field: "monitoring.auto_detect_interval_secs".to_string(),
            message: "Auto-detect interval must be positive".to_string(),
            suggestion: Some("Use a value of at least 1 second".to_string()),
        });
    }

    if let Some(poll_interval) = config.monitoring.poll_interval_secs {
        if poll_interval == 0 {
            errors.push(ValidationError {
                field: "monitoring.poll_interval_secs".to_string(),
                message: "Polling interval must be positive".to_string(),
                suggestion: Some("Use a value of at least 1 second".to_string()),
            });
        }
    }

    if config.resume.base_delay_secs == 0 {
        errors.push(ValidationError {
            field: "resume.base_delay_secs".to_string(),
            message: "Base delay cannot be zero".to_string(),
            suggestion: Some("Use a value of at least 1 second".to_string()),
        });
    }

    if config.resume.max_delay_secs == 0 {
        errors.push(ValidationError {
            field: "resume.max_delay_secs".to_string(),
            message: "Max delay cannot be zero".to_string(),
            suggestion: Some("Use a value of at least 1 second".to_string()),
        });
    }

    if config.resume.max_delay_secs < config.resume.base_delay_secs {
        errors.push(ValidationError {
            field: "resume.max_delay_secs".to_string(),
            message: "Max delay cannot be less than base delay".to_string(),
            suggestion: None,
        });
    }

    if config.resume.enabled && config.resume.max_retries == 0 {
        warnings.push(ValidationWarning {
            field: "resume.max_retries".to_string(),
            message: "Resume enabled but max_retries is 0 (will never retry)".to_string(),
        });
    }

    if let Some(ref webhook) = config.notifications.webhook {
        if !is_http_url(&webhook.url) {
            errors.push(ValidationError {
                field: "notifications.webhook.url".to_string(),
                message: "Webhook URL must start with http:// or https://".to_string(),
                suggestion: None,
            });
        }
    }

    if let Some(ref ntfy) = config.notifications.ntfy {
        if ntfy.topic.trim().is_empty() {
            errors.push(ValidationError {
                field: "notifications.ntfy.topic".to_string(),
                message: "ntfy topic cannot be empty".to_string(),
                suggestion: None,
            });
        }
        if let Some(ref server) = ntfy.server {
            if !is_http_url(server) {
                errors.push(ValidationError {
                    field: "notifications.ntfy.server".to_string(),
                    message: "ntfy server must start with http:// or https://".to_string(),
                    suggestion: None,
                });
            }
        }
    }

    if let Some(ref otel) = config.otel {
        if let Some(ref endpoint) = otel.endpoint {
            if !is_http_url(endpoint) {
                errors.push(ValidationError {
                    field: "otel.endpoint".to_string(),
                    message: "OpenTelemetry endpoint must start with http:// or https://"
                        .to_string(),
                    suggestion: None,
                });
            }
        }
    }

    validate_bot_config(config, &mut errors, &mut warnings);

    ValidationResult { errors, warnings }
}

fn validate_bot_config(
    config: &Config,
    errors: &mut Vec<ValidationError>,
    warnings: &mut Vec<ValidationWarning>,
) {
    let bot = &config.bot;
    if !bot.enabled {
        return;
    }

    if bot.discord_public_key.is_none() && bot.slack_signing_secret.is_none() {
        errors.push(ValidationError {
            field: "bot.enabled".to_string(),
            message: "Bot enabled but no signing keys configured".to_string(),
            suggestion: Some("Set bot.discord_public_key or bot.slack_signing_secret".to_string()),
        });
    }

    if let Some(ref key) = bot.discord_public_key {
        let trimmed = key.trim();
        if trimmed.is_empty() {
            errors.push(ValidationError {
                field: "bot.discord_public_key".to_string(),
                message: "Discord public key cannot be empty".to_string(),
                suggestion: None,
            });
        } else if hex::decode(trimmed).is_err() {
            errors.push(ValidationError {
                field: "bot.discord_public_key".to_string(),
                message: "Discord public key must be hex-encoded".to_string(),
                suggestion: Some(
                    "Use the hex public key from the Discord developer portal".to_string(),
                ),
            });
        }
    }

    if let Some(ref secret) = bot.slack_signing_secret {
        if secret.trim().is_empty() {
            errors.push(ValidationError {
                field: "bot.slack_signing_secret".to_string(),
                message: "Slack signing secret cannot be empty".to_string(),
                suggestion: None,
            });
        }
    }

    if bot.authorized_users.is_empty() && !bot.allow_all_users {
        warnings.push(ValidationWarning {
            field: "bot.authorized_users".to_string(),
            message: "No authorized users configured; commands will be rejected".to_string(),
        });
    }

    for (index, user) in bot.authorized_users.iter().enumerate() {
        if user.user_id.trim().is_empty() {
            errors.push(ValidationError {
                field: format!("bot.authorized_users[{index}].user_id"),
                message: "Authorized user ID cannot be empty".to_string(),
                suggestion: None,
            });
        }
    }
}

fn validate_log_level(level: &str, errors: &mut Vec<ValidationError>) {
    let level = level.trim().to_lowercase();
    let valid = ["trace", "debug", "info", "warn", "error"];
    if !valid.iter().any(|value| *value == level) {
        errors.push(ValidationError {
            field: "daemon.log_level".to_string(),
            message: format!("Invalid log level: {level}"),
            suggestion: Some(format!("Valid levels: {}", valid.join(", "))),
        });
    }
}

fn validate_dir_path(
    field: &str,
    path: &Path,
    errors: &mut Vec<ValidationError>,
    warnings: &mut Vec<ValidationWarning>,
) {
    if path.exists() {
        if !path.is_dir() {
            errors.push(ValidationError {
                field: field.to_string(),
                message: format!("Path is not a directory: {}", path.display()),
                suggestion: None,
            });
        }
        return;
    }

    match path.parent() {
        Some(parent) if parent.exists() => warnings.push(ValidationWarning {
            field: field.to_string(),
            message: format!(
                "Directory does not exist yet but can be created: {}",
                path.display()
            ),
        }),
        Some(parent) => errors.push(ValidationError {
            field: field.to_string(),
            message: format!("Parent directory does not exist: {}", parent.display()),
            suggestion: Some("Create the parent directory or update the path".to_string()),
        }),
        None => errors.push(ValidationError {
            field: field.to_string(),
            message: "Invalid directory path".to_string(),
            suggestion: Some("Update the path to a valid directory".to_string()),
        }),
    }
}

fn validate_file_parent_path(
    field: &str,
    path: Option<&std::path::PathBuf>,
    errors: &mut Vec<ValidationError>,
    warnings: &mut Vec<ValidationWarning>,
) {
    let Some(path) = path else {
        return;
    };

    if path.exists() && path.is_dir() {
        errors.push(ValidationError {
            field: field.to_string(),
            message: format!(
                "Expected a file path but found a directory: {}",
                path.display()
            ),
            suggestion: None,
        });
        return;
    }

    match path.parent() {
        Some(parent) if parent.exists() => {}
        Some(parent) => warnings.push(ValidationWarning {
            field: field.to_string(),
            message: format!(
                "Parent directory does not exist yet but can be created: {}",
                parent.display()
            ),
        }),
        None => errors.push(ValidationError {
            field: field.to_string(),
            message: "Invalid file path".to_string(),
            suggestion: Some("Update the path to a valid file location".to_string()),
        }),
    }
}

fn is_http_url(value: &str) -> bool {
    let value = value.trim().to_lowercase();
    value.starts_with("http://") || value.starts_with("https://")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::schema::Config;

    #[test]
    fn test_validate_config_reports_invalid_log_level() {
        let mut config = Config::default();
        config.daemon.log_level = "verbose".to_string();
        let result = validate_config(&config);
        assert!(result
            .errors
            .iter()
            .any(|err| err.field == "daemon.log_level"));
    }

    #[test]
    fn test_validate_config_reports_zero_base_delay() {
        let mut config = Config::default();
        config.resume.base_delay_secs = 0;
        let result = validate_config(&config);
        assert!(result
            .errors
            .iter()
            .any(|err| err.field == "resume.base_delay_secs"));
    }

    #[test]
    fn test_validate_config_reports_invalid_webhook_url() {
        let mut config = Config::default();
        config.notifications.webhook = Some(crate::config::schema::WebhookConfig {
            url: "ftp://example.com".to_string(),
            headers: None,
        });
        let result = validate_config(&config);
        assert!(result
            .errors
            .iter()
            .any(|err| err.field == "notifications.webhook.url"));
    }

    #[test]
    fn test_validate_config_reports_missing_bot_keys() {
        let mut config = Config::default();
        config.bot.enabled = true;
        let result = validate_config(&config);
        assert!(result.errors.iter().any(|err| err.field == "bot.enabled"));
    }

    #[test]
    fn test_validate_config_reports_invalid_discord_key() {
        let mut config = Config::default();
        config.bot.enabled = true;
        config.bot.discord_public_key = Some("not-hex".to_string());
        let result = validate_config(&config);
        assert!(result
            .errors
            .iter()
            .any(|err| err.field == "bot.discord_public_key"));
    }
}
