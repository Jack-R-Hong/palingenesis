use clap::Parser;
use palingenesis::cli::{Cli, Commands};

#[test]
fn test_cli_available_from_library() {
    let cli = Cli::try_parse_from(["palingenesis", "status"]).unwrap();
    assert!(matches!(cli.command, Some(Commands::Status)));
}
