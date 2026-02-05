//! Axum HTTP server module.

pub mod events;
pub mod handlers;
pub mod server;

pub use events::EventBroadcaster;
pub use server::{AppState, HttpServer};
