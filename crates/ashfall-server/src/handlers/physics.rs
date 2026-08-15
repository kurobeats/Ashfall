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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::Session;
    use crate::world::objects::Player;
    use crate::world::registry::ObjectRegistry;
    use std::net::SocketAddr;

    fn session_with(id: Option<NetworkID>) -> Session {
        let mut s = Session::new(
            NetworkID::new(1),
            "127.0.0.1:9000".parse::<SocketAddr>().unwrap(),
            "Wanderer".into(),
        );
        s.player_id = id;
        s
    }

    #[test]
    fn owner_can_update_velocity() {
        let registry = Arc::new(ObjectRegistry::new());
        let id = registry.insert(Player::new(NetworkID::new(2), 0x100, 0x7, 0x1));
        let session = session_with(Some(id));
        let result = handle_update_velocity(&registry, &session, id, [1.0, 2.0, 3.0], true);
        assert!(result.is_some());
        // velocity landed on the object
        let obj = registry.get(id).unwrap();
        let guard = obj.read();
        let player = guard.as_any().downcast_ref::<Player>().unwrap();
        assert_eq!(player.actor.container.object.velocity, [1.0, 2.0, 3.0]);
        assert!(player.actor.container.object.on_ground);
    }

    #[test]
    fn non_owner_rejected() {
        let registry = Arc::new(ObjectRegistry::new());
        let id = registry.insert(Player::new(NetworkID::new(2), 0x100, 0x7, 0x1));
        // session owns a DIFFERENT player
        let session = session_with(Some(NetworkID::new(99)));
        assert!(handle_update_velocity(&registry, &session, id, [1.0, 0.0, 0.0], true).is_none());
    }

    #[test]
    fn invalid_velocity_rejected() {
        let registry = Arc::new(ObjectRegistry::new());
        let id = registry.insert(Player::new(NetworkID::new(2), 0x100, 0x7, 0x1));
        let session = session_with(Some(id));
        // absurd speed (1e9 u/s) fails AntiCheat::validate_velocity
        assert!(handle_update_velocity(&registry, &session, id, [1e9, 0.0, 0.0], true).is_none());
    }
}
