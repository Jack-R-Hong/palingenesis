//! Daemon orchestration module.

pub mod core;
pub mod pid;
pub mod shutdown;
pub mod signals;
pub mod state;

pub use core::Daemon;
pub use state::DaemonState;
