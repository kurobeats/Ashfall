//! Master server announcer — periodic heartbeat to master registry.
//!
//! Sends MasterAnnounce every 60s with current player count.
//! Uses the same UDP socket as the main server (shared Arc).

use ashfall_core::protocol::Packet;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Instant;
use tokio::net::UdpSocket;

pub struct MasterAnnouncer {
    master_addr: SocketAddr,
    socket: Arc<UdpSocket>,
    server_name: String,
    server_map: String,
    game_type: String,
    max_players: u32,
    last_announce: Instant,
}

impl MasterAnnouncer {
    pub fn new(
        master_addr: SocketAddr,
        socket: Arc<UdpSocket>,
        name: String,
        map: String,
        game_type: String,
        max_players: u32,
    ) -> Self {
        MasterAnnouncer {
            master_addr,
            socket,
            server_name: name,
            server_map: map,
            game_type,
            max_players,
            last_announce: Instant::now(),
        }
    }

    /// Heartbeat — called from tick loop. Sends announce if 60s elapsed.
    pub async fn heartbeat(&mut self, players: u32) {
        if self.last_announce.elapsed().as_secs() < 60 {
            return;
        }
        self.last_announce = Instant::now();

        if let Err(e) = self.announce(players).await {
            tracing::warn!("Master announce failed: {e}");
        }
    }

    /// Send MasterAnnounce to the master registry.
    async fn announce(&self, players: u32) -> anyhow::Result<()> {
        let packet = Packet::MasterAnnounce {
            name: self.server_name.clone(),
            map: self.server_map.clone(),
            players,
            max_players: self.max_players,
            rules: HashMap::new(),
            mod_files: vec![],
            game_type: self.game_type.clone(),
        };

        let payload = postcard::to_stdvec(&packet)?;

        // Simple wire format: [2B len][1B channel=0][payload]
        let mut buf = Vec::with_capacity(3 + payload.len());
        buf.extend_from_slice(&(payload.len() as u16).to_le_bytes());
        buf.push(0u8); // channel 0 = System
        buf.extend_from_slice(&payload);

        self.socket.send_to(&buf, self.master_addr).await?;
        Ok(())
    }

    /// Deregister from master (send empty update or just stop).
    /// ponytail: master culls stale entries, so no explicit deregister needed.
    pub async fn _deregister(&self, players: u32) -> anyhow::Result<()> {
        let packet = Packet::MasterUpdate {
            name: String::new(),
            map: String::new(),
            players,
            max_players: 0,
        };

        let payload = postcard::to_stdvec(&packet)?;
        let mut buf = Vec::with_capacity(3 + payload.len());
        buf.extend_from_slice(&(payload.len() as u16).to_le_bytes());
        buf.push(0u8);
        buf.extend_from_slice(&payload);
        self.socket.send_to(&buf, self.master_addr).await?;
        Ok(())
    }
}
