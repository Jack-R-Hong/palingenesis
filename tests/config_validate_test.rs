// Allow deprecated cargo_bin - the deprecation is for custom build-dir edge case
// which doesn't apply to this project. See: https://docs.rs/assert_cmd
#![allow(deprecated)]

use std::fs;

use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn test_config_validate_with_valid_file() {
    let temp = tempfile::tempdir().unwrap();
    let config_path = temp.path().join("config.toml");
    fs::write(&config_path, "").unwrap();

    Command::cargo_bin("palingenesis")
        .unwrap()
        .args(["config", "validate"])
        .env("PALINGENESIS_CONFIG", &config_path)
        .assert()
        .success()
        .stdout(predicate::str::contains("Configuration valid"));
}

#[test]
fn test_config_validate_with_no_file_uses_defaults() {
    let temp = tempfile::tempdir().unwrap();
    let config_path = temp.path().join("missing.toml");

    Command::cargo_bin("palingenesis")
        .unwrap()
        .args(["config", "validate"])
        .env("PALINGENESIS_CONFIG", &config_path)
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "No config file found, will use defaults",
        ));
}

#[test]
fn test_config_validate_reports_syntax_error() {
    let temp = tempfile::tempdir().unwrap();
    let config_path = temp.path().join("config.toml");
    fs::write(&config_path, "[daemon\nhttp_port = 7777").unwrap();

    Command::cargo_bin("palingenesis")
        .unwrap()
        .args(["config", "validate"])
        .env("PALINGENESIS_CONFIG", &config_path)
        .assert()
        .failure()
        .stderr(predicate::str::contains("Configuration syntax error"))
        .stderr(predicate::str::contains("line"));
}

#[test]
fn test_config_validate_reports_type_error() {
    let temp = tempfile::tempdir().unwrap();
    let config_path = temp.path().join("config.toml");
    fs::write(
        &config_path,
        r#"
[daemon]
http_port = "not-a-number"
"#,
    )
    .unwrap();

    Command::cargo_bin("palingenesis")
        .unwrap()
        .args(["config", "validate"])
        .env("PALINGENESIS_CONFIG", &config_path)
        .assert()
        .failure()
        .stderr(predicate::str::contains("Configuration value error"));
}

#[test]
fn test_config_validate_reports_semantic_error() {
    let temp = tempfile::tempdir().unwrap();
    let config_path = temp.path().join("config.toml");
    fs::write(
        &config_path,
        r#"
[resume]
base_delay_secs = 0
"#,
    )
    .unwrap();

    Command::cargo_bin("palingenesis")
        .unwrap()
        .args(["config", "validate"])
        .env("PALINGENESIS_CONFIG", &config_path)
        .assert()
        .failure()
        .stderr(predicate::str::contains("Configuration errors"))
        .stderr(predicate::str::contains("resume.base_delay_secs"));
}

#[test]
fn test_config_validate_uses_custom_path() {
    let temp = tempfile::tempdir().unwrap();
    let default_path = temp.path().join("default.toml");
    let custom_path = temp.path().join("custom.toml");

    fs::write(&default_path, "[daemon\nhttp_port = 7777").unwrap();
    fs::write(&custom_path, "").unwrap();

    Command::cargo_bin("palingenesis")
        .unwrap()
        .args([
            "config",
            "validate",
            "--path",
            custom_path.to_str().unwrap(),
        ])
        .env("PALINGENESIS_CONFIG", &default_path)
        .assert()
        .success()
        .stdout(predicate::str::contains("Configuration valid"));
}
