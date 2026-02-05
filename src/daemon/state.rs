use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::RwLock;
use std::time::{Duration, Instant};

use tracing::{error, info, warn};

use crate::config::schema::Config;
use crate::config::validation::validate_config;
use crate::config::Paths;
use crate::ipc::protocol::DaemonStatus;
use crate::ipc::socket::DaemonStateAccess;
use crate::monitor::detection::detect_assistants;

pub struct DaemonState {
    start_time: Instant,
    paused: AtomicBool,
    sessions_count: AtomicU64,
    resumes_count: AtomicU64,
    config: RwLock<Config>,
    auto_detect_active: AtomicBool,
}

impl DaemonState {
    pub fn new() -> Self {
        let mut config = load_config_from_disk().unwrap_or_else(|err| {
            warn!(error = %err, "Failed to load config; using defaults");
            Config::default()
        });
        let auto_detect_active = apply_auto_detection(&mut config);
        Self {
            start_time: Instant::now(),
            paused: AtomicBool::new(false),
            sessions_count: AtomicU64::new(0),
            resumes_count: AtomicU64::new(0),
            config: RwLock::new(config),
            auto_detect_active: AtomicBool::new(auto_detect_active),
        }
    }

    pub fn uptime(&self) -> Duration {
        self.start_time.elapsed()
    }

    pub fn is_paused(&self) -> bool {
        self.paused.load(Ordering::SeqCst)
    }

    pub fn daemon_config(&self) -> Option<crate::config::schema::DaemonConfig> {
        match self.config.read() {
            Ok(guard) => Some(guard.daemon.clone()),
            Err(_) => None,
        }
    }

