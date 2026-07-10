//! Script opcode interception engine.
//!
//! Ported from vaultmp-extended vaultmpdll/vaultmp.cpp `ExecuteCommand()`
//! and `BethesdaDelegator()` patterns.
//!
//! NVSE/FOSE intercepts script commands via `ScriptRunner::Execute` VTable patch.
//! The dispatcher checks registered handlers before allowing original execution.
//! Delegator pattern: blocks local execution of multiplayer-sensitive opcodes
//! (PlaceAtMe, AddItem, SetStage, SetAV, EquipItem), relaying them via pipe
//! for server-side validation.

use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

// ── Handler types ──

/// Return value from an opcode handler.
/// - `Allow`: let the original engine opcode execute normally.
/// - `Block`: suppress original execution (we handled it).
/// - `Replace(bytes)`: skip original, return these bytes as the result.
pub enum OpcodeAction {
    Allow,
    Block,
    Replace(Vec<u8>),
}

/// An opcode handler receives the raw opcode and its parameter bytes,
/// and returns whether to allow, block, or replace the original execution.
pub type OpcodeHandler = fn(opcode: u16, params: &[u32]) -> OpcodeAction;

/// Handler that always allows.
fn allow_all(_opcode: u16, _params: &[u32]) -> OpcodeAction {
    OpcodeAction::Allow
}

/// Handler that always blocks (fully delegated — server must respond).
fn block_all(_opcode: u16, _params: &[u32]) -> OpcodeAction {
    OpcodeAction::Block
}

// ── Handler table ──

static OPCODE_HANDLERS: LazyLock<Mutex<HashMap<u16, OpcodeHandler>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Register a handler for a specific opcode.
pub fn register_handler(opcode: u16, handler: OpcodeHandler) {
    let mut table = OPCODE_HANDLERS.lock().unwrap();
    table.insert(opcode, handler);
}

/// Unregister a handler.
pub fn unregister_handler(opcode: u16) {
    let mut table = OPCODE_HANDLERS.lock().unwrap();
    table.remove(&opcode);
}

/// Intercept an opcode execution. Returns the action to take.
/// Called from the ScriptRunner::Execute VTable patch.
pub fn intercept(opcode: u16, params: &[u32]) -> OpcodeAction {
    let table = OPCODE_HANDLERS.lock().unwrap();
    if let Some(handler) = table.get(&opcode) {
        handler(opcode, params)
    } else {
        OpcodeAction::Allow
    }
}

/// Check if an opcode has a registered handler (without locking twice).
pub fn has_handler(opcode: u16) -> bool {
    OPCODE_HANDLERS.lock().unwrap().contains_key(&opcode)
}

/// Count registered handlers.
pub fn handler_count() -> usize {
    OPCODE_HANDLERS.lock().unwrap().len()
}

// ── Default delegator handlers ──
// ponytail: these block local execution. Client sends opcode params via pipe,
// server validates and sends GECK commands back. Delegate prevents double-execution.

/// Known multiplayer-delegated opcodes from vaultmp.
/// Values from API.hpp / GECK command database.
pub mod delegated_opcodes {
    // Item ops
    pub const PLACE_AT_ME: u16 = 0x1007;
    pub const ADD_ITEM: u16 = 0x1002;
    pub const REMOVE_ITEM: u16 = 0x1052;
    pub const EQUIP_ITEM: u16 = 0x10EE;
    pub const UNEQUIP_ITEM: u16 = 0x10EF;

    // Actor state ops
    pub const SET_AV: u16 = 0x110E; // ForceActorValue
    pub const KILL: u16 = 0x108B;
    pub const SET_RESTRAINED: u16 = 0x10F3;
    pub const PLAY_GROUP: u16 = 0x1013;

    // World ops
    pub const LOCK: u16 = 0x1072;
    pub const UNLOCK: u16 = 0x1073;
    pub const SET_OWNERSHIP: u16 = 0x1117;
    pub const ACTIVATE: u16 = 0x100C; // ponytail: not 100% sure on opcode

    // Quest
    pub const SET_STAGE: u16 = 0x101B;

    // FO3 specific
    pub const SET_ALERT: u16 = 0x101E;
}

/// Register all default delegated opcodes (block local execution, relay to server).
pub fn register_defaults() {
    use delegated_opcodes::*;
    for op in [
        PLACE_AT_ME, ADD_ITEM, REMOVE_ITEM, EQUIP_ITEM, UNEQUIP_ITEM,
        SET_AV, KILL, SET_RESTRAINED, PLAY_GROUP,
        LOCK, UNLOCK, SET_OWNERSHIP, ACTIVATE,
        SET_STAGE, SET_ALERT,
    ] {
        register_handler(op, block_all);
    }
}

// ── VAULTFUNCTION opcode table ──
//
// vaultmp's custom opcodes (0xE000–0xE036). These bypass the engine's
// FuncLookup dispatch and are handled directly in vaultfunction().
// Ashfall implements them in commands.rs; this table maps opcode → description.

pub const VAULTFUNCTION_MASK: u16 = 0xE000;

/// Check if an opcode is a VAULTFUNCTION (custom vaultmp opcode).
pub fn is_vaultfunction(opcode: u16) -> bool {
    (opcode & VAULTFUNCTION_MASK) == VAULTFUNCTION_MASK
}

/// Strip the VAULTFUNCTION mask to get the base index.
pub fn vaultfunction_index(opcode: u16) -> u16 {
    opcode & !VAULTFUNCTION_MASK
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_handler(_opcode: u16, _params: &[u32]) -> OpcodeAction {
        OpcodeAction::Block
    }

    #[test]
    fn test_register_and_intercept() {
        register_handler(0x1007, test_handler);
        assert!(has_handler(0x1007));

        let action = intercept(0x1007, &[]);
        match action {
            OpcodeAction::Block => {} // expected
            _ => panic!("expected Block"),
        }

        // Unknown opcode → Allow
        let action = intercept(0x9999, &[]);
        match action {
            OpcodeAction::Allow => {}
            _ => panic!("expected Allow for unknown opcode"),
        }

        unregister_handler(0x1007);
        assert!(!has_handler(0x1007));
    }

    #[test]
    fn test_handler_count() {
        let before = handler_count();
        register_handler(0xAAAA, test_handler);
        assert_eq!(handler_count(), before + 1);
        unregister_handler(0xAAAA);
        assert_eq!(handler_count(), before);
    }

    #[test]
    fn test_register_defaults() {
        register_defaults();
        assert!(handler_count() >= 10);
    }

    #[test]
    fn test_vaultfunction_mask() {
        assert!(is_vaultfunction(0xE001));
        assert!(is_vaultfunction(0xE036));
        assert!(!is_vaultfunction(0x1007));
        assert!(is_vaultfunction(0xFFFF)); // 0xFFFF & 0xE000 = 0xE000

        assert_eq!(vaultfunction_index(0xE001), 0x0001);
        assert_eq!(vaultfunction_index(0xE036), 0x0036);
        assert_eq!(vaultfunction_index(0x1007), 0x1007); // non-VAULTFUNCTION unchanged
    }

    #[test]
    fn test_allow_all_handler() {
        let action = allow_all(0, &[]);
        match action {
            OpcodeAction::Allow => {}
            _ => panic!("allow_all should return Allow"),
        }
    }
}
