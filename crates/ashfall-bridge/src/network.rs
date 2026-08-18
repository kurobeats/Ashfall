//! TCP server inside the Wine/Proton process.
//!
//! Listens on loopback only (127.0.0.1:1771). Accepts a single connection
//! from the native Linux ashfall-client. Decodes pipe-protocol commands
//! and dispatches to the command handler.

use std::collections::VecDeque;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{LazyLock, Mutex};

use crate::commands;
use crate::RUNNING;

/// Pipe protocol opcodes (match original vaultmp.hpp).
pub const PIPE_SYS_WAKEUP: u8 = 0x01;
pub const PIPE_OP_COMMAND: u8 = 0x02;
pub const PIPE_OP_RETURN: u8 = 0x03;
pub const PIPE_OP_RETURN_BIG: u8 = 0x04; // reserved for large responses
pub const PIPE_OP_RETURN_RAW: u8 = 0x05; // reserved for raw binary
pub const PIPE_ERROR_CLOSE: u8 = 0x06;
pub const PIPE_OP_EVENT: u8 = 0x07; // engine → client event frame

/// Outbound event frames waiting for the TCP writer (pushed from hook
/// callbacks on any thread, drained by the connection loop).
static EVENT_QUEUE: LazyLock<Mutex<VecDeque<Vec<u8>>>> =
    LazyLock::new(|| Mutex::new(VecDeque::new()));

/// Queue an event frame for delivery to the connected client.
pub fn push_event_frame(frame: Vec<u8>) {
    EVENT_QUEUE.lock().unwrap().push_back(frame);
}

/// Take all queued event frames.
pub fn take_event_frames() -> Vec<Vec<u8>> {
    let mut q = EVENT_QUEUE.lock().unwrap();
    q.drain(..).collect()
}

/// Local player reference id (vaultmp convention, matches the server's
/// Player::new(ref_id 0x14)).
pub const LOCAL_PLAYER_REF: u32 = 0x14;

/// Sample the local player's state and queue it as an EVENT_PLAYER_STATE
/// frame for the client. On non-Windows (tests) the getters return defaults,
/// so the event still flows — the frame is the contract.
/// ponytail: called by the debug command today; the real trigger is a
/// per-frame game-loop hook (RE: locate the frame function on the Steam
/// build, verify on host — steam-re.md).
pub fn report_player_state() -> Vec<u8> {
    use ashfall_core::event::{encode_player_state_event, PlayerStateEvent};
    let pos = crate::hooks::get_pos(LOCAL_PLAYER_REF);
    let angle = crate::hooks::get_angle(LOCAL_PLAYER_REF);
    let (idle, moving, weapon, moving_xy, alerted, sneaking) =
        crate::hooks::get_actor_state(LOCAL_PLAYER_REF);
    let health = crate::hooks::get_actor_value(LOCAL_PLAYER_REF, 0x14);
    let event = PlayerStateEvent {
        ref_id: LOCAL_PLAYER_REF,
        pos,
        angle,
        idle,
        moving,
        moving_xy,
        weapon,
        alerted,
        sneaking,
        health,
    };
    let frame = encode_player_state_event(&event);
    push_event_frame(frame.clone());
    frame
}

/// Last player-state sample time — the per-frame hook calls
/// [`report_player_state_due`], which throttles to 10 Hz (STR
/// `RunLocalUpdates`: `cDelayBetweenSnapshots = 100ms`).
static LAST_REPORT: LazyLock<Mutex<Option<std::time::Instant>>> =
    LazyLock::new(|| Mutex::new(None));

/// NPCs this client owns (simulates) — sampled each tick and reported as
/// EVENT_NPC_STATE so the server relays the owner's simulation. The client
/// commands TRACK on OwnershipGranted, UNTRACK on OwnershipReleased.
static TRACKED_ACTORS: LazyLock<Mutex<Vec<u32>>> = LazyLock::new(|| Mutex::new(Vec::new()));

