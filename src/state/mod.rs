//! State persistence module.

pub mod schema;
pub mod store;

pub use schema::{CurrentSession, DaemonState, StateFile, Stats, STATE_VERSION};
pub use store::{StateError, StateStore};
