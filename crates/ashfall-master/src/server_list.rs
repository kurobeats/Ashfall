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
    #[allow(clippy::too_many_arguments)] // maps 1:1 to the wire listing packet
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
        self.servers
            .retain(|_, entry| entry.last_seen.elapsed() < cutoff);
        let removed = before - self.servers.len();
        if removed > 0 {
            tracing::info!(
                "Culled {removed} stale server(s) — {} active",
                self.servers.len()
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{IpAddr, Ipv4Addr};

    fn addr(n: u8) -> SocketAddr {
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, n)), 1770)
    }

    fn entry(players: u32, max: u32) -> ServerEntry {
        ServerEntry {
            name: "s".into(),
            map: "m".into(),
            players,
            max_players: max,
            game_type: "fo3".into(),
            mod_files: Vec::new(),
            last_seen: Instant::now(),
        }
    }

    #[test]
    fn upsert_replaces_and_lists() {
        let mut list = ServerList::new();
        list.upsert(addr(1), "alpha".into(), "map".into(), 1, 4, "fo3".into(), vec![]);
        list.upsert(addr(2), "beta".into(), "map".into(), 2, 4, "fo3".into(), vec![]);
        assert_eq!(list.all().len(), 2);
        // re-upsert updates the same entry (no duplicate)
        list.upsert(addr(1), "alpha2".into(), "map".into(), 3, 4, "fo3".into(), vec![]);
        assert_eq!(list.all().len(), 2);
        assert!(list.all().iter().any(|e| e.players == 3));
    }

    #[test]
    fn remove_drops_entry() {
        let mut list = ServerList::new();
        list.upsert(addr(1), "alpha".into(), "map".into(), 1, 4, "fo3".into(), vec![]);
        list.remove(addr(1));
        assert!(list.all().is_empty());
    }

    #[test]
    fn cull_stale_keeps_fresh_drops_old() {
        let mut list = ServerList::new();
        let mut fresh = entry(1, 4);
        let mut stale = entry(1, 4);
        // force staleness by backdating last_seen
        unsafe {
            stale.last_seen = Instant::now() - Duration::from_secs(200);
            fresh.last_seen = Instant::now() - Duration::from_secs(1);
        }
        list.servers.insert(addr(1), fresh);
        list.servers.insert(addr(2), stale);
        list.cull_stale();
        assert_eq!(list.all().len(), 1);
        assert!(list.servers.contains_key(&addr(1)));
    }
}
