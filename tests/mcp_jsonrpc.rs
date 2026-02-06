use std::sync::Arc;

use serde_json::Value;

use palingenesis::ipc::protocol::DaemonStatus;
use palingenesis::ipc::socket::DaemonStateAccess;
use palingenesis::mcp::McpServer;

struct MockState;

impl DaemonStateAccess for MockState {
    fn get_status(&self) -> DaemonStatus {
        DaemonStatus {
            state: "monitoring".to_string(),
            uptime_secs: 0,
            current_session: None,
            saves_count: 0,
            total_resumes: 0,
            time_saved_seconds: 0.0,
            time_saved_human: None,
        }
    }

    fn pause(&self) -> Result<(), String> {
        Ok(())
    }

    fn resume(&self) -> Result<(), String> {
        Ok(())
    }

    fn new_session(&self) -> Result<(), String> {
        Ok(())
    }

    fn reload_config(&self) -> Result<(), String> {
        Ok(())
    }
}

#[test]
fn test_mcp_server_request_response_cycle() {
    let server = McpServer::new(Arc::new(MockState));
    let response = server
        .process_json_rpc(r#"{"jsonrpc":"2.0","method":"initialize","id":1}"#)
        .expect("response");
    let value: Value = serde_json::from_str(&response).expect("json");
    assert_eq!(value["id"], 1);
    assert_eq!(value["jsonrpc"], "2.0");
    assert!(value.get("result").is_some());
}

#[test]
fn test_mcp_server_batch_processing() {
    let server = McpServer::new(Arc::new(MockState));
    let response = server
        .process_json_rpc(
            r#"[{"jsonrpc":"2.0","method":"initialize","id":1},{"jsonrpc":"2.0","method":"tools/list","id":2}]"#,
        )
        .expect("response");
    let value: Value = serde_json::from_str(&response).expect("json");
    assert_eq!(value.as_array().unwrap().len(), 2);
    assert_eq!(value[0]["id"], 1);
    assert_eq!(value[1]["id"], 2);
}
