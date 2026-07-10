//! AI package types + state machine.
//!
//! ponytail: stubs — package execution deferred to Phase 5 (WASM scripting).
//! Package names and transitions defined, but no AI update loop yet.

/// AI package definitions.
pub const PACKAGE_NONE: u32 = 0;
pub const PACKAGE_WANDER: u32 = 1;
pub const PACKAGE_TRAVEL: u32 = 2;
pub const PACKAGE_COMBAT: u32 = 3;
pub const PACKAGE_GUARD: u32 = 4;
pub const PACKAGE_SLEEP: u32 = 5;
pub const PACKAGE_EAT: u32 = 6;
pub const PACKAGE_FLEE: u32 = 7;
pub const PACKAGE_USE_ITEM: u32 = 8;
pub const PACKAGE_DIALOGUE: u32 = 9;

/// Package execution priority (lower = higher priority).
pub fn package_priority(package_id: u32) -> u8 {
    match package_id {
        PACKAGE_COMBAT => 10,
        PACKAGE_FLEE => 20,
        PACKAGE_DIALOGUE => 30,
        PACKAGE_USE_ITEM => 40,
        PACKAGE_TRAVEL => 50,
        PACKAGE_EAT => 60,
        PACKAGE_SLEEP => 70,
        PACKAGE_WANDER => 80,
        PACKAGE_GUARD => 90,
        _ => 100,
    }
}

/// Check if a package interrupts another.
pub fn can_interrupt(current: u32, new: u32) -> bool {
    package_priority(new) < package_priority(current)
}
