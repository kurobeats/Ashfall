//! Shared pipe-frame framing + bridge event types.
//!
//! Both the bridge DLL (inside Wine) and the native client speak this wire
//! format. Frames are length-prefixed so command responses and engine events
//! can share one TCP stream without ambiguity:
//!
//! ```text
//! Frame: [len:2 LE][opcode:1][payload...]
//! ```
//!
//! Event payloads are fixed-size C structs (the bridge packs game memory
//! directly). Event type 0-6 are the NVSE sink events (bridge-only); 7+ are
//! the pipe-level events the client acts on.

use serde::{Deserialize, Serialize};

/// Pipe event frame opcode (engine → client).
pub const PIPE_OP_EVENT: u8 = 0x07;

/// Own-player state sample (bridge → client), the coop movement loop:
/// the client turns it into UpdatePos/UpdateAngle/ActorStateDelta packets.
pub const EVENT_PLAYER_STATE: u32 = 7;

/// NPC observed in the player's loaded cell (bridge → client): the client
/// reports it as ActorNew + claims simulation ownership.
pub const EVENT_NPC_SPAWN: u32 = 8;

/// Own-player state sample.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PlayerStateEvent {
    pub ref_id: u32,
    pub pos: [f32; 3],
    pub angle: [f32; 3],
    pub idle: u32,
    pub moving: u8,
    pub moving_xy: u8,
    pub weapon: u8,
    pub alerted: bool,
    pub sneaking: bool,
    pub health: f32,
}

/// NPC observed in the loaded cell.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct NpcSpawnEvent {
    pub ref_id: u32,
    pub base_id: u32,
    pub pos: [f32; 3],
    pub cell: u32,
}

/// A decoded pipe frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PipeFrame {
    pub opcode: u8,
    pub payload: Vec<u8>,
}

/// Encode a length-prefixed pipe frame.
pub fn encode_frame(opcode: u8, payload: &[u8]) -> Vec<u8> {
    let mut frame = Vec::with_capacity(2 + 1 + payload.len());
    frame.extend_from_slice(&((payload.len() as u16).to_le_bytes()));
    frame.push(opcode);
    frame.extend_from_slice(payload);
    frame
}

/// Split a byte stream into complete frames. Returns (frames, leftover bytes
/// awaiting the rest of the next frame).
pub fn split_frames(buf: &[u8]) -> (Vec<PipeFrame>, &[u8]) {
    let mut frames = Vec::new();
    let mut off = 0;
    while off + 2 <= buf.len() {
        let len = u16::from_le_bytes([buf[off], buf[off + 1]]) as usize;
        let total = 2 + 1 + len;
        if off + total > buf.len() {
            break;
        }
        frames.push(PipeFrame {
            opcode: buf[off + 2],
            payload: buf[off + 3..off + total].to_vec(),
        });
        off += total;
    }
    (frames, &buf[off..])
}

/// Encode a player-state event frame.
pub fn encode_player_state_event(e: &PlayerStateEvent) -> Vec<u8> {
    let mut payload = Vec::with_capacity(4 + std::mem::size_of::<PlayerStateEvent>());
    payload.extend_from_slice(&EVENT_PLAYER_STATE.to_le_bytes());
    payload.extend_from_slice(unsafe { core::slice::from_raw_parts(e as *const _ as *const u8, std::mem::size_of::<PlayerStateEvent>()) });
    encode_frame(PIPE_OP_EVENT, &payload)
}

/// Encode an NPC-spawn event frame.
pub fn encode_npc_spawn_event(e: &NpcSpawnEvent) -> Vec<u8> {
    let mut payload = Vec::with_capacity(4 + std::mem::size_of::<NpcSpawnEvent>());
    payload.extend_from_slice(&EVENT_NPC_SPAWN.to_le_bytes());
    payload.extend_from_slice(unsafe { core::slice::from_raw_parts(e as *const _ as *const u8, std::mem::size_of::<NpcSpawnEvent>()) });
    encode_frame(PIPE_OP_EVENT, &payload)
}

