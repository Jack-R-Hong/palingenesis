//! State persistence module.

pub mod schema;
pub mod store;

pub use schema::{CurrentSession, DaemonState, STATE_VERSION, StateFile, Stats};
pub use store::{StateError, StateStore};
