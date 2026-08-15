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

    /// Database path for --import-esm. Defaults to ./data/fallout3/fallout3.sqlite3.
    #[arg(long, default_value = "./data/fallout3/fallout3.sqlite3")]
    import_db: String,

    /// Load-order index byte for --import-esm (0 = base/no remap, 1-5 = DLC).
    /// Distinct indices keep DLC records from colliding in one DB — the
    /// engine normally rewrites this byte at startup by real load order.
    #[arg(long, default_value_t = 0)]
    import_index: u8,

    /// Tool mode: print config-ready `mod = "file:CRC"` lines for every
    /// .esm/.esp in a directory (base master first, others sorted). Paste
    /// the output into server.ini under [server] — this is what the
    /// GameModList policy verifies clients against.
    #[arg(long)]
    list_mod_crc: Option<String>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();

    tracing::info!(
        "Ashfall dedicated server v{}",
        ashfall_core::constants::DEDICATED_VERSION
    );

    // Tool mode: ESM import (no server startup)
    if let Some(esm_path) = cli.import_esm {
        let game: GameId = cli.import_game.parse()?;
        let db = ashfall_server::db::Database::open(std::path::Path::new(&cli.import_db))?;
        let stats = db.import_plugin_at(std::path::Path::new(&esm_path), game, cli.import_index)?;
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

    // Tool mode: list mod CRCs (no server startup)
    if let Some(dir) = cli.list_mod_crc {
        let mut files: Vec<std::path::PathBuf> = std::fs::read_dir(&dir)?
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| {
                p.extension()
                    .map(|ext| ext == "esm" || ext == "esp")
                    .unwrap_or(false)
            })
            .collect();
        // Base master first (Fallout3.esm / FalloutNV.esm), rest sorted.
        files.sort_by(|a, b| {
            let an = a.file_name().unwrap_or_default();
            let bn = b.file_name().unwrap_or_default();
            let a_base = an == "Fallout3.esm" || an == "FalloutNV.esm";
            let b_base = bn == "Fallout3.esm" || bn == "FalloutNV.esm";
            b_base.cmp(&a_base).then_with(|| an.cmp(bn))
        });
        for f in &files {
            let crc = ashfall_core::crc32::file_crc32(f)?;
            let name = f.file_name().unwrap_or_default().to_string_lossy();
            println!("mod = \"{}:{:08X}\"", name, crc);
        }
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
