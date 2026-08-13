//! TCP server inside the Wine/Proton process.
//!
//! Listens on loopback only (127.0.0.1:1771). Accepts a single connection
//! from the native Linux ashfall-client. Decodes pipe-protocol commands
//! and dispatches to the command handler.

use std::collections::VecDeque;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{LazyLock, Mutex};

use crate::RUNNING;
use crate::commands;

/// Pipe protocol opcodes (match original vaultmp.hpp).
pub const PIPE_SYS_WAKEUP: u8    = 0x01;
pub const PIPE_OP_COMMAND: u8    = 0x02;
pub const PIPE_OP_RETURN: u8     = 0x03;
pub const PIPE_OP_RETURN_BIG: u8 = 0x04; // reserved for large responses
pub const PIPE_OP_RETURN_RAW: u8 = 0x05; // reserved for raw binary
pub const PIPE_ERROR_CLOSE: u8   = 0x06;
pub const PIPE_OP_EVENT: u8      = 0x07; // engine → client event frame

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
    let key = u32::from_le_bytes([frame.payload[0], frame.payload[1], frame.payload[2], frame.payload[3]]);
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

    // Accept one connection (single client)
    for stream in listener.incoming() {
        if !RUNNING.load(std::sync::atomic::Ordering::SeqCst) {
            break;
        }
        match stream {
            Ok(stream) => {
                handle_client(stream);
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
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
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
        PIPE_ERROR_CLOSE => {
            ashfall_core::event::encode_frame(PIPE_ERROR_CLOSE, &[])
        }
        _ => {
            // Unknown opcode
            ashfall_core::event::encode_frame(PIPE_ERROR_CLOSE, &[])
        }
    }
}
