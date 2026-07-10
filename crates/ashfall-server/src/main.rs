use ashfall_server::config::ServerConfig;
use ashfall_server::dedicated::DedicatedServer;
use clap::Parser;

/// Ashfall dedicated server — Fallout 3 / New Vegas multiplayer.
#[derive(Parser)]
#[command(name = "ashfall-server", version = ashfall_core::constants::DEDICATED_VERSION)]
struct Cli {
    /// Path to config file
    #[arg(short, long, default_value = "~/.config/ashfall/server.ini")]
    config: String,

    /// Override server port
    #[arg(short, long)]
    port: Option<u16>,

    /// Override game type (fo3 / fnv)
    #[arg(long)]
    game_type: Option<String>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();

    tracing::info!("Ashfall dedicated server v{}", ashfall_core::constants::DEDICATED_VERSION);

    let mut config = ServerConfig::load(Some(&cli.config))?;

    if let Some(port) = cli.port {
        config.server.port = port;
    }
    if let Some(game_type) = cli.game_type {
        config.server.game_type = game_type;
    }

    let server = DedicatedServer::new(config).await?;

    // Graceful shutdown on SIGINT
    tokio::select! {
        result = server.run() => {
            if let Err(e) = result {
                tracing::error!("Server error: {e}");
            }
        }
        _ = tokio::signal::ctrl_c() => {
            tracing::info!("Shutting down...");
        }
    }

    Ok(())
}
