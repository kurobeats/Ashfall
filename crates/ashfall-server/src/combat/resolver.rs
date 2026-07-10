//! Combat resolver — server-authoritative hit validation + damage application.

use ashfall_core::math::distance;
use ashfall_core::protocol::{self, Packet};
use crate::combat::DamageFormula;
use crate::world::objects::Actor;
use crate::world::registry::ObjectRegistry;
use std::sync::Arc;

/// Server-side combat resolution.
pub struct CombatResolver;

impl CombatResolver {
    /// Process an ActorHit from a client. Validate, calculate damage, apply.
    /// Returns packets to broadcast.
    pub fn resolve_hit(
        registry: &Arc<ObjectRegistry>,
        hit: &Packet,
    ) -> Option<Vec<Packet>> {
        let (target_id, attacker_id, limb, base_damage, flags, weapon_id, projectile) = match hit {
            Packet::ActorHit { target, attacker, limb, base_damage, flags, weapon_id, projectile } => {
                (*target, *attacker, *limb, *base_damage, *flags, *weapon_id, *projectile)
            }
            _ => return None,
        };

        // Validate target exists and is alive
        let target_actor = registry.get_typed::<Actor>(target_id)?;
        if target_actor.dead {
            return None;
        }

        // Validate attacker exists
        let attacker_actor = registry.get_typed::<Actor>(attacker_id)?;

        // Validate distance (anti-teleport-hack)
        let target_arc = registry.get(target_id)?;
        let target_pos = {
            let guard = target_arc.read();
            guard.as_any().downcast_ref::<Actor>()?.object.net_pos
        };
        let attacker_arc = registry.get(attacker_id)?;
        let attacker_pos = {
            let guard = attacker_arc.read();
            guard.as_any().downcast_ref::<Actor>()?.object.net_pos
        };

        let dist = distance(target_pos, attacker_pos);
        let max_range = 5000.0; // ponytail: generous max weapon range
        if dist > max_range {
            tracing::warn!("Combat: hit rejected — distance {dist} exceeds max range");
            return None;
        }

        // Calculate damage
        let limb_mult = DamageFormula::limb_multiplier(limb);
        let dr = Self::get_actor_dr(&target_actor);
        let dt = Self::get_actor_dt(&target_actor); // 0 for FO3
        let crit_mult = if flags & protocol::HIT_FLAG_CRITICAL != 0 { 1.5 } else { 1.0 };

        let final_damage = DamageFormula::calculate(base_damage, limb_mult, dr, dt, crit_mult);

        // Apply damage to target's health (actor value index 0x14 = health)
        let current_health = target_actor.get_value(0x14);
        let new_health = (current_health - final_damage).max(0.0);

        // Update actor value in registry
        if let Some(arc) = registry.get(target_id) {
            let mut guard = arc.write();
            if let Some(actor) = guard.as_any_mut().downcast_mut::<Actor>() {
                actor.set_value(0x14, new_health, false);
            }
        }

        // Check for death
        let mut packets = vec![
            Packet::ActorDamaged {
                target: target_id,
                attacker: attacker_id,
                limb,
                final_damage,
                flags,
            }
        ];

        if new_health <= 0.0 {
            let is_headshot = limb == 1;
            let death_flags = if is_headshot {
                protocol::DEATH_FLAG_HEADSHOT | protocol::DEATH_FLAG_DISMEMBER
            } else {
                0
            };

            // Mark actor as dead
            if let Some(arc) = registry.get(target_id) {
                let mut guard = arc.write();
                if let Some(actor) = guard.as_any_mut().downcast_mut::<Actor>() {
                    actor.dead = true;
                    actor.death_limbs = 0x1F; // all limbs damaged
                    actor.death_cause = 1; // killed by weapon
                }
            }

            packets.push(Packet::ActorDeathExt {
                id: target_id,
                killer: attacker_id,
                weapon_id,
                limbs: 0x1F,
                cause: 1,
                death_flags,
            });
        }

        tracing::debug!(
            "Combat: {attacker_id} hit {target_id} limb={limb} base={base_damage} final={final_damage} health={new_health}"
        );

        Some(packets)
    }

    /// Get damage resistance from actor values.
    fn get_actor_dr(actor: &Actor) -> f32 {
        // ponytail: sum armor DR from equipped items
        // For now, use DamageResistance actor value (0x29)
        actor.get_value(0x29).clamp(0.0, 0.85)
    }

    /// Get damage threshold (FNV only). Returns 0 for FO3.
    fn get_actor_dt(actor: &Actor) -> f32 {
        actor.get_value(0x2A).max(0.0) // DamageThreshold actor value
    }

    /// Validate that a hit is plausible (not a speed/teleport hack).
    pub fn validate_hit_bounds(base_damage: f32) -> bool {
        base_damage > 0.0 && base_damage < 10000.0 // no 10k+ damage weapons
    }
}
