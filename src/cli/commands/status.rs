use serde_json::json;

use crate::daemon::pid::PidFile;
use crate::ipc::client::{IpcClient, IpcClientError};

pub async fn handle_status(json: bool) -> anyhow::Result<()> {
    let pid_file = PidFile::new();
    let pid = pid_file.read().ok();

    match IpcClient::status().await {
        Ok(status) => {
            if json {
                let output = json!({
                    "state": status.state,
                    "pid": pid,
                    "uptime_secs": status.uptime_secs,
                    "current_session": status.current_session,
                    "saves_count": status.saves_count,
                    "total_resumes": status.total_resumes,
                });
                println!("{}", serde_json::to_string_pretty(&output)?);
            } else {
                println!("palingenesis daemon: running");
                if let Some(p) = pid {
                    println!("PID: {}", p);
                }
                println!("State: {}", status.state);
                println!("Uptime: {}", format_duration(status.uptime_secs));
                if let Some(session) = &status.current_session {
                    println!("Current session: {}", session);
                } else {
                    println!("Current session: none");
                }
                println!("Saves: {}", status.saves_count);
                println!("Total resumes: {}", status.total_resumes);
            }
            Ok(())
        }
        Err(IpcClientError::NotRunning) => {
            eprintln!("Daemon not running");
            std::process::exit(1);
        }
        Err(IpcClientError::Timeout) => {
            eprintln!("Daemon unresponsive");
            std::process::exit(1);
        }
        Err(err) => Err(err.into()),
    }
}

fn format_duration(secs: u64) -> String {
    let hours = secs / 3600;
    let minutes = (secs % 3600) / 60;
    let seconds = secs % 60;

    if hours > 0 {
        format!("{}h {}m {}s", hours, minutes, seconds)
    } else if minutes > 0 {
        format!("{}m {}s", minutes, seconds)
    } else {
        format!("{}s", seconds)
    }
}
