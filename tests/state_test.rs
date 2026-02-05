use std::sync::Mutex;
use std::thread;

use palingenesis::state::{DaemonState, StateFile, StateStore};

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
fn test_concurrent_write_locking() {
    let _lock = ENV_LOCK.lock().unwrap();
    let temp = tempfile::tempdir().unwrap();
    let state_dir = temp.path().join("state");
    set_env_var("PALINGENESIS_STATE", &state_dir);

    let state_path = state_dir.join("state.json");
    let mut handles = Vec::new();

    for i in 0..5 {
        let path = state_path.clone();
        handles.push(thread::spawn(move || {
            let store = StateStore::with_path_and_timeout(path, std::time::Duration::from_secs(2));
            let mut state = StateFile::default();
            state.stats.saves_count = i;
            store.save(&state).unwrap();
        }));
    }

    for handle in handles {
        handle.join().unwrap();
    }

    let contents = std::fs::read_to_string(&state_path).unwrap();
    let parsed: StateFile = serde_json::from_str(&contents).unwrap();
    assert!(parsed.stats.saves_count <= 4);

    remove_env_var("PALINGENESIS_STATE");
}

#[test]
fn test_state_persists_across_restarts() {
    let _lock = ENV_LOCK.lock().unwrap();
    let temp = tempfile::tempdir().unwrap();
    let state_dir = temp.path().join("state");
    set_env_var("PALINGENESIS_STATE", &state_dir);

    let store = StateStore::new();
    let mut state = StateFile::default();
    state.daemon_state = DaemonState::Monitoring;
    state.stats.saves_count = 42;
    store.save(&state).unwrap();

    let reloaded = StateStore::new().load();
    assert_eq!(reloaded.daemon_state, DaemonState::Monitoring);
    assert_eq!(reloaded.stats.saves_count, 42);

    remove_env_var("PALINGENESIS_STATE");
}
