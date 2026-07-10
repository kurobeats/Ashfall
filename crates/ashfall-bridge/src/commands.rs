//! Command dispatcher — maps opcodes to engine calls.
//!
//! Ported from vaultmp-extended's API/Interface command set.
//! Each opcode calls a hook function that reads/writes game state.

/// Command opcodes (matching original vaultmp Interface/API opcodes).
pub mod opcodes {
    // ── Original 17 (Phase 10 initial) ──
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

    // ── Tier 1: Position + Actor State Sync ──
    pub const OP_GET_BASE_ACTOR_VALUE: u32 = 0x0018;
    pub const OP_GET_DEAD: u32             = 0x0019;
    pub const OP_SET_CURRENT_HEALTH: u32   = 0x001A;
    pub const OP_IS_MOVING: u32            = 0x001B;
    pub const OP_GET_PARENT_CELL: u32      = 0x001C;

    // ── Tier 2: Item / Inventory Sync ──
    pub const OP_EQUIP_ITEM: u32           = 0x001D;
    pub const OP_UNEQUIP_ITEM: u32         = 0x001E;
    pub const OP_ADD_ITEM: u32             = 0x001F;
    pub const OP_REMOVE_ITEM: u32          = 0x0020;
    pub const OP_REMOVE_ALL_ITEMS: u32     = 0x0021;
    pub const OP_GET_REF_COUNT: u32        = 0x0022;

    // ── Tier 3: Combat + Death ──
    pub const OP_KILL: u32                 = 0x0023;
    pub const OP_DAMAGE_ACTOR_VALUE: u32   = 0x0024;
    pub const OP_RESTORE_ACTOR_VALUE: u32  = 0x0025;
    pub const OP_FORCE_ACTOR_VALUE: u32    = 0x0026;

    // ── Tier 4: AI + World ──
    pub const OP_GET_COMBAT_TARGET: u32    = 0x0027;
    pub const OP_PLAY_GROUP: u32           = 0x0028;
    pub const OP_FORCE_WEATHER: u32        = 0x0029;
    pub const OP_SET_RESTRAINED: u32       = 0x002A;
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

        // ═══════════════════════════════════════════════════════
        // Tier 1: Position + Actor State Sync
        // ═══════════════════════════════════════════════════════

        OP_GET_BASE_ACTOR_VALUE => {
            if params.len() < 5 { return vec![]; }
            let ref_id = u32::from_le_bytes([params[0], params[1], params[2], params[3]]);
            let index = params[4];
            let value = crate::hooks::get_actor_base_value(ref_id, index);
            value.to_le_bytes().to_vec()
        }
        OP_GET_DEAD => {
            let ref_id = match read_u32(params, 0) { Some(v) => v, None => return vec![] };
            let dead = crate::hooks::is_dead(ref_id);
            vec![if dead { 1 } else { 0 }]
        }
        OP_SET_CURRENT_HEALTH => {
            if params.len() < 8 { return vec![]; }
            let ref_id = u32::from_le_bytes([params[0], params[1], params[2], params[3]]);
            let value = f32::from_le_bytes([params[4], params[5], params[6], params[7]]);
            crate::hooks::set_actor_value(ref_id, 0x14, value); // AV_HEALTH = 0x14
            vec![1]
        }
        OP_IS_MOVING => {
            let ref_id = match read_u32(params, 0) { Some(v) => v, None => return vec![] };
            let (_, moving, _, _, _, _) = crate::hooks::get_actor_state(ref_id);
            vec![if moving != 0 { 1 } else { 0 }]
        }
        OP_GET_PARENT_CELL => {
            let ref_id = match read_u32(params, 0) { Some(v) => v, None => return vec![] };
            let cell = crate::hooks::get_parent_cell(ref_id);
            cell.to_le_bytes().to_vec()
        }

        // ═══════════════════════════════════════════════════════
        // Tier 2: Item / Inventory Sync
        // ═══════════════════════════════════════════════════════

