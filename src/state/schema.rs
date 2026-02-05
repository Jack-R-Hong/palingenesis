use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Current version of the state file schema.
pub const STATE_VERSION: u32 = 1;

/// Root state file structure.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StateFile {
    pub version: u32,
    pub daemon_state: DaemonState,
    pub current_session: Option<CurrentSession>,
    pub stats: Stats,
}

impl Default for StateFile {
    fn default() -> Self {
        Self {
            version: STATE_VERSION,
            daemon_state: DaemonState::Stopped,
            current_session: None,
            stats: Stats::default(),
        }
    }
}

/// Daemon operational states.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DaemonState {
    Monitoring,
    Paused,
    Stopped,
}

impl Default for DaemonState {
    fn default() -> Self {
        Self::Stopped
    }
}

/// Current session information.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CurrentSession {
    pub path: PathBuf,
    pub steps_completed: Vec<u32>,
    pub last_step: u32,
    pub total_steps: u32,
}

impl Default for CurrentSession {
    fn default() -> Self {
        Self {
            path: PathBuf::new(),
            steps_completed: Vec::new(),
            last_step: 0,
            total_steps: 0,
        }
    }
}

/// Daemon statistics.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Stats {
    pub saves_count: u64,
    pub total_resumes: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_resume: Option<DateTime<Utc>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_state() {
        let state = StateFile::default();
        assert_eq!(state.version, STATE_VERSION);
        assert_eq!(state.daemon_state, DaemonState::Stopped);
        assert!(state.current_session.is_none());
    }

    #[test]
    fn test_state_serialization_roundtrip() {
        let state = StateFile::default();
        let json = serde_json::to_string(&state).unwrap();
        let parsed: StateFile = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.version, state.version);
        assert_eq!(parsed.daemon_state, state.daemon_state);
        assert!(parsed.current_session.is_none());
    }
}
