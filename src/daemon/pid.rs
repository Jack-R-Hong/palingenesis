use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process;

use tracing::{info, warn};

use crate::config::Paths;

#[derive(Debug, thiserror::Error)]
pub enum PidError {
    #[error("Daemon already running (PID: {pid})")]
    AlreadyRunning { pid: u32 },

    #[error("I/O error: {0}")]
    Io(#[from] io::Error),

    #[error("Failed to parse PID from file: {0}")]
    Parse(String),

    #[error("Failed to check process existence: {0}")]
    ProcessCheck(String),
}

#[derive(Debug)]
pub struct PidFile {
    path: PathBuf,
    acquired: bool,
}

impl PidFile {
    /// Create a new PID file handle pointing at the standard runtime location.
    pub fn new() -> Self {
        Self {
            path: Paths::runtime_dir().join("palingenesis.pid"),
            acquired: false,
        }
    }

    /// Handle an existing PID file: return error if process is running, otherwise remove stale file.
    /// Returns `Ok(())` if file was stale and removed, `Err(AlreadyRunning)` if process is alive.
    fn handle_existing_pid_file(&self) -> Result<(), PidError> {
        match self.read() {
            Ok(existing_pid) => {
                if Self::is_process_running(existing_pid)? {
                    return Err(PidError::AlreadyRunning { pid: existing_pid });
                }
                warn!(
                    pid = existing_pid,
                    path = %self.path.display(),
                    "Removing stale PID file"
                );
                self.remove()?;
            }
            Err(err) => {
                warn!(error = %err, "Failed to read PID file, removing");
                self.remove()?;
            }
        }
        Ok(())
    }

    /// Acquire the PID file lock.
    /// Returns error if another daemon is already running.
    pub fn acquire(&mut self) -> Result<(), PidError> {
        if self.path.exists() {
            self.handle_existing_pid_file()?;
        }

        Paths::ensure_runtime_dir()
            .map_err(|err| io::Error::new(io::ErrorKind::Other, format!("{err}")))?;

        let pid = process::id();
        let mut file = match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&self.path)
        {
            Ok(file) => file,
            Err(err) if err.kind() == io::ErrorKind::AlreadyExists => {
                // Race condition: another process created the file between our check and open
                self.handle_existing_pid_file()?;
                OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&self.path)?
            }
            Err(err) => return Err(err.into()),
        };

        file.write_all(pid.to_string().as_bytes())?;
        file.sync_all()?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&self.path, fs::Permissions::from_mode(0o644))?;
        }

        self.acquired = true;
        info!(pid = pid, path = %self.path.display(), "PID file created");
        Ok(())
    }

    /// Read PID from existing file.
    pub fn read(&self) -> Result<u32, PidError> {
        let mut file = File::open(&self.path)?;
        let mut contents = String::new();
        file.read_to_string(&mut contents)?;

        contents
            .trim()
            .parse()
            .map_err(|_| PidError::Parse(contents.trim().to_string()))
    }

    /// Check if the PID file is stale.
    pub fn check_stale(&self) -> Result<bool, PidError> {
        let pid = self.read()?;
        Ok(!Self::is_process_running(pid)?)
    }

    /// Check if a process with the given PID is running.
    #[cfg(target_os = "linux")]
    pub fn is_process_running(pid: u32) -> Result<bool, PidError> {
        let proc_path = PathBuf::from(format!("/proc/{pid}"));
        Ok(proc_path.exists())
    }

    #[cfg(unix)]
    #[cfg(not(target_os = "linux"))]
    pub fn is_process_running(pid: u32) -> Result<bool, PidError> {
        use nix::sys::signal::kill;
        use nix::unistd::Pid;

        match kill(Pid::from_raw(pid as i32), None) {
            Ok(_) => Ok(true),
            Err(nix::errno::Errno::ESRCH) => Ok(false),
            Err(nix::errno::Errno::EPERM) => Ok(true),
            Err(err) => Err(PidError::ProcessCheck(err.to_string())),
        }
    }

    #[cfg(not(unix))]
    pub fn is_process_running(_pid: u32) -> Result<bool, PidError> {
        Err(PidError::ProcessCheck(
            "process checks are not supported on this platform".to_string(),
        ))
    }

    /// Remove the PID file.
    pub fn remove(&self) -> Result<(), PidError> {
        if self.path.exists() {
            fs::remove_file(&self.path)?;
            info!(path = %self.path.display(), "PID file removed");
        }
        Ok(())
    }

    /// Release the PID file (call on graceful shutdown).
    pub fn release(&mut self) -> Result<(), PidError> {
        if self.acquired {
            self.remove()?;
            self.acquired = false;
        }
        Ok(())
    }

    /// Returns the PID file path.
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for PidFile {
    fn drop(&mut self) {
        if self.acquired {
            if let Err(err) = self.release() {
                eprintln!("Warning: Failed to clean up PID file: {err}");
            }
        }
    }
}

