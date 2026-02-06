//! Configuration management module.

pub mod paths;
pub mod schema;
pub mod validation;

pub use paths::{PathError, Paths};
pub use schema::{
    Config, DaemonConfig, DiscordConfig, McpConfig, MetricsConfig, MonitoringConfig,
    NotificationsConfig, NtfyConfig, OpenCodeConfig, OtelConfig, ResumeConfig, SlackConfig,
    WebhookConfig,
};
pub use validation::{ValidationError, ValidationResult, ValidationWarning, validate_config};
