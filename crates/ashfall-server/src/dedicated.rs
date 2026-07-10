//! Dedicated server main loop — UDP recv + tick + dispatch.

use ashfall_core::id::NetworkID;
use ashfall_core::protocol::Packet;
use crate::config::ServerConfig;
use crate::db::Database;
use crate::dispatch::Dispatcher;
use crate::network::NetworkManager;
use crate::quest::QuestManager;
use crate::script::engine::{ScriptEngine, ScriptState};
use crate::session::Session;
use crate::world::globals::GlobalState;
use crate::world::weather::WeatherState;
use crate::ai::factions::FactionMatrix;
use dashmap::DashMap;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::time::{interval, Duration};

/// The dedicated server.
pub struct DedicatedServer {
    pub config: ServerConfig,
    pub db: Database,
    pub dispatcher: Dispatcher,
    pub network: NetworkManager,
    pub script_engine: ScriptEngine,
    pub sessions: DashMap<SocketAddr, Session>,
    next_guid: AtomicU64,
}

impl DedicatedServer {
    pub async fn new(config: ServerConfig) -> anyhow::Result<Self> {
        let bind_addr = config.bind_addr();
        let network = NetworkManager::bind(bind_addr).await?;

        // Open database
        let db = Database::open(&config.database.path)?;

        let mut dispatcher = Dispatcher::new();

        // Load persistent state into memory
        db.startup_load(&dispatcher.quests, &mut dispatcher.factions);
        tracing::info!("Startup load complete");

        // Initialize script engine
        let mut script_engine = ScriptEngine::new()?;
        script_engine.load_modules(&config.scripts.path)?;
        let state = ScriptState::new(
            dispatcher.registry.clone(),
            WeatherState::default(),
            GlobalState::new(),
            QuestManager::new(),
            FactionMatrix::default(),
            config.server.host.clone(),
            String::new(),
        );
        script_engine.instantiate_all(state)?;
        tracing::info!("Script engine initialized with {} modules", script_engine.module_count());

        Ok(DedicatedServer {
            config,
            db,
            dispatcher,
            network,
            script_engine,
            sessions: DashMap::new(),
            next_guid: AtomicU64::new(1),
        })
    }

    fn allocate_session_id(&self) -> NetworkID {
        NetworkID::new(self.next_guid.fetch_add(1, Ordering::SeqCst))
    }

    /// Main server loop — blocks until shutdown.
    pub async fn run(mut self) -> anyhow::Result<()> {
        let tick_ms = self.config.tick_interval_ms();
        let mut tick = interval(Duration::from_millis(tick_ms));
        let mut buf = vec![0u8; 65536];

        tracing::info!(
            "Server running at {}Hz on {} (game: {})",
            self.config.game.tick_rate,
            self.config.bind_addr(),
            self.config.server.game_type,
        );

        loop {
            tokio::select! {
                _ = tick.tick() => {
                    self.tick().await;
                }
                result = self.network.recv_raw(&mut buf) => {
                    match result {
                        Ok((len, addr)) => {
                            self.handle_recv(addr, &buf[..len]).await;
                        }
                        Err(e) => {
                            tracing::error!("Recv error: {e}");
                        }
                    }
                }
            }
        }
    }

    /// Per-tick work: cull stale sessions, fire script timers.
    async fn tick(&mut self) {
        // Fire script timers
        self.script_engine.tick_timers();

        // Cull stale sessions (>30s inactive)
        self.sessions.retain(|addr, session| {
            if session.is_stale(30) {
                tracing::info!("Culling stale session {} ({}s inactive)", session.player_name, session.last_recv.elapsed().as_secs());
                self.network.remove_session(*addr);
                false
            } else {
                true
            }
        });
    }

