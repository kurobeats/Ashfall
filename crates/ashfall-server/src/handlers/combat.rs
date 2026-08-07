//! Combat handler — routes ActorHit to combat resolver.

use ashfall_core::id::NetworkID;
use ashfall_core::protocol::Packet;
use crate::combat::resolver::CombatResolver;
use crate::session::Session;
use crate::world::registry::ObjectRegistry;
use std::sync::Arc;

/// Handle an ActorHit packet. Validates and resolves damage.
/// Ownership: a client may only report hits it dealt itself — the packet's
/// attacker id must be the session's own player (prevents framing others).
pub fn handle_actor_hit(
    registry: &Arc<ObjectRegistry>,
    session: &Session,
    hit: &Packet,
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