/// Start tracking an NPC (owned) — sample its state at the 10 Hz cadence.
pub fn track_actor(ref_id: u32) {
    let mut tracked = TRACKED_ACTORS.lock().unwrap();
    if !tracked.contains(&ref_id) {
        tracked.push(ref_id);
    }
}

/// Stop tracking an NPC (ownership released / despawned).
pub fn untrack_actor(ref_id: u32) {
    TRACKED_ACTORS.lock().unwrap().retain(|&r| r != ref_id);
}

/// Sample every tracked NPC's state and queue EVENT_NPC_STATE frames.
/// Non-Windows (tests): getters return defaults — the frame is the contract.
pub fn sample_tracked() {
    use ashfall_core::event::{encode_npc_state_event, PlayerStateEvent};
    let tracked: Vec<u32> = TRACKED_ACTORS.lock().unwrap().clone();
    for ref_id in tracked {
        let pos = crate::hooks::get_pos(ref_id);
        let angle = crate::hooks::get_angle(ref_id);
        let (idle, moving, weapon, moving_xy, alerted, sneaking) =
            crate::hooks::get_actor_state(ref_id);
        let health = crate::hooks::get_actor_value(ref_id, 0x14);
        let event = PlayerStateEvent {
            ref_id,
            pos,
            angle,
            idle,
            moving,
            moving_xy,
            weapon,
            alerted,
            sneaking,
            health,
        };
        push_event_frame(encode_npc_state_event(&event));
    }
}

/// Snapshot of tracked actors (tests).
pub fn tracked_actors() -> Vec<u32> {
    TRACKED_ACTORS.lock().unwrap().clone()
}

/// Reset tracking (tests).
pub fn reset_tracked() {
    TRACKED_ACTORS.lock().unwrap().clear();
}

/// Engine calls that must run on the game thread (vaultmp's "the pipe
/// thread cannot call the engine directly" rule — verified live 2026-08-18k:
/// calling 0x7F3200 from the TCP server thread applied no death and killed
/// the server thread). The per-frame game-loop hook drains this queue on the
/// game thread; `enqueue_engine_call` returns immediately.
static PENDING_ENGINE_CALLS: LazyLock<Mutex<Vec<Box<dyn FnOnce() + Send>>>> =
    LazyLock::new(|| Mutex::new(Vec::new()));

/// Queue a closure to run on the game thread (next frame).
pub fn enqueue_engine_call(f: Box<dyn FnOnce() + Send>) {
    PENDING_ENGINE_CALLS.lock().unwrap().push(f);
}

/// Run every queued engine call on the calling (game) thread. Called from
/// the per-actor discovery detour each frame — the only game-thread hook
/// proven to fire live (frame hook 0x9B3D77 never emitted; direct calls
/// from the TCP thread killed it).
pub fn drain_engine_calls() {
    let pending = std::mem::take(&mut *PENDING_ENGINE_CALLS.lock().unwrap());
    for f in pending {
        f();
    }
}

/// Sample the player at most every 100ms (10 Hz). Returns the event frame
/// when a sample was due, None otherwise. The future per-frame game-loop
/// hook calls this every frame and sends whatever comes back.
pub fn report_player_state_due() -> Option<Vec<u8>> {
    // Drain game-thread engine calls first (every frame, before throttling
    // skips the player sample).
    drain_engine_calls();
    let mut last = LAST_REPORT.lock().unwrap();
    let now = std::time::Instant::now();
    if let Some(prev) = *last {
        if now.duration_since(prev) < std::time::Duration::from_millis(100) {
            return None;
        }
    }
    *last = Some(now);
    Some(report_player_state())
}

/// Encode a pipe command: [PIPE_OP_COMMAND][key:4B LE][func:4B LE][param_count:1B][params...]
/// Frames are length-prefixed ([len:2][opcode][payload]) so responses and
/// events share the stream unambiguously.
pub fn encode_pipe_command(key: u32, func: u32, params: &[u8]) -> Vec<u8> {
    let mut payload = Vec::with_capacity(4 + 4 + 1 + params.len());
    payload.push(PIPE_OP_COMMAND);
    payload.extend_from_slice(&key.to_le_bytes());
    payload.extend_from_slice(&func.to_le_bytes());
    payload.push(params.len() as u8);
    payload.extend_from_slice(params);
    ashfall_core::event::encode_frame(PIPE_OP_COMMAND, &payload[1..])
}

