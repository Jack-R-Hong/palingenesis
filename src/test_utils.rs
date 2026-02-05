#[cfg(test)]
use std::sync::Mutex;

#[cfg(test)]
pub(crate) static ENV_LOCK: Mutex<()> = Mutex::new(());
