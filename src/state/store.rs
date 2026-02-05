use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::thread::sleep;
use std::time::{Duration, Instant};

use tracing::{info, warn};

use crate::config::{PathError, Paths};

use super::schema::StateFile;

const DEFAULT_LOCK_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Debug, thiserror::Error)]
pub enum StateError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("State file corrupted: {0}")]
    Corrupted(String),

    #[error("Lock acquisition timeout")]
    LockTimeout,

    #[error("Path error: {0}")]
    Path(#[from] PathError),
}

pub struct StateStore {
    path: PathBuf,
    lock_path: PathBuf,
    lock_timeout: Duration,
}

impl StateStore {
    pub fn new() -> Self {
        let path = Paths::state_dir().join("state.json");
        Self::with_path(path)
    }

    pub fn with_path(path: PathBuf) -> Self {
        let lock_path = path.with_extension("json.lock");
        Self {
            path,
            lock_path,
            lock_timeout: DEFAULT_LOCK_TIMEOUT,
        }
    }

    pub fn with_path_and_timeout(path: PathBuf, lock_timeout: Duration) -> Self {
        let lock_path = path.with_extension("json.lock");
        Self {
            path,
            lock_path,
            lock_timeout,
        }
    }

    /// Load state from file, returning default if not exists or corrupted.
    pub fn load(&self) -> StateFile {
        if !self.path.exists() {
            let default_state = StateFile::default();
            if let Err(err) = self.save(&default_state) {
                warn!(error = %err, "Failed to create initial state file");
            }
            return default_state;
        }

        match self.load_inner() {
            Ok(state) => state,
            Err(err) => {
                warn!(error = %err, "Failed to load state, using defaults");
                StateFile::default()
            }
        }
    }

    fn load_inner(&self) -> Result<StateFile, StateError> {
        let lock_file = self.open_lock_file()?;
        self.lock_shared_with_timeout(&lock_file)?;

        let mut file = File::open(&self.path)?;
        let mut contents = String::new();
        file.read_to_string(&mut contents)?;

        match serde_json::from_str(&contents) {
            Ok(state) => Ok(state),
            Err(err) => {
                self.backup_corrupted()?;
                Err(StateError::Corrupted(err.to_string()))
            }
        }
    }

    /// Save state to file with atomic write.
    pub fn save(&self, state: &StateFile) -> Result<(), StateError> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }

        let lock_file = self.open_lock_file()?;
        self.lock_exclusive_with_timeout(&lock_file)?;

        let temp_path = self.path.with_extension("json.tmp");
        let mut temp_file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&temp_path)?;

        let contents = serde_json::to_string_pretty(state)?;
        temp_file.write_all(contents.as_bytes())?;
        temp_file.sync_all()?;

        self.apply_owner_permissions(&temp_path)?;

        fs::rename(&temp_path, &self.path)?;
        self.apply_owner_permissions(&self.path)?;

        info!(path = %self.path.display(), "State persisted");
        Ok(())
    }

    fn open_lock_file(&self) -> Result<File, StateError> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&self.lock_path)?;
        self.apply_owner_permissions(&self.lock_path)?;
        Ok(file)
    }

    fn lock_shared_with_timeout(&self, file: &File) -> Result<(), StateError> {
        self.lock_with_timeout(|| fs2::FileExt::try_lock_shared(file))
    }

    fn lock_exclusive_with_timeout(&self, file: &File) -> Result<(), StateError> {
        self.lock_with_timeout(|| fs2::FileExt::try_lock_exclusive(file))
    }

    fn lock_with_timeout<F>(&self, mut try_lock: F) -> Result<(), StateError>
    where
        F: FnMut() -> std::io::Result<()>,
    {
        let start = Instant::now();
        loop {
            match try_lock() {
                Ok(()) => return Ok(()),
                Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                    if start.elapsed() >= self.lock_timeout {
                        return Err(StateError::LockTimeout);
                    }
                    sleep(Duration::from_millis(50));
                }
                Err(err) => return Err(StateError::Io(err)),
            }
        }
    }

    fn backup_corrupted(&self) -> Result<(), StateError> {
        let backup_path = self.path.with_extension("json.bak");
        warn!(
            original = %self.path.display(),
            backup = %backup_path.display(),
            "Backing up corrupted state file"
        );
        fs::copy(&self.path, &backup_path)?;
        Ok(())
    }

    fn apply_owner_permissions(&self, path: &Path) -> Result<(), StateError> {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
        }
        Ok(())
    }
}

impl Default for StateStore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn set_env_var(key: &str, value: impl AsRef<std::ffi::OsStr>) {
        unsafe {
            std::env::set_var(key, value);
        }
    }

    fn remove_env_var(key: &str) {
        unsafe {
            std::env::remove_var(key);
        }
    }

    #[test]
    fn test_default_state_initialization_creates_file() {
        let _lock = ENV_LOCK.lock().unwrap();
        let temp = tempfile::tempdir().unwrap();
        let state_dir = temp.path().join("state");
        set_env_var("PALINGENESIS_STATE", &state_dir);

        let store = StateStore::new();
        let state = store.load();

        assert_eq!(state.version, 1);
        assert!(state_dir.join("state.json").exists());

        remove_env_var("PALINGENESIS_STATE");
    }

    #[test]
    fn test_corrupted_file_recovery() {
        let temp = tempfile::tempdir().unwrap();
        let state_path = temp.path().join("state.json");
        fs::write(&state_path, "{ invalid json }").unwrap();

        let store = StateStore::with_path(state_path.clone());
        let state = store.load();

        assert_eq!(state.version, 1);
        assert!(temp.path().join("state.json.bak").exists());
    }

    #[test]
    #[cfg(unix)]
    fn test_state_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let temp = tempfile::tempdir().unwrap();
        let state_path = temp.path().join("state.json");
        let store = StateStore::with_path(state_path.clone());
        store.save(&StateFile::default()).unwrap();

        let metadata = fs::metadata(&state_path).unwrap();
        assert_eq!(metadata.permissions().mode() & 0o777, 0o600);
    }
}
