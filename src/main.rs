use clap::Parser;
use palingenesis::cli::Cli;

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
