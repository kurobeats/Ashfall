//! Player handler — spawn, controls, cell context, console.

use crate::session::Session;
use crate::world::cell::CellContext;
use crate::world::objects::Player;
use crate::world::registry::ObjectRegistry;
use ashfall_core::id::NetworkID;
use ashfall_core::protocol::Packet;
use std::sync::Arc;

/// Handle PlayerNew — create player, insert into registry, broadcast.
pub fn handle_player_new(registry: &Arc<ObjectRegistry>, packet: &Packet) -> Option<Packet> {
    let (id, ref_id, base_id, controls, scale) = match packet {
        Packet::PlayerNew {
            id,
            ref_id,
            base_id,
            controls,
            scale,
        } => (*id, *ref_id, *base_id, controls.clone(), *scale),
        _ => return None,
    };

    if registry.is_deleted(id) {
        return None;
    }

    let mut player = Player::new(id, ref_id, base_id, 0);
    player.controls = controls;
    player.object.scale = scale;

    registry.insert(player);

    Some(packet.clone())
}

/// Handle UpdateControl — player control binding change.
pub fn handle_update_control(
    registry: &Arc<ObjectRegistry>,
    session: &Session,
    id: NetworkID,
    control: u8,
    key: u8,
) -> Option<Packet> {
    // Only the session's own player may set its controls.
    if session.player_id != Some(id) {
        return None;
    }
    if let Some(arc) = registry.get(id) {
        let mut guard = arc.write();
        if let Some(player) = guard.as_any_mut().downcast_mut::<Player>() {
            player.controls.insert(control, (key, true));
        }
    }
    Some(Packet::UpdateControl { id, control, key })
}

/// New-packet for any streamable entity kind (Object/Container/Item/Actor/Player).
fn entity_new_packet(registry: &ObjectRegistry, id: NetworkID) -> Option<Packet> {
    let arc = registry.get(id)?;
    let guard = arc.read();
    if let Some(obj) = guard
        .as_any()
        .downcast_ref::<crate::world::objects::Object>()
    {
        return Some(obj.to_new_packet());
    }
    if let Some(cont) = guard
        .as_any()
        .downcast_ref::<crate::world::objects::Container>()
    {
        // ContainerNew + ObjectNew pair; keep it to one packet here (ObjectNew
        // carries the position/state — ContainerNew is the identity link).
        return Some(cont.object.to_new_packet());
    }
    if let Some(item) = guard.as_any().downcast_ref::<crate::world::objects::Item>() {
        return Some(item.to_new_packet());
    }
    if let Some(actor) = guard
        .as_any()
        .downcast_ref::<crate::world::objects::Actor>()
    {
        return Some(actor.to_new_packet());
    }
    if let Some(player) = guard
        .as_any()
        .downcast_ref::<crate::world::objects::Player>()
    {
        return Some(player.to_new_packet());
    }
    None
}

/// Cell of any position-bearing entity, if known.
fn entity_cell(registry: &ObjectRegistry, id: NetworkID) -> Option<u32> {
    let arc = registry.get(id)?;
    let guard = arc.read();
    if let Some(obj) = guard
        .as_any()
        .downcast_ref::<crate::world::objects::Object>()
    {
        return Some(obj.cell);
    }
    if let Some(actor) = guard
        .as_any()
        .downcast_ref::<crate::world::objects::Actor>()
    {
        return Some(actor.container.object.cell);
    }
    if let Some(player) = guard
        .as_any()
        .downcast_ref::<crate::world::objects::Player>()
    {
        return Some(player.actor.container.object.cell);
    }
    None
}

