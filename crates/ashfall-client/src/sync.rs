//! Bridge ↔ server sync logic — the vanilla-coop loop.
//!
//! Two directions, both pure (testable without a game):
//! - Engine events (bridge pipe) → server packets: own-player state becomes
//!   UpdatePos/UpdateAngle/ActorStateDelta; NPC spawns become ActorNew +
//!   OwnershipClaim (the ownership protocol's client half).
//! - Server packets → engine commands: remote entities are applied to the
//!   local game via OP_SET_POS/OP_SET_ANGLE.

use ashfall_core::event::{
    decode_event, decode_npc_remove, decode_npc_spawn, decode_player_state, PipeFrame,
    EVENT_NPC_REMOVE, EVENT_NPC_SPAWN, EVENT_NPC_STATE, EVENT_PLAYER_STATE,
    decode_ref_event, EVENT_ACTIVATE, EVENT_FIRE, PIPE_OP_EVENT,
};
use ashfall_core::id::NetworkID;
use ashfall_core::protocol::Packet;
use crate::ipc::Param;

/// Client-side entity id space for game refs: the high bit marks ref-derived
/// ids so they never collide with server-assigned ids (which start at 1).
/// Both the owning client and the server agree on the id without a handshake.
pub fn entity_id(ref_id: u32) -> NetworkID {
    NetworkID::new(0x8000_0000u64 | ref_id as u64)
}

/// Recover the game ref id from a client-derived entity id.
pub fn ref_of_entity(id: NetworkID) -> Option<u32> {
    let v = id.as_u64();
    if v & 0x8000_0000 != 0 {
        Some((v & 0x7FFF_FFFF) as u32)
    } else {
        None
    }
}

/// Bridge event frames → server packets.
pub fn events_to_packets(frames: &[PipeFrame], local_id: NetworkID) -> Vec<Packet> {
    let mut out = Vec::new();
    for frame in frames {
        if frame.opcode != PIPE_OP_EVENT {
            continue;
        }
        let Some((event_type, data)) = decode_event(&frame.payload) else { continue };
        match event_type {
            EVENT_PLAYER_STATE => {
                if let Some(e) = decode_player_state(data) {
                    out.push(Packet::UpdatePos { id: local_id, pos: e.pos });
                    out.push(Packet::UpdateAngle { id: local_id, angle: [e.angle[0], e.angle[2]] });
                    out.push(Packet::ActorStateDelta {
                        id: local_id,
                        idle: Some(e.idle),
                        moving: Some(e.moving),
                        moving_xy: Some(e.moving_xy),
                        weapon: Some(e.weapon),
                        alerted: Some(e.alerted),
                        sneaking: Some(e.sneaking),
                        firing: None,
                    });
                    // Health: reported for remote display; damage resolution
                    // still flows through ActorHit (ponytail: NPC-dealt damage
                    // in the local sim reports here, player-dealt via server).
                    out.push(Packet::UpdateActorValue {
                        id: local_id,
                        base: false,
                        index: 0x14,
                        value: e.health,
                    });
                }
            }
            EVENT_NPC_SPAWN => {
                if let Some(e) = decode_npc_spawn(data) {
                    let id = entity_id(e.ref_id);
                    out.push(Packet::ActorNew {
                        id,
                        ref_id: e.ref_id,
                        base_id: e.base_id,
                        values: Default::default(),
                        base_values: Default::default(),
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
                    });
                    out.push(Packet::UpdatePos { id, pos: e.pos });
                    out.push(Packet::OwnershipClaim { id });
                }
            }
            EVENT_NPC_STATE => {
                if let Some(e) = decode_player_state(data) {
                    let id = entity_id(e.ref_id);
                    out.push(Packet::UpdatePos { id, pos: e.pos });
                    out.push(Packet::UpdateAngle { id, angle: [e.angle[0], e.angle[2]] });
                    out.push(Packet::ActorStateDelta {
                        id,
                        idle: Some(e.idle),
                        moving: Some(e.moving),
                        moving_xy: Some(e.moving_xy),
                        weapon: Some(e.weapon),
                        alerted: Some(e.alerted),
                        sneaking: Some(e.sneaking),
                        firing: None,
                    });
                    out.push(Packet::UpdateActorValue {
                        id,
                        base: false,
                        index: 0x14,
                        value: e.health,
                    });
                }
            }
            EVENT_NPC_REMOVE => {
                if let Some(e) = decode_npc_remove(data) {
                    // Despawned / left view: tell the server to stop
                    // replicating it (STR ActorRemovedEvent → removal).
                    out.push(Packet::ObjectRemove { id: entity_id(e.ref_id), silent: false });
                }
            }
            EVENT_ACTIVATE => {
                if let Some(ref_id) = decode_ref_event(data) {
                    // Player activated an object — relay so the server
                    // applies the open/use authoritatively. actor = the
                    // local player (local_id); the activated object is the
                    // ref.
                    out.push(Packet::UpdateActivate {
                        id: entity_id(ref_id),
                        actor: local_id,
                    });
                }
            }
            EVENT_FIRE => {
                if let Some(ref_id) = decode_ref_event(data) {
                    // Player fired — relay so remote players see the shot.
                    out.push(Packet::UpdateFireWeapon {
                        id: local_id,
                        weapon: ref_id,
                    });
                }
            }
            _ => {}
        }
    }
    out
}

