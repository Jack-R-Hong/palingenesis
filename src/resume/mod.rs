//! Resume strategies module.

pub mod context;
pub mod error;
pub mod new_session;
pub mod outcome;
pub mod selector;
pub mod same_session;
pub mod strategy;

pub use context::ResumeContext;
pub use error::ResumeError;
pub use new_session::{BackupHandler, NewSessionConfig, NewSessionStrategy, NextStepInfo, SessionCreator};
pub use outcome::ResumeOutcome;
pub use same_session::{ResumeTrigger, SameSessionConfig, SameSessionStrategy};
pub use selector::{StrategySelector, UnknownStrategy};
pub use strategy::ResumeStrategy;
