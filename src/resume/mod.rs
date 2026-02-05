//! Resume strategies module.

pub mod backoff;
pub mod backup;
pub mod context;
pub mod error;
pub mod new_session;
pub mod outcome;
pub mod same_session;
pub mod selector;
pub mod strategy;
pub mod time_saved;

pub use backoff::{Backoff, BackoffBuilder, BackoffConfig, BackoffError};
pub use backup::{BackupConfig, BackupError, BackupHandler, SessionBackup};
pub use context::ResumeContext;
pub use error::ResumeError;
pub use new_session::{NewSessionConfig, NewSessionStrategy, NextStepInfo, SessionCreator};
pub use outcome::ResumeOutcome;
pub use same_session::{ResumeTrigger, SameSessionConfig, SameSessionStrategy};
pub use selector::{StrategySelector, UnknownStrategy};
pub use strategy::ResumeStrategy;
pub use time_saved::{TimeSavedCalculation, calculate_time_saved, load_metrics_config};