/// Encode a pipe return: [PIPE_OP_RETURN][key:4B LE][result...]
pub fn encode_pipe_return(key: u32, result: &[u8]) -> Vec<u8> {
    let mut payload = Vec::with_capacity(4 + result.len());
    payload.push(PIPE_OP_RETURN);
    payload.extend_from_slice(&key.to_le_bytes());
    payload.extend_from_slice(result);
    ashfall_core::event::encode_frame(PIPE_OP_RETURN, &payload[1..])
}

/// Decode a pipe return frame: returns (opcode, key, result_bytes) or None if
/// malformed. Accepts the full length-prefixed frame bytes.
pub fn decode_pipe_return(data: &[u8]) -> Option<(u8, u32, Vec<u8>)> {
    use ashfall_core::event::split_frames;
    let (mut frames, _) = split_frames(data);
    if frames.is_empty() {
        return None;
    }
    let frame = frames.remove(0);
    if frame.opcode != PIPE_OP_RETURN || frame.payload.len() < 4 {
        return None;
    }
    let key = u32::from_le_bytes([
        frame.payload[0],
        frame.payload[1],
        frame.payload[2],
        frame.payload[3],
    ]);
    Some((frame.opcode, key, frame.payload[4..].to_vec()))
}

const PIPE_LENGTH: usize = 2048;

/// Run the TCP server loop. Blocks until shutdown signaled.
pub fn run_server(addr: &str) {
    // The FO3 GOTY launcher holds 1771 while it spawns the game; the game's
    // bridge would lose the bind if it starts first. Retry for ~30s so the
    // game process wins the port once the launcher exits.
    let mut listener: Option<TcpListener> = None;
    for attempt in 0..60 {
        match TcpListener::bind(addr) {
            Ok(l) => {
                listener = Some(l);
                break;
            }
            Err(_) => {
                if attempt == 59 {
                    return;
                }
                std::thread::sleep(std::time::Duration::from_millis(500));
            }
        }
    }
    let listener = listener.expect("bind retried 60x");

    // 10 Hz NPC seen-set flush (STR cadence) — turns the collector's
    // processed-actor set into spawn/remove event frames for the client.
    crate::hooks::vaultmp::start_npc_flush_thread();

    // Accept one connection (single client)
    for stream in listener.incoming() {
        if !RUNNING.load(std::sync::atomic::Ordering::SeqCst) {
            break;
        }
        match stream {
            Ok(stream) => {
                // Server resilience: a panicking client handler must not take
                // down the accept loop (live session 2026-08-18k: an engine
                // call from the server thread killed the whole TCP thread).
                let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    handle_client(stream);
                }));
            }
            Err(_) => continue,
        }
    }
}

/// Handle a single client connection.
fn handle_client(mut stream: TcpStream) {
    let _ = stream.set_read_timeout(Some(std::time::Duration::from_millis(50)));
    let mut buf = [0u8; PIPE_LENGTH];
    let mut pending: Vec<u8> = Vec::new();

    while RUNNING.load(std::sync::atomic::Ordering::SeqCst) {
        match stream.read(&mut buf) {
            Ok(0) => break, // EOF, client disconnected
            Ok(n) => pending.extend_from_slice(&buf[..n]),
            Err(ref e)
                if matches!(
                    e.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) => {}
            Err(_) => break,
        }

        // Dispatch every complete frame in the buffer.
        let (frames, rest) = ashfall_core::event::split_frames(&pending);
        pending = rest.to_vec();
        let mut responses = Vec::new();
        for frame in frames {
            let response = dispatch(&frame);
            if !response.is_empty() {
                responses.extend_from_slice(&response);
            }
        }

        // Flush command responses + queued engine events.
        let mut out = std::mem::take(&mut responses);
        for event in take_event_frames() {
            out.extend_from_slice(&event);
        }
        if !out.is_empty() {
            let _ = stream.write_all(&out);
        }
    }
}

