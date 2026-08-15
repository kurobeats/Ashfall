//! Actor handler — state/value/race/death sync.

use crate::session::Session;
use crate::world::objects::{Actor, Player};
use crate::world::registry::ObjectRegistry;
use ashfall_core::id::NetworkID;
use ashfall_core::protocol::Packet;
use std::sync::Arc;

/// Ownership rule (STR OwnershipTransfer semantics): a session may mutate an
/// actor if it is its own player object, or if the server granted it
/// simulation ownership of that actor (via ActorNew or OwnershipClaim).
fn can_mutate(registry: &Arc<ObjectRegistry>, session: &Session, id: NetworkID) -> bool {
    if session.player_id == Some(id) {
        return true;
    }
    registry.owner_of(id) == session.player_id
}

/// Handle ActorNew — register the actor, grant the sender simulation
/// ownership, broadcast to everyone else.
/// Returns (response-to-sender, broadcast).
pub fn handle_actor_new(
    registry: &Arc<ObjectRegistry>,
    session: &Session,
    packet: &Packet,
) -> (Option<Packet>, Option<Packet>) {
    let Packet::ActorNew { id, ref_id, .. } = packet else {
        return (None, None);
    };

    if registry.is_deleted(*id) {
        return (None, None);
    }

    // Dedup by ref_id: a second client reporting the same NPC must not
    // create a second actor — the first reporter owns it.
    if let Some(existing) = registry.lookup_ref(*ref_id) {
        if existing != *id {
            tracing::warn!("ActorNew rejected: ref_id {ref_id:#x} already mapped to {existing}");
            return (None, None);
        }
    }

    // Ownership: only an unowned actor may be claimed.
    if let Some(owner) = registry.owner_of(*id) {
        tracing::warn!("ActorNew rejected: {id} already owned by {owner}");
        return (None, None);
    }
    let Some(player_id) = session.player_id else {
        return (None, None);
    };
    registry.set_owner(*id, player_id);
    registry.map_ref(*ref_id, *id);

    let Packet::ActorNew {
        id,
        ref_id,
        base_id,
        values,
        base_values,
        race,
        age,
        idle,
        moving,
        moving_xy,
        weapon,
        female,
        alerted,
        sneaking,
        dead,
        death_limbs,
        death_cause,
        scale,
    } = packet
    else {
        return (None, None);
    };
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
    // The NPC lives in its owner's cell — register it there so cell-context
    // streaming (UpdateContext enter/leave) picks it up for other players.
    actor.object.cell = session.current_cell;
    registry.insert(actor);
    registry.add_to_cell(session.current_cell, *id);

    let grant = Packet::OwnershipGranted { id: *id };
    (Some(grant), Some(packet.clone()))
}

/// Handle OwnershipClaim — grant simulation ownership of an unowned actor.
pub fn handle_ownership_claim(
    registry: &Arc<ObjectRegistry>,
    session: &Session,
    id: NetworkID,
) -> Option<Packet> {
    let player_id = session.player_id?;
    registry.get(id)?;
    if !registry.set_owner(id, player_id) {
        return None; // already owned
    }
    tracing::info!("{} granted ownership of {id}", session.player_name);
    Some(Packet::OwnershipGranted { id })
}

#[allow(clippy::too_many_arguments)] // wire packet fields, relayed as-is
pub fn handle_actor_state(
    registry: &Arc<ObjectRegistry>,
    session: &crate::session::Session,
    id: NetworkID,
    idle: u32,
    moving: u8,
    moving_xy: u8,
    weapon: u8,
    alerted: bool,
    sneaking: bool,
    firing: bool,
) -> Option<Packet> {
    if !can_mutate(registry, session, id) {
        return None;
    }
    if let Some(arc) = registry.get(id) {
        let mut guard = arc.write();
        if let Some(actor) = guard.as_any_mut().downcast_mut::<Actor>() {
            actor.idle_anim = idle;
            actor.moving_anim = moving;
            actor.moving_xy = moving_xy;
            actor.weapon_anim = weapon;
            actor.alerted = alerted;
            actor.sneaking = sneaking;
        } else if let Some(player) = guard.as_any_mut().downcast_mut::<Player>() {
            player.actor.idle_anim = idle;
            player.actor.moving_anim = moving;
            player.actor.moving_xy = moving_xy;
            player.actor.weapon_anim = weapon;
            player.actor.alerted = alerted;
            player.actor.sneaking = sneaking;
        }
    }
    Some(Packet::UpdateActorState {
        id,
        idle,
        moving,
        moving_xy,
        weapon,
        alerted,
        sneaking,
        firing,
    })
}

