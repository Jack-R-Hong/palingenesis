use serde::{Deserialize, Serialize};

/// Commands that can be sent to the daemon via Unix socket.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IpcCommand {
    /// Request current daemon status.
    Status,
    /// Pause session monitoring.
    Pause,
    /// Resume session monitoring.
    Resume,
    /// Force a new session.
    NewSession,
    /// Reload configuration file.
    Reload,
}

impl IpcCommand {
    /// Parse command from text line (without newline).
    pub fn parse(line: &str) -> Option<Self> {
        match line.trim().to_ascii_uppercase().as_str() {
            "STATUS" => Some(Self::Status),
            "PAUSE" => Some(Self::Pause),
            "RESUME" => Some(Self::Resume),
            "NEW_SESSION" | "NEW-SESSION" => Some(Self::NewSession),
            "RELOAD" => Some(Self::Reload),
            _ => None,
        }
    }
}

/// Response types from the daemon.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum IpcResponse {
    /// Success response.
    Ok,
    /// Error response with message.
    Error { message: String },
    /// Status response with JSON data.
    Status(DaemonStatus),
}

/// Daemon status for STATUS command response.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DaemonStatus {
    pub state: String,
    pub uptime_secs: u64,
    pub current_session: Option<String>,
    pub saves_count: u64,
    pub total_resumes: u64,
    pub time_saved_seconds: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub time_saved_human: Option<String>,
}

impl IpcResponse {
    /// Serialize response to text format.
    pub fn to_text(&self) -> String {
        match self {
            Self::Ok => "OK\n".to_string(),
            Self::Error { message } => format!("ERR: {}\n", message),
            // DaemonStatus contains only primitive types (String, u64, Option<String>)
            // which are guaranteed to serialize successfully. unwrap_or_default() is a
            // defensive fallback that should never trigger in practice.
            Self::Status(status) => serde_json::to_string(status).unwrap_or_default() + "\n",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_command_parsing() {
        assert_eq!(IpcCommand::parse("STATUS"), Some(IpcCommand::Status));
        assert_eq!(IpcCommand::parse("status\n"), Some(IpcCommand::Status));
        assert_eq!(IpcCommand::parse("PAUSE"), Some(IpcCommand::Pause));
        assert_eq!(IpcCommand::parse("RESUME"), Some(IpcCommand::Resume));
        assert_eq!(
            IpcCommand::parse("NEW_SESSION"),
            Some(IpcCommand::NewSession)
        );
        assert_eq!(
            IpcCommand::parse("NEW-SESSION"),
            Some(IpcCommand::NewSession)
        );
        assert_eq!(IpcCommand::parse("RELOAD"), Some(IpcCommand::Reload));
        assert_eq!(IpcCommand::parse("UNKNOWN"), None);
    }

    #[test]
    fn test_response_serialization() {
        assert_eq!(IpcResponse::Ok.to_text(), "OK\n");
        assert_eq!(
            IpcResponse::Error {
                message: "test".to_string()
            }
            .to_text(),
            "ERR: test\n"
        );

        let status = DaemonStatus {
            state: "monitoring".to_string(),
            uptime_secs: 42,
            current_session: Some("/tmp/session.md".to_string()),
            saves_count: 7,
            total_resumes: 3,
            time_saved_seconds: 360.0,
            time_saved_human: Some("6.0 minutes".to_string()),
        };
        let text = IpcResponse::Status(status.clone()).to_text();
        let json = text.trim_end();
        let parsed: DaemonStatus = serde_json::from_str(json).unwrap();
        assert_eq!(parsed, status);
    }
}
