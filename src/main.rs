use clap::Parser;
use palingenesis::cli::{Cli, Commands, ConfigAction, DaemonAction, commands};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    let result = match cli.command {
        None => {
            println!("palingenesis - Agent resurrection daemon");
            println!("Use --help to see available commands");
            Ok(())
        }
        Some(Commands::Daemon { action }) => match action {
            DaemonAction::Start { foreground } => commands::daemon::handle_start(foreground).await,
            DaemonAction::Stop => commands::daemon::handle_stop().await,
            DaemonAction::Restart => commands::daemon::handle_restart().await,
            DaemonAction::Reload => commands::daemon::handle_reload().await,
            DaemonAction::Status => commands::daemon::handle_status().await,
        },
        Some(Commands::Status) => commands::status::handle_status().await,
        Some(Commands::Logs {
            follow,
            tail,
            since,
        }) => commands::logs::handle_logs(follow, tail, since).await,
        Some(Commands::Config { action }) => match action {
            ConfigAction::Init => commands::config::handle_init().await,
            ConfigAction::Show => commands::config::handle_show().await,
            ConfigAction::Validate => commands::config::handle_validate().await,
            ConfigAction::Edit => commands::config::handle_edit().await,
        },
        Some(Commands::Pause) => commands::session::handle_pause().await,
        Some(Commands::Resume) => commands::session::handle_resume().await,
        Some(Commands::NewSession) => commands::session::handle_new_session().await,
    };

    if let Err(error) = result {
        eprintln!("{error}");
        std::process::exit(2);
    }

    Ok(())
}