    pub fn monitoring_config(&self) -> Option<crate::config::schema::MonitoringConfig> {
        match self.config.read() {
            Ok(guard) => Some(guard.monitoring.clone()),
            Err(_) => None,
        }
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

    fn new_session(&self) -> Result<(), String> {
        self.sessions_count.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    fn reload_config(&self) -> Result<(), String> {
        let new_config = match load_config_from_disk() {
            Ok(config) => config,
            Err(err) => {
                error!(error = %err, "Invalid config, keeping current");
                return Err(err);
            }
        };

        let validation = validate_config(&new_config);
        if !validation.is_valid() {
            error!("Invalid config, keeping current");
            for issue in validation.errors {
                error!(field = %issue.field, message = %issue.message, "Config validation error");
            }
            return Err("Configuration validation failed".to_string());
        }

        for warning in validation.warnings {
            warn!(field = %warning.field, message = %warning.message, "Config validation warning");
        }

        let current_config = {
            let guard = self
                .config
                .read()
                .map_err(|_| "Config lock poisoned".to_string())?;
            guard.clone()
        };

        log_non_reloadable_changes(&current_config, &new_config);

        let mut new_config = new_config;
        let auto_detect_active = apply_auto_detection(&mut new_config);

        let mut guard = self
            .config
            .write()
            .map_err(|_| "Config lock poisoned".to_string())?;
        *guard = new_config;
        self.auto_detect_active
            .store(auto_detect_active, Ordering::SeqCst);

        info!("Configuration reloaded");
        Ok(())
    }
}

impl DaemonState {
    pub fn auto_detect_active(&self) -> bool {
        self.auto_detect_active.load(Ordering::SeqCst)
    }

    pub fn auto_detect_interval(&self) -> Duration {
        let guard = match self.config.read() {
            Ok(guard) => guard,
            Err(_) => return Duration::from_secs(300),
        };
        let secs = guard.monitoring.auto_detect_interval_secs.max(1);
        Duration::from_secs(secs)
    }

    pub fn refresh_auto_detected_assistants(&self) {
        if !self.auto_detect_active() {
            return;
        }

        let result = detect_assistants();
        if result.assistants.is_empty() {
            warn!("No AI assistants detected");
            return;
        }

        let mut guard = match self.config.write() {
            Ok(guard) => guard,
            Err(_) => {
                warn!("Config lock poisoned; skipping auto-detection update");
                return;
            }
        };

        let mut assistants = guard.monitoring.assistants.clone();
        let mut newly_detected = Vec::new();

        for assistant in result.assistants {
            if !assistants.contains(&assistant.name) {
                assistants.push(assistant.name.clone());
                newly_detected.push(assistant.name.clone());
                info!(
                    assistant = %assistant.name,
                    method = assistant.detected_by.as_str(),
                    session_dir = %assistant.session_dir.display(),
                    "Newly detected assistant"
                );
            }
        }

        if !newly_detected.is_empty() {
            info!("Auto-detected assistants: {:?}", assistants);
            guard.monitoring.assistants = assistants;
        }
    }
}

fn load_config_from_disk() -> Result<Config, String> {
    let path = Paths::config_file();
    if !path.exists() {
        return Ok(Config::default());
    }

    let contents = std::fs::read_to_string(&path)
        .map_err(|err| format!("Failed to read config file {}: {err}", path.display()))?;
    toml::from_str(&contents)
        .map_err(|err| format!("Failed to parse config file {}: {err}", path.display()))
}

fn log_non_reloadable_changes(old: &Config, new: &Config) {
    if old.daemon.pid_file != new.daemon.pid_file {
        warn!("Setting daemon.pid_file requires restart to take effect");
    }
    if old.daemon.socket_path != new.daemon.socket_path {
        warn!("Setting daemon.socket_path requires restart to take effect");
    }
    if old.daemon.http_enabled != new.daemon.http_enabled {
        warn!("Setting daemon.http_enabled requires restart to take effect");
    }
    if old.daemon.http_bind != new.daemon.http_bind {
        warn!("Setting daemon.http_bind requires restart to take effect");
    }
    if old.daemon.http_port != new.daemon.http_port {
        warn!("Setting daemon.http_port requires restart to take effect");
    }
    if old.daemon.log_file != new.daemon.log_file {
        warn!("Setting daemon.log_file requires restart to take effect");
    }
    if old.otel != new.otel {
        warn!("Setting otel requires restart to take effect");
    }
}

fn apply_auto_detection(config: &mut Config) -> bool {
    if !config.monitoring.assistants.is_empty() {
        info!(
            "Using configured assistants: {:?}",
            config.monitoring.assistants
        );
        return false;
    }

    if !config.monitoring.auto_detect {
        warn!("Auto-detect disabled and no assistants configured");
        return false;
    }

    info!("No assistants configured, running auto-detection");
    let detected = detect_assistants();

    if detected.assistants.is_empty() {
        warn!("No AI assistants detected");
        return true;
    }

    let names: Vec<String> = detected
        .assistants
        .iter()
        .map(|assistant| assistant.name.clone())
        .collect();
    info!("Auto-detected assistants: {:?}", names);

    for assistant in &detected.assistants {
        info!(
            assistant = %assistant.name,
            method = assistant.detected_by.as_str(),
            session_dir = %assistant.session_dir.display(),
            "Detected assistant"
        );
    }

    config.monitoring.assistants = names;
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::ENV_LOCK;
    use std::env;
    use tempfile::tempdir;

    fn set_env_var(key: &str, value: impl AsRef<std::ffi::OsStr>) {
        unsafe {
            env::set_var(key, value);
        }
    }

    fn remove_env_var(key: &str) {
        unsafe {
            env::remove_var(key);
        }
    }

    #[test]
    fn test_reload_config_valid_updates_config() {
        let _lock = ENV_LOCK.lock().unwrap();
        let temp = tempdir().unwrap();
        let config_path = temp.path().join("config.toml");
        set_env_var("PALINGENESIS_CONFIG", &config_path);

        std::fs::write(&config_path, "[daemon]\nlog_level = \"debug\"\n").unwrap();

        let state = DaemonState::new();
        assert!(state.reload_config().is_ok());

        let guard = state.config.read().unwrap();
        assert_eq!(guard.daemon.log_level, "debug");

        remove_env_var("PALINGENESIS_CONFIG");
    }

    #[test]
    fn test_reload_config_invalid_keeps_existing() {
        let _lock = ENV_LOCK.lock().unwrap();
        let temp = tempdir().unwrap();
        let config_path = temp.path().join("config.toml");
        set_env_var("PALINGENESIS_CONFIG", &config_path);

        std::fs::write(&config_path, "[daemon]\nlog_level = \"info\"\n").unwrap();
        let state = DaemonState::new();

        std::fs::write(&config_path, "[daemon]\nhttp_port = \"bad\"\n").unwrap();
        assert!(state.reload_config().is_err());

        let guard = state.config.read().unwrap();
        assert_eq!(guard.daemon.log_level, "info");

        remove_env_var("PALINGENESIS_CONFIG");
    }
}
