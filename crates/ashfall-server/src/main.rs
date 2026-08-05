use ashfall_server::config::ServerConfig;
use ashfall_server::db::GameId;
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

    /// Tool mode: import a plugin file (.esm/.esp) into the database.
    /// Example: ashfall-server --import-esm Fallout3.esm --import-game fo3 --import-db fallout3.sqlite3
    #[arg(long)]
    import_esm: Option<String>,

    /// Game for --import-esm (fo3 / fnv). Defaults to fo3.
    #[arg(long, default_value = "fo3")]
    import_game: String,

    /// Database path for --import-esm. Defaults to ./data/fallout3.sqlite3.
    #[arg(long, default_value = "./data/fallout3.sqlite3")]
    import_db: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();

    tracing::info!("Ashfall dedicated server v{}", ashfall_core::constants::DEDICATED_VERSION);

    // Tool mode: ESM import (no server startup)
    if let Some(esm_path) = cli.import_esm {
        let game: GameId = cli.import_game.parse()?;
        let db = ashfall_server::db::Database::open(std::path::Path::new(&cli.import_db))?;
        let stats = db.import_plugin(std::path::Path::new(&esm_path), game)?;
        tracing::info!(
            "Import complete — {} records, {} weapons, {} npcs, {} items, {} containers, {} references",
            stats.records,
            stats.weapons,
            stats.npcs,
            stats.items,
            stats.containers,
            stats.references,
        );
        return Ok(());
    }

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