        OP_EQUIP_ITEM => {
            if params.len() < 13 { return vec![]; }
            let ref_id = u32::from_le_bytes([params[0], params[1], params[2], params[3]]);
            let item_id = u32::from_le_bytes([params[4], params[5], params[6], params[7]]);
            let equip_slot = u32::from_le_bytes([params[8], params[9], params[10], params[11]]);
            let prevent_removal = params[12];
            crate::hooks::equip_item(ref_id, item_id, equip_slot, prevent_removal);
            vec![1]
        }
        OP_UNEQUIP_ITEM => {
            if params.len() < 13 { return vec![]; }
            let ref_id = u32::from_le_bytes([params[0], params[1], params[2], params[3]]);
            let item_id = u32::from_le_bytes([params[4], params[5], params[6], params[7]]);
            let equip_slot = u32::from_le_bytes([params[8], params[9], params[10], params[11]]);
            let prevent_removal = params[12];
            crate::hooks::unequip_item(ref_id, item_id, equip_slot, prevent_removal);
            vec![1]
        }
        OP_ADD_ITEM => {
            if params.len() < 13 { return vec![]; }
            let ref_id = u32::from_le_bytes([params[0], params[1], params[2], params[3]]);
            let item_id = u32::from_le_bytes([params[4], params[5], params[6], params[7]]);
            let count = u32::from_le_bytes([params[8], params[9], params[10], params[11]]);
            let silent = params[12];
            crate::hooks::add_item(ref_id, item_id, count, silent);
            vec![1]
        }
        OP_REMOVE_ITEM => {
            if params.len() < 13 { return vec![]; }
            let ref_id = u32::from_le_bytes([params[0], params[1], params[2], params[3]]);
            let item_id = u32::from_le_bytes([params[4], params[5], params[6], params[7]]);
            let count = u32::from_le_bytes([params[8], params[9], params[10], params[11]]);
            let silent = params[12];
            crate::hooks::remove_item(ref_id, item_id, count, silent);
            vec![1]
        }
        OP_REMOVE_ALL_ITEMS => {
            if params.len() < 8 { return vec![]; }
            let ref_id = u32::from_le_bytes([params[0], params[1], params[2], params[3]]);
            let transfer_to = u32::from_le_bytes([params[4], params[5], params[6], params[7]]);
            crate::hooks::remove_all_items(ref_id, transfer_to);
            vec![1]
        }
        OP_GET_REF_COUNT => {
            let ref_id = match read_u32(params, 0) { Some(v) => v, None => return vec![] };
            let count = crate::hooks::get_ref_count(ref_id);
            count.to_le_bytes().to_vec()
        }

        // ═══════════════════════════════════════════════════════
        // Tier 3: Combat + Death
        // ═══════════════════════════════════════════════════════

        OP_KILL => {
            if params.len() < 10 { return vec![]; }
            let ref_id = u32::from_le_bytes([params[0], params[1], params[2], params[3]]);
            let killer_id = u32::from_le_bytes([params[4], params[5], params[6], params[7]]);
            let limb = params[8] as i8;
            let cause = params[9] as i8;
            crate::hooks::kill_actor(ref_id, killer_id, limb, cause);
            vec![1]
        }
        OP_DAMAGE_ACTOR_VALUE => {
            if params.len() < 9 { return vec![]; }
            let ref_id = u32::from_le_bytes([params[0], params[1], params[2], params[3]]);
            let index = params[4];
            let damage = f32::from_le_bytes([params[5], params[6], params[7], params[8]]);
            crate::hooks::damage_actor_value(ref_id, index, damage);
            vec![1]
        }
        OP_RESTORE_ACTOR_VALUE => {
            if params.len() < 9 { return vec![]; }
            let ref_id = u32::from_le_bytes([params[0], params[1], params[2], params[3]]);
            let index = params[4];
            let amount = f32::from_le_bytes([params[5], params[6], params[7], params[8]]);
            crate::hooks::restore_actor_value(ref_id, index, amount);
            vec![1]
        }
        OP_FORCE_ACTOR_VALUE => {
            if params.len() < 9 { return vec![]; }
            let ref_id = u32::from_le_bytes([params[0], params[1], params[2], params[3]]);
            let index = params[4];
            let value = f32::from_le_bytes([params[5], params[6], params[7], params[8]]);
            crate::hooks::force_actor_value(ref_id, index, value);
            vec![1]
        }

