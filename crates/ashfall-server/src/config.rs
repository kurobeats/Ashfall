//! Server configuration (ini-style, matches vaultserver.ini).

use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub server: ServerSection,
    pub scripts: ScriptSection,
    pub database: DatabaseSection,
    #[serde(default)]
    pub game: GameSection,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerSection {
    #[serde(default = "default_host")]
    pub host: String,
    #[serde(default = "default_port")]
    pub port: u16,
    #[serde(default = "default_connections")]
    pub connections: usize,
    #[serde(default = "default_announce")]
    pub announce: String,
    #[serde(default)]
    pub master_port: u16,
    #[serde(default = "default_game_type")]
    pub game_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScriptSection {
    #[serde(default = "default_scripts_path")]
    pub path: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseSection {
    #[serde(default = "default_db_path")]
    pub path: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GameSection {
    #[serde(default = "default_tick_rate")]
    pub tick_rate: u32,
    #[serde(default = "default_time_scale")]
    pub time_scale: f32,
}

// ── defaults ──

fn default_host() -> String { "0.0.0.0".into() }
fn default_port() -> u16 { 1770 }
fn default_connections() -> usize { 4 }
fn default_announce() -> String { "127.0.0.1".into() }
fn default_game_type() -> String { "fo3".into() }
fn default_scripts_path() -> PathBuf { PathBuf::from("./scripts") }
fn default_db_path() -> PathBuf { PathBuf::from("./data/fallout3.sqlite3") }
fn default_tick_rate() -> u32 { 30 }
fn default_time_scale() -> f32 { 30.0 }

impl Default for ServerConfig {
    fn default() -> Self {
        ServerConfig {
            server: ServerSection {
                host: default_host(),
                port: default_port(),
                connections: default_connections(),
                announce: default_announce(),
                master_port: 1660,
                game_type: default_game_type(),
            },
            scripts: ScriptSection { path: default_scripts_path() },
            database: DatabaseSection { path: default_db_path() },
            game: GameSection {
                tick_rate: default_tick_rate(),
                time_scale: default_time_scale(),
            },
        }
    }
}

impl ServerConfig {
    pub fn load(path: Option<&str>) -> anyhow::Result<Self> {
        let path = path.unwrap_or("~/.config/ashfall/server.ini");
        let expanded = shellexpand::tilde(path).to_string();

        match std::fs::read_to_string(&expanded) {
            Ok(content) => {
                let config: ServerConfig = toml::from_str(&content)
                    .or_else(|_| Self::parse_ini(&content))
                    .unwrap_or_default();
                Ok(config)
            }
            Err(_) => {
                tracing::warn!("No config found at {expanded}, using defaults");
                Ok(ServerConfig::default())
            }
        }
    }

    /// Simple key=value parser for ini-style configs.
    fn parse_ini(content: &str) -> anyhow::Result<ServerConfig> {
        let mut config = ServerConfig::default();

        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with(';') || line.starts_with('#') {
                continue;
            }
            if let Some((key, value)) = line.split_once('=') {
                let key = key.trim();
                let value = value.trim();

                match key {
                    "host" => config.server.host = value.to_string(),
                    "port" => config.server.port = value.parse().unwrap_or(1770),
                    "connections" => config.server.connections = value.parse().unwrap_or(4),
                    "announce" => config.server.announce = value.to_string(),
                    "master_port" => config.server.master_port = value.parse().unwrap_or(1660),
                    "game_type" => config.server.game_type = value.to_string(),
                    "scripts_path" => config.scripts.path = PathBuf::from(value),
                    "db_path" => config.database.path = PathBuf::from(value),
                    "tick_rate" => config.game.tick_rate = value.parse().unwrap_or(30),
                    "time_scale" => config.game.time_scale = value.parse().unwrap_or(30.0),
                    _ => {}
                }
            }
        }

        Ok(config)
    }

    pub fn bind_addr(&self) -> SocketAddr {
        format!("{}:{}", self.server.host, self.server.port)
            .parse()
            .unwrap_or_else(|_| "0.0.0.0:1770".parse().unwrap())
    }

    pub fn master_addr(&self) -> SocketAddr {
        format!("{}:{}", self.server.announce, self.server.master_port)
            .parse()
            .unwrap_or_else(|_| "127.0.0.1:1660".parse().unwrap())
    }

    pub fn tick_interval_ms(&self) -> u64 {
        (1000 / self.game.tick_rate) as u64
    }
}
