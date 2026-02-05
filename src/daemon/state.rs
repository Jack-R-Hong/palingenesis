use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};

use crate::ipc::protocol::DaemonStatus;
use crate::ipc::socket::DaemonStateAccess;

pub struct DaemonState {
    start_time: Instant,
    paused: AtomicBool,
    sessions_count: AtomicU64,
    resumes_count: AtomicU64,
}

impl DaemonState {
    pub fn new() -> Self {
        Self {
            start_time: Instant::now(),
            paused: AtomicBool::new(false),
            sessions_count: AtomicU64::new(0),
            resumes_count: AtomicU64::new(0),
        }
    }

    pub fn uptime(&self) -> Duration {
        self.start_time.elapsed()
    }
}

impl Default for DaemonState {
    fn default() -> Self {
        Self::new()
    }
}

impl DaemonStateAccess for DaemonState {
    fn get_status(&self) -> DaemonStatus {
        DaemonStatus {
            state: if self.paused.load(Ordering::SeqCst) {
                "paused".to_string()
            } else {
                "monitoring".to_string()
            },
            uptime_secs: self.uptime().as_secs(),
            current_session: None,
            saves_count: self.sessions_count.load(Ordering::SeqCst),
            total_resumes: self.resumes_count.load(Ordering::SeqCst),
        }
    }

    fn pause(&self) -> Result<(), String> {
        if self.paused.swap(true, Ordering::SeqCst) {
            return Err("Daemon already paused".to_string());
        }
        Ok(())
    }

    fn resume(&self) -> Result<(), String> {
        let was_paused = self.paused.swap(false, Ordering::SeqCst);
        if !was_paused {
            return Err("Daemon is not paused".to_string());
        }
        self.resumes_count.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    fn reload_config(&self) -> Result<(), String> {
        Ok(())
    }
}
