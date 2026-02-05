use std::env;
use std::sync::Mutex;

use palingenesis::config::Paths;

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
fn test_env_override_integration() {
    let _lock = ENV_LOCK.lock().unwrap();
    let temp = tempfile::tempdir().unwrap();
    let config_path = temp.path().join("config.toml");

    set_env_var("PALINGENESIS_CONFIG", &config_path);
    let resolved = Paths::config_file();
    assert_eq!(resolved, config_path);
    remove_env_var("PALINGENESIS_CONFIG");
}
