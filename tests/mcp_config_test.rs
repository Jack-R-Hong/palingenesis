// Allow deprecated cargo_bin - the deprecation is for custom build-dir edge case
// which doesn't apply to this project. See: https://docs.rs/assert_cmd
#![allow(deprecated)]

use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn test_mcp_config_command_outputs_opencode_snippet() {
    Command::cargo_bin("palingenesis")
        .unwrap()
        .args(["mcp", "config"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"mcpServers\""))
        .stdout(predicate::str::contains("\"palingenesis\""))
        .stdout(predicate::str::contains("\"command\""))
        .stdout(predicate::str::contains("mcp"))
        .stdout(predicate::str::contains("serve"))
        .stdout(predicate::str::contains("~/.config/opencode/opencode.json"));
}
