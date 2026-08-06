//! Item handler — inventory, count, condition, equip.

use ashfall_core::id::NetworkID;
use ashfall_core::protocol::Packet;
use crate::anti_cheat::AntiCheat;
use crate::session::Session;
use crate::world::inventory::Inventory;
use crate::world::objects::{Container, Item, Player};
use ashfall_core::types::GameObject;
use crate::world::registry::ObjectRegistry;
use std::sync::Arc;

/// Ownership: a client may only mutate items whose container chain leads to
/// its own player (its inventory). Items in world containers (footlockers,
/// NPC inventories) are server-managed.
fn owned(registry: &Arc<ObjectRegistry>, session: &Session, id: NetworkID) -> bool {
    let Some(owner) = session.player_id else { return false };
    let Some(arc) = registry.get(id) else { return false };
    let guard = arc.read();
    let Some(item) = guard.as_any().downcast_ref::<Item>() else { return false };
    if item.container == owner {
        return true;
    }
    // The item's container may itself be the player (equipped items).
    let Some(carc) = registry.get(item.container) else { return false };
    let cguard = carc.read();
    matches!(cguard.as_any().downcast_ref::<Player>(), Some(p) if p.id() == owner)
}

/// Handle ItemNew.
pub fn handle_item_new(registry: &Arc<ObjectRegistry>, packet: &Packet) -> Option<Packet> {
    if let Packet::ItemNew { id, ref_id, base_id, container, count, condition, equipped, silent, stick, scale } = packet {
        if registry.is_deleted(*id) { return None; }
        let mut item = Item::new(*id, *ref_id, *base_id, *container);
        item.count = *count;
        item.condition = *condition;
        item.equipped = *equipped;
        item.silent = *silent;
        item.stick = *stick;
        item.scale = *scale;

        if let Some(arc) = registry.get(*container) {
            let mut guard = arc.write();
            if let Some(cont) = guard.as_any_mut().downcast_mut::<Container>() {
                Inventory::add(&mut cont.items, *id);
            }
        }

        registry.insert(item);
        Some(packet.clone())
    } else { None }
}

/// Handle UpdateItemCount.
pub fn handle_item_count(registry: &Arc<ObjectRegistry>, session: &Session, id: NetworkID, count: u32, silent: bool) -> Option<Packet> {
    if !owned(registry, session, id) {
        tracing::warn!("Item count rejected for {id} from {} (not owner)", session.player_name);
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
pub fn handle_item_condition(registry: &Arc<ObjectRegistry>, session: &Session, id: NetworkID, condition: f32, health: u32) -> Option<Packet> {
    if !owned(registry, session, id) {
        return None;
    }
    if let Some(arc) = registry.get(id) {
        let mut guard = arc.write();
        if let Some(item) = guard.as_any_mut().downcast_mut::<Item>() {
            item.condition = condition;
        }
    }
    Some(Packet::UpdateItemCondition { id, condition, health })
}

/// Handle UpdateItemEquipped.
pub fn handle_item_equipped(registry: &Arc<ObjectRegistry>, session: &Session, id: NetworkID, equipped: bool, silent: bool, stick: bool) -> Option<Packet> {
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
    Some(Packet::UpdateItemEquipped { id, equipped, silent, stick })
}
