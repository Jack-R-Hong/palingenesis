//! Configuration management module.

pub mod paths;
pub mod schema;

pub use paths::{PathError, Paths};
pub use schema::{
    Config, DaemonConfig, DiscordConfig, MonitoringConfig, NotificationsConfig, NtfyConfig,
    OtelConfig, ResumeConfig, SlackConfig, WebhookConfig,
};
