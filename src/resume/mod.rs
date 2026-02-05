//! Resume strategies module.

pub mod context;
pub mod error;
pub mod outcome;
pub mod selector;
pub mod same_session;
pub mod strategy;

pub use context::ResumeContext;
pub use error::ResumeError;
pub use outcome::ResumeOutcome;
pub use same_session::{ResumeTrigger, SameSessionConfig, SameSessionStrategy};
pub use selector::{NewSessionStrategy, StrategySelector, UnknownStrategy};
pub use strategy::ResumeStrategy;