    /// Handle incoming UDP data.
    async fn handle_recv(&mut self, addr: SocketAddr, data: &[u8]) {
        // Try to reassemble a packet
        let packet = match self.network.try_recv(addr, data) {
            Some(p) => p,
            None => return, // out of order, buffered
        };

        // Check if this is a new connection (GameAuth)
        if matches!(packet, Packet::GameAuth { .. }) {
            self.handle_auth(addr, packet).await;
            return;
        }

        // Route to existing session
        let should_disconnect = {
            let mut session = match self.sessions.get_mut(&addr) {
                Some(s) => s,
                None => return,
            };
            session.record_recv(data.len() as u64);

            let result = self.dispatcher.dispatch(&mut session, packet);

            // Send responses to this client
            for pkt in &result.responses {
                let _ = self.network.send_reliable(addr, pkt).await;
            }

            // Broadcast to all other clients
            for pkt in &result.broadcasts {
                let targets: Vec<SocketAddr> = self.sessions
                    .iter()
                    .filter(|entry| entry.key() != &addr && entry.value().is_ingame())
                    .map(|entry| *entry.key())
                    .collect();

                for target in &targets {
                    let _ = self.network.send_reliable(*target, pkt).await;
                }
            }

            result.disconnect
        }; // session borrow released here

        if should_disconnect {
            self.disconnect(addr).await;
        }
    }

    /// Handle a new GameAuth connection.
    async fn handle_auth(&mut self, addr: SocketAddr, packet: Packet) {
        let (name, password) = match &packet {
            Packet::GameAuth { name, password } => (name.clone(), password.clone()),
            _ => return,
        };

        // Check max connections
        if self.sessions.len() >= self.config.server.connections {
            tracing::warn!("Connection rejected: server full from {addr}");
            let end = Packet::GameEnd { reason: 5 }; // ponytail: full
            let _ = self.network.send_reliable(addr, &end).await;
            return;
        }

        let session_id = self.allocate_session_id();

        let (session, responses) = self.dispatcher.handle_connection(
            addr, name.clone(), password, session_id,
        );

        let mut session = match session {
            Some(s) => s,
            None => {
                for pkt in responses {
                    let _ = self.network.send_reliable(addr, &pkt).await;
                }
                return;
            }
        };

        // Send initial responses (GameLoad)
        for pkt in &responses {
            let _ = self.network.send_reliable(addr, pkt).await;
        }

        // G8: Create player object in registry
        let player_id = self.dispatcher.registry.allocate_id();
        let player = crate::world::objects::Player::new(player_id, 0x14, 0x07, 0);
        self.dispatcher.registry.insert(player);
        session.player_id = Some(player_id);
        session.state = crate::session::SessionState::InGame;

        // Broadcast PlayerNew to the new player
        if let Some(arc) = self.dispatcher.registry.get(player_id) {
            let guard = arc.read();
            if let Some(p) = guard.as_any().downcast_ref::<crate::world::objects::Player>() {
                let _ = self.network.send_reliable(addr, &p.to_new_packet()).await;
            }
        }

        // Send world state (weather, globals, quests, existing players, cell objects)
        let world_packets = self.dispatcher.send_world_state(&session);
        for pkt in &world_packets {
            let _ = self.network.send_reliable(addr, pkt).await;
        }

        // Broadcast PlayerNew to all existing players
        let other_addrs: Vec<SocketAddr> = self.sessions
            .iter()
            .filter(|entry| entry.value().is_ingame())
            .map(|entry| *entry.key())
            .collect();
        if let Some(arc) = self.dispatcher.registry.get(player_id) {
            let guard = arc.read();
            if let Some(p) = guard.as_any().downcast_ref::<crate::world::objects::Player>() {
                let player_pkt = p.to_new_packet();
                for other_addr in &other_addrs {
                    if *other_addr != addr {
                        let _ = self.network.send_reliable(*other_addr, &player_pkt).await;
                    }
                }
            }
        }

        // Register network session
        self.network.register_session(addr);

        // Insert session
        self.sessions.insert(addr, session);

        tracing::info!("Player {name} (id={player_id}) connected from {addr}");
    }

    /// Disconnect a session.
    async fn disconnect(&mut self, addr: SocketAddr) {
        if let Some((_, session)) = self.sessions.remove(&addr) {
            tracing::info!("Session {} disconnected", session.player_name);

            // Remove player object
            if let Some(pid) = session.player_id {
                self.dispatcher.registry.remove(pid);
            }

            self.network.remove_session(addr);
        }
    }
}
