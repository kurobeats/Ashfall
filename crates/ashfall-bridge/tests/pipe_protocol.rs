//! Pipe protocol round-trip tests.
//!
//! Wire format (length-prefixed frames so responses and events share the
//! stream unambiguously — see `ashfall_core::event`):
//! ```text
//! Frame:    [len:2 LE][opcode:1][payload...]
//! Command:  payload = [key:4B LE][func:4B LE][param_count:1B][params...]
//! Response: payload = [key:4B LE][result...]
//! Event:    payload = [event_type:4B LE][event data...]
//! ```

use ashfall_bridge::network;

#[test]
fn test_pipe_wakeup() {
    // PIPE_SYS_WAKEUP is just 0x01
    assert_eq!(network::PIPE_SYS_WAKEUP, 0x01);
    // distinct (compile-time constants)
    const _: () = assert!(network::PIPE_SYS_WAKEUP != network::PIPE_ERROR_CLOSE);
}

#[test]
fn test_pipe_error_close() {
    assert_eq!(network::PIPE_ERROR_CLOSE, 0x06);
    const _: () = assert!(network::PIPE_ERROR_CLOSE != network::PIPE_OP_RETURN);
}

#[test]
fn test_pipe_command_roundtrip() {
    // Encode GET_POS command for ref_id = 0x42
    let key = 1u32;
    let func = 0x0001u32; // OP_GET_POS
    let params = 0x42u32.to_le_bytes().to_vec();

    let cmd = network::encode_pipe_command(key, func, &params);

    // Verify frame structure: [len][opcode][key][func][count][params]
    assert_eq!(cmd[2], network::PIPE_OP_COMMAND);
    let (frames, rest) = ashfall_core::event::split_frames(&cmd);
    assert!(rest.is_empty(), "one complete frame");
    assert_eq!(frames.len(), 1);
    let payload = &frames[0].payload;
    assert_eq!(
        u32::from_le_bytes([payload[0], payload[1], payload[2], payload[3]]),
        key
    );
    assert_eq!(
        u32::from_le_bytes([payload[4], payload[5], payload[6], payload[7]]),
        func
    );
    assert_eq!(payload[8], 4); // param_count = 4 bytes for ref_id
    assert_eq!(&payload[9..13], &0x42u32.to_le_bytes());

    // Encode response
    let result = vec![0u8; 12]; // GET_POS returns 12 bytes (3 f32s)
    let response = network::encode_pipe_return(key, &result);
    let (frames, _) = ashfall_core::event::split_frames(&response);
    assert_eq!(frames[0].opcode, network::PIPE_OP_RETURN);
    assert_eq!(response.len(), 2 + 1 + 4 + 12);
}

#[test]
fn test_pipe_return_encoding() {
    let key = 42u32;
    let result = vec![1, 2, 3, 4]; // 4-byte success result

    let encoded = network::encode_pipe_return(key, &result);
    let decoded = network::decode_pipe_return(&encoded);
    assert!(decoded.is_some());
    let (opcode, decoded_key, decoded_result) = decoded.unwrap();

    assert_eq!(opcode, network::PIPE_OP_RETURN);
    assert_eq!(decoded_key, key);
    assert_eq!(decoded_result, result);
}

#[test]
fn test_pipe_return_decode_short() {
    // Truncated frame (no length prefix) → None.
    assert!(network::decode_pipe_return(&[0x03, 0x01, 0x00]).is_none());
}

#[test]
fn test_pipe_return_decode_empty_result() {
    let key = 99u32;
    let encoded = network::encode_pipe_return(key, &[]);
    let decoded = network::decode_pipe_return(&encoded);
    assert!(decoded.is_some());
    let (opcode, decoded_key, decoded_result) = decoded.unwrap();

    assert_eq!(opcode, network::PIPE_OP_RETURN);
    assert_eq!(decoded_key, key);
    assert!(decoded_result.is_empty());
}

