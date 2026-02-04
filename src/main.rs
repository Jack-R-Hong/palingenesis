use clap::Parser;

/// Agent resurrection system for continuous AI workflow execution
#[derive(Parser, Debug)]
#[command(name = "palingenesis", author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(clap::Subcommand, Debug)]
enum Commands {
    /// Start the daemon
    Daemon {
        #[command(subcommand)]
        action: DaemonAction,
    },
    /// Show daemon status
    Status,
    /// View daemon logs
    Logs {
        /// Follow log output
        #[arg(short, long)]
        follow: bool,
        /// Number of lines to show
        #[arg(short, long, default_value = "20")]
        tail: u32,
    },
    /// Pause monitoring
    Pause,
    /// Resume monitoring
    Resume,
    /// Configuration management
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
}

#[derive(clap::Subcommand, Debug)]
enum DaemonAction {
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
}

#[derive(clap::Subcommand, Debug)]
enum ConfigAction {
    /// Initialize configuration file
    Init,
    /// Show current configuration
    Show,
    /// Validate configuration
    Validate,
    /// Edit configuration
    Edit,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Some(cmd) => {
            println!("Command received: {:?}", cmd);
            println!("Implementation pending - Story 1.2+");
        }
        None => {
            println!("palingenesis - Agent resurrection daemon");
            println!("Use --help to see available commands");
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

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
    fn test_status_command() {
        let cli = Cli::try_parse_from(["palingenesis", "status"]).unwrap();
        assert!(matches!(cli.command, Some(Commands::Status)));
    }

    #[test]
    fn test_logs_command_defaults() {
        let cli = Cli::try_parse_from(["palingenesis", "logs"]).unwrap();
        match cli.command {
            Some(Commands::Logs { follow, tail }) => {
                assert!(!follow);
                assert_eq!(tail, 20);
            }
            _ => panic!("Expected Logs command"),
        }
    }

    #[test]
    fn test_logs_command_with_flags() {
        let cli =
            Cli::try_parse_from(["palingenesis", "logs", "--follow", "--tail", "50"]).unwrap();
        match cli.command {
            Some(Commands::Logs { follow, tail }) => {
                assert!(follow);
                assert_eq!(tail, 50);
            }
            _ => panic!("Expected Logs command with flags"),
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
