use async_trait::async_trait;

use crate::notify::error::NotifyError;
use crate::notify::events::NotificationEvent;

#[async_trait]
pub trait NotificationChannel: Send + Sync {
    fn name(&self) -> &'static str;
    async fn send(&self, event: &NotificationEvent) -> Result<(), NotifyError>;
    fn is_enabled(&self) -> bool;
}
