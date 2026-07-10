//! Server list — registry of active dedicated servers.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::time::{Duration, Instant};

/// Entry for one dedicated server.
#[derive(Debug, Clone)]
pub struct ServerEntry {
    pub name: String,
    pub map: String,
    pub players: u32,
    pub max_players: u32,
    pub game_type: String,
    pub mod_files: Vec<String>,
    pub last_seen: Instant,
}

/// In-memory server registry. Culls entries older than 120s.
pub struct ServerList {
    servers: HashMap<SocketAddr, ServerEntry>,
}

impl ServerList {
    pub fn new() -> Self {
        ServerList {
            servers: HashMap::new(),
        }
    }

    /// Insert or update a server entry.
    pub fn upsert(
        &mut self,
        addr: SocketAddr,
        name: String,
        map: String,
        players: u32,
        max_players: u32,
        game_type: String,
        mod_files: Vec<String>,
    ) {
        self.servers.insert(
            addr,
            ServerEntry {
                name,
                map,
                players,
                max_players,
                game_type,
                mod_files,
                last_seen: Instant::now(),
            },
        );
    }

    /// Remove a server (deregister).
    pub fn remove(&mut self, addr: SocketAddr) {
        self.servers.remove(&addr);
    }

    /// Get all active server entries.
    pub fn all(&self) -> Vec<&ServerEntry> {
        self.servers.values().collect()
    }

    /// Remove entries not seen for >120s.
    pub fn cull_stale(&mut self) {
        let cutoff = Duration::from_secs(120);
        let before = self.servers.len();
        self.servers.retain(|_, entry| entry.last_seen.elapsed() < cutoff);
        let removed = before - self.servers.len();
        if removed > 0 {
            tracing::info!("Culled {removed} stale server(s) — {} active", self.servers.len());
        }
    }
}
