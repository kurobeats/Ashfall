//! Dedicated server main loop — UDP recv + tick + dispatch.

use ashfall_core::id::NetworkID;
use ashfall_core::protocol::Packet;
use ashfall_core::types::Reason;
use crate::config::ServerConfig;
use crate::db::Database;
use crate::dispatch::Dispatcher;
use crate::master::MasterAnnouncer;
use crate::network::NetworkManager;
use crate::script::engine::{ScriptEngine, ScriptState};
use crate::script::state::{GameTime, ScriptEffect};
use crate::session::Session;
use dashmap::DashMap;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::time::{interval, Duration};

/// The dedicated server.
pub struct DedicatedServer {
    pub config: ServerConfig,
    pub db: Database,
    pub dispatcher: Dispatcher,
    pub network: NetworkManager,
    pub script_engine: ScriptEngine,
    pub master_announcer: MasterAnnouncer,
    pub sessions: DashMap<SocketAddr, Session>,
    next_guid: AtomicU64,
    /// Last weather sent to clients — tick sync detects script changes.
    last_weather: u32,
    /// Last quest stages sent to clients — tick sync detects script changes.
    last_quest_stages: HashMap<u32, u16>,
    /// Last game clock seen — tick sync notifies scripts on change.
    last_game_time: Option<GameTime>,
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
        // Scripts share the dispatcher's world state — mutations are visible
        // to clients via tick-level delta sync.
        let state = ScriptState::new(
            dispatcher.registry.clone(),
            dispatcher.weather.clone(),
            dispatcher.globals.clone(),
            dispatcher.quests.clone(),
            dispatcher.factions.clone(),
            config.server.host.clone(),
            String::new(),
            config.server.connections as u32,
        );
        script_engine.instantiate_all(state)?;
        tracing::info!("Script engine initialized with {} modules", script_engine.module_count());

        // Snapshot initial world state for tick-level delta sync.
        let last_weather = dispatcher.weather.get();
        let last_quest_stages = dispatcher.quests.all_stages().into_iter().collect();
        let last_game_time = None;

        // Master server announcer
        let master_announcer = MasterAnnouncer::new(
            config.master_addr(),
            network.socket(),
            "Ashfall Server".into(),
            "Wasteland".into(),
            config.server.game_type.clone(),
            config.server.connections as u32,
        );

