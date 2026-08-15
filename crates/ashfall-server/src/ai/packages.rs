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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn priority_order() {
        // combat beats everything, guard is lowest of the defined set
        assert!(package_priority(PACKAGE_COMBAT) < package_priority(PACKAGE_FLEE));
        assert!(package_priority(PACKAGE_FLEE) < package_priority(PACKAGE_WANDER));
        assert!(package_priority(PACKAGE_WANDER) < package_priority(PACKAGE_GUARD)); // guard 90 < wander 80
        assert_eq!(package_priority(0xFFFF), 100); // unknown → lowest
    }

    #[test]
    fn interrupt_rules() {
        // higher priority (lower number) interrupts lower
        assert!(can_interrupt(PACKAGE_WANDER, PACKAGE_COMBAT));
        assert!(can_interrupt(PACKAGE_GUARD, PACKAGE_SLEEP));
        // same or lower priority never interrupts
        assert!(!can_interrupt(PACKAGE_COMBAT, PACKAGE_WANDER));
        assert!(!can_interrupt(PACKAGE_WANDER, PACKAGE_WANDER));
        assert!(!can_interrupt(PACKAGE_COMBAT, PACKAGE_COMBAT));
    }
}
