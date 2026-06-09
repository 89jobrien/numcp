use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "numcp", about = "MCP server exposing Nushell tool handlers")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Start the MCP server (stdio transport)
    Serve {
        /// Path to config file
        #[arg(short, long, default_value = "numcp.toml")]
        config: String,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Serve { config } => {
            eprintln!("numcp: loading config from {config}");
            eprintln!("numcp: MCP server not yet implemented — skeleton only");
            Ok(())
        }
    }
}
