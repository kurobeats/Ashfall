//! Item handler — inventory, count, condition, equip.

use crate::anti_cheat::AntiCheat;
use crate::session::Session;
use crate::world::objects::{Item, Player};
use crate::world::registry::ObjectRegistry;
use ashfall_core::id::NetworkID;
use ashfall_core::protocol::Packet;
use ashfall_core::types::GameObject;
use std::sync::Arc;

/// Ownership: a client may only mutate items whose container chain leads to
/// its own player (its inventory). Items in world containers (footlockers,
/// NPC inventories) are server-managed.
fn owned(registry: &Arc<ObjectRegistry>, session: &Session, id: NetworkID) -> bool {
    let Some(owner) = session.player_id else {
        return false;
    };
    let Some(arc) = registry.get(id) else {
        return false;
    };
    let guard = arc.read();
    let Some(item) = guard.as_any().downcast_ref::<Item>() else {
        return false;
    };
    if item.container == owner {
        return true;
    }
    // The item's container may itself be the player (equipped items).
    let Some(carc) = registry.get(item.container) else {
        return false;
    };
    let cguard = carc.read();
    matches!(cguard.as_any().downcast_ref::<Player>(), Some(p) if p.id() == owner)
}

/// Handle ItemNew — server-authoritative item creation only.
///
/// Clients never legitimately send ItemNew (they receive it when the server
/// spawns inventory); accepting it from a client would let anyone mint items
/// into arbitrary containers. Reject with a warning.
pub fn handle_item_new(
    registry: &Arc<ObjectRegistry>,
    session: &Session,
    packet: &Packet,
) -> Option<Packet> {
    if let Packet::ItemNew { id, container, .. } = packet {
        tracing::warn!(
            "ItemNew rejected from {} (id={id}, container={container}) — server-authoritative creation only",
            session.player_name
        );
    }
    let _ = registry;
    None
}

/// Handle UpdateItemCount.
pub fn handle_item_count(
    registry: &Arc<ObjectRegistry>,
    session: &Session,
    id: NetworkID,
    count: u32,
    silent: bool,
) -> Option<Packet> {
    if !owned(registry, session, id) {
        tracing::warn!(
            "Item count rejected for {id} from {} (not owner)",
            session.player_name
        );
        return None;
    }
    if !AntiCheat::validate_item_count(count) {
        tracing::warn!("AntiCheat: item count rejected — {count}");
        return None;
    }
    if let Some(arc) = registry.get(id) {
        let mut guard = arc.write();
        if let Some(item) = guard.as_any_mut().downcast_mut::<Item>() {
            item.count = count;
        }
    }
    Some(Packet::UpdateItemCount { id, count, silent })
}

/// Handle UpdateItemCondition.
pub fn handle_item_condition(
    registry: &Arc<ObjectRegistry>,
    session: &Session,
    id: NetworkID,
    condition: f32,
    health: u32,
) -> Option<Packet> {
    if !owned(registry, session, id) {
        return None;
    }
    if !AntiCheat::validate_item_condition(condition) {
        tracing::warn!("AntiCheat: item condition rejected — {condition}");
        return None;
    }
    if let Some(arc) = registry.get(id) {
        let mut guard = arc.write();
        if let Some(item) = guard.as_any_mut().downcast_mut::<Item>() {
            item.condition = condition;
        }
    }
    Some(Packet::UpdateItemCondition {
        id,
        condition,
        health,
    })
}

/// Handle UpdateItemEquipped.
pub fn handle_item_equipped(
    registry: &Arc<ObjectRegistry>,
    session: &Session,
    id: NetworkID,
    equipped: bool,
    silent: bool,
    stick: bool,
) -> Option<Packet> {
    if !owned(registry, session, id) {
        return None;
    }
    if let Some(arc) = registry.get(id) {
        let mut guard = arc.write();
        if let Some(item) = guard.as_any_mut().downcast_mut::<Item>() {
            item.equipped = equipped;
            item.silent = silent;
            item.stick = stick;
        }
    }
    Some(Packet::UpdateItemEquipped {
        id,
        equipped,
        silent,
        stick,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::Session;
    use crate::world::objects::Player;
    use crate::world::registry::ObjectRegistry;
    use std::net::SocketAddr;

    fn setup() -> (Arc<ObjectRegistry>, Session, NetworkID, NetworkID) {
        let registry = Arc::new(ObjectRegistry::new());
        let player_id = registry.insert(Player::new(NetworkID::new(2), 0x100, 0x7, 0x1));
        let item_id = registry.insert(Item::new(NetworkID::new(3), 0x200, 0x201, player_id));
        let mut session = Session::new(
            NetworkID::new(1),
            "127.0.0.1:9000".parse::<SocketAddr>().unwrap(),
            "Wanderer".into(),
        );
        session.player_id = Some(player_id);
        (registry, session, player_id, item_id)
    }

    #[test]
    fn owner_can_update_item_count() {
        let (registry, session, _, item_id) = setup();
        let result = handle_item_count(&registry, &session, item_id, 12, false);
        assert!(result.is_some());
        let arc = registry.get(item_id).unwrap();
        let guard = arc.read();
        let item = guard.as_any().downcast_ref::<Item>().unwrap();
        assert_eq!(item.count, 12);
    }

    #[test]
    fn non_owner_item_count_rejected() {
        let (registry, session, _, item_id) = setup();
        let mut other = session;
        other.player_id = Some(NetworkID::new(99));
        assert!(handle_item_count(&registry, &other, item_id, 12, false).is_none());
    }

    #[test]
    fn overstack_item_count_rejected() {
        let (registry, session, _, item_id) = setup();
        let bad = ashfall_core::constants::MAX_ITEM_STACK + 1;
        assert!(handle_item_count(&registry, &session, item_id, bad, false).is_none());
    }

    #[test]
    fn invalid_item_condition_rejected() {
        let (registry, session, _, item_id) = setup();
        assert!(handle_item_condition(&registry, &session, item_id, 101.0, 100).is_none());
        assert!(handle_item_condition(&registry, &session, item_id, -1.0, 100).is_none());
    }

    #[test]
    fn item_new_from_client_rejected() {
        // server-authoritative creation only — ItemNew from a client is a mint.
        let (registry, session, _, _) = setup();
        let packet = Packet::ItemNew {
            id: NetworkID::new(50),
            ref_id: 0x300,
            base_id: 0x301,
            container: NetworkID::new(2),
            count: 1,
            condition: 1.0,
            equipped: false,
            silent: false,
            stick: false,
            scale: 1.0,
        };
        assert!(handle_item_new(&registry, &session, &packet).is_none());
    }
}
