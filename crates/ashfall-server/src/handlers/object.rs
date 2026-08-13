//! Object handler — create/update/remove objects, position/angle sync.

use ashfall_core::id::NetworkID;
use ashfall_core::math::is_valid_angle3;
use ashfall_core::protocol::Packet;
use ashfall_core::string_cache::CachedString;
use crate::anti_cheat::AntiCheat;
use crate::session::Session;
use crate::world::objects::{Actor, Container, Object, Player};
use crate::world::registry::ObjectRegistry;
use std::sync::Arc;
use std::time::Duration;

/// A client may mutate entities it owns: its own player object, or actors
/// the server granted it simulation ownership of (STR OwnershipTransfer).
fn owned(registry: &Arc<ObjectRegistry>, session: &Session, id: NetworkID) -> bool {
    if session.player_id == Some(id) {
        return true;
    }
    registry.owner_of(id) == session.player_id
}

/// Read the position of any position-bearing entity (Object, Actor, Player).
fn read_pos(registry: &Arc<ObjectRegistry>, id: NetworkID) -> Option<[f32; 3]> {
    let arc = registry.get(id)?;
    let guard = arc.read();
    if let Some(obj) = guard.as_any().downcast_ref::<Object>() {
        return Some(obj.net_pos);
    }
    if let Some(actor) = guard.as_any().downcast_ref::<Actor>() {
        return Some(actor.container.object.net_pos);
    }
    if let Some(player) = guard.as_any().downcast_ref::<Player>() {
        return Some(player.actor.container.object.net_pos);
    }
    None
}

/// Apply a closure to the position-bearing container of an entity.
fn with_entity_mut(
    registry: &Arc<ObjectRegistry>,
    id: NetworkID,
    f: impl FnOnce(&mut Object),
) -> bool {
    let Some(arc) = registry.get(id) else { return false };
    let mut guard = arc.write();
    if let Some(obj) = guard.as_any_mut().downcast_mut::<Object>() {
        f(obj);
        return true;
    }
    if let Some(actor) = guard.as_any_mut().downcast_mut::<Actor>() {
        f(&mut actor.container.object);
        return true;
    }
    if let Some(player) = guard.as_any_mut().downcast_mut::<Player>() {
        f(&mut player.actor.container.object);
        return true;
    }
    false
}

/// Handle UpdatePos — validate and update position.
/// Ownership: only the session's own player may be moved via client packets;
/// world/NPC objects are server-authoritative (scripts, bridge commands).
pub fn handle_update_pos(
    registry: &Arc<ObjectRegistry>,
    session: &Session,
    id: NetworkID,
    pos: [f32; 3],
) -> Option<Packet> {
    if !owned(registry, session, id) {
        tracing::warn!("Rejected UpdatePos for {id} from {} (not owner)", session.player_name);
        return None;
    }
    let Some(prev) = read_pos(registry, id) else { return None };
    let delta = session.last_recv.elapsed().min(Duration::from_secs(1));

    // Anti-cheat: validate position with speed + teleport check
    if !AntiCheat::validate_position(pos, Some(prev), delta) {
        tracing::warn!("AntiCheat: position rejected from {}", session.player_name);
        return None;
    }

    with_entity_mut(registry, id, |obj| {
        obj.net_pos = pos;
        obj.game_pos = pos;
    });

    Some(Packet::UpdatePos { id, pos })
}

/// Handle UpdateAngle.
/// Ownership: same rule as position — own player only.
pub fn handle_update_angle(
    registry: &Arc<ObjectRegistry>,
    session: &Session,
    id: NetworkID,
    angle: [f32; 2],
) -> Option<Packet> {
    if !owned(registry, session, id) {
        return None;
    }
    let angle3 = [angle[0], 0.0, angle[1]];
    if !is_valid_angle3(angle3) {
        return None;
    }
    with_entity_mut(registry, id, |obj| {
        obj.angle = angle3;
    });

    Some(Packet::UpdateAngle { id, angle })
}