#[test]
fn test_pipe_command_all_opcodes() {
    // Verify all opcode constants are distinct
    let ops = [
        network::PIPE_SYS_WAKEUP,
        network::PIPE_OP_COMMAND,
        network::PIPE_OP_RETURN,
        network::PIPE_OP_RETURN_BIG,
        network::PIPE_OP_RETURN_RAW,
        network::PIPE_ERROR_CLOSE,
        network::PIPE_OP_EVENT,
    ];
    for i in 0..ops.len() {
        for j in i + 1..ops.len() {
            assert_ne!(ops[i], ops[j], "opcodes at {i} and {j} must differ");
        }
    }
}

#[test]
fn test_pipe_command_e2e_get_pos() {
    // Full end-to-end: encode GET_POS → dispatch through execute → decode response
    let key = 5u32;
    let func = 0x0001u32; // OP_GET_POS
    let params = 1u32.to_le_bytes().to_vec();

    let cmd = network::encode_pipe_command(key, func, &params);
    let (frames, _) = ashfall_core::event::split_frames(&cmd);
    let payload = &frames[0].payload;

    let parsed_key = u32::from_le_bytes([payload[0], payload[1], payload[2], payload[3]]);
    assert_eq!(parsed_key, key);
    let parsed_func = u32::from_le_bytes([payload[4], payload[5], payload[6], payload[7]]);
    assert_eq!(parsed_func, func);
    let param_count = payload[8] as usize;
    assert_eq!(param_count, 4);
    let parsed_params = &payload[9..];
    assert_eq!(parsed_params, &1u32.to_le_bytes());

    // Execute
    let result = ashfall_bridge::commands::execute(parsed_func, parsed_params);
    // GET_POS stub returns [0.0; 3] = 12 zero bytes
    assert_eq!(result.len(), 12);

    // Encode response
    let response = network::encode_pipe_return(key, &result);
    let decoded = network::decode_pipe_return(&response);
    assert!(decoded.is_some());
    let (op, dk, dr) = decoded.unwrap();
    assert_eq!(op, network::PIPE_OP_RETURN);
    assert_eq!(dk, key);
    assert_eq!(dr, result);
}

#[test]
fn test_report_player_state_emits_event_frame() {
    // The debug reporter queues an EVENT_PLAYER_STATE frame (the client's
    // coop loop consumes it). On non-Windows the getters return defaults, but
    // the frame structure is the contract.
    let frame = network::report_player_state();
    assert_eq!(frame[2], network::PIPE_OP_EVENT);
    let (frames, _) = ashfall_core::event::split_frames(&frame);
    assert_eq!(frames.len(), 1);
    let (event_type, data) =
        ashfall_core::event::decode_event(&frames[0].payload).expect("event header");
    assert_eq!(event_type, ashfall_core::event::EVENT_PLAYER_STATE);
    let e = ashfall_core::event::decode_player_state(data).expect("player state");
    assert_eq!(e.ref_id, network::LOCAL_PLAYER_REF);
    assert_eq!(e.pos, [0.0; 3], "stub getters return defaults");
}

#[test]
fn test_command_queues_duplicate_event() {
    // execute(OP_REPORT_PLAYER_STATE) returns the event frame bytes as its
    // result (the response echoes what was queued).
    let result = ashfall_bridge::commands::execute(
        ashfall_bridge::commands::opcodes::OP_REPORT_PLAYER_STATE,
        &[],
    );
    assert_eq!(
        result[2],
        network::PIPE_OP_EVENT,
        "response is the event frame"
    );
}

