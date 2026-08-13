//! Bridge ↔ server sync logic — the vanilla-coop loop.
//!
//! Two directions, both pure (testable without a game):
//! - Engine events (bridge pipe) → server packets: own-player state becomes
//!   UpdatePos/UpdateAngle/ActorStateDelta; NPC spawns become ActorNew +
//!   OwnershipClaim (the ownership protocol's client half).
//! - Server packets → engine commands: remote entities are applied to the
//!   local game via OP_SET_POS/OP_SET_ANGLE.

use ashfall_core::event::{
    decode_event, decode_npc_spawn, decode_player_state, PipeFrame, EVENT_NPC_SPAWN,
    EVENT_PLAYER_STATE, PIPE_OP_EVENT,
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
            _ => {}
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use ashfall_core::event::{encode_npc_spawn_event, encode_player_state_event, NpcSpawnEvent, PlayerStateEvent};

    #[test]
    fn test_entity_id_roundtrip() {
        assert_eq!(entity_id(0x14).as_u64(), 0x8000_0014);
        assert_eq!(entity_id(0x1234).as_u64(), 0x8000_1234);
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
