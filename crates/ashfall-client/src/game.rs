//! Client game orchestrator — state machine + network + registry.

use ashfall_core::id::NetworkID;
use ashfall_core::protocol::Packet;
use crate::config::ClientConfig;
use crate::dispatch;
use crate::network::ClientNetwork;
use crate::ui::widgets::GuiState;
use crate::world::registry::ClientRegistry;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::time::Instant;

/// Server-authoritative game clock (GameTime packets, STR CalendarService).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct GameClock {
    pub year: u32,
    pub month: u32,
    pub day: u32,
    pub hour: u32,
    pub time_scale: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
// ponytail: Loading/InGame are never constructed in stub mode; the real flow
// reaches them after GameStart arrives from the server.
#[allow(dead_code)]
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
    /// Server rule: player-vs-player combat allowed (ServerSettings on join).
    pub pvp_enabled: bool,
    /// Authoritative game clock (None until the first GameTime packet).
    pub game_time: Option<GameClock>,
    pub reputation: HashMap<u32, i32>,
    pub hardcore_hunger: f32,
    pub hardcore_thirst: f32,
    pub hardcore_sleep: f32,
    pub gui: GuiState,
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
            pvp_enabled: false,
            game_time: None,
            reputation: HashMap::new(),
            hardcore_hunger: 0.0,
            hardcore_thirst: 0.0,
            hardcore_sleep: 0.0,
            gui: GuiState::new(),
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
        let auth = Packet::GameAuth {
            name: self.config.name.clone(),
            password: String::new(),
            version: ashfall_core::constants::CLIENT_VERSION.into(),
        };
        self.send_reliable(auth).await?;
        // Load-order verification (STR ModPolicy) — sent right after auth so
        // the server checks it before the world-state handoff completes.
        let mods = self
            .config
            .mods
            .iter()
            .filter_map(|s| {
                let (file, crc) = s.rsplit_once(':')?;
                let crc = u32::from_str_radix(crc.trim(), 16).ok()?;
                Some((file.trim().to_string(), crc))
            })
            .collect();
        self.send_reliable(Packet::GameModList { mods }).await?;
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
        self.send_reliable(Packet::GameChat { message: message.into() }).await
    }

    /// Request simulation ownership of an actor (STR OwnershipTransfer). The
    /// server answers with `OwnershipGranted` or stays silent (already owned).
    /// ponytail: no caller yet — the bridge will call this when it reports an
    /// NPC spawn (events.rs ActorNew wiring).
    #[allow(dead_code)]
    pub async fn claim_ownership(&mut self, id: ashfall_core::id::NetworkID) -> anyhow::Result<()> {
        self.send_reliable(Packet::OwnershipClaim { id }).await
    }

    /// Whether this client currently simulates `id` (server-granted).
    /// ponytail: no caller yet — the bridge checks this before sending actor
    /// state updates for remote NPCs.
    #[allow(dead_code)]
    pub fn owns(&self, id: ashfall_core::id::NetworkID) -> bool {
        self.registry.owned_actors.contains(&id)
    }

    /// Report a spell cast by `id` (own player or an owned NPC) to the server.
    /// ponytail: no caller yet — the bridge will call this on spell events.
    #[allow(dead_code)]
    pub async fn send_spell_cast(
        &mut self,
        id: ashfall_core::id::NetworkID,
        spell: u32,
        source: i32,
        dual: bool,
        target: ashfall_core::id::NetworkID,
    ) -> anyhow::Result<()> {
        self.send_reliable(Packet::SpellCast { id, spell, source, dual, target }).await
    }

    /// Send any server-GUI widget clicks queued by the renderer.
    pub async fn flush_gui_clicks(&mut self) -> anyhow::Result<()> {
        let clicks: Vec<ashfall_core::id::NetworkID> =
            std::mem::take(&mut self.gui.pending_clicks);
        for id in clicks {
            self.send_reliable(Packet::UpdateWindowClick { id }).await?;
        }
        Ok(())
    }
}