/// Handle UpdateContext — cell context change.
pub fn handle_update_context(
    registry: &Arc<ObjectRegistry>,
    session: &mut Session,
    id: NetworkID,
    cells: [u32; 9],
    spawn: bool,
) -> Vec<Packet> {
    // Only the session's own player may move its cell context.
    if session.player_id != Some(id) {
        return Vec::new();
    }
    let old_ctx = CellContext {
        cells: session.cell_context,
    };
    let new_ctx = CellContext { cells };

    session.update_cell_context(cells);

    if let Some(arc) = registry.get(id) {
        let mut guard = arc.write();
        if let Some(player) = guard.as_any_mut().downcast_mut::<Player>() {
            player.actor.container.object.cell = cells[4];
        }
    }

    let (enter, leave) = old_ctx.diff(&new_ctx);
    let mut packets = Vec::new();

    // Send New packets for entities in entered cells (any kind — object,
    // actor, container, item, player). Self is skipped (PlayerNew is
    // delivered by the auth flow).
    for cell in &enter {
        for obj_id in registry.get_by_cell(*cell) {
            if obj_id == id {
                continue;
            }
            if let Some(pkt) = entity_new_packet(registry, obj_id) {
                packets.push(pkt);
            }
        }
    }

    // Send ObjectRemove for entities exclusive to left cells
    let new_cells: std::collections::HashSet<u32> = cells.iter().copied().collect();
    for cell in &leave {
        for obj_id in registry.get_by_cell(*cell) {
            // Check if the entity is in any remaining context cell
            let still_visible = entity_cell(registry, obj_id)
                .map(|c| new_cells.contains(&c))
                .unwrap_or(false);
            if !still_visible {
                packets.push(Packet::ObjectRemove {
                    id: obj_id,
                    silent: true,
                });
            }
        }
    }

    packets.push(Packet::UpdateContext { id, cells, spawn });
    packets
}

/// Handle console toggle.
pub fn handle_console(
    registry: &Arc<ObjectRegistry>,
    id: NetworkID,
    enabled: bool,
) -> Option<Packet> {
    if let Some(arc) = registry.get(id) {
        let mut guard = arc.write();
        if let Some(player) = guard.as_any_mut().downcast_mut::<Player>() {
            player.console_enabled = enabled;
        }
    }
    Some(Packet::UpdateConsole { id, enabled })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::Session;
    use crate::world::objects::{Actor, Item, Object};
    use crate::world::registry::ObjectRegistry;
    use std::net::SocketAddr;

    fn session_for(id: NetworkID) -> Session {
        let mut s = Session::new(
            NetworkID::new(1),
            "127.0.0.1:9000".parse::<SocketAddr>().unwrap(),
            "Wanderer".into(),
        );
        s.player_id = Some(id);
        s
    }

    #[test]
    fn update_control_owner_only() {
        let registry = Arc::new(ObjectRegistry::new());
        let pid = registry.insert(Player::new(NetworkID::new(2), 0x100, 0x7, 0x1));
        let session = session_for(pid);
        // own player → accepted, control recorded
        assert!(handle_update_control(&registry, &session, pid, 0x30, 1).is_some());
        let arc = registry.get(pid).unwrap();
        let guard = arc.read();
        let player = guard.as_any().downcast_ref::<Player>().unwrap();
        assert!(player.controls.contains_key(&0x30));
        // other id → rejected
        assert!(handle_update_control(&registry, &session, NetworkID::new(99), 0x30, 1).is_none());
    }

    #[test]
    fn entity_new_packet_dispatches_all_kinds() {
        let registry = Arc::new(ObjectRegistry::new());
        let oid = registry.insert(Object::new(NetworkID::new(1), 0x100, 0x7, 0x5));
        let iid = registry.insert(Item::new(
            NetworkID::new(2),
            0x200,
            0x201,
            NetworkID::new(1),
        ));
        let aid = registry.insert(Actor::new(NetworkID::new(3), 0x300, 0x7, 0x5));
        let pid = registry.insert(Player::new(NetworkID::new(4), 0x400, 0x7, 0x5));
        assert!(matches!(
            entity_new_packet(&registry, oid),
            Some(Packet::ObjectNew { .. })
        ));
        assert!(matches!(
            entity_new_packet(&registry, iid),
            Some(Packet::ItemNew { .. })
        ));
        assert!(matches!(
            entity_new_packet(&registry, aid),
            Some(Packet::ActorNew { .. })
        ));
        assert!(matches!(
            entity_new_packet(&registry, pid),
            Some(Packet::PlayerNew { .. })
        ));
        assert!(entity_new_packet(&registry, NetworkID::new(999)).is_none());
    }

    #[test]
    fn entity_cell_dispatches_kinds() {
        let registry = Arc::new(ObjectRegistry::new());
        let oid = registry.insert(Object::new(NetworkID::new(1), 0x100, 0x7, 0x5));
        let aid = registry.insert(Actor::new(NetworkID::new(3), 0x300, 0x7, 0x9));
        let pid = registry.insert(Player::new(NetworkID::new(4), 0x400, 0x7, 0x11));
        assert_eq!(entity_cell(&registry, oid), Some(0x5));
        assert_eq!(entity_cell(&registry, aid), Some(0x9));
        assert_eq!(entity_cell(&registry, pid), Some(0x11));
        assert_eq!(entity_cell(&registry, NetworkID::new(999)), None);
    }
}