/// Handle UpdateCell — move object between cells.
/// Handle UpdateCell — move object between cells.
/// Ownership: own player only (cell changes are server-authoritative for
/// world/NPC objects).
pub fn handle_update_cell(
    registry: &Arc<ObjectRegistry>,
    session: &Session,
    id: NetworkID,
    cell: u32,
    pos: [f32; 3],
) -> Option<Packet> {
    if !owned(registry, session, id) {
        return None;
    }
    with_entity_mut(registry, id, |obj| {
        obj.cell = cell;
        obj.net_pos = pos;
    });
    registry.add_to_cell(cell, id);

    Some(Packet::UpdateCell { id, cell, pos })
}

/// Handle UpdateName.
/// Ownership: only the session's own player may be renamed by the client.
pub fn handle_update_name(
    registry: &Arc<ObjectRegistry>,
    session: &Session,
    id: NetworkID,
    name: CachedString,
) -> Option<Packet> {
    if !owned(registry, session, id) {
        return None;
    }
    // Client sends Plain; Id/Inline are meaningless inbound.
    let CachedString::Plain(name) = name else { return None };
    with_entity_mut(registry, id, |obj| {
        obj.name = name.clone();
    });
    Some(Packet::UpdateName { id, name: name.into() })
}

/// Handle UpdateScale.
pub fn handle_update_scale(
    registry: &Arc<ObjectRegistry>,
    session: &Session,
    id: NetworkID,
    scale: f32,
) -> Option<Packet> {
    if !owned(registry, session, id) {
        return None;
    }
    if !AntiCheat::validate_scale(scale) {
        tracing::warn!("AntiCheat: scale rejected — {scale}");
        return None;
    }
    with_entity_mut(registry, id, |obj| {
        obj.scale = scale;
    });
    Some(Packet::UpdateScale { id, scale })
}

// ═══════════════════════════════════════════════════════════════
// Create / Remove handlers
// ═══════════════════════════════════════════════════════════════

/// Handle ObjectNew — create object, insert into registry, broadcast.
pub fn handle_object_new(
    registry: &Arc<ObjectRegistry>,
    packet: &Packet,
) -> Option<Packet> {
    let (id, ref_id, base_id, name, game_pos, net_pos, angle, scale, cell, enabled, lock, owner) =
        match packet {
            Packet::ObjectNew { id, ref_id, base_id, name, game_pos, net_pos, angle, scale, cell, enabled, lock, owner } => {
                (*id, *ref_id, *base_id, name.clone(), *game_pos, *net_pos, *angle, *scale, *cell, *enabled, *lock, *owner)
            }
            _ => return None,
        };

    if registry.is_deleted(id) {
        return None;
    }

    let mut obj = Object::new(id, ref_id, base_id, cell);
    // CachedString → stored string: Plain/Inline carry bytes, Id is unusable
    // from a client (the server is the only id assigner) — treat as unnamed.
    obj.name = match name {
        ashfall_core::string_cache::CachedString::Plain(s)
        | ashfall_core::string_cache::CachedString::Inline { value: s, .. } => s,
        ashfall_core::string_cache::CachedString::Id(_) => String::new(),
    };
    obj.game_pos = game_pos;
    obj.net_pos = net_pos;
    obj.angle = angle;
    obj.scale = scale;
    obj.enabled = enabled;
    obj.lock_level = lock;
    obj.owner = owner;

    registry.insert(obj);
    registry.add_to_cell(cell, id);

    Some(packet.clone())
}

/// Handle ContainerNew — create container, insert into registry, broadcast.
pub fn handle_container_new(
    registry: &Arc<ObjectRegistry>,
    packet: &Packet,
) -> Option<Packet> {
    let (id, ref_id, base_id) = match packet {
        Packet::ContainerNew { id, ref_id, base_id } => (*id, *ref_id, *base_id),
        _ => return None,
    };

    if registry.is_deleted(id) {
        return None;
    }

    let container = Container::new(id, ref_id, base_id, 0);
    registry.insert(container);

    Some(packet.clone())
}

/// Handle ObjectRemove — remove object from registry.
pub fn handle_object_remove(
    registry: &Arc<ObjectRegistry>,
    id: NetworkID,
    silent: bool,
) -> Option<Packet> {
    registry.remove(id);
    if silent {
        None
    } else {
        Some(Packet::ObjectRemove { id, silent: false })
    }
}
