use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn test_help_shows_all_subcommands() {
    Command::cargo_bin("palingenesis")
        .unwrap()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("daemon"))
        .stdout(predicate::str::contains("status"))
        .stdout(predicate::str::contains("logs"))
        .stdout(predicate::str::contains("config"))
        .stdout(predicate::str::contains("pause"))
        .stdout(predicate::str::contains("resume"))
        .stdout(predicate::str::contains("new-session"));
}

#[test]
fn test_version_is_semver() {
    Command::cargo_bin("palingenesis")
        .unwrap()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::is_match(r"\d+\.\d+\.\d+").unwrap());
}

#[test]
fn test_daemon_help_lists_all_subcommands() {
    Command::cargo_bin("palingenesis")
        .unwrap()
        .args(["daemon", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("start"))
        .stdout(predicate::str::contains("stop"))
        .stdout(predicate::str::contains("restart"))
        .stdout(predicate::str::contains("reload"))
        .stdout(predicate::str::contains("status"));
}

#[test]
fn test_logs_help_shows_all_options() {
    Command::cargo_bin("palingenesis")
        .unwrap()
        .args(["logs", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--follow"))
        .stdout(predicate::str::contains("--tail"))
        .stdout(predicate::str::contains("--since"));
}
