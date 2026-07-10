//! Packet dispatch — match incoming packet → handler function.
//!
//! All server packet processing routes through here.

use ashfall_core::protocol::Packet;
use crate::ai::factions::FactionMatrix;
use crate::handlers::{actor, auth, chat, combat, game, gui, item, object, physics, player, quest};
use crate::quest::QuestManager;
use crate::session::Session;
use crate::world::globals::GlobalState;
use crate::world::objects::{Actor, Container, Item, Object, Player};
use crate::world::registry::ObjectRegistry;
use crate::world::weather::WeatherState;
use std::net::SocketAddr;
use std::sync::Arc;

/// Result of dispatching a packet.
pub struct DispatchResult {
    /// Packets to send to the sender.
    pub responses: Vec<Packet>,
    /// Packets to broadcast to all other sessions.
    pub broadcasts: Vec<Packet>,
    /// If true, the session should be terminated.
    pub disconnect: bool,
}

impl DispatchResult {
    pub fn new() -> Self {
        DispatchResult { responses: vec![], broadcasts: vec![], disconnect: false }
    }

    pub fn response(mut self, pkt: Packet) -> Self {
        self.responses.push(pkt);
        self
    }

    pub fn broadcast(mut self, pkt: Packet) -> Self {
        self.broadcasts.push(pkt);
        self
    }

    pub fn responses(mut self, pkts: Vec<Packet>) -> Self {
        self.responses.extend(pkts);
        self
    }

    pub fn with_disconnect(mut self) -> Self {
        self.disconnect = true;
        self
    }
}

/// Central packet dispatcher.
pub struct Dispatcher {
    pub registry: Arc<ObjectRegistry>,
    pub weather: WeatherState,
    pub globals: GlobalState,
    pub quests: QuestManager,
    pub factions: FactionMatrix,
}

impl Dispatcher {
    pub fn new() -> Self {
        Dispatcher {
            registry: Arc::new(ObjectRegistry::new()),
            weather: WeatherState::default(),
            globals: GlobalState::new(),
            quests: QuestManager::new(),
            factions: FactionMatrix::default(),
        }
    }

