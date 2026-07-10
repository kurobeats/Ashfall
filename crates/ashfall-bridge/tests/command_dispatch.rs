//! Command dispatch tests — verify all 17 opcodes execute without panic.

use ashfall_bridge::commands::{self, opcodes};

/// All 17 command opcodes defined in the dispatcher.
const ALL_OPCODES: [u32; 17] = [
    opcodes::OP_GET_POS,
    opcodes::OP_SET_POS,
    opcodes::OP_GET_ANGLE,
    opcodes::OP_SET_ANGLE,
    opcodes::OP_GET_CELL,
    opcodes::OP_SET_CELL,
    opcodes::OP_GET_ACTOR_STATE,
    opcodes::OP_GET_ACTOR_VALUE,
    opcodes::OP_SET_ACTOR_VALUE,
    opcodes::OP_GET_CONTROL,
    opcodes::OP_SET_CONTROL,
    opcodes::OP_GET_ACTIVATE,
    opcodes::OP_FIRE_WEAPON,
    opcodes::OP_GET_NAME,
    opcodes::OP_SET_NAME,
    opcodes::OP_GET_ENABLED,
    opcodes::OP_SET_ENABLED,
];

#[test]
fn test_all_17_opcodes_defined() {
    // Verify no duplicates
    for i in 0..ALL_OPCODES.len() {
        for j in i + 1..ALL_OPCODES.len() {
            assert_ne!(ALL_OPCODES[i], ALL_OPCODES[j]);
        }
    }
    // Verify opcode values are 0x0001 through 0x0011
    assert_eq!(opcodes::OP_GET_POS, 0x0001);
    assert_eq!(opcodes::OP_SET_NAME, 0x000F);
    assert_eq!(opcodes::OP_GET_ENABLED, 0x0010);
    assert_eq!(opcodes::OP_SET_ENABLED, 0x0011);
}

#[test]
fn test_all_17_opcodes_dont_panic() {
    // Every opcode should produce a result (even if stub zeros)
    let params_4b = 1u32.to_le_bytes(); // 4-byte ref_id param
    let params_16b = [0u8; 16]; // 16 bytes for set operations

    for opcode in &ALL_OPCODES {
        // Try with 4-byte params (GET operations)
        let result = commands::execute(*opcode, &params_4b);
        // All defined GET ops return data or empty; none should panic
        // SET ops with 4b return empty (short params)
        drop(result);

        // Try with 16-byte params (SET operations)
        let result = commands::execute(*opcode, &params_16b);
        drop(result);
    }
}

#[test]
fn test_opcode_params_set_pos() {
    // OP_SET_POS with full 16-byte payload returns success byte
    let mut params = vec![0u8; 16];
    // ref_id = 42
    params[0..4].copy_from_slice(&42u32.to_le_bytes());
    // pos = (1.0, 2.0, 3.0)
    params[4..8].copy_from_slice(&1.0f32.to_le_bytes());
    params[8..12].copy_from_slice(&2.0f32.to_le_bytes());
    params[12..16].copy_from_slice(&3.0f32.to_le_bytes());

    let result = commands::execute(opcodes::OP_SET_POS, &params);
    assert_eq!(result, vec![1]); // success byte
}

#[test]
fn test_opcode_params_get_pos() {
    // OP_GET_POS with 4-byte ref_id returns 12 bytes (3 f32s)
    let params = 1u32.to_le_bytes();
    let result = commands::execute(opcodes::OP_GET_POS, &params);
    assert_eq!(result.len(), 12);

    // Parse as f32s (stub returns 0.0, 0.0, 0.0)
    let x = f32::from_le_bytes([result[0], result[1], result[2], result[3]]);
    let y = f32::from_le_bytes([result[4], result[5], result[6], result[7]]);
    let z = f32::from_le_bytes([result[8], result[9], result[10], result[11]]);
    assert_eq!(x, 0.0);
    assert_eq!(y, 0.0);
    assert_eq!(z, 0.0);
}

#[test]
fn test_short_params_rejected() {
    // GET_POS with 2-byte payload → returns empty (short params)
    let result = commands::execute(opcodes::OP_GET_POS, &[0x00, 0x00]);
    assert!(result.is_empty());

    // SET_POS with 2-byte payload → returns empty (short params)
    let result = commands::execute(opcodes::OP_SET_POS, &[0x00, 0x00]);
    assert!(result.is_empty());

    // GET_ANGLE with 2-byte payload → returns empty
    let result = commands::execute(opcodes::OP_GET_ANGLE, &[0x00, 0x00]);
    assert!(result.is_empty());
}

#[test]
fn test_unknown_opcode() {
    // Unknown opcode 0xDEAD → returns empty
    let result = commands::execute(0xDEAD, &[]);
    assert!(result.is_empty());
}

#[test]
fn test_opcode_bounds() {
    // All defined ops are in range 0x0001..=0x0017
    for op in &ALL_OPCODES {
        assert!(*op >= 0x0001 && *op <= 0x0017);
    }
}