        // ═══════════════════════════════════════════════════════
        // Tier 4: AI + World
        // ═══════════════════════════════════════════════════════

        OP_GET_COMBAT_TARGET => {
            let ref_id = match read_u32(params, 0) { Some(v) => v, None => return vec![] };
            let target = crate::hooks::get_combat_target(ref_id);
            target.to_le_bytes().to_vec()
        }
        OP_PLAY_GROUP => {
            if params.len() < 12 { return vec![]; }
            let ref_id = u32::from_le_bytes([params[0], params[1], params[2], params[3]]);
            let group_id = u32::from_le_bytes([params[4], params[5], params[6], params[7]]);
            let flags = u32::from_le_bytes([params[8], params[9], params[10], params[11]]);
            crate::hooks::play_group(ref_id, group_id, flags);
            vec![1]
        }
        OP_FORCE_WEATHER => {
            if params.len() < 4 { return vec![]; }
            let weather_id = u32::from_le_bytes([params[0], params[1], params[2], params[3]]);
            crate::hooks::force_weather(weather_id);
            vec![1]
        }
        OP_SET_RESTRAINED => {
            if params.len() < 5 { return vec![]; }
            let ref_id = u32::from_le_bytes([params[0], params[1], params[2], params[3]]);
            let restrained = params[4];
            crate::hooks::set_restrained(ref_id, restrained);
            vec![1]
        }

        // ── Unknown ──
        _ => vec![],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── RefID constant for tests ──
    const REF_ID: [u8; 4] = [0x42, 0, 0, 0];

    #[test]
    fn test_original_17_opcodes_still_work() {
        // Original getters
        assert!(!execute(opcodes::OP_GET_POS, &REF_ID).is_empty());
        assert!(!execute(opcodes::OP_GET_ANGLE, &REF_ID).is_empty());
        assert!(!execute(opcodes::OP_GET_CELL, &REF_ID).is_empty());
        assert!(!execute(opcodes::OP_GET_ACTOR_STATE, &REF_ID).is_empty());
        assert!(!execute(opcodes::OP_GET_NAME, &REF_ID).is_empty());
        assert!(!execute(opcodes::OP_GET_ENABLED, &REF_ID).is_empty());
        assert!(!execute(opcodes::OP_GET_LOCK, &REF_ID).is_empty());
        assert!(!execute(opcodes::OP_GET_BASE, &REF_ID).is_empty());
        assert!(!execute(opcodes::OP_GET_ACTIVATE, &REF_ID).is_empty());

        let refid_with_index = [0x42u8, 0, 0, 0, 0x14];
        assert!(!execute(opcodes::OP_GET_ACTOR_VALUE, &refid_with_index).is_empty());
        assert!(!execute(opcodes::OP_GET_CONTROL, &refid_with_index).is_empty());
    }

