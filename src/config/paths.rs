use std::env;
use std::fs;
use std::io;
use std::path::PathBuf;

/// Platform-specific path resolution for palingenesis.
pub struct Paths;

#[derive(Debug, thiserror::Error)]
pub enum PathError {
    #[error("Home directory not found")]
    HomeNotFound,

    #[error("Failed to create directory {path}: {source}")]
    CreateDirectory { path: PathBuf, source: io::Error },
}

impl Paths {
    /// Returns the configuration directory path.
    /// - Linux: ~/.config/palingenesis/
    /// - macOS: ~/Library/Application Support/palingenesis/
    /// - Override: PALINGENESIS_CONFIG env var (directory derived from file path)
    pub fn config_dir() -> PathBuf {
        if let Ok(path) = env::var("PALINGENESIS_CONFIG") {
            let path = PathBuf::from(path);
            return path
                .parent()
                .filter(|p| !p.as_os_str().is_empty())
                .map(PathBuf::from)
                .unwrap_or(path);
        }

        #[cfg(target_os = "linux")]
        {
            dirs::config_dir()
                .or_else(|| dirs::home_dir().map(|home| home.join(".config")))
                .unwrap_or_else(|| PathBuf::from(".config"))
                .join("palingenesis")
        }

        #[cfg(target_os = "macos")]
        {
            dirs::config_dir()
                .or_else(|| {
                    dirs::home_dir().map(|home| home.join("Library").join("Application Support"))
                })
                .unwrap_or_else(|| PathBuf::from("~/Library/Application Support"))
                .join("palingenesis")
        }

        #[cfg(not(any(target_os = "linux", target_os = "macos")))]
        {
            PathBuf::from(".palingenesis")
        }
    }

    /// Returns the full config file path.
    pub fn config_file() -> PathBuf {
        if let Ok(path) = env::var("PALINGENESIS_CONFIG") {
            return PathBuf::from(path);
        }
        Self::config_dir().join("config.toml")
    }

    /// Returns the state directory path.
    /// - Linux: ~/.local/state/palingenesis/
    /// - macOS: ~/Library/Application Support/palingenesis/
    /// - Override: PALINGENESIS_STATE env var
    pub fn state_dir() -> PathBuf {
        if let Ok(path) = env::var("PALINGENESIS_STATE") {
            return PathBuf::from(path);
        }

        #[cfg(target_os = "linux")]
        {
            dirs::state_dir()
                .or_else(|| dirs::home_dir().map(|home| home.join(".local/state")))
                .unwrap_or_else(|| PathBuf::from(".local/state"))
                .join("palingenesis")
        }

        #[cfg(target_os = "macos")]
        {
            Self::config_dir()
        }

        #[cfg(not(any(target_os = "linux", target_os = "macos")))]
        {
            PathBuf::from(".palingenesis")
        }
    }

    /// Returns the runtime directory path (for PID file, Unix socket).
    /// - Linux: /run/user/{uid}/palingenesis/
    /// - macOS: /tmp/palingenesis-{uid}/
    /// - Override: PALINGENESIS_RUNTIME env var
    pub fn runtime_dir() -> PathBuf {
        if let Ok(path) = env::var("PALINGENESIS_RUNTIME") {
            return PathBuf::from(path);
        }

        #[cfg(target_os = "linux")]
        {
            let runtime_root = dirs::runtime_dir().unwrap_or_else(|| {
                let uid = unsafe { libc::getuid() };
                PathBuf::from(format!("/run/user/{uid}"))
            });
            runtime_root.join("palingenesis")
        }

        #[cfg(target_os = "macos")]
        {
            let uid = unsafe { libc::getuid() };
            PathBuf::from(format!("/tmp/palingenesis-{uid}"))
        }

        #[cfg(not(any(target_os = "linux", target_os = "macos")))]
        {
            PathBuf::from(".palingenesis/run")
        }
    }

    /// Ensures the config directory exists, creating it if necessary.
    pub fn ensure_config_dir() -> Result<PathBuf, PathError> {
        let dir = Self::config_dir();
        fs::create_dir_all(&dir).map_err(|source| PathError::CreateDirectory {
            path: dir.clone(),
            source,
        })?;
        Ok(dir)
    }

    /// Ensures the state directory exists, creating it if necessary.
    pub fn ensure_state_dir() -> Result<PathBuf, PathError> {
        let dir = Self::state_dir();
        fs::create_dir_all(&dir).map_err(|source| PathError::CreateDirectory {
            path: dir.clone(),
            source,
        })?;
        Ok(dir)
    }

