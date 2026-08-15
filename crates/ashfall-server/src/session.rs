//! Client session — per-connection state machine.

use ashfall_core::id::NetworkID;
use ashfall_core::protocol::Packet;
use ashfall_core::string_cache::StringTable;
use std::net::SocketAddr;
use std::time::Instant;

/// Session state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionState {
    Connecting,
    Authenticating,
    Loading,
    InGame,
    Disconnecting,
}

/// Per-client session data.
pub struct Session {
    pub guid: NetworkID,
    pub addr: SocketAddr,
    pub state: SessionState,
    pub player_name: String,
    pub player_id: Option<NetworkID>,
    pub current_cell: u32,
    pub cell_context: [u32; 9],
    pub created_at: Instant,
    pub last_recv: Instant,
    pub bytes_sent: u64,
    pub bytes_recv: u64,
    /// Anti-replay: last seen reliable sequence number.
    pub last_seq: Option<u16>,
    /// String dictionary for this connection — the server assigns ids, the
    /// client learns them from `Inline` payloads (see `ashfall_core::string_cache`).
    pub string_table: StringTable,
}

impl Session {
    pub fn new(guid: NetworkID, addr: SocketAddr, name: String) -> Self {
        Session {
            guid,
            addr,
            state: SessionState::Connecting,
            player_name: name,
            player_id: None,
            current_cell: 0,
            cell_context: [0; 9],
            created_at: Instant::now(),
            last_recv: Instant::now(),
            bytes_sent: 0,
            bytes_recv: 0,
            last_seq: None,
            string_table: StringTable::new(),
        }
    }

    pub fn transition(&mut self, new_state: SessionState) {
        tracing::debug!(
            "Session {} ({}) {:?} → {:?}",
            self.guid,
            self.player_name,
            self.state,
            new_state
        );
        self.state = new_state;
    }

    pub fn is_active(&self) -> bool {
        !matches!(self.state, SessionState::Disconnecting)
    }

    pub fn is_ingame(&self) -> bool {
        self.state == SessionState::InGame
    }

    /// Check if session is stale (no activity for 30s).
    pub fn is_stale(&self, timeout_secs: u64) -> bool {
        self.last_recv.elapsed().as_secs() > timeout_secs
    }

    pub fn record_recv(&mut self, bytes: u64) {
        self.last_recv = Instant::now();
        self.bytes_recv += bytes;
    }

    pub fn record_sent(&mut self, bytes: u64) {
        self.bytes_sent += bytes;
    }

    pub fn update_cell_context(&mut self, cells: [u32; 9]) {
        self.current_cell = cells[4]; // center cell
        self.cell_context = cells;
    }

    /// Build a GameEnd packet for this session.
    pub fn end_packet(&self, reason: u8) -> Packet {
        Packet::GameEnd { reason }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session() -> Session {
        Session::new(
            NetworkID::new(1),
            "127.0.0.1:9000".parse().unwrap(),
            "Wanderer".into(),
        )
    }

    #[test]
    fn state_transitions() {
        let mut s = session();
        assert!(!s.is_ingame());
        s.transition(SessionState::InGame);
        assert!(s.is_ingame());
        assert!(s.is_active());
        s.transition(SessionState::Disconnecting);
        assert!(!s.is_active());
    }

    #[test]
    fn staleness_by_last_recv() {
        let mut s = session();
        s.record_recv(10);
        assert!(!s.is_stale(30));
        // backdate: a fresh session's last_recv is recent; simulate by
        // constructing with an ancient last_recv via direct field access
        s.last_recv = std::time::Instant::now() - std::time::Duration::from_secs(60);
        assert!(s.is_stale(30));
    }

    #[test]
    fn byte_accounting() {
        let mut s = session();
        s.record_recv(100);
        s.record_sent(50);
        assert_eq!(s.bytes_recv, 100);
        assert_eq!(s.bytes_sent, 50);
    }
}
