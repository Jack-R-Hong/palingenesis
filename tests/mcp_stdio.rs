use std::process::Stdio;
use std::time::Duration;

use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;
use tokio::time::timeout;

fn initialize_request(id: u64) -> String {
    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "initialize",
        "params": {
            "protocolVersion": "2024-11-05",
            "clientInfo": {
                "name": "palingenesis-test",
                "version": "0.1.0"
            },
            "capabilities": {}
        }
    });
    format!("{}\n", request)
}

fn initialized_notification() -> String {
    "{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\"}\n".to_string()
}

async fn read_json_line(reader: &mut BufReader<tokio::process::ChildStdout>) -> Value {
    let mut line = String::new();
    let bytes = timeout(Duration::from_secs(5), reader.read_line(&mut line))
        .await
        .expect("timeout waiting for MCP response")
        .expect("failed to read MCP response");
    assert!(bytes > 0, "EOF received from MCP server");
    assert!(line.ends_with('\n'));
    serde_json::from_str(line.trim_end()).expect("response was not valid JSON")
}

fn spawn_mcp_server() -> tokio::process::Child {
    Command::new(assert_cmd::cargo::cargo_bin!("palingenesis"))
        .args(["mcp", "serve"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("failed to spawn MCP server")
}

#[tokio::test]
#[ignore]
async fn test_mcp_serve_handles_initialize_request() {
    let mut child = spawn_mcp_server();
    let mut stdin = child.stdin.take().expect("missing stdin");
    let stdout = child.stdout.take().expect("missing stdout");
    let mut reader = BufReader::new(stdout);

    stdin
        .write_all(initialize_request(1).as_bytes())
        .await
        .expect("failed to write initialize request");
    stdin.flush().await.expect("failed to flush stdin");

    let response = read_json_line(&mut reader).await;
    assert_eq!(response["jsonrpc"], "2.0");
    assert_eq!(response["id"], 1);
    assert!(response.get("result").is_some() || response.get("error").is_some());

    stdin
        .write_all(initialized_notification().as_bytes())
        .await
        .expect("failed to write initialized notification");
    stdin.flush().await.expect("failed to flush stdin");

    drop(stdin);
    let status = timeout(Duration::from_secs(5), child.wait())
        .await
        .expect("timeout waiting for server exit")
        .expect("failed to wait on server");
    assert!(status.success());
}

#[tokio::test]
#[ignore]
async fn test_mcp_serve_returns_parse_error_and_recovers() {
    let mut child = spawn_mcp_server();
    let mut stdin = child.stdin.take().expect("missing stdin");
    let stdout = child.stdout.take().expect("missing stdout");
    let mut reader = BufReader::new(stdout);

    stdin
        .write_all(initialize_request(2).as_bytes())
        .await
        .expect("failed to write initialize request");
    stdin.flush().await.expect("failed to flush stdin");

    let response = read_json_line(&mut reader).await;
    assert_eq!(response["jsonrpc"], "2.0");
    assert_eq!(response["id"], 2);

    stdin
        .write_all(initialized_notification().as_bytes())
        .await
        .expect("failed to write initialized notification");
    stdin.flush().await.expect("failed to flush stdin");

    stdin
        .write_all(b"{not-json}\n")
        .await
        .expect("failed to write malformed JSON");
    stdin.flush().await.expect("failed to flush stdin");

    let response = read_json_line(&mut reader).await;
    assert_eq!(response["error"]["code"].as_i64(), Some(-32700));

    stdin
        .write_all(b"{\"jsonrpc\":\"2.0\",\"id\":3,\"method\":\"ping\"}\n")
        .await
        .expect("failed to write ping request");
    stdin.flush().await.expect("failed to flush stdin");

    let response = read_json_line(&mut reader).await;
    assert_eq!(response["jsonrpc"], "2.0");
    assert_eq!(response["id"], 3);

    drop(stdin);
    let status = timeout(Duration::from_secs(5), child.wait())
        .await
        .expect("timeout waiting for server exit")
        .expect("failed to wait on server");
    assert!(status.success());
}

#[tokio::test]
#[ignore]
async fn test_mcp_serve_shutdowns_on_eof() {
    let mut child = spawn_mcp_server();
    let mut stdin = child.stdin.take().expect("missing stdin");
    let stdout = child.stdout.take().expect("missing stdout");
    let mut reader = BufReader::new(stdout);

    stdin
        .write_all(initialize_request(3).as_bytes())
        .await
        .expect("failed to write initialize request");
    stdin.flush().await.expect("failed to flush stdin");
    let _ = read_json_line(&mut reader).await;

    stdin
        .write_all(initialized_notification().as_bytes())
        .await
        .expect("failed to write initialized notification");
    stdin.flush().await.expect("failed to flush stdin");

    drop(stdin);

    let status = timeout(Duration::from_secs(5), child.wait())
        .await
        .expect("timeout waiting for server exit")
        .expect("failed to wait on server");
    assert!(status.success());
}
