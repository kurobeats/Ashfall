//! Physics handler — validate and relay velocity updates.

use ashfall_core::id::NetworkID;
use ashfall_core::protocol::Packet;
use crate::physics::PhysicsValidator;
use crate::session::Session;
use crate::world::objects::Object;
use crate::world::registry::ObjectRegistry;
use std::sync::Arc;

/// Handle UpdateVelocity — validate and relay.
pub fn handle_update_velocity(
    registry: &Arc<ObjectRegistry>,
    session: &Session,
    id: NetworkID,
    vel: [f32; 3],
    on_ground: bool,
) -> Option<Packet> {
    if !PhysicsValidator::validate_velocity(vel) {
        tracing::warn!("Physics: velocity rejected from {} ({vel:?})", session.player_name);
        return None;
    }

    if let Some(arc) = registry.get(id) {
        let mut guard = arc.write();
        if let Some(obj) = guard.as_any_mut().downcast_mut::<Object>() {
            obj.velocity = vel;
            obj.on_ground = on_ground;
        }
    }

    Some(Packet::UpdateVelocity { id, vel, on_ground })
}
