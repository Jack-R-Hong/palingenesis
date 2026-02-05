use std::env;
use std::sync::Mutex;

use palingenesis::daemon::pid::{PidError, PidFile};

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
fn test_acquire_release_lifecycle() {
    let _lock = ENV_LOCK.lock().unwrap();
    let temp = tempfile::tempdir().unwrap();
    set_env_var("PALINGENESIS_RUNTIME", temp.path());

    let mut pid_file = PidFile::new();
    pid_file.acquire().unwrap();
    assert!(pid_file.path().exists());

    pid_file.release().unwrap();
    assert!(!pid_file.path().exists());

    remove_env_var("PALINGENESIS_RUNTIME");
}

#[test]
fn test_concurrent_acquire_attempts() {
    let _lock = ENV_LOCK.lock().unwrap();
    let temp = tempfile::tempdir().unwrap();
    set_env_var("PALINGENESIS_RUNTIME", temp.path());

    let mut first = PidFile::new();
    first.acquire().unwrap();

    let mut second = PidFile::new();
    let err = second.acquire().unwrap_err();
    match err {
        PidError::AlreadyRunning { .. } => {}
        other => panic!("unexpected error: {other:?}"),
    }

    first.release().unwrap();
    remove_env_var("PALINGENESIS_RUNTIME");
}