/// Parse and dispatch a pipe-protocol frame.
fn dispatch(frame: &ashfall_core::event::PipeFrame) -> Vec<u8> {
    let opcode = frame.opcode;
    let payload = &frame.payload;

    match opcode {
        PIPE_SYS_WAKEUP => {
            // Keep-alive / heartbeat, respond with same
            ashfall_core::event::encode_frame(PIPE_SYS_WAKEUP, &[])
        }
        PIPE_OP_COMMAND => {
            // [key:4B][func:4B][param_count:1B][params...]
            if payload.len() < 9 {
                return ashfall_core::event::encode_frame(PIPE_ERROR_CLOSE, &[]);
            }
            let key = u32::from_le_bytes([payload[0], payload[1], payload[2], payload[3]]);
            let func = u32::from_le_bytes([payload[4], payload[5], payload[6], payload[7]]);
            let _param_count = payload[8] as usize;
            let params = &payload[9..];

            let result = commands::execute(func, params);
            encode_pipe_return(key, &result)
        }
        PIPE_ERROR_CLOSE => ashfall_core::event::encode_frame(PIPE_ERROR_CLOSE, &[]),
        _ => {
            // Unknown opcode
            ashfall_core::event::encode_frame(PIPE_ERROR_CLOSE, &[])
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{LazyLock, Mutex};

    static TEST_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

    #[test]
    fn pipe_command_roundtrip() {
        let frame = encode_pipe_command(0x42, crate::commands::opcodes::OP_GET_POS, &[0xAA; 8]);
        // split_frames should yield one frame, opcode COMMAND, key 0x42
        let (mut frames, _) = ashfall_core::event::split_frames(&frame);
        assert_eq!(frames.len(), 1);
        let f = frames.remove(0);
        assert_eq!(f.opcode, PIPE_OP_COMMAND);
        // payload after the frame header: [func:4][count:1][params...]
        assert_eq!(f.payload[0..4], 0x42u32.to_le_bytes()); // key
        assert_eq!(
            f.payload[4..8],
            crate::commands::opcodes::OP_GET_POS.to_le_bytes()
        ); // func
        assert_eq!(f.payload[8], 8); // param count
        assert_eq!(&f.payload[9..], &[0xAA; 8]);
    }

    #[test]
    fn pipe_return_roundtrip() {
        let result = vec![1, 2, 3, 4];
        let frame = encode_pipe_return(0x7F, &result);
        let decoded = decode_pipe_return(&frame).expect("decodable");
        assert_eq!(decoded.0, PIPE_OP_RETURN);
        assert_eq!(decoded.1, 0x7F);
        assert_eq!(decoded.2, result);
    }

    #[test]
    fn decode_rejects_wrong_opcode_or_short() {
        let bad = ashfall_core::event::encode_frame(PIPE_OP_EVENT, &[0; 4]);
        assert!(decode_pipe_return(&bad).is_none());
        assert!(decode_pipe_return(&[]).is_none());
    }

    #[test]
    fn tracked_actor_set_add_remove_snapshot() {
        let _g = TEST_LOCK.lock().unwrap();
        reset_tracked();
        track_actor(0x100);
        track_actor(0x200);
        track_actor(0x100); // dedupe
        assert_eq!(tracked_actors(), vec![0x100, 0x200]);
        untrack_actor(0x100);
        assert_eq!(tracked_actors(), vec![0x200]);
        reset_tracked();
        assert!(tracked_actors().is_empty());
    }

    #[test]
    fn player_state_due_throttles_to_10hz() {
        let _g = TEST_LOCK.lock().unwrap();
        // first call is due
        assert!(report_player_state_due().is_some());
        // immediate second call is throttled
        assert!(report_player_state_due().is_none());
    }
}