    #[test]
    fn test_original_setters_return_success() {
        let params = [0x42u8, 0, 0, 0, 0, 0, 0x80, 0x3F, 0, 0, 0, 0, 0, 0, 0, 0];
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

    // ═══════════════════════════════════════════════════════
    // Tier 1 tests
    // ═══════════════════════════════════════════════════════

    #[test]
    fn test_tier1_get_base_actor_value() {
        let params = [0x42u8, 0, 0, 0, 0x14]; // refID=0x42, index=0x14 (health)
        let result = execute(opcodes::OP_GET_BASE_ACTOR_VALUE, &params);
        assert!(!result.is_empty());
        assert_eq!(result.len(), 4); // f32
    }

    #[test]
    fn test_tier1_get_dead() {
        let result = execute(opcodes::OP_GET_DEAD, &REF_ID);
        assert!(!result.is_empty());
        assert!(result[0] == 0 || result[0] == 1);
    }

    #[test]
    fn test_tier1_set_current_health() {
        let mut params = [0u8; 8];
        params[0] = 0x42; // refID = 0x42
        let health: f32 = 50.0;
        params[4..8].copy_from_slice(&health.to_le_bytes());
        let result = execute(opcodes::OP_SET_CURRENT_HEALTH, &params);
        assert!(!result.is_empty());
        assert_eq!(result[0], 1);
    }

    #[test]
    fn test_tier1_is_moving() {
        let result = execute(opcodes::OP_IS_MOVING, &REF_ID);
        assert!(!result.is_empty());
        assert!(result[0] == 0 || result[0] == 1);
    }

    #[test]
    fn test_tier1_get_parent_cell() {
        let result = execute(opcodes::OP_GET_PARENT_CELL, &REF_ID);
        assert!(!result.is_empty());
        assert_eq!(result.len(), 4); // u32
    }

    // ═══════════════════════════════════════════════════════
    // Tier 2 tests
    // ═══════════════════════════════════════════════════════

    #[test]
    fn test_tier2_equip_unequip_item() {
        let mut params = [0u8; 13];
        params[0] = 0x42; // refID
        params[4] = 0x01; // itemID
        params[12] = 0;    // prevent_removal
        let r1 = execute(opcodes::OP_EQUIP_ITEM, &params);
        assert_eq!(r1[0], 1);
        let r2 = execute(opcodes::OP_UNEQUIP_ITEM, &params);
        assert_eq!(r2[0], 1);
    }

    #[test]
    fn test_tier2_add_remove_item() {
        let mut params = [0u8; 13];
        params[0] = 0x42; // refID
        params[4] = 0x01; // itemID
        params[8] = 0x03; // count = 3
        params[12] = 1;   // silent
        let r1 = execute(opcodes::OP_ADD_ITEM, &params);
        assert_eq!(r1[0], 1);
        let r2 = execute(opcodes::OP_REMOVE_ITEM, &params);
        assert_eq!(r2[0], 1);
    }

    #[test]
    fn test_tier2_remove_all_items() {
        let mut params = [0u8; 8];
        params[0] = 0x42; // refID
        params[4] = 0x00; // transfer_to = 0
        let result = execute(opcodes::OP_REMOVE_ALL_ITEMS, &params);
        assert_eq!(result[0], 1);
    }

    #[test]
    fn test_tier2_get_ref_count() {
        let result = execute(opcodes::OP_GET_REF_COUNT, &REF_ID);
        assert!(!result.is_empty());
        assert_eq!(result.len(), 4); // u32
    }

    // ═══════════════════════════════════════════════════════
    // Tier 3 tests
    // ═══════════════════════════════════════════════════════

    #[test]
    fn test_tier3_kill() {
        let mut params = [0u8; 10];
        params[0] = 0x42; // refID
        params[4] = 0xFF; // killerID
        params[8] = 1;    // limb = 1 (head)
        params[9] = 2;    // cause = 2 (gun)
        let result = execute(opcodes::OP_KILL, &params);
        assert_eq!(result[0], 1);
    }

    #[test]
    fn test_tier3_damage_restore_force_actor_value() {
        let mut params = [0u8; 9];
        params[0] = 0x42; // refID
        params[4] = 0x14; // index = health
        let damage: f32 = 10.0;
        params[5..9].copy_from_slice(&damage.to_le_bytes());

        let r1 = execute(opcodes::OP_DAMAGE_ACTOR_VALUE, &params);
        assert_eq!(r1[0], 1);

        let rest: f32 = 5.0;
        params[5..9].copy_from_slice(&rest.to_le_bytes());
        let r2 = execute(opcodes::OP_RESTORE_ACTOR_VALUE, &params);
        assert_eq!(r2[0], 1);

        let force: f32 = 100.0;
        params[5..9].copy_from_slice(&force.to_le_bytes());
        let r3 = execute(opcodes::OP_FORCE_ACTOR_VALUE, &params);
        assert_eq!(r3[0], 1);
    }

    // ═══════════════════════════════════════════════════════
    // Tier 4 tests
    // ═══════════════════════════════════════════════════════

    #[test]
    fn test_tier4_get_combat_target() {
        let result = execute(opcodes::OP_GET_COMBAT_TARGET, &REF_ID);
        assert!(!result.is_empty());
        assert_eq!(result.len(), 4); // u32
    }

    #[test]
    fn test_tier4_play_group() {
        let mut params = [0u8; 12];
        params[0] = 0x42; // refID
        params[4] = 0x01; // group_id
        params[8] = 0x01; // flags
        let result = execute(opcodes::OP_PLAY_GROUP, &params);
        assert_eq!(result[0], 1);
    }

    #[test]
    fn test_tier4_force_weather() {
        let mut params = [0u8; 4];
        params[0] = 0x42; // weather_id
        let result = execute(opcodes::OP_FORCE_WEATHER, &params);
        assert_eq!(result[0], 1);
    }

    #[test]
    fn test_tier4_set_restrained() {
        let mut params = [0u8; 5];
        params[0] = 0x42; // refID
        params[4] = 1;    // restrained = true
        let result = execute(opcodes::OP_SET_RESTRAINED, &params);
        assert_eq!(result[0], 1);
    }

    // ── Edge cases ──

    #[test]
    fn test_unknown_opcode_returns_empty() {
        assert!(execute(0xFFFF, &[]).is_empty());
        assert!(execute(0xDEAD, &[0u8; 32]).is_empty());
    }

    #[test]
    fn test_short_params_return_empty() {
        assert!(execute(opcodes::OP_GET_POS, &[]).is_empty());
        assert!(execute(opcodes::OP_SET_POS, &[0; 8]).is_empty());
        assert!(execute(opcodes::OP_GET_ACTOR_STATE, &[]).is_empty());
        assert!(execute(opcodes::OP_GET_ACTOR_VALUE, &[0; 3]).is_empty());
        // New opcodes
        assert!(execute(opcodes::OP_GET_BASE_ACTOR_VALUE, &[0; 3]).is_empty());
        assert!(execute(opcodes::OP_SET_CURRENT_HEALTH, &[0; 4]).is_empty());
        assert!(execute(opcodes::OP_EQUIP_ITEM, &[0; 8]).is_empty());
        assert!(execute(opcodes::OP_KILL, &[0; 5]).is_empty());
        assert!(execute(opcodes::OP_DAMAGE_ACTOR_VALUE, &[0; 5]).is_empty());
        assert!(execute(opcodes::OP_PLAY_GROUP, &[0; 8]).is_empty());
        assert!(execute(opcodes::OP_FORCE_WEATHER, &[]).is_empty());
        assert!(execute(opcodes::OP_SET_RESTRAINED, &[0; 3]).is_empty());
    }

    #[test]
    fn test_read_u32_bounds() {
        assert_eq!(read_u32(&[], 0), None);
        assert_eq!(read_u32(&[0x01, 0x02, 0x03], 0), None);
        assert_eq!(read_u32(&[0x01, 0x02, 0x03, 0x04], 0), Some(0x0403_0201));
        assert_eq!(read_u32(&[0xFF, 0x01, 0x02, 0x03, 0x04], 1), Some(0x0403_0201));
    }
}
