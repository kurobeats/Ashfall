//! Object handler — create/update/remove objects, position/angle sync.

use ashfall_core::id::NetworkID;
use ashfall_core::math::is_valid_angle3;
use ashfall_core::protocol::Packet;
use crate::anti_cheat::AntiCheat;
use crate::physics::PhysicsValidator;
use crate::session::Session;
use crate::world::objects::{Container, Object};
use crate::world::registry::ObjectRegistry;
use std::sync::Arc;
use std::time::Duration;

// ponytail: generous delta time for position validation.
// Real time deltas come from session last_recv.
const DEFAULT_DELTA: Duration = Duration::from_millis(33);

/// Handle UpdatePos — validate and update position.
pub fn handle_update_pos(
    registry: &Arc<ObjectRegistry>,
    session: &Session,
    id: NetworkID,
    pos: [f32; 3],
) -> Option<Packet> {
    if let Some(arc) = registry.get(id) {
        let guard = arc.read();
        if let Some(obj) = guard.as_any().downcast_ref::<Object>() {
            let prev = obj.net_pos;
            let delta = session.last_recv.elapsed().min(Duration::from_secs(1));
            drop(guard);

            // Anti-cheat: validate position with speed + teleport check
            if !AntiCheat::validate_position(pos, Some(prev), delta) {
                tracing::warn!("AntiCheat: position rejected from {}", session.player_name);
                return None;
            }

            let mut guard = arc.write();
            if let Some(obj) = guard.as_any_mut().downcast_mut::<Object>() {
                obj.net_pos = pos;
                obj.game_pos = pos;
            }
        }
    }

    Some(Packet::UpdatePos { id, pos })
}

/// Handle UpdateAngle.
pub fn handle_update_angle(
    registry: &Arc<ObjectRegistry>,
    id: NetworkID,
    angle: [f32; 2],
) -> Option<Packet> {
    let angle3 = [angle[0], 0.0, angle[1]];
    if !is_valid_angle3(angle3) {
        return None;
    }

    if let Some(arc) = registry.get(id) {
        let mut guard = arc.write();
        if let Some(obj) = guard.as_any_mut().downcast_mut::<Object>() {
            obj.angle = angle3;
        }
    }

    Some(Packet::UpdateAngle { id, angle })
}

/// Handle UpdateCell — move object between cells.
pub fn handle_update_cell(
    registry: &Arc<ObjectRegistry>,
    id: NetworkID,
    cell: u32,
    pos: [f32; 3],
) -> Option<Packet> {
    if let Some(arc) = registry.get(id) {
        let mut guard = arc.write();
        if let Some(obj) = guard.as_any_mut().downcast_mut::<Object>() {
            obj.cell = cell;
            obj.net_pos = pos;
        }
        registry.add_to_cell(cell, id);
    }

    Some(Packet::UpdateCell { id, cell, pos })
}

/// Handle UpdateName.
pub fn handle_update_name(
    registry: &Arc<ObjectRegistry>,
    id: NetworkID,
    name: String,
) -> Option<Packet> {
    if let Some(arc) = registry.get(id) {
        let mut guard = arc.write();
        if let Some(obj) = guard.as_any_mut().downcast_mut::<Object>() {
            obj.name = name.clone();
        }
    }
    Some(Packet::UpdateName { id, name })
}

/// Handle UpdateScale.
pub fn handle_update_scale(
    registry: &Arc<ObjectRegistry>,
    id: NetworkID,
    scale: f32,
) -> Option<Packet> {
    if !AntiCheat::validate_scale(scale) {
        tracing::warn!("AntiCheat: scale rejected — {scale}");
        return None;
    }
    if let Some(arc) = registry.get(id) {
        let mut guard = arc.write();
        if let Some(obj) = guard.as_any_mut().downcast_mut::<Object>() {
            obj.scale = scale;
        }
    }
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
    obj.name = name;
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
