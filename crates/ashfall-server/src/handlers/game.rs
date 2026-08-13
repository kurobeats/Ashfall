//! Game lifecycle handlers — GameLoad, GameStart, GameWeather, GameGlobal, etc.

use ashfall_core::id::NetworkID;
use ashfall_core::protocol::Packet;
use ashfall_core::types::{GameObject, Reason};
use crate::session::Session;
use crate::world::globals::GlobalState;
use crate::world::objects::{Actor, Container, Item, Object, Player};
use crate::world::registry::ObjectRegistry;
use crate::world::weather::WeatherState;
use crate::quest::QuestManager;
use std::sync::Arc;

/// Parse a config mod entry "filename:crc" (crc hex) into a pair.
pub fn parse_mod_entry(s: &str) -> Option<(String, u32)> {
    let (file, crc) = s.rsplit_once(':')?;
    let crc = u32::from_str_radix(crc.trim(), 16).ok()?;
    Some((file.trim().to_string(), crc))
}

/// Handle GameModList — verify the client's load order matches the server's
/// expected list (STR ModPolicy). Returns a GameEnd to reject on mismatch,
/// None when accepted (or when the server enforces no list).
pub fn handle_mod_list(expected: &[(String, u32)], mods: &[(String, u32)]) -> Option<Packet> {
    if expected.is_empty() {
        return None; // policy off — accept anything
    }
    if mods.len() != expected.len() {
        tracing::warn!("ModList rejected: {} mods, server expects {}", mods.len(), expected.len());
        return Some(Packet::GameEnd { reason: Reason::Denied as u8 });
    }
    for (i, (client_m, client_crc)) in mods.iter().enumerate() {
        let (server_m, server_crc) = &expected[i];
        if client_m != server_m || client_crc != server_crc {
            tracing::warn!("ModList rejected: index {i} {:?} != expected {:?}", client_m, server_m);
            return Some(Packet::GameEnd { reason: Reason::Denied as u8 });
        }
    }
    tracing::info!("ModList accepted: {} mods match", mods.len());
    None
}

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_mod_entry() {
        assert_eq!(
            parse_mod_entry("Fallout3.esm:1C877592"),
            Some(("Fallout3.esm".to_string(), 0x1C877592))
        );
        assert_eq!(
            parse_mod_entry(" mods/example.esp : deadbeef "),
            Some(("mods/example.esp".to_string(), 0xDEADBEEF))
        );
        assert_eq!(parse_mod_entry("nocolon"), None);
        assert_eq!(parse_mod_entry("file:zzz"), None, "non-hex crc");
        assert_eq!(parse_mod_entry(""), None);
    }

    #[test]
    fn test_mod_policy_off_accepts_anything() {
        assert!(handle_mod_list(&[], &vec![("anything.esp".into(), 0)]).is_none());
        assert!(handle_mod_list(&[], &[]).is_none());
    }

    #[test]
    fn test_mod_policy_exact_match_required() {
        let expected = vec![
            ("Fallout3.esm".to_string(), 0x1C877592),
            ("example.esp".to_string(), 0xDEADBEEF),
        ];
        // Exact order + crc → accept.
        assert!(handle_mod_list(&expected, &expected).is_none());

        // Wrong count → reject.
        assert!(handle_mod_list(&expected, &[]).is_some());
        assert!(handle_mod_list(&expected, &expected[..1]).is_some());

        // Wrong order (same files) → reject.
        let swapped = vec![expected[1].clone(), expected[0].clone()];
        assert!(handle_mod_list(&expected, &swapped).is_some());

        // Wrong crc → reject.
        let bad_crc = vec![("Fallout3.esm".to_string(), 0x1234), expected[1].clone()];
        assert!(handle_mod_list(&expected, &bad_crc).is_some());

        // Wrong filename → reject.
        let bad_name = vec![("Oblivion.esm".to_string(), expected[0].1), expected[1].clone()];
        assert!(handle_mod_list(&expected, &bad_name).is_some());
    }
}