/// Handle ActorStateDelta — apply only the present fields, relay the delta.
#[allow(clippy::too_many_arguments)] // wire packet fields, relayed as-is
pub fn handle_actor_state_delta(
    registry: &Arc<ObjectRegistry>,
    session: &crate::session::Session,
    id: NetworkID,
    idle: Option<u32>,
    moving: Option<u8>,
    moving_xy: Option<u8>,
    weapon: Option<u8>,
    alerted: Option<bool>,
    sneaking: Option<bool>,
    firing: Option<bool>,
) -> Option<Packet> {
    if !can_mutate(registry, session, id) {
        return None;
    }
    if let Some(arc) = registry.get(id) {
        let mut guard = arc.write();
        let actor = if let Some(a) = guard.as_any_mut().downcast_mut::<Actor>() {
            Some(a)
        } else if let Some(p) = guard.as_any_mut().downcast_mut::<Player>() {
            Some(&mut p.actor)
        } else {
            None
        };
        if let Some(a) = actor {
            if let Some(v) = idle {
                a.idle_anim = v;
            }
            if let Some(v) = moving {
                a.moving_anim = v;
            }
            if let Some(v) = moving_xy {
                a.moving_xy = v;
            }
            if let Some(v) = weapon {
                a.weapon_anim = v;
            }
            if let Some(v) = alerted {
                a.alerted = v;
            }
            if let Some(v) = sneaking {
                a.sneaking = v;
            }
        }
    }
    Some(Packet::ActorStateDelta {
        id,
        idle,
        moving,
        moving_xy,
        weapon,
        alerted,
        sneaking,
        firing,
    })
}

/// Handle UpdateActorValue.
pub fn handle_actor_value(
    registry: &Arc<ObjectRegistry>,
    session: &crate::session::Session,
    id: NetworkID,
    base: bool,
    index: u8,
    value: f32,
) -> Option<Packet> {
    if !can_mutate(registry, session, id) {
        return None;
    }
    if let Some(arc) = registry.get(id) {
        let mut guard = arc.write();
        if let Some(actor) = guard.as_any_mut().downcast_mut::<Actor>() {
            actor.set_value(index, value, base);
        } else if let Some(player) = guard.as_any_mut().downcast_mut::<Player>() {
            player.actor.set_value(index, value, base);
        }
    }
    Some(Packet::UpdateActorValue {
        id,
        base,
        index,
        value,
    })
}

/// Handle UpdateActorDead — mark actor as dead.
pub fn handle_actor_dead(
    registry: &Arc<ObjectRegistry>,
    session: &crate::session::Session,
    id: NetworkID,
    dead: bool,
    limbs: u16,
    cause: i8,
) -> Option<Packet> {
    if !can_mutate(registry, session, id) {
        return None;
    }
    if let Some(arc) = registry.get(id) {
        let mut guard = arc.write();
        if let Some(actor) = guard.as_any_mut().downcast_mut::<Actor>() {
            actor.dead = dead;
            actor.death_limbs = limbs;
            actor.death_cause = cause;
        } else if let Some(player) = guard.as_any_mut().downcast_mut::<Player>() {
            player.actor.dead = dead;
            player.actor.death_limbs = limbs;
            player.actor.death_cause = cause;
        }
    }
    Some(Packet::UpdateActorDead {
        id,
        dead,
        limbs,
        cause,
    })
}

/// Handle UpdateFireWeapon.
pub fn handle_fire_weapon(
    _registry: &Arc<ObjectRegistry>,
    session: &crate::session::Session,
    id: NetworkID,
    weapon: u32,
) -> Option<Packet> {
    if !can_mutate(_registry, session, id) {
        return None;
    }
    Some(Packet::UpdateFireWeapon { id, weapon })
}