/// Server packets → engine commands for remote entities (never the local
/// player). `resolve_ref` maps a server NetworkID to the game ref id (the
/// client registry holds it from ActorNew/PlayerNew/ObjectNew).
pub fn packets_to_commands(
    packets: &[Packet],
    local_id: NetworkID,
    resolve_ref: impl Fn(NetworkID) -> Option<u32>,
) -> Vec<(u32, Vec<Param>)> {
    let mut out = Vec::new();
    for pkt in packets {
        match pkt {
            Packet::UpdatePos { id, pos } if *id != local_id => {
                let Some(ref_id) = resolve_ref(*id) else { continue };
                out.push((
                    crate::ipc::OP_SET_POS,
                    vec![Param::U32(ref_id), Param::F32(pos[0]), Param::F32(pos[1]), Param::F32(pos[2])],
                ));
            }
            Packet::UpdateAngle { id, angle } if *id != local_id => {
                let Some(ref_id) = resolve_ref(*id) else { continue };
                out.push((
                    crate::ipc::OP_SET_ANGLE,
                    vec![Param::U32(ref_id), Param::F32(angle[0]), Param::F32(0.0), Param::F32(angle[1])],
                ));
            }
            // Actor value (health/AP/DR...) applied to the local copy.
            Packet::UpdateActorValue { id, index, value, .. } if *id != local_id => {
                let Some(ref_id) = resolve_ref(*id) else { continue };
                out.push((
                    crate::ipc::OP_SET_ACTOR_VALUE,
                    vec![Param::U32(ref_id), Param::U8(*index), Param::F32(*value)],
                ));
            }
            // Death applied to the local copy (respawn stays server-driven).
            Packet::UpdateActorDead { id, dead: true, .. } if *id != local_id => {
                let Some(ref_id) = resolve_ref(*id) else { continue };
                out.push((
                    crate::ipc::OP_KILL,
                    vec![Param::U32(ref_id), Param::U32(0), Param::U8(0), Param::U8(0)],
                ));
            }
            // Remote player activated an object — apply it locally so the
            // world stays shared (opening doors/containers propagates).
            Packet::UpdateActivate { id, .. } => {
                let Some(ref_id) = resolve_ref(*id) else { continue };
                out.push((
                    crate::ipc::OP_GET_ACTIVATE,
                    vec![Param::U32(ref_id)],
                ));
            }
            // Remote player fired — apply locally so shots propagate.
            // The weapon id is the firing actor's weapon ref (bridge reads
            // it from the engine); we only need the shooter, which the
            // server relays as the packet id.
            Packet::UpdateFireWeapon { id, weapon } if *id != local_id => {
                let Some(shooter) = resolve_ref(*id) else { continue };
                out.push((
                    crate::ipc::OP_FIRE_WEAPON,
                    vec![Param::U32(shooter), Param::U32(*weapon)],
                ));
            }
            // Remote lock-state change (server broadcasts from the script
            // host) — apply the lock byte locally.
            Packet::UpdateLock { id, lock } => {
                let Some(ref_id) = resolve_ref(*id) else { continue };
                out.push((
                    crate::ipc::OP_SET_LOCK,
                    vec![Param::U32(ref_id), Param::U32(*lock)],
                ));
            }
            // Remote scale change — apply the scale field locally.
            Packet::UpdateScale { id, scale } => {
                let Some(ref_id) = resolve_ref(*id) else { continue };
                out.push((
                    crate::ipc::OP_SET_SCALE,
                    vec![Param::U32(ref_id), Param::F32(*scale)],
                ));
            }
            _ => {}
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use ashfall_core::event::{
        encode_npc_remove_event, encode_npc_spawn_event, encode_npc_state_event,
        encode_player_state_event, encode_ref_event, NpcRemoveEvent, NpcSpawnEvent,
        PlayerStateEvent, EVENT_ACTIVATE, EVENT_FIRE,
    };

    #[test]
    fn test_activate_event_to_packet() {
        let local = NetworkID::new(1);
        let frame = encode_ref_event(EVENT_ACTIVATE, 0x9999);
        let (frames, _) = ashfall_core::event::split_frames(&frame);
        let packets = events_to_packets(&frames, local);
        // Activated object 0x9999, actor = local player.
        assert!(packets.iter().any(|p| matches!(
            p, Packet::UpdateActivate { id, actor } if *id == entity_id(0x9999) && *actor == local
        )));
    }

    #[test]
    fn test_fire_event_to_packet() {
        let local = NetworkID::new(1);
        let frame = encode_ref_event(EVENT_FIRE, 0x7777);
        let (frames, _) = ashfall_core::event::split_frames(&frame);
        let packets = events_to_packets(&frames, local);
        // Fire relay: weapon ref 0x7777, firing id = local player.
        assert!(packets.iter().any(|p| matches!(
            p, Packet::UpdateFireWeapon { id, weapon } if *id == local && *weapon == 0x7777
        )));
    }

    #[test]
    fn test_npc_state_event_to_packets() {
        let local = NetworkID::new(1);
        let e = PlayerStateEvent {
            ref_id: 0x1234, pos: [1.0, 2.0, 3.0], angle: [0.0, 0.0, 45.0],
            idle: 0, moving: 1, moving_xy: 0, weapon: 0x2A,
            alerted: true, sneaking: false, health: 55.0,
        };
        let frame = encode_npc_state_event(&e);
        let (frames, _) = ashfall_core::event::split_frames(&frame);
        let packets = events_to_packets(&frames, local);

        let expected_id = entity_id(0x1234);
        assert!(packets.iter().any(|p| matches!(p, Packet::UpdatePos { id, pos } if *id == expected_id && *pos == [1.0, 2.0, 3.0])));
        assert!(packets.iter().any(|p| matches!(p, Packet::UpdateAngle { id, angle } if *id == expected_id && *angle == [0.0, 45.0])));
        assert!(packets.iter().any(|p| matches!(p, Packet::ActorStateDelta { id, weapon: Some(w), moving: Some(m), .. } if *id == expected_id && *w == 0x2A && *m == 1)));
        assert!(packets.iter().any(|p| matches!(p, Packet::UpdateActorValue { id, index: 0x14, value, .. } if *id == expected_id && *value == 55.0)));
        // Must NOT target the local player id.
        assert!(packets.iter().all(|p| {
            match p {
                Packet::UpdatePos { id, .. } | Packet::UpdateAngle { id, .. } | Packet::UpdateActorValue { id, .. } => *id != local,
                Packet::ActorStateDelta { id, .. } => *id != local,
                _ => true,
            }
        }));
    }

    #[test]
    fn test_remote_values_and_death_to_commands() {
        let local = NetworkID::new(1);
        let remote = NetworkID::new(9);
        let packets = vec![
            Packet::UpdateActorValue { id: remote, base: false, index: 0x14, value: 40.0 },
            Packet::UpdateActorDead { id: remote, dead: true, limbs: 0, cause: 1 },
            // Local player's own value must NOT become a command.
            Packet::UpdateActorValue { id: local, base: false, index: 0x14, value: 100.0 },
        ];
        let resolve = |id: NetworkID| if id == remote { Some(0x50) } else { None };
        let cmds = packets_to_commands(&packets, local, resolve);
        assert_eq!(cmds.len(), 2, "remote value + death only");
        let (op0, p0) = &cmds[0];
        assert_eq!(*op0, crate::ipc::OP_SET_ACTOR_VALUE);
        assert!(matches!(p0[0], Param::U32(0x50)));
        assert!(matches!(p0[1], Param::U8(0x14)), "index is a single byte");
        assert!(matches!(p0[2], Param::F32(40.0)));
        let (op1, _) = &cmds[1];
        assert_eq!(*op1, crate::ipc::OP_KILL);
    }

    #[test]
    fn test_remote_activate_fire_to_commands() {
        let local = NetworkID::new(1);
        let remote = NetworkID::new(9);
        let packets = vec![
            // Remote player activated an object (id = the ref, actor = remote).
            Packet::UpdateActivate { id: entity_id(0x60), actor: remote },
            // Remote player fired (id = shooter, weapon = weapon ref).
            Packet::UpdateFireWeapon { id: remote, weapon: 0x77 },
            // Own activation must NOT become a command.
            Packet::UpdateActivate { id: entity_id(0x61), actor: local },
        ];
        let resolve = |id: NetworkID| {
            if id == remote { Some(0x50) } else if id == entity_id(0x60) { Some(0x60) } else { None }
        };
        let cmds = packets_to_commands(&packets, local, resolve);
        // remote activate + remote fire; own activate skipped.
        assert_eq!(cmds.len(), 2);
        let (op0, p0) = &cmds[0];
        assert_eq!(*op0, crate::ipc::OP_GET_ACTIVATE);
        assert!(matches!(p0[0], Param::U32(0x60)));
        let (op1, p1) = &cmds[1];
        assert_eq!(*op1, crate::ipc::OP_FIRE_WEAPON);
        assert!(matches!(p1[0], Param::U32(0x50)), "shooter is the remote actor");
        assert!(matches!(p1[1], Param::U32(0x77)), "weapon ref");
    }

    #[test]
    fn test_remote_lock_to_command() {
        let local = NetworkID::new(1);
        let obj_id = entity_id(0x70);
        let packets = vec![
            Packet::UpdateLock { id: obj_id, lock: 1 },
        ];
        let resolve = |id: NetworkID| if id == obj_id { Some(0x70) } else { None };
        let cmds = packets_to_commands(&packets, local, resolve);
        assert_eq!(cmds.len(), 1);
        let (op, p) = &cmds[0];
        assert_eq!(*op, crate::ipc::OP_SET_LOCK);
        assert!(matches!(p[0], Param::U32(0x70)));
        assert!(matches!(p[1], Param::U32(1)));
    }

    #[test]
    fn test_remote_scale_to_command() {
        let local = NetworkID::new(1);
        let obj_id = entity_id(0x80);
        let packets = vec![
            Packet::UpdateScale { id: obj_id, scale: 1.5 },
        ];
        let resolve = |id: NetworkID| if id == obj_id { Some(0x80) } else { None };
        let cmds = packets_to_commands(&packets, local, resolve);
        assert_eq!(cmds.len(), 1);
        let (op, p) = &cmds[0];
        assert_eq!(*op, crate::ipc::OP_SET_SCALE);
        assert!(matches!(p[0], Param::U32(0x80)));
        assert!(matches!(p[1], Param::F32(1.5)));
    }

    #[test]
    fn test_entity_id_ref_roundtrip() {
        assert_eq!(entity_id(0x1234).as_u64(), 0x8000_1234);
        assert_eq!(ref_of_entity(entity_id(0x1234)), Some(0x1234));
        assert_eq!(ref_of_entity(NetworkID::new(7)), None, "server ids have no ref");
    }

    #[test]
    fn test_player_state_event_to_packets() {
        let local = NetworkID::new(1);
        let e = PlayerStateEvent {
            ref_id: 0x14, pos: [1.0, 2.0, 3.0], angle: [0.0, 0.0, 90.0],
            idle: 0, moving: 2, moving_xy: 1, weapon: 0x2A,
            alerted: true, sneaking: false, health: 77.0,
        };
        let frame = encode_player_state_event(&e);
        let frames = {
            let (fs, _) = ashfall_core::event::split_frames(&frame);
            fs
        };
        let packets = events_to_packets(&frames, local);

        assert!(packets.iter().any(|p| matches!(p, Packet::UpdatePos { id, pos } if *id == local && *pos == [1.0, 2.0, 3.0])));
        assert!(packets.iter().any(|p| matches!(p, Packet::UpdateAngle { id, angle } if *id == local && *angle == [0.0, 90.0])));
        assert!(packets.iter().any(|p| matches!(p, Packet::ActorStateDelta { id, weapon: Some(w), moving: Some(m), alerted: Some(a), .. } if *id == local && *w == 0x2A && *m == 2 && *a)));
        assert!(packets.iter().any(|p| matches!(p, Packet::UpdateActorValue { id, index: 0x14, value, .. } if *id == local && *value == 77.0)));
    }

    #[test]
    fn test_npc_spawn_event_to_packets() {
        let local = NetworkID::new(1);
        let e = NpcSpawnEvent { ref_id: 0x1234, base_id: 0x5678, pos: [5.0, 6.0, 7.0], cell: 42 };
        let frame = encode_npc_spawn_event(&e);
        let (frames, _) = ashfall_core::event::split_frames(&frame);
        let packets = events_to_packets(&frames, local);

        let expected_id = entity_id(0x1234);
        assert!(packets.iter().any(|p| matches!(p, Packet::ActorNew { id, ref_id, base_id, .. } if *id == expected_id && *ref_id == 0x1234 && *base_id == 0x5678)));
        assert!(packets.iter().any(|p| matches!(p, Packet::OwnershipClaim { id } if *id == expected_id)));
        assert!(packets.iter().any(|p| matches!(p, Packet::UpdatePos { id, pos } if *id == expected_id && *pos == [5.0, 6.0, 7.0])));
    }

    #[test]
    fn test_npc_remove_event_to_packets() {
        let local = NetworkID::new(1);
        let e = NpcRemoveEvent { ref_id: 0x1234 };
        let frame = encode_npc_remove_event(&e);
        let (frames, _) = ashfall_core::event::split_frames(&frame);
        let packets = events_to_packets(&frames, local);

        assert!(packets.iter().any(|p| matches!(p, Packet::ObjectRemove { id, .. } if *id == entity_id(0x1234))));
    }

    #[test]
    fn test_remote_packets_to_commands() {
        let local = NetworkID::new(1);
        let remote = NetworkID::new(9);
        let packets = vec![
            Packet::UpdatePos { id: remote, pos: [10.0, 20.0, 30.0] },
            Packet::UpdateAngle { id: remote, angle: [90.0, 45.0] },
            // Local player updates must NOT become commands.
            Packet::UpdatePos { id: local, pos: [0.0, 0.0, 0.0] },
        ];
        let resolve = |id: NetworkID| -> Option<u32> {
            if id == remote { Some(0x50) } else { None }
        };
        let cmds = packets_to_commands(&packets, local, resolve);
        assert_eq!(cmds.len(), 2, "remote packets only");
        let (op0, params0) = &cmds[0];
        assert_eq!(*op0, crate::ipc::OP_SET_POS);
        assert!(matches!(params0[0], Param::U32(0x50)));
        assert!(matches!(params0[3], Param::F32(30.0)));
        let (op1, _) = &cmds[1];
        assert_eq!(*op1, crate::ipc::OP_SET_ANGLE);
    }
}
