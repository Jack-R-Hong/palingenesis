//! Notification dispatcher module.

pub mod channel;
pub mod discord;
pub mod dispatcher;
pub mod error;
pub mod events;
pub mod ntfy;
pub mod slack;
pub mod webhook;

pub use channel::NotificationChannel;
pub use dispatcher::{DispatchSummary, Dispatcher};
pub use error::NotifyError;
pub use events::{EventSeverity, NotificationEvent};
