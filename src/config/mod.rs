//! Configuration management module.

pub mod paths;
pub mod schema;
pub mod validation;

pub use paths::{PathError, Paths};
pub use schema::{
    Config, DaemonConfig, DiscordConfig, MonitoringConfig, NotificationsConfig, NtfyConfig,
    OtelConfig, ResumeConfig, SlackConfig, WebhookConfig,
};
pub use validation::{validate_config, ValidationError, ValidationResult, ValidationWarning};
