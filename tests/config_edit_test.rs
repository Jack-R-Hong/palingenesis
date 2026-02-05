// Allow deprecated cargo_bin - the deprecation is for custom build-dir edge case
// which doesn't apply to this project. See: https://docs.rs/assert_cmd
#![allow(deprecated)]

use std::fs;

use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn test_config_edit_creates_file_and_validates() {
    let temp = tempfile::tempdir().unwrap();
    let config_path = temp.path().join("config.toml");

    Command::cargo_bin("palingenesis")
        .unwrap()
        .args(["config", "edit"])
        .env("PALINGENESIS_CONFIG", &config_path)
        .env("EDITOR", "cat")
        .assert()
        .success()
        .stdout(predicate::str::contains("Config created at"))
        .stdout(predicate::str::contains("Configuration valid"));

    assert!(config_path.exists());
}

#[test]
fn test_config_edit_custom_path_skips_validation() {
    let temp = tempfile::tempdir().unwrap();
    let config_path = temp.path().join("custom.toml");
    fs::write(&config_path, "").unwrap();

    Command::cargo_bin("palingenesis")
        .unwrap()
        .args([
            "config",
            "edit",
            "--path",
            config_path.to_str().unwrap(),
            "--no-validate",
        ])
        .env("EDITOR", "cat")
        .assert()
        .success()
        .stdout(predicate::str::contains("Validation skipped"))
        .stdout(predicate::str::contains("Configuration valid").not());
}

#[test]
fn test_config_edit_uses_visual_env() {
    let temp = tempfile::tempdir().unwrap();
    let config_path = temp.path().join("config.toml");

    Command::cargo_bin("palingenesis")
        .unwrap()
        .args(["config", "edit", "--no-validate"])
        .env("PALINGENESIS_CONFIG", &config_path)
        .env("EDITOR", "")
        .env("VISUAL", "cat")
        .assert()
        .success();

    assert!(config_path.exists());
}
