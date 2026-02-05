use clap::Parser;

#[derive(Parser, Debug)]
#[command(name = "palingenesis", author, version, about, long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(clap::Subcommand, Debug)]
pub enum Commands {
    /// Start the daemon
    Daemon {
        #[command(subcommand)]
        action: DaemonAction,
    },
    /// Show daemon status
    Status {
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// View daemon logs
    Logs {
        /// Follow log output
        #[arg(short, long)]
        follow: bool,
        /// Number of lines to show
        #[arg(short, long, default_value = "20")]
        tail: u32,
        /// Show logs since duration (e.g., "1h", "30m", "1d")
        #[arg(short, long)]
        since: Option<String>,
    },
    /// Pause monitoring
    Pause,
    /// Resume monitoring
    Resume,
    /// Start a new session
    NewSession,
    /// Configuration management
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
}

#[derive(clap::Subcommand, Debug)]
pub enum DaemonAction {
    /// Start the daemon
    Start {
        /// Run in foreground (don't daemonize)
        #[arg(short, long)]
        foreground: bool,
    },
    /// Stop the daemon
    Stop,
    /// Restart the daemon
    Restart,
    /// Reload configuration
    Reload,
    /// Show daemon status
    Status {
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
}

#[derive(clap::Subcommand, Debug)]
pub enum ConfigAction {
    /// Initialize configuration file
    Init,
    /// Show current configuration
    Show,
    /// Validate configuration
    Validate,
    /// Edit configuration
    Edit,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cli_parses_with_no_subcommand() {
        let cli = Cli::try_parse_from(["palingenesis"]);
        assert!(cli.is_ok());
        assert!(cli.unwrap().command.is_none());
    }

    #[test]
    fn test_cli_help_flag_exits_with_help_error() {
        let result = Cli::try_parse_from(["palingenesis", "--help"]);
        assert!(result.is_err());
    }

    #[test]
    fn test_cli_version_flag_exits_with_version_error() {
        let result = Cli::try_parse_from(["palingenesis", "--version"]);
        assert!(result.is_err());
    }

    #[test]
    fn test_daemon_start_command() {
        let cli = Cli::try_parse_from(["palingenesis", "daemon", "start"]).unwrap();
        match cli.command {
            Some(Commands::Daemon {
                action: DaemonAction::Start { foreground },
            }) => {
                assert!(!foreground);
            }
            _ => panic!("Expected Daemon Start command"),
        }
    }

    #[test]
    fn test_daemon_start_foreground() {
        let cli = Cli::try_parse_from(["palingenesis", "daemon", "start", "--foreground"]).unwrap();
        match cli.command {
            Some(Commands::Daemon {
                action: DaemonAction::Start { foreground },
            }) => {
                assert!(foreground);
            }
            _ => panic!("Expected Daemon Start command with foreground"),
        }
    }

    #[test]
    fn test_daemon_stop_command() {
        let cli = Cli::try_parse_from(["palingenesis", "daemon", "stop"]).unwrap();
        match cli.command {
            Some(Commands::Daemon {
                action: DaemonAction::Stop,
            }) => {}
            _ => panic!("Expected Daemon Stop command"),
        }
    }

    #[test]
    fn test_daemon_restart_command() {
        let cli = Cli::try_parse_from(["palingenesis", "daemon", "restart"]).unwrap();
        match cli.command {
            Some(Commands::Daemon {
                action: DaemonAction::Restart,
            }) => {}
            _ => panic!("Expected Daemon Restart command"),
        }
    }

    #[test]
    fn test_daemon_reload_command() {
        let cli = Cli::try_parse_from(["palingenesis", "daemon", "reload"]).unwrap();
        match cli.command {
            Some(Commands::Daemon {
                action: DaemonAction::Reload,
            }) => {}
            _ => panic!("Expected Daemon Reload command"),
        }
    }

    #[test]
    fn test_daemon_status_command() {
        let cli = Cli::try_parse_from(["palingenesis", "daemon", "status"]).unwrap();
        match cli.command {
            Some(Commands::Daemon {
                action: DaemonAction::Status { json },
            }) => {
                assert!(!json);
            }
            _ => panic!("Expected Daemon Status command"),
        }
    }

    #[test]
    fn test_daemon_status_command_with_json() {
        let cli = Cli::try_parse_from(["palingenesis", "daemon", "status", "--json"]).unwrap();
        match cli.command {
            Some(Commands::Daemon {
                action: DaemonAction::Status { json },
            }) => {
                assert!(json);
            }
            _ => panic!("Expected Daemon Status command with json flag"),
        }
    }