        Ok(DedicatedServer {
            config,
            db,
            dispatcher,
            network,
            script_engine,
            master_announcer,
            sessions: DashMap::new(),
            next_guid: AtomicU64::new(1),
            last_weather,
            last_quest_stages,
            last_game_time,
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
        // Fire script timers → route to WASM callback exports
        let timers = self.script_engine.tick_timers();
        for (id, callback) in timers {
            self.script_engine.dispatch_timer(id, &callback);
        }

        // Apply script-queued side effects (chat, kick)
        let effects = self.script_engine.drain_effects();
        self.apply_effects(effects).await;

        // Sync server-authored world-state deltas (weather, quest stages)
        self.sync_world_state().await;

        // Notify scripts when the game clock changed
        if let Some(gt) = self.script_engine.game_time.clone() {
            let now = gt.get();
            if self.last_game_time != Some(now) {
                self.last_game_time = Some(now);
                self.script_engine.notify_game_time(now);
            }
        }

        // Live player count for scripts
        let player_count = self.sessions.iter().filter(|e| e.value().is_ingame()).count() as u32;
        if let Some(arc) = self.script_engine.player_count.clone() {
            arc.store(player_count, Ordering::Relaxed);
        }

        // Reliability maintenance: flush ACK/NACK frames, retransmit expired
        self.network.tick().await;

        // Master server heartbeat (every 60s)
        self.master_announcer.heartbeat(player_count).await;

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

    /// Apply script side effects (chat, kick) to sessions.
    async fn apply_effects(&mut self, effects: Vec<ScriptEffect>) {
        for effect in effects {
            match effect {
                ScriptEffect::PrivateChat { player_id, message } => {
                    let pkt = Packet::GameChat { message };
                    let targets: Vec<SocketAddr> = self
                        .sessions
                        .iter()
                        .filter(|e| e.value().player_id.map(|p| p.as_u64()) == Some(player_id))
                        .map(|e| *e.key())
                        .collect();
                    for addr in targets {
                        let _ = self.network.send_reliable(addr, &pkt).await;
                    }
                }
                ScriptEffect::BroadcastChat { message } => {
                    let pkt = Packet::GameChat { message };
                    let addrs: Vec<SocketAddr> = self
                        .sessions
                        .iter()
                        .filter(|e| e.value().is_ingame())
                        .map(|e| *e.key())
                        .collect();
                    for addr in addrs {
                        let _ = self.network.send_reliable(addr, &pkt).await;
                    }
                }
                ScriptEffect::Kick { player_id } => {
                    let targets: Vec<SocketAddr> = self
                        .sessions
                        .iter()
                        .filter(|e| e.value().player_id.map(|p| p.as_u64()) == Some(player_id))
                        .map(|e| *e.key())
                        .collect();
                    for addr in targets {
                        let _ = self.network.send_reliable(addr, &Packet::GameEnd { reason: Reason::Quit as u8 }).await;
                        self.disconnect(addr).await;
                    }
                }
            }
        }
    }

    /// Broadcast server-authored world-state deltas (weather, quest stages)
    /// to ingame clients. Catches script-driven changes between ticks.
    async fn sync_world_state(&mut self) {
        let ingame_addrs = || -> Vec<SocketAddr> {
            self.sessions
                .iter()
                .filter(|e| e.value().is_ingame())
                .map(|e| *e.key())
                .collect()
        };

        let weather = self.dispatcher.weather.get();
        if weather != self.last_weather {
            self.last_weather = weather;
            let pkt = Packet::GameWeather { weather };
            for addr in ingame_addrs() {
                let _ = self.network.send_reliable(addr, &pkt).await;
            }
        }

        for (quest_id, stage) in self.dispatcher.quests.all_stages() {
            if self.last_quest_stages.get(&quest_id) != Some(&stage) {
                self.last_quest_stages.insert(quest_id, stage);
                let pkt = Packet::QuestStage { quest_id, stage };
                for addr in ingame_addrs() {
                    let _ = self.network.send_reliable(addr, &pkt).await;
                }
            }
        }
    }

    /// Handle incoming UDP data.
    async fn handle_recv(&mut self, addr: SocketAddr, data: &[u8]) {
        // Per-address rate limit (200 pkt/s, burst 100) — drop silently when exceeded
        if !self.network.check_rate(addr) {
            tracing::debug!("Rate limit exceeded for {addr}, dropping datagram");
            return;
        }

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

            // Script chat gate: on_player_chat may block the message.
            if let Packet::GameChat { message } = &packet {
                let player_id = session.player_id.map(|p| p.as_u64()).unwrap_or(0);
                if !self.script_engine.dispatch_chat(player_id, message) {
                    return;
                }
            }

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

        // Script auth callback: any module vote of 0 rejects the connection.
        if !self.script_engine.dispatch_auth(&name, &password) {
            tracing::warn!("Script denied auth for {name} from {addr}");
            let end = Packet::GameEnd { reason: Reason::Denied as u8 };
            let _ = self.network.send_reliable(addr, &end).await;
            return;
        }

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

        // G8: Create player object in registry — spawn cell from script callback
        let player_id = self.dispatcher.registry.allocate_id();
        let spawn_cell = self.script_engine.dispatch_spawn_cell(player_id.as_u64());
        let player = crate::world::objects::Player::new(player_id, 0x14, 0x07, spawn_cell);
        self.dispatcher.registry.insert(player);
        session.player_id = Some(player_id);
        session.state = crate::session::SessionState::InGame;

        // Script on_spawn notification
        self.script_engine.notify_spawn(player_id.as_u64());

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

            // Script on_player_disconnect notification
            if let Some(pid) = session.player_id {
                self.script_engine.notify_disconnect(pid.as_u64(), 4);
            }

            // Remove player object
            if let Some(pid) = session.player_id {
                self.dispatcher.registry.remove(pid);
            }

            self.network.remove_session(addr);
        }
    }
}