    /// Ensures the runtime directory exists, creating it with secure permissions.
    pub fn ensure_runtime_dir() -> Result<PathBuf, PathError> {
        let dir = Self::runtime_dir();
        fs::create_dir_all(&dir).map_err(|source| PathError::CreateDirectory {
            path: dir.clone(),
            source,
        })?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Err(source) = fs::set_permissions(&dir, fs::Permissions::from_mode(0o700)) {
                return Err(PathError::CreateDirectory {
                    path: dir.clone(),
                    source,
                });
            }
        }
        Ok(dir)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::ENV_LOCK;

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
    fn test_env_override_config_file() {
        let _lock = ENV_LOCK.lock().unwrap();
        let temp = tempfile::tempdir().unwrap();
        let config_path = temp.path().join("config.toml");

        set_env_var("PALINGENESIS_CONFIG", &config_path);
        assert_eq!(Paths::config_file(), config_path);
        remove_env_var("PALINGENESIS_CONFIG");
    }

    #[test]
    fn test_env_override_config_dir() {
        let _lock = ENV_LOCK.lock().unwrap();
        let temp = tempfile::tempdir().unwrap();
        let config_path = temp.path().join("config.toml");

        set_env_var("PALINGENESIS_CONFIG", &config_path);
        assert_eq!(Paths::config_dir(), temp.path());
        remove_env_var("PALINGENESIS_CONFIG");
    }

    #[test]
    fn test_env_override_state_dir() {
        let _lock = ENV_LOCK.lock().unwrap();
        let temp = tempfile::tempdir().unwrap();
        let state_path = temp.path().join("state");

        set_env_var("PALINGENESIS_STATE", &state_path);
        assert_eq!(Paths::state_dir(), state_path);
        remove_env_var("PALINGENESIS_STATE");
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn test_linux_paths() {
        let _lock = ENV_LOCK.lock().unwrap();
        remove_env_var("PALINGENESIS_CONFIG");
        remove_env_var("PALINGENESIS_STATE");

        let config_dir = Paths::config_dir();
        let state_dir = Paths::state_dir();

        assert!(
            config_dir
                .to_string_lossy()
                .contains(".config/palingenesis")
        );
        assert!(
            state_dir
                .to_string_lossy()
                .contains(".local/state/palingenesis")
        );
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn test_macos_paths() {
        let _lock = ENV_LOCK.lock().unwrap();
        remove_env_var("PALINGENESIS_CONFIG");
        remove_env_var("PALINGENESIS_STATE");

        let config_dir = Paths::config_dir();
        let state_dir = Paths::state_dir();

        assert!(
            config_dir
                .to_string_lossy()
                .contains("Application Support/palingenesis")
        );
        assert!(
            state_dir
                .to_string_lossy()
                .contains("Application Support/palingenesis")
        );
    }

    #[test]
    fn test_directory_creation_helpers() {
        let _lock = ENV_LOCK.lock().unwrap();
        let temp = tempfile::tempdir().unwrap();
        let config_path = temp.path().join("config.toml");
        let state_path = temp.path().join("state");

        set_env_var("PALINGENESIS_CONFIG", &config_path);
        set_env_var("PALINGENESIS_STATE", &state_path);

        let config_dir = Paths::ensure_config_dir().unwrap();
        let state_dir = Paths::ensure_state_dir().unwrap();

        assert!(config_dir.exists());
        assert!(state_dir.exists());

        remove_env_var("PALINGENESIS_CONFIG");
        remove_env_var("PALINGENESIS_STATE");
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn test_runtime_dir_uses_xdg_runtime_dir() {
        let _lock = ENV_LOCK.lock().unwrap();
        remove_env_var("PALINGENESIS_RUNTIME");
        let temp = tempfile::tempdir().unwrap();
        set_env_var("XDG_RUNTIME_DIR", temp.path());

        let runtime_dir = Paths::runtime_dir();
        assert!(runtime_dir.starts_with(temp.path()));

        remove_env_var("XDG_RUNTIME_DIR");
    }

    #[test]
    fn test_env_override_runtime_dir() {
        let _lock = ENV_LOCK.lock().unwrap();
        let temp = tempfile::tempdir().unwrap();
        let runtime_path = temp.path().join("runtime");

        set_env_var("PALINGENESIS_RUNTIME", &runtime_path);
        assert_eq!(Paths::runtime_dir(), runtime_path);
        remove_env_var("PALINGENESIS_RUNTIME");
    }
}
