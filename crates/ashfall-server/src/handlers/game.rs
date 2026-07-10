//! Game lifecycle handlers — GameLoad, GameStart, GameWeather, GameGlobal, etc.

use ashfall_core::id::NetworkID;
use ashfall_core::protocol::Packet;
use ashfall_core::types::GameObject;
use crate::session::Session;
use crate::world::globals::GlobalState;
use crate::world::objects::{Actor, Container, Item, Object, Player};
use crate::world::registry::ObjectRegistry;
use crate::world::weather::WeatherState;
use crate::quest::QuestManager;
use std::sync::Arc;

/// Send initial world state to a newly connected client.
pub fn send_world_state(
    session: &Session,
    weather: &WeatherState,
    globals: &GlobalState,
    quests: &QuestManager,
    registry: &Arc<ObjectRegistry>,
) -> Vec<Packet> {
    let mut packets = Vec::new();

    // Weather
    packets.push(Packet::GameWeather { weather: weather.get() });

    // All global variables
    for (id, value) in globals.all() {
        packets.push(Packet::GameGlobal { global: id, value });
    }

    // All quest stages
    for (quest_id, stage) in quests.all_stages() {
        packets.push(Packet::QuestStage { quest_id, stage });
    }

    // Cell snapshot for player's current cell context — send New packets for all objects
    let cell_objects = registry.get_by_cells(&session.cell_context);
    for obj_id in &cell_objects {
        if let Some(arc) = registry.get(*obj_id) {
            let guard = arc.read();
            let packet: Option<Packet> = if let Some(cont) = guard.as_any().downcast_ref::<Container>() {
                let (cid, ref_id, base_id) = (cont.id(), cont.ref_data.ref_id, cont.ref_data.base_id);
                drop(guard);
                Some(Packet::ContainerNew { id: cid, ref_id, base_id })
            } else if let Some(obj) = guard.as_any().downcast_ref::<Object>() {
                let pkt = obj.to_new_packet();
                drop(guard);
                Some(pkt)
            } else if let Some(item) = guard.as_any().downcast_ref::<Item>() {
                let pkt = item.to_new_packet();
                drop(guard);
                Some(pkt)
            } else if let Some(actor) = guard.as_any().downcast_ref::<Actor>() {
                let pkt = actor.to_new_packet();
                drop(guard);
                Some(pkt)
            } else if let Some(player) = guard.as_any().downcast_ref::<Player>() {
                // Skip self — PlayerNew sent separately
                if player.id() == session.player_id.unwrap_or(NetworkID::NULL) {
                    None
                } else {
                    let pkt = player.to_new_packet();
                    drop(guard);
                    Some(pkt)
                }
            } else {
                None
            };
            if let Some(pkt) = packet {
                packets.push(pkt);
            }
        }
    }

    // Existing players (PlayerNew for each)
    let player_ids = registry.get_by_kind(
        ashfall_core::types::ObjectKind::Player as u32,
    );
    for pid in player_ids {
        if let Some(player) = registry.get_typed::<crate::world::objects::Player>(pid) {
            if pid != session.player_id.unwrap_or(NetworkID::NULL) {
                packets.push(player.to_new_packet());
            }
        }
    }

    // GameStart
    packets.push(Packet::GameStart);

    packets
}

/// Handle weather change.
pub fn handle_weather(weather: &WeatherState, value: u32) -> Packet {
    weather.set(value);
    Packet::GameWeather { weather: value }
}

/// Handle global variable change.
pub fn handle_global(globals: &GlobalState, id: u32, value: i32) -> Packet {
    globals.set(id, value);
    Packet::GameGlobal { global: id, value }
}
