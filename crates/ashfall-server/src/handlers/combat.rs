//! Combat handler — routes ActorHit to combat resolver.

use crate::combat::resolver::CombatResolver;
use crate::session::Session;
use crate::world::objects::Player;
use crate::world::registry::ObjectRegistry;
use ashfall_core::id::NetworkID;
use ashfall_core::protocol::Packet;
use std::sync::Arc;

/// Handle an ActorHit packet. Validates and resolves damage.
/// Ownership: a client may only report hits it dealt itself — the packet's
/// attacker id must be the session's own player (prevents framing others).
pub fn handle_actor_hit(
    registry: &Arc<ObjectRegistry>,
    session: &Session,
    hit: &Packet,
    pvp_enabled: bool,
) -> Option<Vec<Packet>> {
    // Validate base damage bounds
    if let Packet::ActorHit { base_damage, .. } = hit {
        if !CombatResolver::validate_hit_bounds(*base_damage) {
            tracing::warn!("Combat: hit rejected — invalid base_damage={base_damage}");
            return None;
        }
    }

    // Ownership: attacker must be the session's own player
    let attacker_id: Option<NetworkID> = match hit {
        Packet::ActorHit { attacker, .. } => Some(*attacker),
        _ => None,
    };
    if attacker_id != session.player_id {
        tracing::warn!(
            "Combat: hit rejected from {} — attacker {} is not own player",
            session.player_name,
            attacker_id.map_or(0, |n| n.as_u64())
        );
        return None;
    }

    // PvP rule: when disabled, a player may not damage another player.
    let is_player = |id: NetworkID| -> bool {
        registry
            .get(id)
            .map(|arc| {
                let guard = arc.read();
                guard.as_any().downcast_ref::<Player>().is_some()
            })
            .unwrap_or(false)
    };
    if !pvp_enabled {
        if let Packet::ActorHit {
            target, attacker, ..
        } = hit
        {
            if is_player(*target) && is_player(*attacker) {
                tracing::warn!(
                    "Combat: hit rejected — PvP disabled ({} -> {})",
                    attacker,
                    target
                );
                return None;
            }
        }
    }

    CombatResolver::resolve_hit(registry, hit)
}

/// Handle an ActorPunch packet (unarmed swing notification).
///
/// Ownership: a client may only report its own punch (same rule as
/// ActorHit — prevents framing others). No damage is resolved here;
/// melee damage still flows through ActorHit.
pub fn handle_actor_punch(session: &Session, id: NetworkID, power: bool) -> Option<Packet> {
    if Some(id) != session.player_id {
        tracing::warn!(
            "Combat: punch rejected from {} — actor {} is not own player",
            session.player_name,
            id.as_u64()
        );
        return None;
    }
    Some(Packet::ActorPunch { id, power })
}

/// Handle projectile/explosion — relay to all clients.
pub fn handle_projectile_new(packet: &Packet) -> Option<Packet> {
    Some(packet.clone())
}

pub fn handle_projectile_remove(packet: &Packet) -> Option<Packet> {
    Some(packet.clone())
}

pub fn handle_explosion_new(packet: &Packet) -> Option<Packet> {
    Some(packet.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::Session;
    use crate::world::objects::Player;
    use crate::world::registry::ObjectRegistry;
    use std::net::SocketAddr;

    fn hit(attacker: NetworkID, target: NetworkID, dmg: f32) -> Packet {
        Packet::ActorHit {
            target,
            attacker,
            limb: 0,
            base_damage: dmg,
            flags: 0,
            weapon_id: 0x1000,
            projectile: 0,
        }
    }

    fn setup() -> (Arc<ObjectRegistry>, Session, NetworkID, NetworkID) {
        let registry = Arc::new(ObjectRegistry::new());
        let a = registry.insert(Player::new(NetworkID::new(1), 0x100, 0x7, 0x1));
        let t = registry.insert(Player::new(NetworkID::new(2), 0x200, 0x7, 0x1));
        for id in [a, t] {
            let arc = registry.get(id).unwrap();
            let mut guard = arc.write();
            let p = guard.as_any_mut().downcast_mut::<Player>().unwrap();
            p.actor.container.object.net_pos = [0.0; 3];
            p.actor.set_value(0x14, 100.0, false);
        }
        let mut session = Session::new(
            NetworkID::new(1),
            "127.0.0.1:9000".parse::<SocketAddr>().unwrap(),
            "Alice".into(),
        );
        session.player_id = Some(a);
        (registry, session, a, t)
    }

    #[test]
    fn pvp_off_blocks_player_vs_player() {
        let (registry, session, a, t) = setup();
        let result = handle_actor_hit(&registry, &session, &hit(a, t, 10.0), false);
        assert!(result.is_none(), "PvP off must block player damage");
    }

    #[test]
    fn pvp_on_allows_player_vs_player() {
        let (registry, session, a, t) = setup();
        let result = handle_actor_hit(&registry, &session, &hit(a, t, 10.0), true);
        assert!(result.is_some(), "PvP on must allow player damage");
    }

    #[test]
    fn non_owner_attacker_rejected() {
        let (registry, session, _, t) = setup();
        // attacker id not owned by the session → reject even with PvP on
        let rogue = NetworkID::new(99);
        assert!(handle_actor_hit(&registry, &session, &hit(rogue, t, 10.0), true).is_none());
    }

    #[test]
    fn invalid_base_damage_rejected() {
        let (registry, session, a, t) = setup();
        assert!(handle_actor_hit(&registry, &session, &hit(a, t, 0.0), true).is_none());
        assert!(handle_actor_hit(&registry, &session, &hit(a, t, 50_000.0), true).is_none());
    }
}
