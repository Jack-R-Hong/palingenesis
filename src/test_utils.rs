#[cfg(test)]
use std::sync::Mutex;

#[cfg(test)]
pub(crate) static ENV_LOCK: Mutex<()> = Mutex::new(());

#[cfg(test)]
pub(crate) static TRACING_LOCK: Mutex<()> = Mutex::new(());
