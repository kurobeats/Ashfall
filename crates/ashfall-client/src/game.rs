//! Client game orchestrator — state machine + network + registry.

use ashfall_core::id::NetworkID;
use ashfall_core::protocol::Packet;
use crate::config::ClientConfig;
use crate::dispatch;
use crate::network::ClientNetwork;
use crate::world::registry::ClientRegistry;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::time::Instant;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClientState {
    Disconnected,
    Connecting,
    Authenticating,
    Loading,
    InGame,
}

pub struct Game {
    pub state: ClientState,
    pub config: ClientConfig,
    pub network: Option<ClientNetwork>,
    pub registry: ClientRegistry,
    pub local_player_id: Option<NetworkID>,
    pub connected_at: Option<Instant>,
    pub chat_messages: Vec<(String, String)>,
    pub weather: u32,
    pub karma: i32,
    pub reputation: HashMap<u32, i32>,
    pub hardcore_hunger: f32,
    pub hardcore_thirst: f32,
    pub hardcore_sleep: f32,
}

impl Game {
    pub fn new(config: ClientConfig) -> Self {
        Game {
            state: ClientState::Disconnected,
            config,
            network: None,
            registry: ClientRegistry::new(),
            local_player_id: None,
            connected_at: None,
            chat_messages: Vec::new(),
            weather: 0,
            karma: 0,
            reputation: HashMap::new(),
            hardcore_hunger: 0.0,
            hardcore_thirst: 0.0,
            hardcore_sleep: 0.0,
        }
    }

    pub async fn connect(&mut self, addr: SocketAddr) -> anyhow::Result<()> {
        self.state = ClientState::Connecting;
        let network = ClientNetwork::connect(addr).await?;
        self.network = Some(network);
        self.connected_at = Some(Instant::now());
        Ok(())
    }

    pub async fn authenticate(&mut self) -> anyhow::Result<()> {
        self.state = ClientState::Authenticating;
        let auth = Packet::GameAuth { name: self.config.name.clone(), password: String::new() };
        self.send_reliable(auth).await?;
        Ok(())
    }

    pub async fn poll(&mut self) -> anyhow::Result<Vec<Packet>> {
        let network = self.network.as_mut().ok_or_else(|| anyhow::anyhow!("Not connected"))?;
        network.poll().await
    }

    pub fn handle_packet(&mut self, packet: Packet) {
        self.registry.apply_packet(&packet);
        dispatch::dispatch(self, &packet);
    }

    pub async fn send_reliable(&mut self, packet: Packet) -> anyhow::Result<()> {
        let network = self.network.as_mut().ok_or_else(|| anyhow::anyhow!("Not connected"))?;
        network.send(&packet).await
    }

    pub async fn send_chat(&mut self, message: String) -> anyhow::Result<()> {
        self.chat_messages.push((self.config.name.clone(), message.clone()));
        self.send_reliable(Packet::GameChat { message }).await
    }
}
