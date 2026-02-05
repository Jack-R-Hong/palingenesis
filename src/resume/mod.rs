//! Resume strategies module.

pub mod context;
pub mod error;
pub mod outcome;
pub mod selector;
pub mod strategy;

pub use context::ResumeContext;
pub use error::ResumeError;
pub use outcome::ResumeOutcome;
pub use selector::{NewSessionStrategy, SameSessionStrategy, StrategySelector, UnknownStrategy};
pub use strategy::ResumeStrategy;
