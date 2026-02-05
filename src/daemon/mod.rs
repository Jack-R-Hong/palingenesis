//! Daemon orchestration module.

pub mod pid;
pub mod shutdown;
pub mod signals;
pub mod core;
pub mod state;

pub use core::Daemon;
pub use state::DaemonState;
