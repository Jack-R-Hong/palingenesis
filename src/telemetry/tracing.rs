use crate::config::paths::{PathError, Paths};
use crate::config::schema::OtelConfig;
use crate::telemetry::otel;
use std::fs::File;
use std::io::{self, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tracing::Level;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::fmt::MakeWriter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

#[derive(Debug, Clone)]
pub struct TracingConfig {
    pub level: Level,
    pub log_to_file: bool,
    pub log_to_stderr: bool,
    pub json_format: bool,
}

impl Default for TracingConfig {
    fn default() -> Self {
        Self {
            level: Level::INFO,
            log_to_file: false,
            log_to_stderr: true,
            json_format: false,
        }
    }
}

impl TracingConfig {
    pub fn from_env(debug: bool) -> Self {
        let level = if debug { Level::DEBUG } else { Level::INFO };
        Self {
            level,
            ..Self::default()
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum TracingError {
    #[error("Failed to initialize state directory: {0}")]
    StateDir(#[from] PathError),

    #[error("Failed to open log file {path}: {source}")]
    LogFileOpen { path: PathBuf, source: io::Error },
}

#[derive(Debug)]
pub struct TracingGuard {
    _default_guard: tracing::subscriber::DefaultGuard,
    file: Option<Arc<Mutex<File>>>,
    otel_enabled: bool,
}

impl Drop for TracingGuard {
    fn drop(&mut self) {
        if let Some(file) = &self.file {
            if let Ok(mut handle) = file.lock() {
                let _ = handle.flush();
            }
        }

        if self.otel_enabled {
            otel::shutdown_otel();
        }
    }
}

struct FileWriter {
    file: Arc<Mutex<File>>,
}

impl io::Write for FileWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let mut guard = self
            .file
            .lock()
            .map_err(|_| io::Error::new(io::ErrorKind::Other, "log file mutex poisoned"))?;
        guard.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        let mut guard = self
            .file
            .lock()
            .map_err(|_| io::Error::new(io::ErrorKind::Other, "log file mutex poisoned"))?;
        guard.flush()
    }
}

#[derive(Clone)]
struct FileMakeWriter {
    file: Arc<Mutex<File>>,
}

impl FileMakeWriter {
    fn new(file: Arc<Mutex<File>>) -> Self {
        Self { file }
    }
}

impl<'a> MakeWriter<'a> for FileMakeWriter {
    type Writer = FileWriter;

    fn make_writer(&'a self) -> Self::Writer {
        FileWriter {
            file: Arc::clone(&self.file),
        }
    }
}

pub fn init_tracing(
    config: &TracingConfig,
    otel_config: Option<&OtelConfig>,
) -> Result<TracingGuard, TracingError> {
    let env_filter = resolve_env_filter(config);

    let file = if config.log_to_file {
        let dir = Paths::ensure_state_dir()?;
        let path = dir.join("daemon.log");
        let file = File::options()
            .create(true)
            .append(true)
            .open(&path)
            .map_err(|source| TracingError::LogFileOpen { path, source })?;
        Some(Arc::new(Mutex::new(file)))
    } else {
        None
    };

    #[cfg(feature = "otel")]
    let otel_layer = otel_config.and_then(otel::build_otel_layer);
    #[cfg(feature = "otel")]
    let otel_logs_layer = otel_config.and_then(otel::build_otel_logs_layer);
    #[cfg(feature = "otel")]
    let otel_enabled = otel_layer.is_some() || otel_logs_layer.is_some();

    #[cfg(not(feature = "otel"))]
    let otel_enabled = {
        if let Some(otel_config) = otel_config {
            if otel_config.enabled && (otel_config.traces || otel_config.logs) {
                tracing::warn!("OpenTelemetry feature not enabled; rebuild with --features otel");
            }
        }
        false
    };

    #[cfg(feature = "otel")]
    let default_guard = match (
        config.log_to_stderr,
        file.as_ref(),
        otel_layer,
        otel_logs_layer,
    ) {
        (true, Some(file_ref), otel_layer, otel_logs_layer) => {
            let file_writer = FileMakeWriter::new(Arc::clone(file_ref));
            let base = tracing_subscriber::registry()
                .with(otel_layer)
                .with(otel_logs_layer)
                .with(env_filter);
            if config.json_format {
                let stderr_layer = tracing_subscriber::fmt::layer()
                    .json()
                    .with_writer(std::io::stderr)
                    .with_target(true)
                    .with_level(true)
                    .with_timer(tracing_subscriber::fmt::time::SystemTime);
                let file_layer = tracing_subscriber::fmt::layer()
                    .json()
                    .with_writer(file_writer)
                    .with_target(true)
                    .with_level(true)
                    .with_timer(tracing_subscriber::fmt::time::SystemTime);
                base.with(stderr_layer).with(file_layer).set_default()
            } else {
                let stderr_layer = tracing_subscriber::fmt::layer()
                    .with_writer(std::io::stderr)
                    .with_target(true)
                    .with_level(true)
                    .with_timer(tracing_subscriber::fmt::time::SystemTime);
                let file_layer = tracing_subscriber::fmt::layer()
                    .with_writer(file_writer)
                    .with_target(true)
                    .with_level(true)
                    .with_timer(tracing_subscriber::fmt::time::SystemTime);
                base.with(stderr_layer).with(file_layer).set_default()
            }
        }
        (true, None, otel_layer, otel_logs_layer) => {
            let base = tracing_subscriber::registry()
                .with(otel_layer)
                .with(otel_logs_layer)
                .with(env_filter);
            if config.json_format {
                let layer = tracing_subscriber::fmt::layer()
                    .json()
                    .with_writer(std::io::stderr)
                    .with_target(true)
                    .with_level(true)
                    .with_timer(tracing_subscriber::fmt::time::SystemTime);
                base.with(layer).set_default()
            } else {
                let layer = tracing_subscriber::fmt::layer()
                    .with_writer(std::io::stderr)
                    .with_target(true)
                    .with_level(true)
                    .with_timer(tracing_subscriber::fmt::time::SystemTime);
                base.with(layer).set_default()
            }
        }
        (false, Some(file_ref), otel_layer, otel_logs_layer) => {
            let file_writer = FileMakeWriter::new(Arc::clone(file_ref));
            let base = tracing_subscriber::registry()
                .with(otel_layer)
                .with(otel_logs_layer)
                .with(env_filter);
            if config.json_format {
                let layer = tracing_subscriber::fmt::layer()
                    .json()
                    .with_writer(file_writer)
                    .with_target(true)
                    .with_level(true)
                    .with_timer(tracing_subscriber::fmt::time::SystemTime);
                base.with(layer).set_default()
            } else {
                let layer = tracing_subscriber::fmt::layer()
                    .with_writer(file_writer)
                    .with_target(true)
                    .with_level(true)
                    .with_timer(tracing_subscriber::fmt::time::SystemTime);
                base.with(layer).set_default()
            }
        }
        (false, None, otel_layer, otel_logs_layer) => tracing_subscriber::registry()
            .with(otel_layer)
            .with(otel_logs_layer)
            .with(env_filter)
            .set_default(),
    };

    #[cfg(not(feature = "otel"))]
    let default_guard = match (config.log_to_stderr, file.as_ref()) {
        (true, Some(file_ref)) => {
            let file_writer = FileMakeWriter::new(Arc::clone(file_ref));
            if config.json_format {
                let stderr_layer = tracing_subscriber::fmt::layer()
                    .json()
                    .with_writer(std::io::stderr)
                    .with_target(true)
                    .with_level(true)
                    .with_timer(tracing_subscriber::fmt::time::SystemTime);
                let file_layer = tracing_subscriber::fmt::layer()
                    .json()
                    .with_writer(file_writer)
                    .with_target(true)
                    .with_level(true)
                    .with_timer(tracing_subscriber::fmt::time::SystemTime);
                tracing_subscriber::registry()
                    .with(env_filter)
                    .with(stderr_layer)
                    .with(file_layer)
                    .set_default()
            } else {
                let stderr_layer = tracing_subscriber::fmt::layer()
                    .with_writer(std::io::stderr)
                    .with_target(true)
                    .with_level(true)
                    .with_timer(tracing_subscriber::fmt::time::SystemTime);
                let file_layer = tracing_subscriber::fmt::layer()
                    .with_writer(file_writer)
                    .with_target(true)
                    .with_level(true)
                    .with_timer(tracing_subscriber::fmt::time::SystemTime);
                tracing_subscriber::registry()
                    .with(env_filter)
                    .with(stderr_layer)
                    .with(file_layer)
                    .set_default()
            }
        }
        (true, None) => {
            if config.json_format {
                let layer = tracing_subscriber::fmt::layer()
                    .json()
                    .with_writer(std::io::stderr)
                    .with_target(true)
                    .with_level(true)
                    .with_timer(tracing_subscriber::fmt::time::SystemTime);
                tracing_subscriber::registry()
                    .with(env_filter)
                    .with(layer)
                    .set_default()
            } else {
                let layer = tracing_subscriber::fmt::layer()
                    .with_writer(std::io::stderr)
                    .with_target(true)
                    .with_level(true)
                    .with_timer(tracing_subscriber::fmt::time::SystemTime);
                tracing_subscriber::registry()
                    .with(env_filter)
                    .with(layer)
                    .set_default()
            }
        }
        (false, Some(file_ref)) => {
            let file_writer = FileMakeWriter::new(Arc::clone(file_ref));
            if config.json_format {
                let layer = tracing_subscriber::fmt::layer()
                    .json()
                    .with_writer(file_writer)
                    .with_target(true)
                    .with_level(true)
                    .with_timer(tracing_subscriber::fmt::time::SystemTime);
                tracing_subscriber::registry()
                    .with(env_filter)
                    .with(layer)
                    .set_default()
            } else {
                let layer = tracing_subscriber::fmt::layer()
                    .with_writer(file_writer)
                    .with_target(true)
                    .with_level(true)
                    .with_timer(tracing_subscriber::fmt::time::SystemTime);
                tracing_subscriber::registry()
                    .with(env_filter)
                    .with(layer)
                    .set_default()
            }
        }
        (false, None) => tracing_subscriber::registry()
            .with(env_filter)
            .set_default(),
    };

    Ok(TracingGuard {
        _default_guard: default_guard,
        file,
        otel_enabled,
    })
}

fn resolve_env_filter(config: &TracingConfig) -> EnvFilter {
    if config.level == Level::DEBUG {
        EnvFilter::new(Level::DEBUG.as_str())
    } else {
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(config.level.as_str()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::ENV_LOCK;
    use std::env;

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
    fn default_config_is_info_stderr_pretty() {
        let config = TracingConfig::default();
        assert_eq!(config.level, Level::INFO);
        assert!(!config.log_to_file);
        assert!(config.log_to_stderr);
        assert!(!config.json_format);
    }

    #[test]
    fn env_filter_uses_rust_log_when_set() {
        let _lock = ENV_LOCK.lock().unwrap();
        set_env_var("RUST_LOG", "warn");
        let config = TracingConfig::default();
        let filter = resolve_env_filter(&config);
        assert!(filter.to_string().contains("warn"));
        remove_env_var("RUST_LOG");
    }

    #[test]
    fn debug_level_overrides_rust_log() {
        let _lock = ENV_LOCK.lock().unwrap();
        set_env_var("RUST_LOG", "error");
        let config = TracingConfig {
            level: Level::DEBUG,
            ..TracingConfig::default()
        };
        let filter = resolve_env_filter(&config);
        assert!(filter.to_string().contains("debug"));
        remove_env_var("RUST_LOG");
    }

    #[test]
    fn init_tracing_writes_json_log_entry() {
        let _lock = ENV_LOCK.lock().unwrap();
        let temp = tempfile::tempdir().unwrap();

        set_env_var("PALINGENESIS_STATE", temp.path());
        remove_env_var("RUST_LOG");

        let config = TracingConfig {
            level: Level::INFO,
            log_to_file: true,
            log_to_stderr: false,
            json_format: true,
        };

        let guard = init_tracing(&config, None).unwrap();
        tracing::info!(test_field = 42, "telemetry test log");
        drop(guard);

        let log_path = temp.path().join("daemon.log");
        let contents = std::fs::read_to_string(&log_path).unwrap();
        assert!(contents.contains("telemetry test log"));
        assert!(contents.contains("\"level\""));
        assert!(contents.contains("\"target\""));
        assert!(contents.contains("test_field"));

        remove_env_var("PALINGENESIS_STATE");
    }
}