#[test]
fn test_bridge_server_pushes_event_over_tcp() {
    use std::io::{Read, Write};
    use std::net::{TcpListener, TcpStream};

    // Pick a free port, then let run_server bind it.
    let probe = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = probe.local_addr().unwrap().port();
    drop(probe);

    let addr = format!("127.0.0.1:{port}");
    let server_addr = addr.clone();
    std::thread::spawn(move || network::run_server(&server_addr));
    std::thread::sleep(std::time::Duration::from_millis(200));

    let mut stream = TcpStream::connect(&addr).unwrap();
    // Ask the bridge to report its player state: the response echoes the
    // event frame AND the event is queued for the connection loop to push.
    let cmd = network::encode_pipe_command(
        1,
        ashfall_bridge::commands::opcodes::OP_REPORT_PLAYER_STATE,
        &[],
    );
    stream.write_all(&cmd).unwrap();

    let mut buf = Vec::new();
    let mut chunk = [0u8; 4096];
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    while buf.len() < 64 && std::time::Instant::now() < deadline {
        match stream.read(&mut chunk) {
            Ok(0) => break,
            Ok(n) => buf.extend_from_slice(&chunk[..n]),
            Err(_) => break,
        }
    }

    let (frames, _) = ashfall_core::event::split_frames(&buf);
    let events: Vec<_> = frames
        .iter()
        .filter(|f| f.opcode == network::PIPE_OP_EVENT)
        .collect();
    assert!(!events.is_empty(), "event frame pushed to the client");
    let (event_type, data) =
        ashfall_core::event::decode_event(&events[0].payload).expect("event header");
    assert_eq!(event_type, ashfall_core::event::EVENT_PLAYER_STATE);
    let e = ashfall_core::event::decode_player_state(data).expect("player state");
    assert_eq!(e.ref_id, network::LOCAL_PLAYER_REF);
}

#[test]
fn test_reporter_throttles_to_10hz() {
    // Immediate samples flow; a sample within 100ms is skipped.
    let first = network::report_player_state_due();
    assert!(first.is_some(), "first sample due");
    let second = network::report_player_state_due();
    assert!(second.is_none(), "second sample within 100ms throttled");
    std::thread::sleep(std::time::Duration::from_millis(120));
    assert!(
        network::report_player_state_due().is_some(),
        "sample after window due"
    );
}

#[test]
fn test_track_and_sample_npc_state() {
    let _ = network::take_event_frames(); // drain leftovers from other tests
    network::reset_tracked();
    assert!(network::tracked_actors().is_empty());

    // Track two NPCs (dedup on repeat).
    network::track_actor(0x1234);
    network::track_actor(0x1234);
    network::track_actor(0x5678);
    let tracked = network::tracked_actors();
    assert_eq!(tracked.len(), 2);
    assert!(tracked.contains(&0x1234) && tracked.contains(&0x5678));

    // Sample → two EVENT_NPC_STATE frames queued (stub getters → defaults).
    network::sample_tracked();
    let frames = network::take_event_frames();
    assert_eq!(frames.len(), 2, "one frame per tracked actor");
    for frame in &frames {
        let (fs, _) = ashfall_core::event::split_frames(frame);
        let (event_type, data) = ashfall_core::event::decode_event(&fs[0].payload).expect("event");
        assert_eq!(event_type, ashfall_core::event::EVENT_NPC_STATE);
        let e = ashfall_core::event::decode_player_state(data).expect("state");
        assert!(tracked.contains(&e.ref_id));
    }

    // Untrack both → no more frames.
    network::untrack_actor(0x1234);
    network::untrack_actor(0x5678);
    network::sample_tracked();
    assert!(network::take_event_frames().is_empty());
    network::reset_tracked();
}

#[test]
fn test_track_untrack_commands() {
    network::reset_tracked();
    let result = ashfall_bridge::commands::execute(
        ashfall_bridge::commands::opcodes::OP_TRACK_ACTOR,
        &0x1234u32.to_le_bytes(),
    );
    assert_eq!(result, vec![1], "track acknowledged");
    assert!(network::tracked_actors().contains(&0x1234));

    let result = ashfall_bridge::commands::execute(
        ashfall_bridge::commands::opcodes::OP_UNTRACK_ACTOR,
        &0x1234u32.to_le_bytes(),
    );
    assert_eq!(result, vec![1]);
    assert!(network::tracked_actors().is_empty());
    network::reset_tracked();
}