impl Default for PidFile {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

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
    fn test_pid_file_creation() {
        let _lock = ENV_LOCK.lock().unwrap();
        let temp = tempfile::tempdir().unwrap();
        set_env_var("PALINGENESIS_RUNTIME", temp.path());

        let mut pid_file = PidFile::new();
        pid_file.acquire().unwrap();

        let pid_path = temp.path().join("palingenesis.pid");
        assert!(pid_path.exists());

        let contents = fs::read_to_string(&pid_path).unwrap();
        assert_eq!(contents.trim().parse::<u32>().unwrap(), process::id());

        pid_file.release().unwrap();
        remove_env_var("PALINGENESIS_RUNTIME");
    }

    #[test]
    fn test_stale_pid_detection() {
        let _lock = ENV_LOCK.lock().unwrap();
        let temp = tempfile::tempdir().unwrap();
        set_env_var("PALINGENESIS_RUNTIME", temp.path());

        let pid_path = temp.path().join("palingenesis.pid");
        fs::create_dir_all(temp.path()).unwrap();
        fs::write(&pid_path, "4294967295").unwrap();

        let mut pid_file = PidFile::new();
        pid_file.acquire().unwrap();

        pid_file.release().unwrap();
        remove_env_var("PALINGENESIS_RUNTIME");
    }

    #[test]
    fn test_already_running_error() {
        let _lock = ENV_LOCK.lock().unwrap();
        let temp = tempfile::tempdir().unwrap();
        set_env_var("PALINGENESIS_RUNTIME", temp.path());

        let pid_path = temp.path().join("palingenesis.pid");
        fs::create_dir_all(temp.path()).unwrap();
        fs::write(&pid_path, process::id().to_string()).unwrap();

        let mut pid_file = PidFile::new();
        let err = pid_file.acquire().unwrap_err();
        match err {
            PidError::AlreadyRunning { pid } => assert_eq!(pid, process::id()),
            other => panic!("unexpected error: {other:?}"),
        }

        remove_env_var("PALINGENESIS_RUNTIME");
    }

    #[test]
    fn test_cleanup_on_release() {
        let _lock = ENV_LOCK.lock().unwrap();
        let temp = tempfile::tempdir().unwrap();
        set_env_var("PALINGENESIS_RUNTIME", temp.path());

        let pid_path = temp.path().join("palingenesis.pid");

        let mut pid_file = PidFile::new();
        pid_file.acquire().unwrap();
        assert!(pid_path.exists());

        pid_file.release().unwrap();
        assert!(!pid_path.exists());

        remove_env_var("PALINGENESIS_RUNTIME");
    }

    #[test]
    #[cfg(unix)]
    fn test_file_permissions() {
        let _lock = ENV_LOCK.lock().unwrap();
        let temp = tempfile::tempdir().unwrap();
        set_env_var("PALINGENESIS_RUNTIME", temp.path());

        let mut pid_file = PidFile::new();
        pid_file.acquire().unwrap();

        let pid_path = temp.path().join("palingenesis.pid");
        let metadata = fs::metadata(&pid_path).unwrap();
        use std::os::unix::fs::PermissionsExt;
        let mode = metadata.permissions().mode() & 0o777;
        assert_eq!(mode, 0o644);

        pid_file.release().unwrap();
        remove_env_var("PALINGENESIS_RUNTIME");
    }

    #[test]
    fn test_check_stale_returns_true_for_nonexistent_process() {
        let _lock = ENV_LOCK.lock().unwrap();
        let temp = tempfile::tempdir().unwrap();
        set_env_var("PALINGENESIS_RUNTIME", temp.path());

        let pid_path = temp.path().join("palingenesis.pid");
        fs::create_dir_all(temp.path()).unwrap();
        fs::write(&pid_path, "4294967295").unwrap();

        let pid_file = PidFile::new();
        assert!(pid_file.check_stale().unwrap());

        remove_env_var("PALINGENESIS_RUNTIME");
    }

    #[test]
    fn test_check_stale_returns_false_for_running_process() {
        let _lock = ENV_LOCK.lock().unwrap();
        let temp = tempfile::tempdir().unwrap();
        set_env_var("PALINGENESIS_RUNTIME", temp.path());

        let pid_path = temp.path().join("palingenesis.pid");
        fs::create_dir_all(temp.path()).unwrap();
        fs::write(&pid_path, process::id().to_string()).unwrap();

        let pid_file = PidFile::new();
        assert!(!pid_file.check_stale().unwrap());

        remove_env_var("PALINGENESIS_RUNTIME");
    }

    #[test]
    fn test_read_returns_error_for_invalid_content() {
        let _lock = ENV_LOCK.lock().unwrap();
        let temp = tempfile::tempdir().unwrap();
        set_env_var("PALINGENESIS_RUNTIME", temp.path());

        let pid_path = temp.path().join("palingenesis.pid");
        fs::create_dir_all(temp.path()).unwrap();
        fs::write(&pid_path, "not_a_number").unwrap();

        let pid_file = PidFile::new();
        let err = pid_file.read().unwrap_err();
        match err {
            PidError::Parse(content) => assert_eq!(content, "not_a_number"),
            other => panic!("unexpected error: {other:?}"),
        }

        remove_env_var("PALINGENESIS_RUNTIME");
    }
}
