//! Combat resolver — server-authoritative hit validation + damage application.

use crate::combat::DamageFormula;
use crate::world::objects::{Actor, Player};
use crate::world::position_history::PositionHistory;
use crate::world::registry::ObjectRegistry;
use ashfall_core::id::NetworkID;
use ashfall_core::math::distance;
use ashfall_core::protocol::{self, Packet};
use std::sync::Arc;
use std::time::Instant;

/// Read-only actor access covering both Actor and Player entities
/// (Player wraps an Actor by composition, not inheritance).
fn with_actor(registry: &ObjectRegistry, id: NetworkID, f: impl FnOnce(&Actor)) -> Option<()> {
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
    pub fn resolve_hit(registry: &Arc<ObjectRegistry>, hit: &Packet) -> Option<Vec<Packet>> {
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
            Packet::ActorHit {
                target,
                attacker,
                limb,
                base_damage,
                flags,
                weapon_id,
                projectile,
            } => (
                *target,
                *attacker,
                *limb,
                *base_damage,
                *flags,
                *weapon_id,
                *projectile,
            ),
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
        with_actor(registry, target_id, |a| {
            target_pos = a.container.object.net_pos
        })?;
        let mut attacker_pos = [0.0; 3];
        with_actor(registry, attacker_id, |a| {
            attacker_pos = a.container.object.net_pos
        })?;
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
        let crit_mult = if flags & protocol::HIT_FLAG_CRITICAL != 0 {
            1.5
        } else {
            1.0
        };

        let final_damage = DamageFormula::calculate(base_damage, limb_mult, dr, dt, crit_mult);

        // Apply damage to target's health (actor value index 0x14 = health)
        let new_health = (target_health - final_damage).max(0.0);
        with_actor_mut(registry, target_id, |a| {
            a.set_value(0x14, new_health, false)
        });

        // Check for death
        let mut packets = vec![Packet::ActorDamaged {
            target: target_id,
            attacker: attacker_id,
            limb,
            final_damage,
            flags,
        }];

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::registry::ObjectRegistry;
    use ashfall_core::protocol;
    use std::sync::Arc;

    fn actor_pair() -> (Arc<ObjectRegistry>, NetworkID, NetworkID) {
        let registry = Arc::new(ObjectRegistry::new());
        let a = registry.insert(Actor::new(NetworkID::new(1), 0x100, 0x7, 0x1));
        let t = registry.insert(Actor::new(NetworkID::new(2), 0x200, 0x7, 0x1));
        // both at the same position → in range
        for id in [a, t] {
            let arc = registry.get(id).unwrap();
            let mut guard = arc.write();
            let actor = guard.as_any_mut().downcast_mut::<Actor>().unwrap();
            actor.container.object.net_pos = [0.0, 0.0, 0.0];
            actor.set_value(0x14, 100.0, false); // full health
        }
        (registry, a, t)
    }

    fn hit(attacker: NetworkID, target: NetworkID, dmg: f32, limb: u8, flags: u8) -> Packet {
        Packet::ActorHit {
            target,
            attacker,
            limb,
            base_damage: dmg,
            flags,
            weapon_id: 0x1000,
            projectile: 0,
        }
    }

    #[test]
    fn hit_lands_damage_and_death() {
        let (registry, a, t) = actor_pair();
        let packets = CombatResolver::resolve_hit(&registry, &hit(a, t, 100.0, 0, 0))
            .expect("hit resolves");
        // 100 dmg, no DR/DT → target dead
        assert!(packets.iter().any(|p| matches!(p, Packet::ActorDamaged { .. })));
        assert!(packets.iter().any(|p| matches!(p, Packet::ActorDeathExt { .. })));
        let arc = registry.get(t).unwrap();
        let guard = arc.read();
        let actor = guard.as_any().downcast_ref::<Actor>().unwrap();
        assert!(actor.dead);
        assert_eq!(actor.get_value(0x14), 0.0);
    }

    #[test]
    fn non_lethal_hit_keeps_target_alive() {
        let (registry, a, t) = actor_pair();
        let packets = CombatResolver::resolve_hit(&registry, &hit(a, t, 30.0, 0, 0))
            .expect("hit resolves");
        assert!(packets.iter().any(|p| matches!(p, Packet::ActorDamaged { .. })));
        assert!(!packets.iter().any(|p| matches!(p, Packet::ActorDeathExt { .. })));
        let arc = registry.get(t).unwrap();
        let guard = arc.read();
        let actor = guard.as_any().downcast_ref::<Actor>().unwrap();
        assert!(!actor.dead);
        assert!((actor.get_value(0x14) - 70.0).abs() < 1e-3);
    }

    #[test]
    fn dead_target_rejected() {
        let (registry, a, t) = actor_pair();
        let arc = registry.get(t).unwrap();
        let mut guard = arc.write();
        let actor = guard.as_any_mut().downcast_mut::<Actor>().unwrap();
        actor.dead = true;
        drop(guard);
        assert!(CombatResolver::resolve_hit(&registry, &hit(a, t, 10.0, 0, 0)).is_none());
    }

    #[test]
    fn out_of_range_rejected() {
        let (registry, a, t) = actor_pair();
        // move the attacker 6000 units away
        let arc = registry.get(a).unwrap();
        let mut guard = arc.write();
        let actor = guard.as_any_mut().downcast_mut::<Actor>().unwrap();
        actor.container.object.net_pos = [6000.0, 0.0, 0.0];
        drop(guard);
        assert!(CombatResolver::resolve_hit(&registry, &hit(a, t, 10.0, 0, 0)).is_none());
    }

    #[test]
    fn missing_attacker_rejected() {
        let (registry, _, t) = actor_pair();
        let ghost = NetworkID::new(99); // not in the registry
        assert!(CombatResolver::resolve_hit(&registry, &hit(ghost, t, 10.0, 0, 0)).is_none());
    }

    #[test]
    fn critical_flag_scales_damage() {
        let (registry, a, t) = actor_pair();
        let packets = CombatResolver::resolve_hit(&registry, &hit(a, t, 100.0, 0, protocol::HIT_FLAG_CRITICAL))
            .expect("hit resolves");
        // 100 * 1.5 crit = 150 → death regardless; check the damaged value
        if let Some(Packet::ActorDamaged { final_damage, .. }) =
            packets.iter().find(|p| matches!(p, Packet::ActorDamaged { .. }))
        {
            assert!((*final_damage - 150.0).abs() < 1e-3);
        } else {
            panic!("no ActorDamaged packet");
        }
    }
}
