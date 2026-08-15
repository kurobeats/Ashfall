//! Client game orchestrator — state machine + network + registry.

use crate::config::ClientConfig;
use crate::dispatch;
use crate::network::ClientNetwork;
use crate::ui::widgets::GuiState;
use crate::world::registry::ClientRegistry;
use ashfall_core::id::NetworkID;
use ashfall_core::protocol::Packet;
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
    /// Game-engine bridge (None in stub tests; connected from config).
    pub ipc: Option<crate::ipc::IpcClient>,
    /// Engine commands queued from remote packets, drained by `flush_commands`.
    pub pending_commands: Vec<(u32, Vec<crate::ipc::Param>)>,
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
            ipc: None,
            pending_commands: Vec::new(),
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
        // Engine bridge (config ipc_mode: stub / tcp / unix). Stub mode is
        // the default and always succeeds — the real game hooks in later.
        let mode = match self.config.ipc_mode.as_str() {
            "unix" => crate::ipc::IpcMode::Native {
                path: "/tmp/ashfall-ipc.sock".into(),
            },
            "tcp" => crate::ipc::IpcMode::Proton {
                port: self.config.ipc_port,
            },
            _ => crate::ipc::IpcMode::Stub,
        };
        self.ipc = Some(crate::ipc::IpcClient::connect(mode).await?);
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
        let network = self
            .network
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("Not connected"))?;
        network.poll().await
    }

    pub fn handle_packet(&mut self, packet: Packet) {
        self.registry.apply_packet(&packet);
        // Remote entities → engine commands (applied on flush_commands).
        if let Some(local) = self.local_player_id {
            let cmds =
                crate::sync::packets_to_commands(std::slice::from_ref(&packet), local, |id| {
                    self.registry.ref_of(id)
                });
            self.pending_commands.extend(cmds);
        }
        // Ownership changes → tell the bridge to (un)track the NPC so its
        // simulation state gets sampled and relayed (the owner's half of
        // NPC sync).
        match &packet {
            Packet::OwnershipGranted { id } => {
                if let Some(ref_id) = crate::sync::ref_of_entity(*id) {
                    self.pending_commands.push((
                        crate::ipc::OP_TRACK_ACTOR,
                        vec![crate::ipc::Param::U32(ref_id)],
                    ));
                }
            }
            Packet::OwnershipReleased { id } => {
                if let Some(ref_id) = crate::sync::ref_of_entity(*id) {
                    self.pending_commands.push((
                        crate::ipc::OP_UNTRACK_ACTOR,
                        vec![crate::ipc::Param::U32(ref_id)],
                    ));
                }
            }
            _ => {}
        }
        dispatch::dispatch(self, &packet);
    }

    /// Drain engine events: bridge → packets → server (the coop loop's
    /// client side). No-op when no bridge is connected.
    pub async fn poll_bridge(&mut self) -> anyhow::Result<()> {
        let Some(ipc) = self.ipc.as_mut() else {
            return Ok(());
        };
        let frames = ipc.poll_events();
        tracing::info!("poll_bridge: {} event frames", frames.len());
        if frames.is_empty() {
            return Ok(());
        }
        let Some(local) = self.local_player_id else {
            return Ok(());
        };
        let packets = crate::sync::events_to_packets(&frames, local);
        tracing::info!("poll_bridge: {} packets to send", packets.len());
        for pkt in packets {
            self.send_reliable(pkt).await?;
        }
        Ok(())
    }

    /// Execute queued engine commands (remote positions/angles applied to
    /// the local game).
    pub async fn flush_commands(&mut self) -> anyhow::Result<()> {
        let commands = std::mem::take(&mut self.pending_commands);
        if commands.is_empty() {
            return Ok(());
        }
        let Some(ipc) = self.ipc.as_mut() else {
            self.pending_commands = commands; // no bridge yet — keep queued
            return Ok(());
        };
        for (opcode, params) in commands {
            let _ = ipc.execute(opcode, &params).await;
        }
        Ok(())
    }

    pub async fn send_reliable(&mut self, packet: Packet) -> anyhow::Result<()> {
        let network = self
            .network
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("Not connected"))?;
        network.send(&packet).await
    }

    pub async fn send_chat(&mut self, message: String) -> anyhow::Result<()> {
        self.chat_messages
            .push((self.config.name.clone(), message.clone()));
        self.send_reliable(Packet::GameChat {
            message: message.into(),
        })
        .await
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
        self.send_reliable(Packet::SpellCast {
            id,
            spell,
            source,
            dual,
            target,
        })
        .await
    }

    /// Send any server-GUI widget clicks queued by the renderer.
    pub async fn flush_gui_clicks(&mut self) -> anyhow::Result<()> {
        let clicks: Vec<ashfall_core::id::NetworkID> = std::mem::take(&mut self.gui.pending_clicks);
        for id in clicks {
            self.send_reliable(Packet::UpdateWindowClick { id }).await?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ClientConfig;

    #[test]
    fn test_ownership_grant_queues_track_command() {
        let mut g = Game::new(ClientConfig::default());
        g.local_player_id = Some(NetworkID::new(1));

        let npc_id = crate::sync::entity_id(0x1234);
        g.handle_packet(Packet::OwnershipGranted { id: npc_id });

        let has_track = g.pending_commands.iter().any(|(op, params)| {
            *op == crate::ipc::OP_TRACK_ACTOR
                && matches!(params.first(), Some(crate::ipc::Param::U32(0x1234)))
        });
        assert!(has_track, "grant → bridge tracks the NPC");
        assert!(g.registry.owned_actors.contains(&npc_id));

        g.handle_packet(Packet::OwnershipReleased { id: npc_id });
        let has_untrack = g.pending_commands.iter().any(|(op, params)| {
            *op == crate::ipc::OP_UNTRACK_ACTOR
                && matches!(params.first(), Some(crate::ipc::Param::U32(0x1234)))
        });
        assert!(has_untrack, "release → bridge untracks");
        assert!(!g.registry.owned_actors.contains(&npc_id));
    }
}