/// Decode an event frame's payload into (event_type, event_bytes).
pub fn decode_event(payload: &[u8]) -> Option<(u32, &[u8])> {
    if payload.len() < 4 {
        return None;
    }
    let event_type = u32::from_le_bytes([payload[0], payload[1], payload[2], payload[3]]);
    Some((event_type, &payload[4..]))
}

/// Decode a player-state event payload.
pub fn decode_player_state(data: &[u8]) -> Option<PlayerStateEvent> {
    if data.len() < std::mem::size_of::<PlayerStateEvent>() {
        return None;
    }
    Some(unsafe { core::ptr::read_unaligned(data.as_ptr() as *const PlayerStateEvent) })
}

/// Decode an NPC-spawn event payload.
pub fn decode_npc_spawn(data: &[u8]) -> Option<NpcSpawnEvent> {
    if data.len() < std::mem::size_of::<NpcSpawnEvent>() {
        return None;
    }
    Some(unsafe { core::ptr::read_unaligned(data.as_ptr() as *const NpcSpawnEvent) })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_frame_roundtrip_and_split() {
        let frames = vec![
            encode_frame(0x02, &[1, 2, 3, 4, 5]),
            encode_frame(PIPE_OP_EVENT, &[9, 9, 9]),
        ];
        let stream: Vec<u8> = frames.iter().flat_map(|f| f.iter().copied()).collect();
        // Split must handle the concatenated stream.
        let (decoded, rest) = split_frames(&stream);
        assert!(rest.is_empty());
        assert_eq!(decoded.len(), 2);
        assert_eq!(decoded[0].opcode, 0x02);
        assert_eq!(decoded[0].payload, vec![1, 2, 3, 4, 5]);
        assert_eq!(decoded[1].opcode, PIPE_OP_EVENT);
    }

    #[test]
    fn test_split_partial_frame() {
        let frame = encode_frame(0x02, &[1, 2, 3, 4, 5]);
        let (decoded, rest) = split_frames(&frame[..frame.len() - 2]);
        assert!(decoded.is_empty(), "incomplete frame waits");
        assert_eq!(rest.len(), frame.len() - 2);
        // Feeding the remainder completes it.
        let mut combined = rest.to_vec();
        combined.extend_from_slice(&frame[frame.len() - 2..]);
        let (decoded2, _) = split_frames(&combined);
        assert_eq!(decoded2.len(), 1);
        assert_eq!(decoded2[0].payload, vec![1, 2, 3, 4, 5]);
    }

    #[test]
    fn test_player_state_roundtrip() {
        let e = PlayerStateEvent {
            ref_id: 0x14,
            pos: [1.0, 2.0, 3.0],
            angle: [0.5, 0.0, 1.5],
            idle: 7,
            moving: 2,
            moving_xy: 1,
            weapon: 0x2A,
            alerted: true,
            sneaking: false,
            health: 87.5,
        };
        let frame = encode_player_state_event(&e);
        let (frames, _) = split_frames(&frame);
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].opcode, PIPE_OP_EVENT);
        let (event_type, data) = decode_event(&frames[0].payload).unwrap();
        assert_eq!(event_type, EVENT_PLAYER_STATE);
        assert_eq!(decode_player_state(data).unwrap(), e);
    }

    #[test]
    fn test_npc_spawn_roundtrip() {
        let e = NpcSpawnEvent { ref_id: 0x1234, base_id: 0x5678, pos: [5.0, 6.0, 7.0], cell: 42 };
        let frame = encode_npc_spawn_event(&e);
        let (frames, _) = split_frames(&frame);
        let (event_type, data) = decode_event(&frames[0].payload).unwrap();
        assert_eq!(event_type, EVENT_NPC_SPAWN);
        assert_eq!(decode_npc_spawn(data).unwrap(), e);
    }

    #[test]
    fn test_decode_truncated() {
        assert!(decode_event(&[0, 0]).is_none());
        assert!(decode_player_state(&[0u8; 4]).is_none());
        assert!(decode_npc_spawn(&[0u8; 8]).is_none());
    }
}
