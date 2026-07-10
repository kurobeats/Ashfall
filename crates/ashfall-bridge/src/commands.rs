//! Command dispatcher — maps opcodes to engine calls.
//!
//! Ported from vaultmp-extended's API/Interface command set.
//! Each opcode calls a hook function that reads/writes game state.

/// Command opcodes (subset matching original vaultmp Interface/API opcodes).
pub mod opcodes {
    pub const OP_GET_POS: u32            = 0x0001;
    pub const OP_SET_POS: u32            = 0x0002;
    pub const OP_GET_ANGLE: u32          = 0x0003;
    pub const OP_SET_ANGLE: u32          = 0x0004;
    pub const OP_GET_CELL: u32           = 0x0005;
    pub const OP_SET_CELL: u32           = 0x0006;
    pub const OP_GET_ACTOR_STATE: u32    = 0x0007;
    pub const OP_GET_ACTOR_VALUE: u32    = 0x0008;
    pub const OP_SET_ACTOR_VALUE: u32    = 0x0009;
    pub const OP_GET_CONTROL: u32        = 0x000A;
    pub const OP_SET_CONTROL: u32        = 0x000B;
    pub const OP_GET_ACTIVATE: u32       = 0x000C;
    pub const OP_FIRE_WEAPON: u32        = 0x000D;
    pub const OP_GET_NAME: u32           = 0x000E;
    pub const OP_SET_NAME: u32           = 0x000F;
    pub const OP_GET_ENABLED: u32        = 0x0010;
    pub const OP_SET_ENABLED: u32        = 0x0011;
    pub const OP_GET_LOCK: u32           = 0x0012;
    pub const OP_SET_LOCK: u32           = 0x0013;
    pub const OP_MOVE_TO: u32            = 0x0014;
    pub const OP_PLAY_SOUND: u32         = 0x0015;
    pub const OP_PLACE_AT_ME: u32        = 0x0016;
    pub const OP_GET_BASE: u32           = 0x0017;
    // ... ~70 more opcodes (ponytail: add as needed)
}

/// Execute a command by opcode. Returns raw result bytes for pipe protocol.
pub fn execute(func: u32, params: &[u8]) -> Vec<u8> {
    use opcodes::*;
    match func {
        OP_GET_POS => {
            // params: refID (4 bytes LE)
            if params.len() < 4 { return vec![]; }
            let ref_id = u32::from_le_bytes([params[0], params[1], params[2], params[3]]);
            let pos = crate::hooks::get_pos(ref_id);
            // Return: [X:4B][Y:4B][Z:4B]
            let mut out = Vec::with_capacity(12);
            out.extend_from_slice(&pos[0].to_le_bytes());
            out.extend_from_slice(&pos[1].to_le_bytes());
            out.extend_from_slice(&pos[2].to_le_bytes());
            out
        }
        OP_SET_POS => {
            // params: refID(4B) X(4B) Y(4B) Z(4B)
            if params.len() < 16 { return vec![]; }
            let ref_id = u32::from_le_bytes([params[0], params[1], params[2], params[3]]);
            let x = f32::from_le_bytes([params[4], params[5], params[6], params[7]]);
            let y = f32::from_le_bytes([params[8], params[9], params[10], params[11]]);
            let z = f32::from_le_bytes([params[12], params[13], params[14], params[15]]);
            crate::hooks::set_pos(ref_id, [x, y, z]);
            vec![1] // success
        }
        OP_GET_ANGLE => {
            if params.len() < 4 { return vec![]; }
            let ref_id = u32::from_le_bytes([params[0], params[1], params[2], params[3]]);
            let angle = crate::hooks::get_angle(ref_id);
            let mut out = Vec::with_capacity(12);
            out.extend_from_slice(&angle[0].to_le_bytes());
            out.extend_from_slice(&angle[1].to_le_bytes());
            out.extend_from_slice(&angle[2].to_le_bytes());
            out
        }
        OP_SET_ANGLE => {
            if params.len() < 16 { return vec![]; }
            let ref_id = u32::from_le_bytes([params[0], params[1], params[2], params[3]]);
            let x = f32::from_le_bytes([params[4], params[5], params[6], params[7]]);
            let y = f32::from_le_bytes([params[8], params[9], params[10], params[11]]);
            let z = f32::from_le_bytes([params[12], params[13], params[14], params[15]]);
            crate::hooks::set_angle(ref_id, [x, y, z]);
            vec![1]
        }
        // ponytail: add remaining ~70 opcodes when hook implementations exist
        _ => {
            // Unknown opcode → return empty (caller handles timeout)
            vec![]
        }
    }
}
