// Allow deprecated cargo_bin - the deprecation is for custom build-dir edge case
// which doesn't apply to this project. See: https://docs.rs/assert_cmd
#![allow(deprecated)]

use std::fs;

use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn test_config_show_with_existing_file() {
    let temp = tempfile::tempdir().unwrap();
    let config_path = temp.path().join("config.toml");
    fs::write(
        &config_path,
        r#"
[daemon]
log_level = "debug"
http_port = 7777
"#,
    )
    .unwrap();

    Command::cargo_bin("palingenesis")
        .unwrap()
        .args(["config", "show"])
        .env("PALINGENESIS_CONFIG", &config_path)
        .assert()
        .success()
        .stdout(predicate::str::contains("log_level = \"debug\""))
        .stdout(predicate::str::contains("http_port = 7777"));
}

#[test]
fn test_config_show_with_no_file_uses_defaults() {
    let temp = tempfile::tempdir().unwrap();
    let config_path = temp.path().join("missing.toml");

    Command::cargo_bin("palingenesis")
        .unwrap()
        .args(["config", "show"])
        .env("PALINGENESIS_CONFIG", &config_path)
        .assert()
        .success()
        .stderr(predicate::str::contains("Using default configuration"))
        .stdout(predicate::str::contains("log_level = \"info\""));
}

#[test]
fn test_config_show_json_output() {
    let temp = tempfile::tempdir().unwrap();
    let config_path = temp.path().join("missing.toml");

    Command::cargo_bin("palingenesis")
        .unwrap()
        .args(["config", "show", "--json"])
        .env("PALINGENESIS_CONFIG", &config_path)
        .assert()
        .success()
        .stdout(predicate::str::starts_with("{"))
        .stdout(predicate::str::contains("\"log_level\": \"info\""));
}

#[test]
fn test_config_show_section_filter() {
    let temp = tempfile::tempdir().unwrap();
    let config_path = temp.path().join("config.toml");
    fs::write(
        &config_path,
        r#"
[daemon]
log_level = "debug"
http_port = 7777

[monitoring]
auto_detect = false
"#,
    )
    .unwrap();

    Command::cargo_bin("palingenesis")
        .unwrap()
        .args(["config", "show", "--section", "daemon"])
        .env("PALINGENESIS_CONFIG", &config_path)
        .assert()
        .success()
        .stdout(predicate::str::contains("http_port = 7777"))
        .stdout(predicate::str::contains("log_level = \"debug\""))
        .stdout(predicate::str::contains("monitoring").not());
}

#[test]
fn test_config_show_effective_env_overrides() {
    let temp = tempfile::tempdir().unwrap();
    let config_path = temp.path().join("missing.toml");

    Command::cargo_bin("palingenesis")
        .unwrap()
        .args(["config", "show", "--effective", "--json"])
        .env("PALINGENESIS_CONFIG", &config_path)
        .env("PALINGENESIS_LOG_LEVEL", "trace")
        .env("PALINGENESIS_HTTP_PORT", "9999")
        .assert()
        .success()
        .stderr(predicate::str::contains("Using environment overrides"))
        .stdout(predicate::str::contains("\"log_level\": \"trace\""))
        .stdout(predicate::str::contains("\"http_port\": 9999"));
}

#[test]
fn test_config_show_invalid_section_fails() {
    Command::cargo_bin("palingenesis")
        .unwrap()
        .args(["config", "show", "--section", "unknown"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Unknown section"));
}
