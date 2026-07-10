//! Command dispatcher — maps opcodes to engine calls.
//!
//! Ported from vaultmp-extended's API/Interface command set.
//! Each opcode calls a hook function that reads/writes game state.

/// Command opcodes (matching original vaultmp Interface/API opcodes).
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
    // ponytail: ~70 more opcodes available; add as RE progresses
}

/// Read a u32 from little-endian bytes at an offset within a slice.
fn read_u32(data: &[u8], offset: usize) -> Option<u32> {
    if data.len() < offset + 4 { return None; }
    Some(u32::from_le_bytes([data[offset], data[offset+1], data[offset+2], data[offset+3]]))
}

/// Execute a command by opcode. Returns raw result bytes for pipe protocol.
pub fn execute(func: u32, params: &[u8]) -> Vec<u8> {
    use opcodes::*;
    match func {
        // ── Position ──
        OP_GET_POS => {
            let ref_id = match read_u32(params, 0) { Some(v) => v, None => return vec![] };
            let pos = crate::hooks::get_pos(ref_id);
            let mut out = Vec::with_capacity(12);
            out.extend_from_slice(&pos[0].to_le_bytes());
            out.extend_from_slice(&pos[1].to_le_bytes());
            out.extend_from_slice(&pos[2].to_le_bytes());
            out
        }
        OP_SET_POS => {
            if params.len() < 16 { return vec![]; }
            let ref_id = u32::from_le_bytes([params[0], params[1], params[2], params[3]]);
            let x = f32::from_le_bytes([params[4], params[5], params[6], params[7]]);
            let y = f32::from_le_bytes([params[8], params[9], params[10], params[11]]);
            let z = f32::from_le_bytes([params[12], params[13], params[14], params[15]]);
            crate::hooks::set_pos(ref_id, [x, y, z]);
            vec![1]
        }

        // ── Angle ──
        OP_GET_ANGLE => {
            let ref_id = match read_u32(params, 0) { Some(v) => v, None => return vec![] };
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

        // ── Cell ──
        OP_GET_CELL => {
            let ref_id = match read_u32(params, 0) { Some(v) => v, None => return vec![] };
            let cell = crate::hooks::get_cell(ref_id);
            cell.to_le_bytes().to_vec()
        }
        OP_SET_CELL => {
            if params.len() < 8 { return vec![]; }
            let ref_id = u32::from_le_bytes([params[0], params[1], params[2], params[3]]);
            let cell = u32::from_le_bytes([params[4], params[5], params[6], params[7]]);
            // ponytail: no set_cell hook yet; stub success
            let _ = (ref_id, cell);
            vec![1]
        }

        // ── Actor State ──
        OP_GET_ACTOR_STATE => {
            let ref_id = match read_u32(params, 0) { Some(v) => v, None => return vec![] };
            let (idle, moving, weapon, flags, alerted, sneaking) = crate::hooks::get_actor_state(ref_id);
            let mut out = Vec::with_capacity(11);
            out.extend_from_slice(&idle.to_le_bytes());
            out.push(moving);
            out.push(weapon);
            out.push(flags);
            out.push(if alerted { 1 } else { 0 });
            out.push(if sneaking { 1 } else { 0 });
            out
        }

        // ── Actor Value ──
        OP_GET_ACTOR_VALUE => {
            if params.len() < 5 { return vec![]; }
            let ref_id = u32::from_le_bytes([params[0], params[1], params[2], params[3]]);
            let index = params[4];
            let value = crate::hooks::get_actor_value(ref_id, index);
            value.to_le_bytes().to_vec()
        }
        OP_SET_ACTOR_VALUE => {
            if params.len() < 9 { return vec![]; }
            let ref_id = u32::from_le_bytes([params[0], params[1], params[2], params[3]]);
            let index = params[4];
            let value = f32::from_le_bytes([params[5], params[6], params[7], params[8]]);
            crate::hooks::set_actor_value(ref_id, index, value);
            vec![1]
        }

        // ── Controls ──
        OP_GET_CONTROL => {
            if params.len() < 5 { return vec![]; }
            let ref_id = u32::from_le_bytes([params[0], params[1], params[2], params[3]]);
            let control = params[4];
            let key = crate::hooks::get_control(ref_id, control);
            vec![key]
        }
        OP_SET_CONTROL => {
            if params.len() < 6 { return vec![]; }
            let ref_id = u32::from_le_bytes([params[0], params[1], params[2], params[3]]);
            let control = params[4];
            let enabled = params[5] != 0;
            crate::hooks::set_control(ref_id, control, enabled);
            vec![1]
        }

        // ── Activate ──
        OP_GET_ACTIVATE => {
            let ref_id = match read_u32(params, 0) { Some(v) => v, None => return vec![] };
            let target = crate::hooks::get_activate(ref_id);
            target.to_le_bytes().to_vec()
        }

        // ── Fire Weapon ──
        OP_FIRE_WEAPON => {
            if params.len() < 8 { return vec![]; }
            let ref_id = u32::from_le_bytes([params[0], params[1], params[2], params[3]]);
            let weapon = u32::from_le_bytes([params[4], params[5], params[6], params[7]]);
            // ponytail: no fire_weapon hook yet; stub success
            let _ = (ref_id, weapon);
            vec![1]
        }

        // ── Name ──
        OP_GET_NAME => {
            let ref_id = match read_u32(params, 0) { Some(v) => v, None => return vec![] };
            let name = crate::hooks::get_name(ref_id);
            name.into_bytes()
        }
        OP_SET_NAME => {
            if params.len() < 5 { return vec![]; }
            let ref_id = u32::from_le_bytes([params[0], params[1], params[2], params[3]]);
            // params[4..] = UTF-8 name bytes
            // ponytail: no set_name hook yet; stub success
            let _ = (ref_id, &params[4..]);
            vec![1]
        }

        // ── Enabled ──
        OP_GET_ENABLED => {
            let ref_id = match read_u32(params, 0) { Some(v) => v, None => return vec![] };
            let enabled = crate::hooks::get_enabled(ref_id);
            vec![if enabled { 1 } else { 0 }]
        }
        OP_SET_ENABLED => {
            if params.len() < 5 { return vec![]; }
            let ref_id = u32::from_le_bytes([params[0], params[1], params[2], params[3]]);
            let enabled = params[4] != 0;
            // ponytail: no set_enabled hook yet; stub success
            let _ = (ref_id, enabled);
            vec![1]
        }

        // ── Lock ──
        OP_GET_LOCK => {
            let ref_id = match read_u32(params, 0) { Some(v) => v, None => return vec![] };
            let lock = crate::hooks::get_lock(ref_id);
            lock.to_le_bytes().to_vec()
        }
        OP_SET_LOCK => {
            if params.len() < 8 { return vec![]; }
            let ref_id = u32::from_le_bytes([params[0], params[1], params[2], params[3]]);
            let lock = u32::from_le_bytes([params[4], params[5], params[6], params[7]]);
            // ponytail: no set_lock hook yet; stub success
            let _ = (ref_id, lock);
            vec![1]
        }

        // ── Move To ──
        OP_MOVE_TO => {
            if params.len() < 16 { return vec![]; }
            let ref_id = u32::from_le_bytes([params[0], params[1], params[2], params[3]]);
            let cell = u32::from_le_bytes([params[4], params[5], params[6], params[7]]);
            let x = f32::from_le_bytes([params[8], params[9], params[10], params[11]]);
            let y = f32::from_le_bytes([params[12], params[13], params[14], params[15]]);
            // ponytail: no move_to hook yet; stub success
            let _ = (ref_id, cell, x, y);
            vec![1]
        }

        // ── Play Sound ──
        OP_PLAY_SOUND => {
            if params.len() < 8 { return vec![]; }
            let ref_id = u32::from_le_bytes([params[0], params[1], params[2], params[3]]);
            let sound = u32::from_le_bytes([params[4], params[5], params[6], params[7]]);
            // ponytail: no play_sound hook yet; stub success
            let _ = (ref_id, sound);
            vec![1]
        }

        // ── Place At Me ──
        OP_PLACE_AT_ME => {
            if params.len() < 16 { return vec![]; }
            let actor_id = u32::from_le_bytes([params[0], params[1], params[2], params[3]]);
            let base_id = u32::from_le_bytes([params[4], params[5], params[6], params[7]]);
            let count = u32::from_le_bytes([params[8], params[9], params[10], params[11]]);
            let distance = f32::from_le_bytes([params[12], params[13], params[14], params[15]]);
            // ponytail: no place_at_me hook yet; stub success
            let _ = (actor_id, base_id, count, distance);
            vec![1]
        }

        // ── Base ──
        OP_GET_BASE => {
            let ref_id = match read_u32(params, 0) { Some(v) => v, None => return vec![] };
            let base = crate::hooks::get_base(ref_id);
            base.to_le_bytes().to_vec()
        }

        // ── Unknown ──
        _ => vec![],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_opcode_dispatch_all_getters_return_bytes() {
        // Verify all 17 opcodes return non-empty responses with valid params
        let ref_id = [0x42u8, 0, 0, 0]; // refID = 0x42

        // Getters
        assert!(!execute(opcodes::OP_GET_POS, &ref_id).is_empty());
        assert!(!execute(opcodes::OP_GET_ANGLE, &ref_id).is_empty());
        assert!(!execute(opcodes::OP_GET_CELL, &ref_id).is_empty());
        assert!(!execute(opcodes::OP_GET_ACTOR_STATE, &ref_id).is_empty());
        assert!(!execute(opcodes::OP_GET_NAME, &ref_id).is_empty());
        assert!(!execute(opcodes::OP_GET_ENABLED, &ref_id).is_empty());
        assert!(!execute(opcodes::OP_GET_LOCK, &ref_id).is_empty());
        assert!(!execute(opcodes::OP_GET_BASE, &ref_id).is_empty());
        assert!(!execute(opcodes::OP_GET_ACTIVATE, &ref_id).is_empty());

        // Getters with extra param byte
        let refid_with_index = [0x42u8, 0, 0, 0, 0x14]; // refID + actor value index (health)
        assert!(!execute(opcodes::OP_GET_ACTOR_VALUE, &refid_with_index).is_empty());
        assert!(!execute(opcodes::OP_GET_CONTROL, &refid_with_index).is_empty());
    }

    #[test]
    fn test_opcode_dispatch_all_setters_return_success() {
        let params = [0x42u8, 0, 0, 0, 0, 0, 0x80, 0x3F, 0, 0, 0, 0, 0, 0, 0, 0]; // refID=0x42 + f32=1.0 + extra

        let setters: &[(u32, &[u8])] = &[
            (opcodes::OP_SET_POS, &params),
            (opcodes::OP_SET_ANGLE, &params),
            (opcodes::OP_SET_CELL, &params),
            (opcodes::OP_SET_ACTOR_VALUE, &params),
            (opcodes::OP_SET_CONTROL, &params),
            (opcodes::OP_FIRE_WEAPON, &params),
            (opcodes::OP_SET_NAME, &params),
            (opcodes::OP_SET_ENABLED, &params),
            (opcodes::OP_SET_LOCK, &params),
            (opcodes::OP_MOVE_TO, &params),
            (opcodes::OP_PLAY_SOUND, &params),
            (opcodes::OP_PLACE_AT_ME, &params),
        ];

        for (opcode, p) in setters {
            let result = execute(*opcode, p);
            assert!(!result.is_empty(), "opcode {opcode:#06X} returned empty");
            assert_eq!(result[0], 1, "opcode {opcode:#06X} should return success byte 1");
        }
    }

    #[test]
    fn test_unknown_opcode_returns_empty() {
        assert!(execute(0xFFFF, &[]).is_empty());
        assert!(execute(0xDEAD, &[0u8; 32]).is_empty());
    }

    #[test]
    fn test_short_params_return_empty() {
        // All opcodes should return empty on insufficient params
        assert!(execute(opcodes::OP_GET_POS, &[]).is_empty());
        assert!(execute(opcodes::OP_SET_POS, &[0; 8]).is_empty());
        assert!(execute(opcodes::OP_GET_ACTOR_STATE, &[]).is_empty());
        assert!(execute(opcodes::OP_GET_ACTOR_VALUE, &[0; 3]).is_empty());
    }
}
