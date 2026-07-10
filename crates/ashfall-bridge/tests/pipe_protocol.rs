//! Pipe protocol round-trip tests.
//!
//! Wire format (matching vaultmp pipe protocol):
//!   Request:  [PIPE_OP_COMMAND(1B)][key(4B LE)][func(4B LE)][param_count(1B)][params...]
//!   Response: [PIPE_OP_RETURN(1B)][key(4B LE)][result...]

use ashfall_bridge::network;

#[test]
fn test_pipe_wakeup() {
    // PIPE_SYS_WAKEUP is just 0x01
    assert_eq!(network::PIPE_SYS_WAKEUP, 0x01);
    assert!(network::PIPE_SYS_WAKEUP != network::PIPE_ERROR_CLOSE);
}

#[test]
fn test_pipe_error_close() {
    assert_eq!(network::PIPE_ERROR_CLOSE, 0x06);
    assert!(network::PIPE_ERROR_CLOSE != network::PIPE_OP_RETURN);
}

#[test]
fn test_pipe_command_roundtrip() {
    // Encode GET_POS command for ref_id = 0x42
    let key = 1u32;
    let func = 0x0001u32; // OP_GET_POS
    let params = 0x42u32.to_le_bytes().to_vec();

    let cmd = network::encode_pipe_command(key, func, &params);

    // Verify structure
    assert_eq!(cmd[0], network::PIPE_OP_COMMAND);
    assert_eq!(u32::from_le_bytes([cmd[1], cmd[2], cmd[3], cmd[4]]), key);
    assert_eq!(u32::from_le_bytes([cmd[5], cmd[6], cmd[7], cmd[8]]), func);
    assert_eq!(cmd[9], 4); // param_count = 4 bytes for ref_id
    assert_eq!(&cmd[10..14], &0x42u32.to_le_bytes());

    // Encode response
    let result = vec![0u8; 12]; // GET_POS returns 12 bytes (3 f32s)
    let response = network::encode_pipe_return(key, &result);

    assert_eq!(response[0], network::PIPE_OP_RETURN);
    assert_eq!(u32::from_le_bytes([response[1], response[2], response[3], response[4]]), key);
    assert_eq!(response.len(), 1 + 4 + 12);
}

#[test]
fn test_pipe_return_encoding() {
    let key = 42u32;
    let result = vec![1, 2, 3, 4]; // 4-byte success result

    let encoded = network::encode_pipe_return(key, &result);

    // Decode
    let decoded = network::decode_pipe_return(&encoded);
    assert!(decoded.is_some());
    let (opcode, decoded_key, decoded_result) = decoded.unwrap();

    assert_eq!(opcode, network::PIPE_OP_RETURN);
    assert_eq!(decoded_key, key);
    assert_eq!(decoded_result, result);
}

#[test]
fn test_pipe_return_decode_short() {
    // 5 bytes is too short (need 1 opcode + 4 key)
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
    // GET_POS with ref_id=1 returns [0.0; 3] from stub hooks
    let key = 5u32;
    let func = 0x0001u32; // OP_GET_POS
    let params = 1u32.to_le_bytes().to_vec();

    let cmd = network::encode_pipe_command(key, func, &params);

    // Parse command like dispatch() does
    let opcode = cmd[0];
    assert_eq!(opcode, network::PIPE_OP_COMMAND);

    let parsed_key = u32::from_le_bytes([cmd[1], cmd[2], cmd[3], cmd[4]]);
    assert_eq!(parsed_key, key);

    let parsed_func = u32::from_le_bytes([cmd[5], cmd[6], cmd[7], cmd[8]]);
    assert_eq!(parsed_func, func);

    let param_count = cmd[9] as usize;
    assert_eq!(param_count, 4);

    let parsed_params = &cmd[10..];
    assert_eq!(parsed_params, &1u32.to_le_bytes());

    // Execute
    let result = ashfall_bridge::commands::execute(parsed_func, parsed_params);

    // GET_POS stub returns [0.0; 3] = 12 zero bytes
    assert_eq!(result.len(), 12);

    // Encode response
    let response = network::encode_pipe_return(key, &result);

    // Decode
    let decoded = network::decode_pipe_return(&response);
    assert!(decoded.is_some());
    let (op, dk, dr) = decoded.unwrap();
    assert_eq!(op, network::PIPE_OP_RETURN);
    assert_eq!(dk, key);
    assert_eq!(dr, result);
}
