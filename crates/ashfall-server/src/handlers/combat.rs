//! Combat handler — routes ActorHit to combat resolver.

use ashfall_core::protocol::Packet;
use crate::combat::resolver::CombatResolver;
use crate::world::registry::ObjectRegistry;
use std::sync::Arc;

/// Handle an ActorHit packet. Validates and resolves damage.
pub fn handle_actor_hit(
    registry: &Arc<ObjectRegistry>,
    hit: &Packet,
) -> Option<Vec<Packet>> {
    // Validate base damage bounds
    if let Packet::ActorHit { base_damage, .. } = hit {
        if !CombatResolver::validate_hit_bounds(*base_damage) {
            tracing::warn!("Combat: hit rejected — invalid base_damage={base_damage}");
            return None;
        }
    }

    CombatResolver::resolve_hit(registry, hit)
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
