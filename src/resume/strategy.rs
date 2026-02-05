use async_trait::async_trait;

use crate::resume::context::ResumeContext;
use crate::resume::error::ResumeError;
use crate::resume::outcome::ResumeOutcome;

/// Trait for resume strategy implementations.
#[async_trait]
pub trait ResumeStrategy: Send + Sync {
    /// Execute the resume strategy.
    async fn execute(&self, ctx: &ResumeContext) -> Result<ResumeOutcome, ResumeError>;

    /// Name of the strategy for logging.
    fn name(&self) -> &'static str;

    /// Check if retry should be attempted after this outcome.
    fn should_retry(&self, outcome: &ResumeOutcome) -> bool {
        outcome.should_retry()
    }
}
