// Allow deprecated cargo_bin - the deprecation is for custom build-dir edge case
// which doesn't apply to this project. See: https://docs.rs/assert_cmd
#![allow(deprecated)]

use std::fs;

use assert_cmd::Command;
use predicates::prelude::*;

fn default_config_header() -> &'static str {
    "# palingenesis configuration file"
}

#[test]
fn test_config_init_creates_file() {
    let temp = tempfile::tempdir().unwrap();
    let config_path = temp.path().join("config.toml");

    Command::cargo_bin("palingenesis")
        .unwrap()
        .args(["config", "init"])
        .env("PALINGENESIS_CONFIG", &config_path)
        .assert()
        .success()
        .stdout(predicate::str::contains("Config created at"));

    assert!(config_path.exists());
    let contents = fs::read_to_string(&config_path).unwrap();
    assert!(contents.contains(default_config_header()));
}

#[test]
fn test_config_init_force_overwrites() {
    let temp = tempfile::tempdir().unwrap();
    let config_path = temp.path().join("config.toml");
    fs::write(&config_path, "existing").unwrap();

    Command::cargo_bin("palingenesis")
        .unwrap()
        .args(["config", "init", "--force"])
        .env("PALINGENESIS_CONFIG", &config_path)
        .assert()
        .success();

    let contents = fs::read_to_string(&config_path).unwrap();
    assert!(contents.contains(default_config_header()));
}

#[test]
fn test_config_init_custom_path_creates_directories() {
    let temp = tempfile::tempdir().unwrap();
    let config_path = temp
        .path()
        .join("nested")
        .join("palingenesis")
        .join("config.toml");

    Command::cargo_bin("palingenesis")
        .unwrap()
        .args(["config", "init", "--path", config_path.to_str().unwrap()])
        .assert()
        .success();

    assert!(config_path.exists());
}

#[test]
fn test_config_init_prompt_declines_overwrite() {
    let temp = tempfile::tempdir().unwrap();
    let config_path = temp.path().join("config.toml");
    fs::write(&config_path, "existing").unwrap();

    Command::cargo_bin("palingenesis")
        .unwrap()
        .args(["config", "init"])
        .env("PALINGENESIS_CONFIG", &config_path)
        .write_stdin("n\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("Aborted."));

    let contents = fs::read_to_string(&config_path).unwrap();
    assert_eq!(contents, "existing");
}

#[test]
#[cfg(unix)]
fn test_config_init_sets_permissions() {
    use std::os::unix::fs::PermissionsExt;

    let temp = tempfile::tempdir().unwrap();
    let config_path = temp.path().join("config.toml");

    Command::cargo_bin("palingenesis")
        .unwrap()
        .args(["config", "init"])
        .env("PALINGENESIS_CONFIG", &config_path)
        .assert()
        .success();

    let metadata = fs::metadata(&config_path).unwrap();
    let mode = metadata.permissions().mode() & 0o777;
    assert_eq!(mode, 0o600);
}
