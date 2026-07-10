//! Actor handler — state/value/race/death sync.

use ashfall_core::id::NetworkID;
use ashfall_core::protocol::Packet;
use crate::world::objects::Actor;
use crate::world::registry::ObjectRegistry;
use std::sync::Arc;

/// Handle ActorNew.
pub fn handle_actor_new(registry: &Arc<ObjectRegistry>, packet: &Packet) -> Option<Packet> {
    if let Packet::ActorNew { id, ref_id, base_id, values, base_values, race, age, idle, moving, moving_xy, weapon, female, alerted, sneaking, dead, death_limbs, death_cause, scale } = packet {
        if registry.is_deleted(*id) { return None; }
        let mut actor = Actor::new(*id, *ref_id, *base_id, 0);
        actor.values = values.clone();
        actor.base_values = base_values.clone();
        actor.race = *race;
        actor.age = *age;
        actor.idle_anim = *idle;
        actor.moving_anim = *moving;
        actor.moving_xy = *moving_xy;
        actor.weapon_anim = *weapon;
        actor.female = *female;
        actor.alerted = *alerted;
        actor.sneaking = *sneaking;
        actor.dead = *dead;
        actor.death_limbs = *death_limbs;
        actor.death_cause = *death_cause;
        actor.object.scale = *scale;
        registry.insert(actor);
        Some(packet.clone())
    } else { None }
}

/// Handle UpdateActorState.
pub fn handle_actor_state(registry: &Arc<ObjectRegistry>, id: NetworkID, idle: u32, moving: u8, moving_xy: u8, weapon: u8, alerted: bool, sneaking: bool, firing: bool) -> Option<Packet> {
    if let Some(arc) = registry.get(id) {
        let mut guard = arc.write();
        if let Some(actor) = guard.as_any_mut().downcast_mut::<Actor>() {
            actor.idle_anim = idle;
            actor.moving_anim = moving;
            actor.moving_xy = moving_xy;
            actor.weapon_anim = weapon;
            actor.alerted = alerted;
            actor.sneaking = sneaking;
        }
    }
    Some(Packet::UpdateActorState { id, idle, moving, moving_xy, weapon, alerted, sneaking, firing })
}

/// Handle UpdateActorValue.
pub fn handle_actor_value(registry: &Arc<ObjectRegistry>, id: NetworkID, base: bool, index: u8, value: f32) -> Option<Packet> {
    if let Some(arc) = registry.get(id) {
        let mut guard = arc.write();
        if let Some(actor) = guard.as_any_mut().downcast_mut::<Actor>() {
            actor.set_value(index, value, base);
        }
    }
    Some(Packet::UpdateActorValue { id, base, index, value })
}

/// Handle UpdateActorDead — mark actor as dead.
pub fn handle_actor_dead(registry: &Arc<ObjectRegistry>, id: NetworkID, dead: bool, limbs: u16, cause: i8) -> Option<Packet> {
    if let Some(arc) = registry.get(id) {
        let mut guard = arc.write();
        if let Some(actor) = guard.as_any_mut().downcast_mut::<Actor>() {
            actor.dead = dead;
            actor.death_limbs = limbs;
            actor.death_cause = cause;
        }
    }
    Some(Packet::UpdateActorDead { id, dead, limbs, cause })
}

/// Handle UpdateFireWeapon.
pub fn handle_fire_weapon(_registry: &Arc<ObjectRegistry>, id: NetworkID, weapon: u32) -> Option<Packet> {
    Some(Packet::UpdateFireWeapon { id, weapon })
}