    #[test]
    fn test_status_command() {
        let cli = Cli::try_parse_from(["palingenesis", "status"]).unwrap();
        match cli.command {
            Some(Commands::Status { json }) => {
                assert!(!json);
            }
            _ => panic!("Expected Status command"),
        }
    }

    #[test]
    fn test_status_command_with_json() {
        let cli = Cli::try_parse_from(["palingenesis", "status", "--json"]).unwrap();
        match cli.command {
            Some(Commands::Status { json }) => {
                assert!(json);
            }
            _ => panic!("Expected Status command with json flag"),
        }
    }

    #[test]
    fn test_logs_command_defaults() {
        let cli = Cli::try_parse_from(["palingenesis", "logs"]).unwrap();
        match cli.command {
            Some(Commands::Logs {
                follow,
                tail,
                since,
            }) => {
                assert!(!follow);
                assert_eq!(tail, 20);
                assert!(since.is_none());
            }
            _ => panic!("Expected Logs command"),
        }
    }

    #[test]
    fn test_logs_command_with_flags() {
        let cli =
            Cli::try_parse_from(["palingenesis", "logs", "--follow", "--tail", "50"]).unwrap();
        match cli.command {
            Some(Commands::Logs {
                follow,
                tail,
                since,
            }) => {
                assert!(follow);
                assert_eq!(tail, 50);
                assert!(since.is_none());
            }
            _ => panic!("Expected Logs command with flags"),
        }
    }

    #[test]
    fn test_logs_command_with_since() {
        let cli = Cli::try_parse_from(["palingenesis", "logs", "--since", "1h"]).unwrap();
        match cli.command {
            Some(Commands::Logs { since, .. }) => {
                assert_eq!(since.as_deref(), Some("1h"));
            }
            _ => panic!("Expected Logs command with since"),
        }
    }

    #[test]
    fn test_pause_command() {
        let cli = Cli::try_parse_from(["palingenesis", "pause"]).unwrap();
        assert!(matches!(cli.command, Some(Commands::Pause)));
    }

    #[test]
    fn test_resume_command() {
        let cli = Cli::try_parse_from(["palingenesis", "resume"]).unwrap();
        assert!(matches!(cli.command, Some(Commands::Resume)));
    }

    #[test]
    fn test_new_session_command() {
        let cli = Cli::try_parse_from(["palingenesis", "new-session"]).unwrap();
        assert!(matches!(cli.command, Some(Commands::NewSession)));
    }

    #[test]
    fn test_config_init_command() {
        let cli = Cli::try_parse_from(["palingenesis", "config", "init"]).unwrap();
        match cli.command {
            Some(Commands::Config {
                action: ConfigAction::Init,
            }) => {}
            _ => panic!("Expected Config Init command"),
        }
    }

    #[test]
    fn test_config_show_command() {
        let cli = Cli::try_parse_from(["palingenesis", "config", "show"]).unwrap();
        match cli.command {
            Some(Commands::Config {
                action: ConfigAction::Show,
            }) => {}
            _ => panic!("Expected Config Show command"),
        }
    }

    #[test]
    fn test_config_validate_command() {
        let cli = Cli::try_parse_from(["palingenesis", "config", "validate"]).unwrap();
        match cli.command {
            Some(Commands::Config {
                action: ConfigAction::Validate,
            }) => {}
            _ => panic!("Expected Config Validate command"),
        }
    }

    #[test]
    fn test_config_edit_command() {
        let cli = Cli::try_parse_from(["palingenesis", "config", "edit"]).unwrap();
        match cli.command {
            Some(Commands::Config {
                action: ConfigAction::Edit,
            }) => {}
            _ => panic!("Expected Config Edit command"),
        }
    }

    #[test]
    fn test_invalid_command_fails() {
        let result = Cli::try_parse_from(["palingenesis", "invalid"]);
        assert!(result.is_err());
    }

    #[test]
    fn test_daemon_requires_subcommand() {
        let result = Cli::try_parse_from(["palingenesis", "daemon"]);
        assert!(result.is_err());
    }

    #[test]
    fn test_config_requires_subcommand() {
        let result = Cli::try_parse_from(["palingenesis", "config"]);
        assert!(result.is_err());
    }
}
