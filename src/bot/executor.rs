use std::fs;
use std::io::{BufRead, BufReader};
use std::sync::Arc;

use crate::bot::commands::{BotCommand, BotCommandField, BotCommandResult};
use crate::config::paths::Paths;
use crate::daemon::state::DaemonState;
use crate::http::handlers::control::{new_session_daemon, pause_daemon, resume_daemon};
use crate::http::handlers::status::build_status_snapshot;
use crate::http::EventBroadcaster;

pub struct CommandExecutor {
    daemon_state: Arc<DaemonState>,
    events: EventBroadcaster,
}

impl CommandExecutor {
    pub fn new(daemon_state: Arc<DaemonState>, events: EventBroadcaster) -> Self {
        Self {
            daemon_state,
            events,
        }
    }

    pub fn execute(&self, command: BotCommand) -> BotCommandResult {
        match command {
            BotCommand::Status => self.execute_status(),
            BotCommand::Pause => self.execute_pause(),
            BotCommand::Resume => self.execute_resume(),
            BotCommand::Logs { tail } => self.execute_logs(tail),
            BotCommand::NewSession => self.execute_new_session(),
            BotCommand::Help => self.execute_help(),
        }
    }

    fn execute_status(&self) -> BotCommandResult {
        let snapshot = build_status_snapshot(&self.daemon_state);
        let state_label = match snapshot.state() {
            "paused" => "paused",
            "monitoring" => "running",
            other => other,
        };
        let uptime = format_duration(snapshot.stats().uptime_secs());
        let current_session = snapshot
            .current_session()
            .cloned()
            .unwrap_or_else(|| "None".to_string());
        let last_event = self
            .events
            .last_event_timestamp()
            .map(|ts| ts.to_rfc3339())
            .unwrap_or_else(|| "No events yet".to_string());

        let fields = vec![
            BotCommandField {
                name: "State".to_string(),
                value: state_label.to_string(),
                inline: true,
            },
            BotCommandField {
                name: "Uptime".to_string(),
                value: uptime,
                inline: true,
            },
            BotCommandField {
                name: "Current session".to_string(),
                value: current_session,
                inline: false,
            },
            BotCommandField {
                name: "Last event".to_string(),
                value: last_event,
                inline: false,
            },
        ];

        BotCommandResult::success("Daemon status").with_fields(fields)
    }

    fn execute_pause(&self) -> BotCommandResult {
        match pause_daemon(&self.daemon_state) {
            Ok(()) => BotCommandResult::success("Daemon paused successfully."),
            Err(err) => BotCommandResult::error(err.message),
        }
    }

    fn execute_resume(&self) -> BotCommandResult {
        match resume_daemon(&self.daemon_state) {
            Ok(()) => BotCommandResult::success("Daemon resumed successfully."),
            Err(err) => BotCommandResult::error(err.message),
        }
    }

    fn execute_new_session(&self) -> BotCommandResult {
        match new_session_daemon(&self.daemon_state) {
            Ok(session_id) => BotCommandResult::success("New session started")
                .with_body(format!("Session ID: {session_id}")),
            Err(err) => BotCommandResult::error(err.message),
        }
    }

    fn execute_logs(&self, tail: usize) -> BotCommandResult {
        let log_path = Paths::state_dir().join("daemon.log");
        if !log_path.exists() {
            return BotCommandResult::error("No log file found");
        }

        match read_log_tail(&log_path, tail) {
            Ok(lines) => {
                if lines.is_empty() {
                    return BotCommandResult::success("No log entries found");
                }
                let content = truncate_log_lines(&lines, 1700);
                let body = format!("```\n{content}\n```");
                BotCommandResult::success(format!("Last {tail} log lines")).with_body(body)
            }
            Err(err) => BotCommandResult::error(format!("Failed to read logs: {err}")),
        }
    }

    fn execute_help(&self) -> BotCommandResult {
        let body = "Available commands:\n\
            - /palin status\n\
            - /palin pause\n\
            - /palin resume\n\
            - /palin logs [--tail|-t N]\n\
            - /palin new-session\n\
            - /palin help";
        BotCommandResult::success("palingenesis bot help").with_body(body)
    }
}

fn read_log_tail(log_path: &std::path::Path, tail: usize) -> anyhow::Result<Vec<String>> {
    let file = fs::File::open(log_path)?;
    let reader = BufReader::new(file);
    let lines: Vec<String> = reader.lines().collect::<Result<_, _>>()?;
    let start = if lines.len() > tail {
        lines.len() - tail
    } else {
        0
    };
    Ok(lines[start..].to_vec())
}

fn truncate_log_lines(lines: &[String], max_chars: usize) -> String {
    let mut result = String::new();
    for line in lines {
        if result.len() + line.len() + 1 > max_chars {
            if result.is_empty() {
                result.push_str(&line[..max_chars.saturating_sub(3).min(line.len())]);
                result.push_str("...");
            } else {
                result.push_str("\n...(truncated)");
            }
            break;
        }
        if !result.is_empty() {
            result.push('\n');
        }
        result.push_str(line);
    }
    result
}

fn format_duration(seconds: u64) -> String {
    let hours = seconds / 3600;
    let minutes = (seconds % 3600) / 60;
    let secs = seconds % 60;
    if hours > 0 {
        format!("{hours}h {minutes}m {secs}s")
    } else if minutes > 0 {
        format!("{minutes}m {secs}s")
    } else {
        format!("{secs}s")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::http::EventBroadcaster;
    use crate::ipc::socket::DaemonStateAccess;

    #[test]
    fn status_command_returns_fields() {
        let executor =
            CommandExecutor::new(Arc::new(DaemonState::new()), EventBroadcaster::default());
        let result = executor.execute(BotCommand::Status);
        assert!(result.success);
        assert!(!result.fields.is_empty());
    }

    #[test]
    fn pause_command_updates_state() {
        let state = Arc::new(DaemonState::new());
        let executor = CommandExecutor::new(Arc::clone(&state), EventBroadcaster::default());
        let result = executor.execute(BotCommand::Pause);
        assert!(result.success);
        assert!(state.is_paused());
    }

    #[test]
    fn resume_command_updates_state() {
        let state = Arc::new(DaemonState::new());
        state.pause().unwrap();
        let executor = CommandExecutor::new(Arc::clone(&state), EventBroadcaster::default());
        let result = executor.execute(BotCommand::Resume);
        assert!(result.success);
        assert!(!state.is_paused());
    }
}
