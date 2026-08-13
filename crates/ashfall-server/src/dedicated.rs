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

/// Advance the game clock by one game hour. Hour-granular clock with lazy
/// rollover: 30-day months, 12-month years (ponytail: no real calendar —
/// Fallout's time-of-day matters, not its astronomy).
fn advance_hour(t: &mut GameTime) {
    t.hour += 1;
    if t.hour < 24 {
        return;
    }
    t.hour = 0;
    t.day += 1;
    if t.day <= 30 {
        return;
    }
    t.day = 1;
    t.month += 1;
    if t.month <= 12 {
        return;
    }
    t.month = 1;
    t.year += 1;
}

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
    /// Fractional game-hours accrued but not yet rolled into the hour-granular
    /// clock (30× scale ≈ 2 real minutes per game hour — sub-hour drift here).
    time_accum: f32,
}

impl DedicatedServer {
    pub async fn new(config: ServerConfig) -> anyhow::Result<Self> {
        let bind_addr = config.bind_addr();
        let network = NetworkManager::bind(bind_addr).await?;

        // Open database
        let db = Database::open(&config.database.path)?;

        let mut dispatcher = Dispatcher::new();
        dispatcher.pvp_enabled = config.server.pvp_enabled;
        dispatcher.expected_mods = config
            .server
            .mods
            .iter()
            .filter_map(|s| crate::handlers::game::parse_mod_entry(s))
            .collect();

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
            time_accum: 0.0,
        })
    }

    fn allocate_session_id(&self) -> NetworkID {
        NetworkID::new(self.next_guid.fetch_add(1, Ordering::SeqCst))
    }

    /// Send a reliable packet to a session, binding string-cache fields
    /// against that session's dictionary first (STR StringCache: repeats go
    /// out as 2-byte ids).
    async fn send(&mut self, addr: SocketAddr, mut packet: Packet) {
        if let Some(mut s) = self.sessions.get_mut(&addr) {
            packet.finalize_strings(&mut s.value_mut().string_table);
        }
        let _ = self.network.send_reliable(addr, &packet).await;
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

        // Advance the authoritative game clock (STR CalendarService: time
        // flows server-side at time_scale × real time; scripts override via
        // set_game_time and the advance continues from there).
        if let Some(gt) = self.script_engine.game_time.clone() {
            let delta_secs = self.config.tick_interval_ms() as f32 / 1000.0;
            self.time_accum += delta_secs * gt.get_scale() / 3600.0;
            if self.time_accum >= 1.0 {
                self.time_accum -= 1.0;
                let mut now = gt.get();
                advance_hour(&mut now);
                gt.set(now);
            }
        }

        // Notify scripts when the game clock changed
        if let Some(gt) = self.script_engine.game_time.clone() {
            let now = gt.get();
            if self.last_game_time != Some(now) {
                self.last_game_time = Some(now);
                self.script_engine.notify_game_time(now);
                // Broadcast the clock to clients (join-time send in handle_auth).
                let addrs: Vec<SocketAddr> = self
                    .sessions
                    .iter()
                    .filter(|e| e.value().is_ingame())
                    .map(|e| *e.key())
                    .collect();
                let pkt = Packet::GameTime {
                    year: now.year, month: now.month, day: now.day, hour: now.hour,
                    time_scale: gt.get_scale(),
                };
                for a in &addrs {
                    self.send(*a, pkt.clone()).await;
                }
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
                    let pkt = Packet::GameChat { message: message.into() };
                    let targets: Vec<SocketAddr> = self
                        .sessions
                        .iter()
                        .filter(|e| e.value().player_id.map(|p| p.as_u64()) == Some(player_id))
                        .map(|e| *e.key())
                        .collect();
                    for addr in targets {
                        self.send(addr, pkt.clone()).await;
                    }
                }
                ScriptEffect::BroadcastChat { message } => {
                    let pkt = Packet::GameChat { message: message.into() };
                    let addrs: Vec<SocketAddr> = self
                        .sessions
                        .iter()
                        .filter(|e| e.value().is_ingame())
                        .map(|e| *e.key())
                        .collect();
                    for addr in addrs {
                        self.send(addr, pkt.clone()).await;
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
                        self.send(addr, Packet::GameEnd { reason: Reason::Quit as u8 }).await;
                        self.disconnect(addr).await;
                    }
                }
                ScriptEffect::BroadcastPacket(pkt) => {
                    let addrs: Vec<SocketAddr> = self
                        .sessions
                        .iter()
                        .filter(|e| e.value().is_ingame())
                        .map(|e| *e.key())
                        .collect();
                    for addr in addrs {
                        self.send(addr, pkt.clone()).await;
                    }
                }
            }
        }
    }

    /// Broadcast server-authored world-state deltas (weather, quest stages)
    /// to ingame clients. Catches script-driven changes between ticks.
    async fn sync_world_state(&mut self) {
        let weather = self.dispatcher.weather.get();
        if weather != self.last_weather {
            self.last_weather = weather;
            let pkt = Packet::GameWeather { weather };
            let addrs: Vec<SocketAddr> = self
                .sessions
                .iter()
                .filter(|e| e.value().is_ingame())
                .map(|e| *e.key())
                .collect();
            for addr in addrs {
                self.send(addr, pkt.clone()).await;
            }
        }

        for (quest_id, stage) in self.dispatcher.quests.all_stages() {
            if self.last_quest_stages.get(&quest_id) != Some(&stage) {
                self.last_quest_stages.insert(quest_id, stage);
                let pkt = Packet::QuestStage { quest_id, stage };
                let addrs: Vec<SocketAddr> = self
                    .sessions
                    .iter()
                    .filter(|e| e.value().is_ingame())
                    .map(|e| *e.key())
                    .collect();
                for addr in addrs {
                    self.send(addr, pkt.clone()).await;
                }
                // Script on_quest_stage notification (covers script- and
                // client-driven changes alike).
                self.script_engine.notify_quest_stage(quest_id, stage);
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

        // Route to existing session. IMPORTANT: the `sessions` DashMap guard
        // (write) must be dropped before iterating the same map for broadcast
        // targets — DashMap locks are not reentrant, and holding the guard
        // across `self.sessions.iter()` deadlocks on every broadcast (chat,
        // positions, quests) once two players are connected.
        let (responses, broadcasts, disconnect, cell_before, cell_after) = {
            let mut session = match self.sessions.get_mut(&addr) {
                Some(s) => s,
                None => return,
            };
            session.record_recv(data.len() as u64);
            let player_id = session.player_id.map(|p| p.as_u64()).unwrap_or(0);

            // Script chat gate: on_player_chat may block the message.
            if let Packet::GameChat { message } = &packet {
                let msg = message.resolve(&mut session.string_table);
                if !self.script_engine.dispatch_chat(player_id, &msg) {
                    return;
                }
            }

            // Script hit gate: on_hit may block combat resolution.
            if let Packet::ActorHit { target, attacker, limb, base_damage, .. } = &packet {
                if !self.script_engine.dispatch_hit(
                    target.as_u64(),
                    attacker.as_u64(),
                    *limb,
                    *base_damage,
                ) {
                    tracing::info!("Script blocked hit from {player_id}");
                    return;
                }
            }

            let cell_before = session.cell_context[4];
            let result = self.dispatcher.dispatch(&mut session, packet);
            let cell_after = session.cell_context[4];
            (result.responses, result.broadcasts, result.disconnect, cell_before, cell_after)
        }; // session guard released here

        // Script notifications from the dispatched packets (create/destroy,
        // equip, item count, activate, GUI clicks, death).
        for pkt in responses.iter().chain(broadcasts.iter()) {
            match pkt {
                Packet::ObjectNew { id, .. } => self.script_engine.notify_create(id.as_u64()),
                Packet::ObjectRemove { id, .. } => self.script_engine.notify_destroy(id.as_u64()),
                Packet::UpdateItemEquipped { id, equipped, .. } => {
                    // The item's owning container is its `container` field.
                    let owner = self
                        .dispatcher
                        .registry
                        .get(*id)
                        .and_then(|arc| {
                            let guard = arc.read();
                            guard
                                .as_any()
                                .downcast_ref::<crate::world::objects::Item>()
                                .map(|i| i.container.as_u64())
                        })
                        .unwrap_or(0);
                    self.script_engine.notify_equip(owner, id.as_u64(), *equipped);
                }
                Packet::UpdateItemCount { id, count, .. } => {
                    self.script_engine.notify_item_count(id.as_u64(), *count);
                }
                Packet::UpdateActivate { id, actor } => {
                    // id here is the ref being activated; actor is the activator.
                    self.script_engine.notify_activate(id.as_u64() as u32, actor.as_u64());
                }
                Packet::UpdateWindowClick { id } => {
                    let pid = self
                        .sessions
                        .get(&addr)
                        .and_then(|s| s.value().player_id)
                        .map(|p| p.as_u64())
                        .unwrap_or(0);
                    self.script_engine.notify_window_click(pid, id.as_u64());
                }
                Packet::UpdateWindowReturn { id } => {
                    let pid = self
                        .sessions
                        .get(&addr)
                        .and_then(|s| s.value().player_id)
                        .map(|p| p.as_u64())
                        .unwrap_or(0);
                    self.script_engine.notify_window_return(pid, id.as_u64());
                }
                Packet::UpdateWindowText { id, text } => {
                    let pid = self.sessions_player_id(&addr);
                    self.script_engine.notify_window_text(pid, id.as_u64(), text);
                }
                Packet::UpdateCheckboxSelected { id, selected } => {
                    let pid = self.sessions_player_id(&addr);
                    self.script_engine.notify_checkbox(pid, id.as_u64(), *selected);
                }
                Packet::UpdateRadioButtonSelected { id, previous, .. } => {
                    let pid = self.sessions_player_id(&addr);
                    self.script_engine.notify_radio(pid, id.as_u64(), previous.as_u64());
                }
                Packet::UpdateListItemSelected { id, selected } => {
                    let pid = self.sessions_player_id(&addr);
                    self.script_engine.notify_list_item(pid, id.as_u64(), *selected);
                }
                Packet::UpdateFireWeapon { id, weapon } => {
                    self.script_engine.notify_fire_weapon(id.as_u64(), *weapon);
                }
                Packet::ActorPunch { id, power } => {
                    self.script_engine.notify_punch(id.as_u64(), *power);
                }
                Packet::UpdateItemCondition { id, condition, .. } => {
                    self.script_engine.notify_item_condition(id.as_u64(), *condition);
                }
                Packet::UpdateActorState { id, alerted, sneaking, .. } => {
                    self.script_engine.notify_actor_alert(id.as_u64(), *alerted);
                    self.script_engine.notify_actor_sneak(id.as_u64(), *sneaking);
                }
                Packet::UpdateActorValue { id, base, index, value } => {
                    self.script_engine.notify_actor_value(id.as_u64(), *index, *value, *base);
                }
                Packet::UpdateWindowMode { enabled } => {
                    let pid = self.sessions_player_id(&addr);
                    self.script_engine.notify_window_mode(pid, *enabled);
                }
                Packet::DialogueChoice { flag_id, choice } => {
                    let pid = self.sessions_player_id(&addr);
                    self.script_engine.notify_dialogue_choice(pid, *flag_id, *choice);
                }
                Packet::UpdateLock { id, lock } => {
                    let pid = self.sessions_player_id(&addr);
                    self.script_engine.notify_lock_change(id.as_u64(), pid, *lock);
                }
                // Death handled below (needs limbs/cause from the packet).
                _ => {}
            }
        }

        // on_cell_change when the player's center cell moved
        if cell_before != cell_after {
            let player_id = self
                .sessions
                .get(&addr)
                .and_then(|s| s.value().player_id)
                .map(|p| p.as_u64())
                .unwrap_or(0);
            self.script_engine.notify_cell_change(player_id, cell_after);
        }

        // Script on_actor_death notification for combat-resolved deaths
        for pkt in responses.iter().chain(broadcasts.iter()) {
            if let Packet::ActorDeathExt { id, killer, limbs, cause, .. } = pkt {
                self.script_engine.notify_actor_death(
                    id.as_u64(),
                    killer.as_u64(),
                    *limbs,
                    *cause,
                );
            }
        }

        // Send responses to this client (string-cache bound per-recipient)
        for pkt in responses {
            self.send(addr, pkt).await;
        }

        // Broadcast to all other clients
        for pkt in broadcasts {

            let targets: Vec<SocketAddr> = self
                .sessions
                .iter()
                .filter(|entry| entry.key() != &addr && entry.value().is_ingame())
                .map(|entry| *entry.key())
                .collect();

            for target in &targets {
                self.send(*target, pkt.clone()).await;
            }
        }

        if disconnect {
            self.disconnect(addr).await;
        }
    }

    /// Handle a new GameAuth connection.
    async fn handle_auth(&mut self, addr: SocketAddr, packet: Packet) {
        let (name, password, version) = match &packet {
            Packet::GameAuth { name, password, version } => {
                (name.clone(), password.clone(), version.clone())
            }
            _ => return,
        };

        // Check max connections
        if self.sessions.len() >= self.config.server.connections {
            tracing::warn!("Connection rejected: server full from {addr}");
            let end = Packet::GameEnd { reason: 5 }; // ponytail: full
            self.send(addr, end).await;
            return;
        }

        let session_id = self.allocate_session_id();

        // Script auth callback: any module vote of 0 rejects the connection.
        if !self.script_engine.dispatch_auth(&name, &password) {
            tracing::warn!("Script denied auth for {name} from {addr}");
            let end = Packet::GameEnd { reason: Reason::Denied as u8 };
            self.send(addr, end).await;
            return;
        }

        let (session, responses) = self.dispatcher.handle_connection(
            addr, name.clone(), password, version, session_id,
        );

        let mut session = match session {
            Some(s) => s,
            None => {
                for pkt in responses {
                    self.send(addr, pkt).await;
                }
                return;
            }
        };

        // Send initial responses (GameLoad)
        for pkt in responses {
            self.send(addr, pkt).await;
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
            let pkt = {
                let guard = arc.read();
                guard
                    .as_any()
                    .downcast_ref::<crate::world::objects::Player>()
                    .map(|p| p.to_new_packet())
            };
            if let Some(pkt) = pkt {
                self.send(addr, pkt).await;
            }
        }

        // Insert the session before world-state sends so the string cache
        // finalize pass binds names to this connection (first sight → Inline).
        // The reliable channel was already bootstrapped on first contact in
        // NetworkManager::try_recv — do NOT re-register it here.
        let world_packets = self.dispatcher.send_world_state(&session);
        self.sessions.insert(addr, session);
        for pkt in world_packets {
            self.send(addr, pkt).await;
        }

        // Server rules + authoritative game clock on join (STR ServerSettings /
        // CalendarService: join-time send, then change broadcasts from the tick).
        self.send(addr, Packet::ServerSettings {
            pvp_enabled: self.config.server.pvp_enabled,
        }).await;
        if let Some(gt) = self.script_engine.game_time.clone() {
            let t = gt.get();
            self.send(addr, Packet::GameTime {
                year: t.year, month: t.month, day: t.day, hour: t.hour,
                time_scale: gt.get_scale(),
            }).await;
        }

        // Broadcast PlayerNew to all existing players
        let other_addrs: Vec<SocketAddr> = self.sessions
            .iter()
            .filter(|entry| entry.value().is_ingame())
            .map(|entry| *entry.key())
            .collect();
        if let Some(arc) = self.dispatcher.registry.get(player_id) {
            let player_pkt = {
                let guard = arc.read();
                guard
                    .as_any()
                    .downcast_ref::<crate::world::objects::Player>()
                    .map(|p| p.to_new_packet())
            };
            if let Some(player_pkt) = player_pkt {
                for other_addr in &other_addrs {
                    if *other_addr != addr {
                        self.send(*other_addr, player_pkt.clone()).await;
                    }
                }
            }
        }

        tracing::info!("Player {name} (id={player_id}) connected from {addr}");
    }

    /// Player id for a session address (0 when absent).
    fn sessions_player_id(&self, addr: &SocketAddr) -> u64 {
        self.sessions
            .get(addr)
            .and_then(|s| s.value().player_id)
            .map(|p| p.as_u64())
            .unwrap_or(0)
    }

    /// Disconnect a session.
    async fn disconnect(&mut self, addr: SocketAddr) {
        if let Some((_, session)) = self.sessions.remove(&addr) {
            tracing::info!("Session {} disconnected", session.player_name);

            // Script on_player_disconnect notification
            if let Some(pid) = session.player_id {
                self.script_engine.notify_disconnect(pid.as_u64(), 4);
            }

            // Release simulation ownership of every actor this player owned,
            // and tell the survivors so they can reclaim (STR OwnershipTransfer).
            if let Some(pid) = session.player_id {
                let released = self.dispatcher.registry.release_player_owned(pid);
                if !released.is_empty() {
                    let addrs: Vec<SocketAddr> = self
                        .sessions
                        .iter()
                        .filter(|e| e.value().is_ingame())
                        .map(|e| *e.key())
                        .collect();
                    for id in released {
                        let pkt = Packet::OwnershipReleased { id };
                        for a in &addrs {
                            self.send(*a, pkt.clone()).await;
                        }
                    }
                }
            }

            // Remove player object
            if let Some(pid) = session.player_id {
                self.dispatcher.registry.remove(pid);
            }

            self.network.remove_session(addr);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_advance_hour_rolls_over() {
        let mut t = GameTime { year: 2277, month: 8, day: 17, hour: 9 };
        advance_hour(&mut t);
        assert_eq!((t.year, t.month, t.day, t.hour), (2277, 8, 17, 10));

        // Day rollover
        let mut t = GameTime { year: 2277, month: 8, day: 17, hour: 23 };
        advance_hour(&mut t);
        assert_eq!((t.year, t.month, t.day, t.hour), (2277, 8, 18, 0));

        // Month rollover (30-day months)
        let mut t = GameTime { year: 2277, month: 8, day: 30, hour: 23 };
        advance_hour(&mut t);
        assert_eq!((t.year, t.month, t.day, t.hour), (2277, 9, 1, 0));

        // Year rollover
        let mut t = GameTime { year: 2277, month: 12, day: 30, hour: 23 };
        advance_hour(&mut t);
        assert_eq!((t.year, t.month, t.day, t.hour), (2278, 1, 1, 0));
    }
}
