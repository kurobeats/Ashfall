//! Combat resolver — server-authoritative hit validation + damage application.

use ashfall_core::id::NetworkID;
use ashfall_core::math::distance;
use ashfall_core::protocol::{self, Packet};
use crate::combat::DamageFormula;
use crate::world::objects::{Actor, Player};
use crate::world::position_history::PositionHistory;
use crate::world::registry::ObjectRegistry;
use std::sync::Arc;
use std::time::Instant;

/// Read-only actor access covering both Actor and Player entities
/// (Player wraps an Actor by composition, not inheritance).
fn with_actor(
    registry: &ObjectRegistry,
    id: NetworkID,
    f: impl FnOnce(&Actor),
) -> Option<()> {
    let arc = registry.get(id)?;
    let guard = arc.read();
    if let Some(a) = guard.as_any().downcast_ref::<Actor>() {
        f(a);
        return Some(());
    }
    if let Some(p) = guard.as_any().downcast_ref::<Player>() {
        f(&p.actor);
        return Some(());
    }
    None
}

/// Mutable actor access covering both Actor and Player entities.
fn with_actor_mut(
    registry: &ObjectRegistry,
    id: NetworkID,
    f: impl FnOnce(&mut Actor),
) -> Option<()> {
    let arc = registry.get(id)?;
    let mut guard = arc.write();
    if let Some(a) = guard.as_any_mut().downcast_mut::<Actor>() {
        f(a);
        return Some(());
    }
    if let Some(p) = guard.as_any_mut().downcast_mut::<Player>() {
        f(&mut p.actor);
        return Some(());
    }
    None
}

/// Server-side combat resolution.
pub struct CombatResolver;

impl CombatResolver {
    /// Process an ActorHit from a client. Validate, calculate damage, apply.
    /// Returns packets to broadcast.
    pub fn resolve_hit(
        registry: &Arc<ObjectRegistry>,
        hit: &Packet,
    ) -> Option<Vec<Packet>> {
        Self::resolve_hit_compensated(registry, hit, &PositionHistory::new())
    }

    /// Resolve a hit with server-side lag compensation: the range check uses
    /// the attacker's position ~1 RTT before the server processed the hit
    /// (the attacker's own view), instead of its current position which is
    /// ahead by one network round-trip.
    pub fn resolve_hit_compensated(
        registry: &Arc<ObjectRegistry>,
        hit: &Packet,
        history: &PositionHistory,
    ) -> Option<Vec<Packet>> {
        let (target_id, attacker_id, limb, base_damage, flags, weapon_id, _projectile) = match hit {
            Packet::ActorHit { target, attacker, limb, base_damage, flags, weapon_id, projectile } => {
                (*target, *attacker, *limb, *base_damage, *flags, *weapon_id, *projectile)
            }
            _ => return None,
        };

        // Validate target exists and is alive (Actor or Player)
        let mut target_dead = true;
        let mut target_health = 0.0f32;
        with_actor(registry, target_id, |a| {
            target_dead = a.dead;
            target_health = a.get_value(0x14);
        })?;
        if target_dead {
            return None;
        }

        // Validate attacker exists (Actor or Player)
        let mut attacker_exists = false;
        with_actor(registry, attacker_id, |_| attacker_exists = true);
        if !attacker_exists {
            return None;
        }

        // Validate distance (anti-teleport-hack) with lag compensation:
        // use the attacker's position as of ~1 RTT ago when available.
        let mut target_pos = [0.0; 3];
        with_actor(registry, target_id, |a| target_pos = a.container.object.net_pos)?;
        let mut attacker_pos = [0.0; 3];
        with_actor(registry, attacker_id, |a| attacker_pos = a.container.object.net_pos)?;
        let compensated = history.lag_compensated(attacker_id, Instant::now());
        if let Some(comp) = compensated {
            attacker_pos = comp;
        }

        let dist = distance(target_pos, attacker_pos);
        let max_range = 5000.0; // ponytail: generous max weapon range
        if dist > max_range {
            tracing::warn!("Combat: hit rejected — distance {dist} exceeds max range");
            return None;
        }

        // Calculate damage (DR/DT from the target's stored actor values)
        let limb_mult = DamageFormula::limb_multiplier(limb);
        let mut dr = 0.0f32;
        let mut dt = 0.0f32;
        with_actor(registry, target_id, |a| {
            dr = Self::get_actor_dr(a);
            dt = Self::get_actor_dt(a); // 0 for FO3
        });
        let crit_mult = if flags & protocol::HIT_FLAG_CRITICAL != 0 { 1.5 } else { 1.0 };

        let final_damage = DamageFormula::calculate(base_damage, limb_mult, dr, dt, crit_mult);

        // Apply damage to target's health (actor value index 0x14 = health)
        let new_health = (target_health - final_damage).max(0.0);
        with_actor_mut(registry, target_id, |a| a.set_value(0x14, new_health, false));

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
            with_actor_mut(registry, target_id, |a| {
                a.dead = true;
                a.death_limbs = 0x1F; // all limbs damaged
                a.death_cause = 1; // killed by weapon
            });

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