    /// Dispatch a packet from a session.
    pub fn dispatch(
        &self,
        session: &mut Session,
        packet: Packet,
    ) -> DispatchResult {
        tracing::trace!("Dispatching {:?} from {}", std::mem::discriminant(&packet), session.player_name);

        match packet {
            // ═══ System ═══
            Packet::GameEnd { reason } => {
                let pkts = auth::handle_disconnect(session, reason);
                DispatchResult::new().responses(pkts).with_disconnect()
            }

            // ═══ Object ═══
            Packet::UpdatePos { id, pos } => {
                match object::handle_update_pos(&self.registry, session, id, pos) {
                    Some(pkt) => DispatchResult::new().broadcast(pkt),
                    None => DispatchResult::new(), // rejected
                }
            }
            Packet::UpdateAngle { id, angle } => {
                match object::handle_update_angle(&self.registry, id, angle) {
                    Some(pkt) => DispatchResult::new().broadcast(pkt),
                    None => DispatchResult::new(),
                }
            }
            Packet::UpdateScale { id, scale } => {
                match object::handle_update_scale(&self.registry, id, scale) {
                    Some(pkt) => DispatchResult::new().broadcast(pkt),
                    None => DispatchResult::new(),
                }
            }
            Packet::UpdateCell { id, cell, pos } => {
                match object::handle_update_cell(&self.registry, id, cell, pos) {
                    Some(pkt) => DispatchResult::new().broadcast(pkt),
                    None => DispatchResult::new(),
                }
            }
            Packet::UpdateName { id, name } => {
                match object::handle_update_name(&self.registry, id, name) {
                    Some(pkt) => DispatchResult::new().broadcast(pkt),
                    None => DispatchResult::new(),
                }
            }

            // ═══ Physics ═══
            Packet::UpdateVelocity { id, vel, on_ground } => {
                match physics::handle_update_velocity(&self.registry, session, id, vel, on_ground) {
                    Some(pkt) => DispatchResult::new().broadcast(pkt),
                    None => DispatchResult::new(),
                }
            }

            // ═══ Item ═══
            Packet::ItemNew { .. } => {
                match item::handle_item_new(&self.registry, &packet) {
                    Some(pkt) => DispatchResult::new().broadcast(pkt),
                    None => DispatchResult::new(),
                }
            }
            Packet::UpdateItemCount { id, count, silent } => {
                match item::handle_item_count(&self.registry, id, count, silent) {
                    Some(pkt) => {
                        if silent { DispatchResult::new().response(pkt) }
                        else { DispatchResult::new().broadcast(pkt) }
                    }
                    None => DispatchResult::new(),
                }
            }
            Packet::UpdateItemCondition { id, condition, health } => {
                match item::handle_item_condition(&self.registry, id, condition, health) {
                    Some(pkt) => DispatchResult::new().broadcast(pkt),
                    None => DispatchResult::new(),
                }
            }
            Packet::UpdateItemEquipped { id, equipped, silent, stick } => {
                match item::handle_item_equipped(&self.registry, id, equipped, silent, stick) {
                    Some(pkt) => {
                        if silent { DispatchResult::new().response(pkt) }
                        else { DispatchResult::new().broadcast(pkt) }
                    }
                    None => DispatchResult::new(),
                }
            }

            // ═══ Actor ═══
            Packet::ActorNew { .. } => {
                match actor::handle_actor_new(&self.registry, &packet) {
                    Some(pkt) => DispatchResult::new().broadcast(pkt),
                    None => DispatchResult::new(),
                }
            }
            Packet::UpdateActorDead { id, dead, limbs, cause } => {
                match actor::handle_actor_dead(&self.registry, id, dead, limbs, cause) {
                    Some(pkt) => DispatchResult::new().broadcast(pkt),
                    None => DispatchResult::new(),
                }
            }
            Packet::UpdateActorState { id, idle, moving, moving_xy, weapon, alerted, sneaking, firing } => {
                match actor::handle_actor_state(&self.registry, id, idle, moving, moving_xy, weapon, alerted, sneaking, firing) {
                    Some(pkt) => DispatchResult::new().broadcast(pkt),
                    None => DispatchResult::new(),
                }
            }
            Packet::UpdateActorValue { id, base, index, value } => {
                match actor::handle_actor_value(&self.registry, id, base, index, value) {
                    Some(pkt) => DispatchResult::new().broadcast(pkt),
                    None => DispatchResult::new(),
                }
            }
            Packet::UpdateFireWeapon { id, weapon } => {
                match actor::handle_fire_weapon(&self.registry, id, weapon) {
                    Some(pkt) => DispatchResult::new().broadcast(pkt),
                    None => DispatchResult::new(),
                }
            }

            // ═══ Combat ═══
            Packet::ActorHit { .. } => {
                match combat::handle_actor_hit(&self.registry, &packet) {
                    Some(pkts) => {
                        let mut result = DispatchResult::new();
                        for p in pkts { result = result.broadcast(p); }
                        result
                    }
                    None => DispatchResult::new(),
                }
            }
            Packet::ProjectileNew { .. } => {
                match combat::handle_projectile_new(&packet) {
                    Some(pkt) => DispatchResult::new().broadcast(pkt),
                    None => DispatchResult::new(),
                }
            }
            Packet::ProjectileRemove { .. } => {
                match combat::handle_projectile_remove(&packet) {
                    Some(pkt) => DispatchResult::new().broadcast(pkt),
                    None => DispatchResult::new(),
                }
            }
            Packet::ExplosionNew { .. } => {
                match combat::handle_explosion_new(&packet) {
                    Some(pkt) => DispatchResult::new().broadcast(pkt),
                    None => DispatchResult::new(),
                }
            }

            // ═══ NPC AI ═══
            Packet::ActorCombatTarget { id, target } => {
                DispatchResult::new().broadcast(Packet::ActorCombatTarget { id, target })
            }
            Packet::ActorAIPackage { id, package_id, flags } => {
                DispatchResult::new().broadcast(Packet::ActorAIPackage { id, package_id, flags })
            }
            Packet::ActorFaction { id, faction_id, rank } => {
                DispatchResult::new().broadcast(Packet::ActorFaction { id, faction_id, rank })
            }

            // ═══ Player ═══
            Packet::UpdateControl { id, control, key } => {
                match player::handle_update_control(&self.registry, id, control, key) {
                    Some(pkt) => DispatchResult::new().broadcast(pkt),
                    None => DispatchResult::new(),
                }
            }
            Packet::UpdateContext { id, cells, spawn } => {
                let pkts = player::handle_update_context(&self.registry, session, id, cells, spawn);
                // Send responses to this player, nothing to broadcast (cell snapshots handled internally)
                DispatchResult::new().responses(pkts)
            }
            Packet::UpdateConsole { id, enabled } => {
                DispatchResult::new().response(Packet::UpdateConsole { id, enabled })
            }

            // ═══ Chat ═══
            Packet::GameChat { message } => {
                match chat::handle_chat(message) {
                    Some(pkt) => DispatchResult::new().broadcast(pkt),
                    None => DispatchResult::new(),
                }
            }

            // ═══ Quest ═══
            Packet::QuestStage { quest_id, stage } => {
                let pkt = quest::handle_quest_stage(&self.quests, quest_id, stage);
                DispatchResult::new().broadcast(pkt)
            }
            Packet::DialogueFlag { flag_id, value } => {
                let pkt = quest::handle_dialogue_flag(&self.quests, flag_id, value);
                DispatchResult::new().broadcast(pkt)
            }
            Packet::DialogueChoice { flag_id, choice } => {
                let pkt = quest::handle_dialogue_choice(&self.quests, flag_id, choice);
                DispatchResult::new().broadcast(pkt)
            }

            // ═══ FO3/FNV Globals ═══
            Packet::KarmaUpdate { value } => {
                DispatchResult::new().broadcast(quest::handle_karma(value))
            }
            Packet::ReputationUpdate { faction, value } => {
                DispatchResult::new().broadcast(quest::handle_reputation(faction, value))
            }
            Packet::HardcoreStats { hunger, thirst, sleep } => {
                DispatchResult::new().broadcast(quest::handle_hardcore_stats(hunger, thirst, sleep))
            }

            // ═══ GUI ═══
            Packet::UpdateWindowMode { enabled } => {
                DispatchResult::new().response(gui::handle_window_mode(enabled))
            }
            Packet::UpdateWindowClick { id } => {
                DispatchResult::new().response(gui::handle_window_click(id))
            }
            Packet::UpdateWindowReturn { id } => {
                DispatchResult::new().response(gui::handle_window_return(id))
            }

            // ═══ Object / Container / Player Create ═══
            Packet::ObjectNew { .. } => {
                match object::handle_object_new(&self.registry, &packet) {
                    Some(pkt) => DispatchResult::new().broadcast(pkt),
                    None => DispatchResult::new(),
                }
            }
            Packet::ContainerNew { .. } => {
                match object::handle_container_new(&self.registry, &packet) {
                    Some(pkt) => DispatchResult::new().broadcast(pkt),
                    None => DispatchResult::new(),
                }
            }
            Packet::PlayerNew { .. } => {
                match player::handle_player_new(&self.registry, &packet) {
                    Some(pkt) => DispatchResult::new().broadcast(pkt),
                    None => DispatchResult::new(),
                }
            }
            Packet::ObjectRemove { id, silent } => {
                match object::handle_object_remove(&self.registry, id, silent) {
                    Some(pkt) => DispatchResult::new().broadcast(pkt),
                    None => DispatchResult::new(),
                }
            }

            // ═══ Passthrough (relay as-is) ═══
            Packet::WindowNew { .. }
            | Packet::WindowRemove { .. }
            | Packet::UpdateLock { .. }
            | Packet::UpdateOwner { .. }
            | Packet::UpdateActivate { .. }
            | Packet::UpdateSound { .. }
            | Packet::UpdateActorRace { .. }
            | Packet::UpdateActorSex { .. }
            | Packet::UpdateActorIdle { .. }
            | Packet::DoorState { .. }
            | Packet::TerminalState { .. }
            | Packet::ItemListNew { .. }
            | Packet::GameWeather { .. }
            | Packet::GameGlobal { .. }
            | Packet::GameBase { .. }
            => {
                DispatchResult::new().broadcast(packet)
            }

            // ═══ Ignored (server-only, no relay) ═══
            Packet::GameLoad
            | Packet::GameStart
            | Packet::GameAuth { .. }
            | Packet::GameMod { .. }
            | Packet::GameMessage { .. }
            | Packet::GameDeleted { .. }
            | Packet::MasterQuery
            | Packet::MasterAnnounce { .. }
            | Packet::MasterUpdate { .. }
            | Packet::ReferenceNew { .. }
            | Packet::CellSnapshot { .. }
            | Packet::VolatileNew { .. }
            | Packet::UpdateInterior { .. }
            | Packet::UpdateExterior { .. }
            | Packet::ActorDamaged { .. } // generated by combat resolver
            | Packet::ActorDeathExt { .. } // generated by combat resolver
            | Packet::ListItemNew { .. }
            | Packet::ListItemRemove { .. }
            | Packet::UpdateWindowPos { .. }
            | Packet::UpdateWindowSize { .. }
            | Packet::UpdateWindowVisible { .. }
            | Packet::UpdateWindowLocked { .. }
            | Packet::UpdateWindowText { .. }
            | Packet::UpdateEditMaxLen { .. }
            | Packet::UpdateEditValidation { .. }
            | Packet::UpdateCheckboxSelected { .. }
            | Packet::UpdateRadioButtonSelected { .. }
            | Packet::UpdateRadioButtonGroup { .. }
            | Packet::UpdateListMultiSelect { .. }
            | Packet::UpdateListItemSelected { .. }
            | Packet::UpdateListItemText { .. }
            | Packet::EditNew { .. }
            | Packet::CheckboxNew { .. }
            | Packet::RadioButtonNew { .. }
            | Packet::ListNew { .. }
            => {
                DispatchResult::new()
            }
        }
    }

    /// Handle initial connection — authenticate, create session, send world state.
    pub fn handle_connection(
        &self,
        addr: SocketAddr,
        name: String,
        password: String,
        session_id: ashfall_core::id::NetworkID,
    ) -> (Option<Session>, Vec<Packet>) {
        auth::handle_auth(addr, name, password, session_id)
    }

    /// Send world state to a newly authenticated session.
    pub fn send_world_state(&self, session: &Session) -> Vec<Packet> {
        game::send_world_state(
            session,
            &self.weather,
            &self.globals,
            &self.quests,
            &self.registry,
        )
    }
}
