use tracing::warn;

use crate::monitor::classifier::StopReason;
use crate::resume::new_session::NewSessionStrategy;
use crate::resume::same_session::SameSessionStrategy;
use crate::resume::strategy::ResumeStrategy;

#[derive(Debug, Clone, Copy)]
pub enum UnknownStrategy {
    SameSession,
    NewSession,
    Skip,
}

/// Selects the appropriate resume strategy based on stop reason.
#[derive(Debug, Clone, Copy)]
pub struct StrategySelector {
    unknown_default: UnknownStrategy,
}

impl StrategySelector {
    pub fn new() -> Self {
        Self {
            unknown_default: UnknownStrategy::Skip,
        }
    }

    pub fn with_unknown_default(unknown_default: UnknownStrategy) -> Self {
        Self { unknown_default }
    }

    /// Select strategy based on stop reason.
    /// Returns None if no resume should occur (user exit, completed).
    pub fn select(&self, reason: &StopReason) -> Option<Box<dyn ResumeStrategy>> {
        match reason {
            StopReason::RateLimit(_) => Some(Box::new(SameSessionStrategy::new())),
            StopReason::ContextExhausted(_) => Some(Box::new(NewSessionStrategy::new())),
            StopReason::UserExit(_) | StopReason::Completed => None,
            StopReason::Unknown(details) => match self.unknown_default {
                UnknownStrategy::SameSession => {
                    warn!(%details, "Unknown stop reason, defaulting to same-session resume");
                    Some(Box::new(SameSessionStrategy::new()))
                }
                UnknownStrategy::NewSession => {
                    warn!(%details, "Unknown stop reason, defaulting to new-session resume");
                    Some(Box::new(NewSessionStrategy::new()))
                }
                UnknownStrategy::Skip => {
                    warn!(%details, "Unknown stop reason, skipping resume");
                    None
                }
            },
        }
    }
}

impl Default for StrategySelector {
    fn default() -> Self {
        Self::new()
    }
}
