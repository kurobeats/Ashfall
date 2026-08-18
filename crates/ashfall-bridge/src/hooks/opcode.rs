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
//!
//! # Thread safety
//!
//! All VTable calls from the bridge must serialize through a single mutex:
//! Gamebryo's ScriptRunner is not reentrant. The in-memory handler table uses
//! `std::sync::Mutex`; the real implementation needs a Windows `CRITICAL_SECTION`
//! (or `parking_lot::Mutex`) so engine threads can block while a handler runs.

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
/// ponytail: kept for callers that want an explicit pass-through handler.
#[allow(dead_code)]
fn allow_all(_opcode: u16, _params: &[u32]) -> OpcodeAction {
    OpcodeAction::Allow
}

/// Handler that always blocks (fully delegated — server must respond).
fn block_all(_opcode: u16, _params: &[u32]) -> OpcodeAction {
    OpcodeAction::Block
}

// ── Handler table ──

/// Direct-indexed handler table: index by `opcode & 0x1FFF`.
/// Real GECK opcodes used for interception are all < 0x2000, so the mask is
/// lossless for them (VAULTFUNCTION opcodes never reach `intercept()`).
/// 8Ki entries × `Option<fn>` (niche: null = None) ≈ 64KiB static — zero
/// allocation, no hashing, single lock acquisition per lookup.
const OPCODE_TABLE_SIZE: usize = 0x2000;

static OPCODE_HANDLERS: LazyLock<Mutex<[Option<OpcodeHandler>; OPCODE_TABLE_SIZE]>> =
    LazyLock::new(|| Mutex::new([None; OPCODE_TABLE_SIZE]));

#[inline]
fn opcode_index(opcode: u16) -> usize {
    (opcode & 0x1FFF) as usize
}

/// Register a handler for a specific opcode.
pub fn register_handler(opcode: u16, handler: OpcodeHandler) {
    let mut table = OPCODE_HANDLERS.lock().unwrap();
    table[opcode_index(opcode)] = Some(handler);
}

/// Unregister a handler.
pub fn unregister_handler(opcode: u16) {
    let mut table = OPCODE_HANDLERS.lock().unwrap();
    table[opcode_index(opcode)] = None;
}

/// Intercept an opcode execution. Returns the action to take.
/// Called from the ScriptRunner::Execute VTable patch.
pub fn intercept(opcode: u16, params: &[u32]) -> OpcodeAction {
    let table = OPCODE_HANDLERS.lock().unwrap();
    match table[opcode_index(opcode)] {
        Some(handler) => handler(opcode, params),
        None => OpcodeAction::Allow,
    }
}

/// Check if an opcode has a registered handler (without locking twice).
pub fn has_handler(opcode: u16) -> bool {
    OPCODE_HANDLERS.lock().unwrap()[opcode_index(opcode)].is_some()
}

/// Count registered handlers.
pub fn handler_count() -> usize {
    OPCODE_HANDLERS
        .lock()
        .unwrap()
        .iter()
        .filter(|h| h.is_some())
        .count()
}

// ── Default delegator handlers ──
// ponytail: these block local execution. Client sends opcode params via pipe,
// server validates and sends GECK commands back. Delegate prevents double-execution.

/// Known multiplayer-delegated opcodes from vaultmp.
/// Values VERIFIED against the real GECK.exe command table (r2, 2026-08-06):
/// the opcode field at CommandInfo+0x08 of each exact command-name entry,
/// cross-checked with xNVSE's SetReturnType list (PlaceAtMe = 0x1025).
pub mod delegated_opcodes {
    // Item ops
    pub const PLACE_AT_ME: u16 = 0x1025;
    pub const ADD_ITEM: u16 = 0x1002;
    pub const REMOVE_ITEM: u16 = 0x1052;
    pub const EQUIP_ITEM: u16 = 0x10EE;
    pub const UNEQUIP_ITEM: u16 = 0x10EF;

    // Actor state ops
    pub const SET_AV: u16 = 0x110E; // ForceActorValue
    pub const KILL: u16 = 0x108B; // KillActor
                                  // 2026-08-17: verified against the Steam FO3 command table 0x110B388
                                  // (name 'SetRestrained', handler 0x7A6670).
    pub const SET_RESTRAINED: u16 = 0x10F3;
    pub const PLAY_GROUP: u16 = 0x1013;

    // World ops
    pub const LOCK: u16 = 0x1072; // adjacent to verified UnLock 0x1073
    pub const UNLOCK: u16 = 0x1073;
    // 2026-08-17: verified against the Steam FO3 command table 0x110B388
    // (name 'SetOwnership', handler 0x7A5C20).
    pub const SET_OWNERSHIP: u16 = 0x1117;
    pub const ACTIVATE: u16 = 0x100D; // verified; 0x100C is GetSecondsPassed

    // Quest
    pub const SET_STAGE: u16 = 0x1039;

    // FO3 specific
    pub const SET_ALERT: u16 = 0x105A;
}

/// Register all default delegated opcodes (block local execution, relay to server).
pub fn register_defaults() {
    use delegated_opcodes::*;
    for op in [
        PLACE_AT_ME,
        ADD_ITEM,
        REMOVE_ITEM,
        EQUIP_ITEM,
        UNEQUIP_ITEM,
        SET_AV,
        KILL,
        SET_RESTRAINED,
        PLAY_GROUP,
        LOCK,
        UNLOCK,
        SET_OWNERSHIP,
        ACTIVATE,
        SET_STAGE,
        SET_ALERT,
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
/// Only defined for VAULTFUNCTION opcodes (0xE000..=0xFFFF); the 0x0FFF mask
/// keeps the low 12 index bits (vaultmp's VAULTFUNCTION table is 0x0000-0x0036).
pub fn vaultfunction_index(opcode: u16) -> u16 {
    opcode & 0x0FFF
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    /// The handler table is a global — serialize the tests that touch it.
    static OP_LOCK: Mutex<()> = Mutex::new(());
    fn lock_ops() -> std::sync::MutexGuard<'static, ()> {
        OP_LOCK.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn test_handler(_opcode: u16, _params: &[u32]) -> OpcodeAction {
        OpcodeAction::Block
    }

    #[test]
    fn test_register_and_intercept() {
        let _guard = lock_ops();
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
        let _guard = lock_ops();
        let before = handler_count();
        register_handler(0xAAAA, test_handler);
        assert_eq!(handler_count(), before + 1);
        unregister_handler(0xAAAA);
        assert_eq!(handler_count(), before);
    }

    #[test]
    fn test_register_defaults() {
        let _guard = lock_ops();
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
        // Mask is 0x0FFF: high nibble bits are excluded from the index
        assert_eq!(vaultfunction_index(0xF0F1), 0x00F1);
    }

    #[test]
    fn test_opcode_table_direct_index() {
        // Registered opcodes collide by design on `opcode & 0x1FFF`:
        // 0x0001 and 0x2001 would share a slot, but only real GECK opcodes
        // (< 0x2000) reach intercept(). Verify wrap keeps table in bounds.
        assert_eq!(OPCODE_TABLE_SIZE, 0x2000);
        assert!(opcode_index(0x1FFF) < OPCODE_TABLE_SIZE);
        assert!(opcode_index(0x2000) < OPCODE_TABLE_SIZE); // wraps to 0
        assert!(opcode_index(0xFFFF) < OPCODE_TABLE_SIZE);
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
