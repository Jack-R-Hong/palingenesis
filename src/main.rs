use clap::Parser;
use palingenesis::cli::{Cli, Commands, ConfigAction, DaemonAction, McpCommands, commands};

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
            DaemonAction::Status { json } => commands::daemon::handle_status(json).await,
        },
        Some(Commands::Status { json }) => commands::status::handle_status(json).await,
        Some(Commands::Logs {
            follow,
            tail,
            since,
        }) => commands::logs::handle_logs(follow, tail, since).await,
        Some(Commands::Config { action }) => match action {
            ConfigAction::Init { force, path } => commands::config::handle_init(force, path).await,
            ConfigAction::Show {
                json,
                section,
                effective,
            } => commands::config::handle_show(json, section, effective).await,
            ConfigAction::Validate { path } => commands::config::handle_validate(path).await,
            ConfigAction::Edit { path, no_validate } => {
                commands::config::handle_edit(path, no_validate).await
            }
        },
        Some(Commands::Mcp { command }) => match command {
            McpCommands::Serve => commands::mcp::handle_serve().await,
            McpCommands::Config => commands::mcp::handle_config().await,
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
