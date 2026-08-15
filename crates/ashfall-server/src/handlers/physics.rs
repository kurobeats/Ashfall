//! Physics handler — validate and relay velocity updates.

use crate::anti_cheat::AntiCheat;
use crate::session::Session;
use crate::world::objects::{Actor, Object, Player};
use crate::world::registry::ObjectRegistry;
use ashfall_core::id::NetworkID;
use ashfall_core::protocol::Packet;
use std::sync::Arc;

/// Handle UpdateVelocity — validate and relay.
/// Ownership: only the session's own player may set velocity via client
/// packets (world/NPC physics is server-authoritative).
pub fn handle_update_velocity(
    registry: &Arc<ObjectRegistry>,
    session: &Session,
    id: NetworkID,
    vel: [f32; 3],
    on_ground: bool,
) -> Option<Packet> {
    if session.player_id != Some(id) {
        tracing::warn!(
            "Rejected UpdateVelocity for {id} from {} (not owner)",
            session.player_name
        );
        return None;
    }
    if !AntiCheat::validate_velocity(vel) {
        tracing::warn!(
            "AntiCheat: velocity rejected from {} ({vel:?})",
            session.player_name
        );
        return None;
    }

    if let Some(arc) = registry.get(id) {
        let mut guard = arc.write();
        if let Some(obj) = guard.as_any_mut().downcast_mut::<Object>() {
            obj.velocity = vel;
            obj.on_ground = on_ground;
        } else if let Some(actor) = guard.as_any_mut().downcast_mut::<Actor>() {
            actor.container.object.velocity = vel;
            actor.container.object.on_ground = on_ground;
        } else if let Some(player) = guard.as_any_mut().downcast_mut::<Player>() {
            player.actor.container.object.velocity = vel;
            player.actor.container.object.on_ground = on_ground;
        }
    }

    Some(Packet::UpdateVelocity { id, vel, on_ground })
}
