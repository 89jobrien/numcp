use anyhow::{Context, Result};
use clap::{CommandFactory, Parser, Subcommand};

use numcp::{config::Config, executor::NuExecutor, registry::ToolRegistry, server::NumcpServer};

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

#[tokio::main]
async fn main() -> Result<()> {
    if std::env::args().nth(1).as_deref() == Some("completions") {
        clap_complete::generate(
            clap_complete_nushell::Nushell,
            &mut Cli::command(),
            "numcp",
            &mut std::io::stdout(),
        );
        return Ok(());
    }

    let cli = Cli::parse();

    match cli.command {
        Command::Serve {
            config: config_path,
        } => {
            let path = std::path::Path::new(&config_path);
            let config =
                Config::from_file(path).with_context(|| format!("loading {config_path}"))?;
            let base = path.parent().unwrap_or(std::path::Path::new("."));
            let registry =
                ToolRegistry::load(&config, base).with_context(|| "building tool registry")?;
            let executor = NuExecutor::new().with_context(|| "initialising Nu engine")?;
            let server = NumcpServer::new(registry, executor);

            rmcp::ServiceExt::serve(server, rmcp::transport::io::stdio())
                .await
                .context("MCP server error")?
                .waiting()
                .await
                .context("waiting for MCP server to finish")?;

            Ok(())
        }
    }
}