/// Handle SpellCast — the caster must be the session's player or an actor it
/// simulates (STR NotifySpellCast relay).
pub fn handle_spell_cast(
    registry: &Arc<ObjectRegistry>,
    session: &crate::session::Session,
    id: NetworkID,
    spell: u32,
    source: i32,
    dual: bool,
    target: NetworkID,
) -> Option<Packet> {
    if !can_mutate(registry, session, id) {
        return None;
    }
    Some(Packet::SpellCast {
        id,
        spell,
        source,
        dual,
        target,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::Session;
    use crate::world::objects::Player;
    use crate::world::registry::ObjectRegistry;
    use std::collections::HashMap;
    use std::net::SocketAddr;

    fn session_owning(id: NetworkID) -> Session {
        let mut s = Session::new(
            NetworkID::new(1),
            "127.0.0.1:9000".parse::<SocketAddr>().unwrap(),
            "Wanderer".into(),
        );
        s.player_id = Some(id);
        s.current_cell = 0x1;
        s
    }

    fn actor_new_packet(id: u64, ref_id: u32) -> Packet {
        Packet::ActorNew {
            id: NetworkID::new(id),
            ref_id,
            base_id: 0x7,
            values: HashMap::new(),
            base_values: HashMap::new(),
            race: 0,
            age: 0,
            idle: 0,
            moving: 0,
            moving_xy: 0,
            weapon: 0,
            female: false,
            alerted: false,
            sneaking: false,
            dead: false,
            death_limbs: 0,
            death_cause: 0,
            scale: 1.0,
        }
    }

    #[test]
    fn actor_new_grants_ownership_and_registers() {
        let registry = Arc::new(ObjectRegistry::new());
        let pid = registry.insert(Player::new(NetworkID::new(1), 0x100, 0x7, 0x1));
        let session = session_owning(pid);
        let pkt = actor_new_packet(50, 0x200);
        let (grant, relay) = handle_actor_new(&registry, &session, &pkt);
        assert!(matches!(grant, Some(Packet::OwnershipGranted { .. })));
        assert!(relay.is_some());
        // ref is mapped to the actor id, owner recorded, actor in cell
        assert_eq!(registry.lookup_ref(0x200), Some(NetworkID::new(50)));
        assert_eq!(registry.owner_of(NetworkID::new(50)), Some(pid));
        assert!(registry.get_by_cell(0x1).contains(&NetworkID::new(50)));
    }

    #[test]
    fn actor_new_dedupes_by_ref_id() {
        let registry = Arc::new(ObjectRegistry::new());
        let pid = registry.insert(Player::new(NetworkID::new(1), 0x100, 0x7, 0x1));
        let session = session_owning(pid);
        handle_actor_new(&registry, &session, &actor_new_packet(50, 0x200));
        // second client reports the same NPC with a different id → rejected
        let (grant, relay) = handle_actor_new(&registry, &session, &actor_new_packet(51, 0x200));
        assert!(grant.is_none() && relay.is_none());
        assert!(registry.lookup_ref(0x200) == Some(NetworkID::new(50)));
    }

    #[test]
    fn actor_new_rejects_owned_actor() {
        let registry = Arc::new(ObjectRegistry::new());
        let pid = registry.insert(Player::new(NetworkID::new(1), 0x100, 0x7, 0x1));
        let session = session_owning(pid);
        // pre-claim the id via a different ref (already owned)
        registry.set_owner(NetworkID::new(50), pid);
        let (grant, relay) = handle_actor_new(&registry, &session, &actor_new_packet(50, 0x300));
        assert!(grant.is_none() && relay.is_none());
    }

    #[test]
    fn ownership_claim_only_for_unowned() {
        let registry = Arc::new(ObjectRegistry::new());
        let pid = registry.insert(Player::new(NetworkID::new(1), 0x100, 0x7, 0x1));
        let session = session_owning(pid);
        let aid = registry.insert(crate::world::objects::Actor::new(
            NetworkID::new(50),
            0x200,
            0x7,
            0x1,
        ));
        assert!(handle_ownership_claim(&registry, &session, aid).is_some());
        // second claim fails — already owned
        assert!(handle_ownership_claim(&registry, &session, aid).is_none());
        assert_eq!(registry.owner_of(aid), Some(pid));
    }
}
