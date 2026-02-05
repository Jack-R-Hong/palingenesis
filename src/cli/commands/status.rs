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
                    "time_saved_seconds": status.time_saved_seconds,
                    "time_saved_human": format_time_saved(status.time_saved_seconds),
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
                println!(
                    "Time saved: {}",
                    format_time_saved(status.time_saved_seconds)
                );
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

fn format_time_saved(seconds: f64) -> String {
    if !seconds.is_finite() || seconds <= 0.0 {
        return "0 seconds".to_string();
    }

    if seconds < 60.0 {
        format!("{:.0} seconds", seconds)
    } else if seconds < 3600.0 {
        format!("{:.1} minutes", seconds / 60.0)
    } else if seconds < 86400.0 {
        format!("{:.1} hours", seconds / 3600.0)
    } else {
        format!("{:.1} days", seconds / 86400.0)
    }
}

#[cfg(test)]
mod tests {
    use super::format_time_saved;

    #[test]
    fn test_format_time_saved_seconds() {
        assert_eq!(format_time_saved(42.0), "42 seconds");
    }

    #[test]
    fn test_format_time_saved_minutes() {
        assert_eq!(format_time_saved(90.0), "1.5 minutes");
    }

    #[test]
    fn test_format_time_saved_hours() {
        assert_eq!(format_time_saved(3600.0), "1.0 hours");
    }

    #[test]
    fn test_format_time_saved_days() {
        assert_eq!(format_time_saved(172800.0), "2.0 days");
    }
}
